//! Opt-in native preparation. Bounds are explicit experiment/rule inputs, not F1 claims.
//! A small fixed pilot estimates demand; it does not prove every future strategy feasible.
use pitgun_contract::{Digest, canonical_json_digest};
use pitgun_racing_solver::{
    ResolvedSimulationRequestV3, ResolvedSimulationSessionV3, ResolvedSimulationStepV3,
    fuel_outcome::{FuelAwareSessionV1, FuelAwareStepV1, FuelProgressV1, FuelShortfallV1},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FuelPreparationVersionV1 {
    #[serde(rename = "pitgun.fuel-preparation/v1")]
    V1,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FuelPreparationPolicyV1 {
    pub schema_version: FuelPreparationVersionV1,
    /// Explicit per-experiment/vehicle/rules limit. No default or inferred tank capacity.
    pub maximum_start_kg: f64,
    pub finish_reserve_kg: f64,
    pub uncertainty_fraction: f64,
    pub rounding_quantum_kg: f64,
    pub pilot_laps_per_compound: u16,
}
impl FuelPreparationPolicyV1 {
    pub fn validate(&self) -> Result<(), String> {
        if !self.maximum_start_kg.is_finite()
            || !(1.0..=200.0).contains(&self.maximum_start_kg)
            || !self.finish_reserve_kg.is_finite()
            || !(0.0..self.maximum_start_kg).contains(&self.finish_reserve_kg)
            || !self.uncertainty_fraction.is_finite()
            || !(0.0..=0.5).contains(&self.uncertainty_fraction)
            || !self.rounding_quantum_kg.is_finite()
            || !(0.01..=5.0).contains(&self.rounding_quantum_kg)
            || !(1..=3).contains(&self.pilot_laps_per_compound)
        {
            return Err("invalid bounded fuel preparation policy".into());
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FuelPilotV1 {
    pub input_digest: Digest,
    pub tire_digest: Digest,
    pub completed_laps: u16,
    pub maximum_consumption_kg_per_m: f64,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FuelPreparationOutcomeV1 {
    Prepared {
        initial_kg: f64,
    },
    EstimateExceedsLimit {
        estimated_start_kg: f64,
        maximum_start_kg: f64,
    },
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FuelPreparationReceiptV1 {
    pub schema_version: FuelPreparationVersionV1,
    pub source_input_digest: Digest,
    pub policy_digest: Digest,
    pub prepared_input_digest: Option<Digest>,
    pub pilots: Vec<FuelPilotV1>,
    pub estimated_driving_kg: f64,
    pub estimated_service_kg: f64,
    pub outcome: FuelPreparationOutcomeV1,
}

/// Returns an atomic resolved request only if the estimate fits the declared limit.
/// Unknown numerical/physical pilot failures remain errors, never fuel infeasibility.
pub fn prepare_fuel_v1(
    input: &ResolvedSimulationRequestV3,
    policy: &FuelPreparationPolicyV1,
) -> Result<
    (
        FuelPreparationReceiptV1,
        Option<ResolvedSimulationRequestV3>,
    ),
    String,
> {
    policy.validate()?;
    if input.thermal_physics_v2.is_none()
        || input.fuel_mass.is_none()
        || !(1..=100).contains(&input.lap_count)
    {
        return Err("fuel preparation requires an explicit thermal candidate and1..100laps".into());
    }
    // Validate the original complete request before dropping its future stops.
    ResolvedSimulationSessionV3::new(input.clone())?;
    let hash = |value: &ResolvedSimulationRequestV3| {
        canonical_json_digest(value).map_err(|e| e.to_string())
    };
    let source_input_digest = hash(input)?;
    let policy_digest = canonical_json_digest(policy).map_err(|e| e.to_string())?;
    let length = *input.track.s.last().ok_or("missing track distance")?;
    let mut compounds = vec![input.vehicle.tire.clone()];
    for stop in &input.pit_plan.stops {
        if !compounds.contains(&stop.tire) {
            compounds.push(stop.tire.clone());
        }
    }
    if compounds.len() > 3 {
        return Err("fuel preparation supports at most3 planned compounds".into());
    }
    let mut pilots = Vec::new();
    let mut maximum_rate: f64 = 0.0;
    for tire in compounds {
        let mut pilot = input.clone();
        pilot.state.fuel_mass = policy.maximum_start_kg;
        pilot.vehicle.tire = tire.clone();
        pilot.lap_count = policy.pilot_laps_per_compound.min(input.lap_count);
        pilot.pit_plan.stops.clear();
        let pilot_digest = hash(&pilot)?;
        let mut session = ResolvedSimulationSessionV3::new(pilot.clone())?;
        let mut prior_fuel = pilot.state.fuel_mass;
        let mut rate: f64 = 0.0;
        let mut count = 0;
        loop {
            match session.advance()? {
                ResolvedSimulationStepV3::Lap(lap) => {
                    rate = rate.max((prior_fuel - lap.state_after_lap.fuel_mass) / length);
                    prior_fuel = lap.state_after_lap.fuel_mass;
                    count += 1;
                    // No final full-history poll: the committed last pilot lap is enough.
                    if count == pilot.lap_count {
                        break;
                    }
                }
                ResolvedSimulationStepV3::Complete(_) => {
                    return Err("pilot completed without all lap observations".into());
                }
            }
        }
        maximum_rate = maximum_rate.max(rate);
        pilots.push(FuelPilotV1 {
            input_digest: pilot_digest,
            tire_digest: canonical_json_digest(&tire).map_err(|e| e.to_string())?,
            completed_laps: count,
            maximum_consumption_kg_per_m: rate,
        });
    }
    let driving = maximum_rate * length * f64::from(input.lap_count);
    let service = input.fuel_mass.unwrap().idle_fuel_flow_kg_per_s
        * input.config.pit_time_penalty_s.max(0.0)
        * input.pit_plan.stops.len() as f64;
    let required =
        ((driving * (1.0 + policy.uncertainty_fraction) + service + policy.finish_reserve_kg)
            / policy.rounding_quantum_kg)
            .ceil()
            * policy.rounding_quantum_kg;
    if !required.is_finite() || required <= 0.0 {
        return Err("nonfinite/empty fuel estimate".into());
    }
    let mut prepared = None;
    let outcome = if required > policy.maximum_start_kg {
        FuelPreparationOutcomeV1::EstimateExceedsLimit {
            estimated_start_kg: required,
            maximum_start_kg: policy.maximum_start_kg,
        }
    } else {
        let mut resolved = input.clone();
        resolved.state.fuel_mass = required;
        ResolvedSimulationSessionV3::new(resolved.clone())?;
        prepared = Some(resolved);
        FuelPreparationOutcomeV1::Prepared {
            initial_kg: required,
        }
    };
    Ok((
        FuelPreparationReceiptV1 {
            schema_version: FuelPreparationVersionV1::V1,
            source_input_digest,
            policy_digest,
            prepared_input_digest: prepared.as_ref().map(hash).transpose()?,
            pilots,
            estimated_driving_kg: driving,
            estimated_service_kg: service,
            outcome,
        },
        prepared,
    ))
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NativeFuelFinishV1 {
    Finished {
        result_digest: Digest,
        laps: u16,
        elapsed_s: f64,
        final_fuel_kg: f64,
    },
    FuelNotCompleted {
        progress: FuelProgressV1,
        shortfall: FuelShortfallV1,
    },
}

/// Offline/native field only. A modeled fuel shortfall is isolated per car;
/// technical failures still abort. No public standings/journal/runtime admission.
pub fn run_fuel_aware_field_v1(
    inputs: BTreeMap<String, ResolvedSimulationRequestV3>,
) -> Result<BTreeMap<String, NativeFuelFinishV1>, String> {
    if inputs.is_empty() || inputs.len() > 10 {
        return Err("native fuel field requires1..10cars".into());
    }
    let first = inputs.values().next().unwrap();
    if inputs
        .iter()
        .any(|(id, i)| id.is_empty() || i.track != first.track || i.lap_count != first.lap_count)
    {
        return Err("native fuel field must share track and horizon".into());
    }
    let mut sessions = inputs
        .into_iter()
        .map(|(id, i)| Ok((id, FuelAwareSessionV1::new(i)?)))
        .collect::<Result<BTreeMap<_, _>, String>>()?;
    let mut output = BTreeMap::new();
    while !sessions.is_empty() {
        let mut completed = Vec::new();
        for (id, session) in &mut sessions {
            let outcome = match session
                .advance()
                .map_err(|e| format!("native field {id}: {e}"))?
            {
                FuelAwareStepV1::Lap { .. } => continue,
                FuelAwareStepV1::Finished { result } => NativeFuelFinishV1::Finished {
                    result_digest: canonical_json_digest(&result).map_err(|e| e.to_string())?,
                    laps: result.lap_times_s.len() as u16,
                    elapsed_s: result.total_time_s,
                    final_fuel_kg: result.final_state.fuel_mass,
                },
                FuelAwareStepV1::FuelNotCompleted {
                    progress,
                    shortfall,
                } => NativeFuelFinishV1::FuelNotCompleted {
                    progress,
                    shortfall,
                },
            };
            output.insert(id.clone(), outcome);
            completed.push(id.clone());
        }
        for id in completed {
            sessions.remove(&id);
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    mod frozen {
        use crate as pitgun_racing_simulator;
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/examples/support/frozen_thermal_input.rs"
        ));
    }
    fn input(id: &str) -> ResolvedSimulationRequestV3 {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let catalog =
            crate::RacingCatalogSnapshot::from_release_dir(root.join("catalogs/racing/v1.8.1"))
                .unwrap();
        frozen::frozen_input(&root, id, &catalog).unwrap().0
    }
    fn policy() -> FuelPreparationPolicyV1 {
        FuelPreparationPolicyV1 {
            schema_version: FuelPreparationVersionV1::V1,
            maximum_start_kg: 110.0,
            finish_reserve_kg: 1.0,
            uncertainty_fraction: 0.08,
            rounding_quantum_kg: 0.5,
            pilot_laps_per_compound: 2,
        }
    }
    #[test]
    fn preparation_is_bounded_deterministic_and_preserves_physics_and_original_input() {
        let input = input("budget-classic15-monza-c2");
        let original = input.clone();
        let (receipt, prepared) = prepare_fuel_v1(&input, &policy()).unwrap();
        let mut prepared = prepared.unwrap();
        assert!(prepared.state.fuel_mass < policy().maximum_start_kg);
        assert_eq!(receipt.pilots.len(), 1);
        assert_eq!(receipt.pilots[0].completed_laps, 2);
        assert_eq!(
            receipt.prepared_input_digest,
            Some(canonical_json_digest(&prepared).unwrap())
        );
        assert_eq!(input, original);
        assert_eq!(
            prepare_fuel_v1(&input, &policy()).unwrap(),
            (receipt, Some(prepared.clone()))
        );
        prepared.state.fuel_mass = input.state.fuel_mass;
        assert_eq!(prepared, input);
    }
    #[test]
    fn estimate_excess_never_expands_the_limit_or_changes_consumption() {
        let input = input("full-classic15-c2");
        let original = input.clone();
        let mut p = policy();
        p.maximum_start_kg = 5.0;
        let (receipt, prepared) = prepare_fuel_v1(&input, &p).unwrap();
        assert!(prepared.is_none());
        assert!(receipt.prepared_input_digest.is_none());
        match receipt.outcome {
            FuelPreparationOutcomeV1::EstimateExceedsLimit {
                estimated_start_kg,
                maximum_start_kg,
            } => {
                assert!(estimated_start_kg > maximum_start_kg);
                assert_eq!(maximum_start_kg, 5.0);
            }
            _ => panic!("expected estimate above limit"),
        }
        assert_eq!(input, original);
        p.uncertainty_fraction = f64::NAN;
        assert!(prepare_fuel_v1(&input, &p).is_err());
    }
    #[test]
    fn rival_fuel_failure_is_an_outcome_and_does_not_change_the_players_result() {
        let healthy = input("budget-classic15-monza-c2");
        let mut rival = healthy.clone();
        rival.state.fuel_mass = 0.000001;
        let expected = pitgun_racing_solver::run_resolved_simulation_v3(&healthy).unwrap();
        let field = run_fuel_aware_field_v1(BTreeMap::from([
            ("player".into(), healthy.clone()),
            ("rival".into(), rival),
        ]))
        .unwrap();
        match &field["player"] {
            NativeFuelFinishV1::Finished { result_digest, .. } => {
                assert_eq!(*result_digest, canonical_json_digest(&expected).unwrap())
            }
            _ => panic!("player did not finish"),
        }
        assert!(
            matches!(&field["rival"],NativeFuelFinishV1::FuelNotCompleted {progress,..} if progress.completed_laps==0)
        );
        let mut invalid = healthy.clone();
        invalid.lap_count += 1;
        assert!(
            run_fuel_aware_field_v1(BTreeMap::from([
                ("player".into(), healthy),
                ("rival".into(), invalid)
            ]))
            .is_err()
        );
    }
}
