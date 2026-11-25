//! Opt-in three-compound operating policy. Never selected by an existing runtime.
//! Public compound names are references; heat coefficients are Pitgun hypotheses.
use crate::tire_performance::{
    CompiledTirePerformanceV1, TirePerformanceProfileV1, compile_tire_performance_v1,
};
use crate::{RacingCatalogSnapshot, thermal_career::ThermalCareerPolicyV1};
use pitgun_contract::{Digest, canonical_json_digest};
use pitgun_racing_solver::{
    FuelMassParamsV3,
    thermal_physics::{ThermalPhysicsV2, TireHeatParamsV2},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TireOperatingPolicyV1 {
    pub schema_version: String,
    pub performance: TirePerformanceProfileV1,
    pub compounds: BTreeMap<String, TireOperatingCompoundV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TireOperatingCompoundV1 {
    pub reference_compound: String,
    /// Own reduced-model hypothesis: endpoints retain 95% of temperature-only grip.
    pub peak_grip_window_c: [f64; 2],
    pub minimum_temperature_grip_factor: f64,
    pub heat: TireHeatParamsV2,
}

#[derive(Debug, Clone, Serialize)]
pub struct CompiledTireOperatingV1 {
    pub policy_digest: Digest,
    pub performance: CompiledTirePerformanceV1,
}

/// Compile the window into the existing Gaussian and retain the existing energy
/// ledger. No heat source, clamp, new wear law or observation-frequency change.
pub fn compile_tire_operating_v1(
    catalog: &RacingCatalogSnapshot,
    baseline: &ThermalCareerPolicyV1,
    policy: &TireOperatingPolicyV1,
) -> Result<CompiledTireOperatingV1, String> {
    policy.performance.validate()?;
    if policy.schema_version != "pitgun.racing-tire-operating/v1" || policy.compounds.len() != 3 {
        return Err("tire operating: exactly soft/C5, medium/C4 and hard/C3 required".into());
    }
    let mut thermal = baseline.clone();
    thermal.compounds.clear();
    for (id, reference) in [("soft", "C5"), ("medium", "C4"), ("hard", "C3")] {
        let c = policy
            .compounds
            .get(id)
            .ok_or("tire operating: missing compound")?;
        let [low, high] = c.peak_grip_window_c;
        let width = (high - low) / (2.0 * (-0.95_f64.ln()).sqrt());
        if c.reference_compound != reference
            || !low.is_finite()
            || !high.is_finite()
            || !(0.0..=200.0).contains(&low)
            || !(0.0..=200.0).contains(&high)
            || high - low < 2.0
            || !c.minimum_temperature_grip_factor.is_finite()
            || !(0.0..0.95).contains(&c.minimum_temperature_grip_factor)
            || (policy.performance.compounds[id].temperature_width_c - width).abs() > 1e-9
            || c.heat.thermal_wear_gain != policy.performance.compounds[id].thermal_wear_gain
        {
            return Err(
                "tire operating: invalid reference, window, grip floor or conflicting coefficients"
                    .into(),
            );
        }
        let mut base = baseline
            .compounds
            .get(id)
            .ok_or("tire operating: missing baseline")?
            .clone();
        base.heat = c.heat;
        base.temperature_optimum_c = (low + high) / 2.0;
        base.temperature_width_c = width;
        base.minimum_temperature_grip_factor = c.minimum_temperature_grip_factor;
        thermal.compounds.insert(id.into(), base);
    }
    let performance = compile_tire_performance_v1(catalog, &thermal, &policy.performance)?;
    // Validate all capacities, partitions and exchanges before any solve, including
    // unused compounds. A malformed future pit replacement must not slip through.
    let check = ThermalPhysicsV2 {
        schema_version: pitgun_racing_solver::thermal_physics::ThermalModelV2::V2,
        engine: thermal.engine,
        compounds: policy
            .compounds
            .iter()
            .map(|(id, c)| {
                Ok(pitgun_racing_solver::thermal_physics::TireCompoundHeatV2 {
                    tire: performance.tire(id)?.clone(),
                    heat: c.heat,
                })
            })
            .collect::<Result<_, String>>()?,
        initial_carcass_temperature_c: policy.performance.mount_temperature_c,
        pit_carcass_temperature_c: policy.performance.mount_temperature_c,
        road_temperature_c: thermal.road_temperature_c,
    };
    check.validate(&FuelMassParamsV3::default())?;
    Ok(CompiledTireOperatingV1 {
        policy_digest: canonical_json_digest(policy).map_err(|e| e.to_string())?,
        performance,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        qualified_career,
        tire_events::{TireWindowPolicyV2, TireWindowPolicyVersionV2},
    };
    use pitgun_racing_solver::tire_temperature_grip_factor;

    fn policy() -> TireOperatingPolicyV1 {
        serde_json::from_str(include_str!(
            "../../../experiments/three_compound_thermal_v1/candidate-v1.json"
        ))
        .unwrap()
    }

    #[test]
    fn named_windows_match_solver_and_recorded_event_thresholds() {
        let catalog = RacingCatalogSnapshot::embedded_qualified_career().unwrap();
        let baseline = qualified_career::profile(&catalog).unwrap();
        let p = policy();
        let compiled = compile_tire_operating_v1(&catalog, &baseline.thermal, &p).unwrap();
        let window = TireWindowPolicyV2 {
            schema_version: TireWindowPolicyVersionV2::V2,
            minimum_peak_grip_milli: 950,
            max_events_per_competitor: 8192,
        };
        for (id, c) in &p.compounds {
            let tire = compiled.performance.tire(id).unwrap();
            let (low, high) = window.bounds(tire).unwrap();
            assert!((low - c.peak_grip_window_c[0]).abs() < 1e-10);
            assert!((high - c.peak_grip_window_c[1]).abs() < 1e-10);
            assert!((tire_temperature_grip_factor(low, tire) - 0.95).abs() < 1e-12);
            assert!(tire_temperature_grip_factor(70., tire) < 0.95);
            assert!(tire_temperature_grip_factor(150., tire) < 0.95);
            assert_eq!(tire_temperature_grip_factor(tire.temp_opt, tire), 1.);
        }
        // The old embedded candidate is immutable; compiling an opt-in policy
        // cannot rewrite its temperature width or fingerprint.
        assert_eq!(baseline.tires.compounds["soft"].temperature_width_c, 117.8);
        assert_ne!(compiled.policy_digest, compiled.performance.policy_digest());
    }

    #[test]
    fn malformed_unused_sets_and_conflicting_coefficients_fail_before_execution() {
        let catalog = RacingCatalogSnapshot::embedded_qualified_career().unwrap();
        let base = qualified_career::profile(&catalog).unwrap();
        let invalid: [fn(&mut TireOperatingPolicyV1); 9] = [
            |p| {
                p.compounds.remove("hard");
            },
            |p| {
                p.compounds
                    .insert("extra".into(), p.compounds["hard"].clone());
            },
            |p| p.compounds.get_mut("hard").unwrap().reference_compound = "C1".into(),
            |p| p.compounds.get_mut("hard").unwrap().peak_grip_window_c = [125., 95.],
            |p| {
                p.compounds
                    .get_mut("hard")
                    .unwrap()
                    .minimum_temperature_grip_factor = 0.95
            },
            |p| {
                p.compounds
                    .get_mut("hard")
                    .unwrap()
                    .heat
                    .rolling_heat_fraction = 1.01
            },
            |p| {
                p.compounds
                    .get_mut("hard")
                    .unwrap()
                    .heat
                    .surface_capacity_j_per_k = -1.
            },
            |p| p.compounds.get_mut("hard").unwrap().heat.thermal_wear_gain = 0.,
            |p| {
                p.performance
                    .compounds
                    .get_mut("hard")
                    .unwrap()
                    .temperature_width_c = f64::NAN
            },
        ];
        for mutate in invalid {
            let mut p = policy();
            mutate(&mut p);
            assert!(compile_tire_operating_v1(&catalog, &base.thermal, &p).is_err());
        }
    }
}
