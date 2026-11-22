//! Opt-in initial tire-resource compilation. No ordinary model selects it.
//! Compile all allowed sets up front so later pit commands use the same physics.
use crate::{EmbeddedCatalog, RacingCatalogSnapshot, thermal_career::ThermalCareerPolicyV1};
use pitgun_contract::{Digest, canonical_json_digest};
use pitgun_racing_solver::{
    ResolvedSimulationRequestV3, ResolvedSimulationSessionV3, TireParams, TireWearGripCurveV1,
    thermal_physics::TireCompoundHeatV2,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TirePerformanceProfileV1 {
    pub schema_version: String,
    pub mount_temperature_c: f64,
    pub compounds: BTreeMap<String, TireCompoundPerformanceV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TireCompoundPerformanceV1 {
    pub wear_curve: TireWearGripCurveV1,
    pub mu_scale: f64,
    pub temperature_width_c: f64,
    pub thermal_wear_gain: f64,
}

impl TirePerformanceProfileV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != "pitgun.racing-tire-performance/v1"
            || !self.mount_temperature_c.is_finite()
            || !(20.0..=110.0).contains(&self.mount_temperature_c)
            || self.compounds.len() != 3
            || ["soft", "medium", "hard"]
                .iter()
                .any(|id| !self.compounds.contains_key(*id))
        {
            return Err(
                "tire performance V1: explicit preparation and three named compounds required"
                    .into(),
            );
        }
        for c in self.compounds.values() {
            c.wear_curve.validate()?;
            if !c.mu_scale.is_finite()
                || c.mu_scale <= 0.0
                || !c.temperature_width_c.is_finite()
                || c.temperature_width_c <= 0.0
                || !c.thermal_wear_gain.is_finite()
                || c.thermal_wear_gain < 0.0
            {
                return Err("tire performance V1: invalid finite compound parameters".into());
            }
        }
        Ok(())
    }
}

/// Compiled locally from retained resources, never deserialized as trusted evidence.
#[derive(Debug, Clone, Serialize)]
pub struct CompiledTirePerformanceV1 {
    policy_digest: Digest,
    baseline_thermal_policy_digest: Digest,
    mount_temperature_c: f64,
    tires: BTreeMap<String, TireParams>,
    compounds: Vec<TireCompoundHeatV2>,
}

impl CompiledTirePerformanceV1 {
    pub fn policy_digest(&self) -> Digest {
        self.policy_digest
    }

    /// A future live pit-command compiler must use this exact set, not a legacy tire.
    pub fn tire(&self, id: &str) -> Result<&TireParams, String> {
        self.tires
            .get(id)
            .ok_or_else(|| format!("tire performance V1: unknown compound {id}"))
    }

    /// Initial preparation only, not an in-race control action or checkpoint restore.
    /// Work/heat/wear laws remain in the Solver. No wear or fuel is reset here.
    pub fn apply_initial_to_input(
        &self,
        input: &mut ResolvedSimulationRequestV3,
        stint_ids: &[String],
    ) -> Result<(), String> {
        if input.thermal_physics_v2.is_none()
            || input.longitudinal_energy.is_none()
            || stint_ids.len() != input.pit_plan.stops.len() + 1
        {
            return Err(
                "tire performance V1: thermal/energy input and exact stint identities required"
                    .into(),
            );
        }
        let mut next = input.clone();
        next.vehicle.tire = self.tire(&stint_ids[0])?.clone();
        for (stop, id) in next.pit_plan.stops.iter_mut().zip(&stint_ids[1..]) {
            stop.tire = self.tire(id)?.clone();
        }
        next.state.tire_temp = self.mount_temperature_c;
        next.config.pit_tire_temp = Some(self.mount_temperature_c);
        let thermal = next.thermal_physics_v2.as_mut().unwrap();
        thermal.compounds = self.compounds.clone();
        thermal.initial_carcass_temperature_c = self.mount_temperature_c;
        thermal.pit_carcass_temperature_c = self.mount_temperature_c;
        ResolvedSimulationSessionV3::new(next.clone())?;
        *input = next;
        Ok(())
    }
}

