use std::collections::{BTreeMap, BTreeSet};

use pitgun_policy::{PlayerTuningRequest, PolicyError, TuningEvalContext, load_tuning_v1_from_str};
use serde_json::json;

const RACING_TUNING_POLICY_V1_YAML: &str = include_str!("fixtures/gametuning.v1.yaml");

fn tuning_policy_v1() -> pitgun_policy::TuningPolicyV1 {
    load_tuning_v1_from_str(RACING_TUNING_POLICY_V1_YAML).expect("policy parses")
}

fn gameplay_ctx() -> TuningEvalContext {
    TuningEvalContext {
        era: 3,
        category_levels: BTreeMap::from([("budget_lvl".to_string(), 100)]),
        owned_upgrades: BTreeSet::new(),
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
    let ctx = gameplay_ctx();
    let req = PlayerTuningRequest {
        parameters: json!({}),
    };

    let canonical = policy.canonicalize(&ctx, &req).expect("canonicalize");
    let gameplay = canonical
        .parameters
        .get("gameplay")
        .expect("gameplay section");
    let engine = gameplay
        .get("engine_points")
        .and_then(|value| value.as_f64())
        .unwrap();
    let downforce = gameplay
        .get("downforce_slider")
        .and_then(|value| value.as_f64())
        .unwrap();

    assert_eq!(engine, 0.0);
    assert_eq!(downforce, 0.5);
}

#[test]
fn canonicalize_clamps_and_quantizes() {
    let policy = tuning_policy_v1();
    let ctx = gameplay_ctx();
    let req = PlayerTuningRequest {
        parameters: json!({
            "gameplay": {
                "engine_points": 22.8,
                "aero_points": 999.0,
                "downforce_slider": 0.537,
                "gear_ratio_slider": -5.0
            }
        }),
    };

    let canonical = policy.canonicalize(&ctx, &req).expect("canonicalize");
    let gameplay = canonical
        .parameters
        .get("gameplay")
        .expect("gameplay section");

    assert_eq!(
        gameplay
            .get("engine_points")
            .and_then(|v| v.as_f64())
            .unwrap(),
        23.0
    );
    assert_eq!(
        gameplay
            .get("aero_points")
            .and_then(|v| v.as_f64())
            .unwrap(),
        100.0
    );
    assert_eq!(
        gameplay
            .get("downforce_slider")
            .and_then(|v| v.as_f64())
            .unwrap(),
        0.54
    );
    assert_eq!(
        gameplay
            .get("gear_ratio_slider")
            .and_then(|v| v.as_f64())
            .unwrap(),
        0.0
    );
}

#[test]
fn unlock_rejects_era_gated_param() {
    let policy = tuning_policy_v1();
    let mut ctx = gameplay_ctx();
    ctx.era = 0;
    let req = PlayerTuningRequest {
        parameters: json!({
            "gameplay": { "engine_points": 10.0 }
        }),
    };

    let err = policy.canonicalize(&ctx, &req).unwrap_err();
    match err {
        PolicyError::InvalidField { path, .. } => {
            assert_eq!(path, "parameters.gameplay.engine_points");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn constraint_rejects_budget_cap() {
    let policy = tuning_policy_v1();
    let mut ctx = gameplay_ctx();
    ctx.category_levels.insert("budget_lvl".to_string(), 40);
    let req = PlayerTuningRequest {
        parameters: json!({
            "gameplay": {
                "aero_points": 15.0,
                "chassis_points": 15.0,
                "cooling_points": 15.0,
                "engine_points": 15.0
            }
        }),
    };

    let canonical = policy.canonicalize(&ctx, &req).expect("canonicalize");
    let err = policy.validate_constraints(&ctx, &canonical).unwrap_err();
    match err {
        PolicyError::InvalidField { path, reason } => {
            assert_eq!(path, "derived_constraints.gameplay_budget_cap");
            assert_eq!(reason, "Gameplay setup exceeds available budget.");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn canonicalize_rejects_unknown_keys() {
    let policy = tuning_policy_v1();
    let ctx = gameplay_ctx();
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
