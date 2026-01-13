use std::collections::{BTreeMap, BTreeSet};

use pitgun_policy::{PlayerTuningRequest, PolicyError, TuningEvalContext, load_tuning_v1_from_str};
use serde_json::json;

const TUNING_POLICY_V1_YAML: &str = include_str!("fixtures/tuning.v1.yaml");

fn tuning_policy_v1() -> pitgun_policy::TuningPolicyV1 {
    load_tuning_v1_from_str(TUNING_POLICY_V1_YAML).expect("policy parses")
}

fn unlocked_ctx() -> TuningEvalContext {
    let mut category_levels = BTreeMap::new();
    category_levels.insert("mech_lvl".to_string(), 10);
    category_levels.insert("testing_lvl".to_string(), 12);
    category_levels.insert("manufacturing_lvl".to_string(), 20);
    category_levels.insert("it_systems_lvl".to_string(), 25);

    let mut owned_upgrades = BTreeSet::new();
    owned_upgrades.insert("e2_turbocharger".to_string());
    owned_upgrades.insert("e2_hybrid_sys".to_string());
    owned_upgrades.insert("e4_active_aero".to_string());
    owned_upgrades.insert("e4_active_suspension".to_string());
    owned_upgrades.insert("e7_precognition".to_string());

    TuningEvalContext {
        era: 3,
        category_levels,
        owned_upgrades,
    }
}

#[test]
fn loads_tuning_policy_v1_and_validates() {
    let policy = tuning_policy_v1();
    policy.validate_static().expect("valid policy");
}

#[test]
fn canonicalize_applies_defaults() {
    let policy = tuning_policy_v1();
    let ctx = unlocked_ctx();
    let req = PlayerTuningRequest {
        parameters: json!({}),
    };

    let canonical = policy.canonicalize(&ctx, &req).expect("canonicalize");
    let front = canonical
        .parameters
        .get("aero")
        .and_then(|value| value.get("front_wing_angle"))
        .and_then(|value| value.as_f64())
        .unwrap();
    let compound = canonical
        .parameters
        .get("chassis")
        .and_then(|value| value.get("tyre_compound"))
        .and_then(|value| value.as_str())
        .unwrap();

    assert_eq!(front, 18.0);
    assert_eq!(compound, "C2_Medium");
}

#[test]
fn canonicalize_clamps_and_quantizes() {
    let policy = tuning_policy_v1();
    let ctx = unlocked_ctx();
    let req = PlayerTuningRequest {
        parameters: json!({
            "powertrain": {
                "turbo_boost_pressure": 2.63,
                "gear_ratio_final": 10.0
            }
        }),
    };

    let canonical = policy.canonicalize(&ctx, &req).expect("canonicalize");
    let turbo = canonical
        .parameters
        .get("powertrain")
        .and_then(|value| value.get("turbo_boost_pressure"))
        .and_then(|value| value.as_f64())
        .unwrap();
    let gear_ratio = canonical
        .parameters
        .get("powertrain")
        .and_then(|value| value.get("gear_ratio_final"))
        .and_then(|value| value.as_f64())
        .unwrap();

    assert_eq!(turbo, 2.6);
    assert_eq!(gear_ratio, 5.0);
}

#[test]
fn unlock_rejects_era_gated_param() {
    let policy = tuning_policy_v1();
    let mut ctx = unlocked_ctx();
    ctx.era = 1;
    let req = PlayerTuningRequest {
        parameters: json!({
            "powertrain": { "fuel_mixture": "rich" }
        }),
    };

    let err = policy.canonicalize(&ctx, &req).unwrap_err();
    match err {
        PolicyError::InvalidField { path, .. } => {
            assert_eq!(path, "parameters.powertrain.fuel_mixture");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn unlock_rejects_upgrade_gated_param() {
    let policy = tuning_policy_v1();
    let mut ctx = unlocked_ctx();
    ctx.owned_upgrades.remove("e2_turbocharger");
    let req = PlayerTuningRequest {
        parameters: json!({
            "powertrain": { "turbo_boost_pressure": 2.0 }
        }),
    };

    let err = policy.canonicalize(&ctx, &req).unwrap_err();
    match err {
        PolicyError::InvalidField { path, .. } => {
            assert_eq!(path, "parameters.powertrain.turbo_boost_pressure");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn unlock_rejects_level_gated_param() {
    let policy = tuning_policy_v1();
    let mut ctx = unlocked_ctx();
    ctx.category_levels.insert("mech_lvl".to_string(), 4);
    let req = PlayerTuningRequest {
        parameters: json!({
            "powertrain": { "gear_ratio_final": 4.0 }
        }),
    };

    let err = policy.canonicalize(&ctx, &req).unwrap_err();
    match err {
        PolicyError::InvalidField { path, .. } => {
            assert_eq!(path, "parameters.powertrain.gear_ratio_final");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn constraint_rejects_wing_balance() {
    let policy = tuning_policy_v1();
    let ctx = unlocked_ctx();
    let req = PlayerTuningRequest {
        parameters: json!({
            "aero": {
                "front_wing_angle": 35.0,
                "rear_wing_angle": 10.0
            }
        }),
    };

    let canonical = policy.canonicalize(&ctx, &req).expect("canonicalize");
    let err = policy
        .validate_constraints(&ctx, &canonical)
        .unwrap_err();
    match err {
        PolicyError::InvalidField { path, reason } => {
            assert_eq!(path, "derived_constraints.wing_balance");
            assert_eq!(reason, "Aero balance extremely unstable.");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn constraint_rejects_turbo_lean_protection() {
    let policy = tuning_policy_v1();
    let ctx = unlocked_ctx();
    let req = PlayerTuningRequest {
        parameters: json!({
            "powertrain": {
                "turbo_boost_pressure": 2.6,
                "fuel_mixture": "lean"
            }
        }),
    };

    let canonical = policy.canonicalize(&ctx, &req).expect("canonicalize");
    let err = policy
        .validate_constraints(&ctx, &canonical)
        .unwrap_err();
    match err {
        PolicyError::InvalidField { path, reason } => {
            assert_eq!(path, "derived_constraints.turbo_lean_protection");
            assert_eq!(reason, "Cannot run high boost with lean mixture (Detonation risk).");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn constraint_rejects_active_suspension_energy() {
    let policy = tuning_policy_v1();
    let ctx = unlocked_ctx();
    let req = PlayerTuningRequest {
        parameters: json!({
            "chassis": { "active_suspension_mode": "full_active" },
            "powertrain": { "ers_deployment_map": "acceleration_bias" }
        }),
    };

    let canonical = policy.canonicalize(&ctx, &req).expect("canonicalize");
    let err = policy
        .validate_constraints(&ctx, &canonical)
        .unwrap_err();
    match err {
        PolicyError::InvalidField { path, reason } => {
            assert_eq!(path, "derived_constraints.active_suspension_energy");
            assert_eq!(
                reason,
                "Full active suspension requires too much power to allow aggressive acceleration bias."
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn canonicalize_rejects_unknown_keys() {
    let policy = tuning_policy_v1();
    let ctx = unlocked_ctx();
    let req = PlayerTuningRequest {
        parameters: json!({ "foo": { "bar": 1 } }),
    };

    let err = policy.canonicalize(&ctx, &req).unwrap_err();
    match err {
        PolicyError::InvalidField { path, .. } => {
            assert_eq!(path, "parameters.foo");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