/// Resource compiler, not an Authority boundary: hosts must resolve/bind its inputs.
pub fn compile_tire_performance_v1(
    snapshot: &RacingCatalogSnapshot,
    baseline: &ThermalCareerPolicyV1,
    profile: &TirePerformanceProfileV1,
) -> Result<CompiledTirePerformanceV1, String> {
    profile.validate()?;
    baseline.validate()?;
    let catalog = EmbeddedCatalog::from_snapshot(snapshot)?;
    let mut tires = BTreeMap::new();
    let mut compounds = Vec::new();
    for (id, p) in &profile.compounds {
        let base = baseline
            .compounds
            .get(id)
            .ok_or_else(|| format!("tire performance V1: missing baseline compound {id}"))?;
        let mut tire = catalog.resolve_tire(id)?;
        tire.mu_scale = p.mu_scale;
        tire.temp_opt = base.temperature_optimum_c;
        tire.temp_sigma = p.temperature_width_c;
        tire.temp_min_k = base.minimum_temperature_grip_factor;
        tire.wear_grip_curve = Some(p.wear_curve.clone());
        let mut heat = base.heat;
        heat.thermal_wear_gain = p.thermal_wear_gain;
        compounds.push(TireCompoundHeatV2 {
            tire: tire.clone(),
            heat,
        });
        tires.insert(id.clone(), tire);
    }
    Ok(CompiledTirePerformanceV1 {
        policy_digest: canonical_json_digest(profile).map_err(|e| e.to_string())?,
        baseline_thermal_policy_digest: canonical_json_digest(baseline)
            .map_err(|e| e.to_string())?,
        mount_temperature_c: profile.mount_temperature_c,
        tires,
        compounds,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RunRaceRequest, thermal_driver::ThermalDriverPolicyV1};

    fn resources() -> (
        RacingCatalogSnapshot,
        ThermalCareerPolicyV1,
        TirePerformanceProfileV1,
    ) {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let snapshot =
            RacingCatalogSnapshot::from_release_dir(root.join("catalogs/racing/v1.8.1")).unwrap();
        let baseline = serde_json::from_slice(
            &std::fs::read(root.join("experiments/career_physics_v1/profile-v1.json")).unwrap(),
        )
        .unwrap();
        let set: serde_json::Value = serde_json::from_slice(
            &std::fs::read(root.join("experiments/tire_performance_v1/profile-candidates-v1.json"))
                .unwrap(),
        )
        .unwrap();
        let profile = serde_json::from_value(set["profiles"]["progressive4"].clone()).unwrap();
        (snapshot, baseline, profile)
    }

    #[test]
    fn complete_named_sets_are_compiled_without_fallback() {
        let (snapshot, baseline, mut profile) = resources();
        let compiled = compile_tire_performance_v1(&snapshot, &baseline, &profile).unwrap();
        for id in ["soft", "medium", "hard"] {
            assert!(compiled.tire(id).unwrap().wear_grip_curve.is_some());
        }
        assert!(compiled.tire("unknown").is_err());
        profile.compounds.remove("medium");
        assert!(compile_tire_performance_v1(&snapshot, &baseline, &profile).is_err());
    }

    #[test]
    fn initial_binding_is_atomic_and_keeps_unused_sets_for_future_services() {
        let (snapshot, baseline, profile) = resources();
        let compiled = compile_tire_performance_v1(&snapshot, &baseline, &profile).unwrap();
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let fixtures: serde_json::Value = serde_json::from_slice(
            &std::fs::read(
                root.join("experiments/opponent_physics_v2/results/career-fixtures-v1.json"),
            )
            .unwrap(),
        )
        .unwrap();
        let mut request: RunRaceRequest = serde_json::from_value(
            fixtures["fixtures"][0]["preparedRequests"][0]["request"].clone(),
        )
        .unwrap();
        request.input.race.competitors.retain(|c| c.is_player);
        request.input.competitor_vehicle_components.clear();
        request.input.competitor_profiles.clear();
        request.input.race.competitors[0].driver_id = Some("balanced_reference".into());
        let ids: Vec<String> = request.input.race.competitors[0]
            .stint_strategy
            .as_ref()
            .unwrap()
            .stints
            .iter()
            .map(|s| s.tire_id.clone())
            .collect();
        let field = crate::thermal_career::resolve_thermal_career_v1(
            &request,
            &snapshot,
            &baseline,
            &ThermalDriverPolicyV1::from_catalog(&snapshot).unwrap(),
            &BTreeMap::new(),
        )
        .unwrap();
        let mut input = field.competitors.values().next().unwrap().input.clone();
        let before = canonical_json_digest(&input).unwrap();
        let mut invalid = ids.clone();
        invalid[0] = "unknown".into();
        assert!(
            compiled
                .apply_initial_to_input(&mut input, &invalid)
                .is_err()
        );
        assert_eq!(canonical_json_digest(&input).unwrap(), before);
        compiled.apply_initial_to_input(&mut input, &ids).unwrap();
        assert_eq!(
            input.thermal_physics_v2.as_ref().unwrap().compounds.len(),
            3
        );
        let unused = ["soft", "medium", "hard"]
            .into_iter()
            .find(|id| !ids.iter().any(|x| x == id))
            .unwrap();
        input.pit_plan.stops[0].tire = compiled.tire(unused).unwrap().clone();
        assert!(
            input
                .thermal_physics_v2
                .as_ref()
                .unwrap()
                .tire(&input.pit_plan.stops[0].tire)
                .is_ok()
        );
        assert!(ResolvedSimulationSessionV3::new(input).is_ok());
    }
}
