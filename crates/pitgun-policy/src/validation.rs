use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use pitgun_contract::{CompetitorSpec, RaceInput, TuningSpec};
use serde_json::Value as JsonValue;
use thiserror::Error;

use crate::tuning::{
    PlayerTuningRequest, TuningEvalContext, TuningPolicyV1, load_tuning_v1_from_str,
};

pub const POLICY_VERSION: &str = "tuning.v1";
const BUDGET_LEVEL_KEY: &str = "budget_lvl";
const EMBEDDED_TUNING_POLICY_V1: &str = include_str!("../../../policies/gametuning.v1.yaml");

#[derive(Error, Debug)]
pub enum PolicyError {
    #[error("Invalid track ID: {0}")]
    InvalidTrackId(String),
    #[error("Invalid lap count: {0} (must be > 0 and <= {1})")]
    InvalidLapCount(u16, u16),
    #[error("Competitor {0} invalid: {1}")]
    CompetitorError(String, String),
    #[error("Failed to load tuning policy: {0}")]
    PolicyLoad(String),
}

pub fn validate_race_input(input: &RaceInput) -> Result<(), PolicyError> {
    normalize_and_validate_race_input(input, 1).map(|_| ())
}

pub fn normalize_and_validate_race_input(
    input: &RaceInput,
    era: u32,
) -> Result<RaceInput, PolicyError> {
    if input.laps == 0 || input.laps > 100 {
        return Err(PolicyError::InvalidLapCount(input.laps, 100));
    }

    if input.track_id.trim().is_empty() {
        return Err(PolicyError::InvalidTrackId("empty".to_string()));
    }

    let policy = tuning_policy()?;

    let mut normalized_competitors = Vec::with_capacity(input.competitors.len());
    for comp in &input.competitors {
        let normalized = normalize_competitor(policy, comp, era)
            .map_err(|e| PolicyError::CompetitorError(comp.id.clone(), e))?;
        normalized_competitors.push(normalized);
    }

    Ok(RaceInput {
        track_id: input.track_id.clone(),
        laps: input.laps,
        competitors: normalized_competitors,
    })
}

fn tuning_policy() -> Result<&'static TuningPolicyV1, PolicyError> {
    static POLICY: OnceLock<Result<TuningPolicyV1, String>> = OnceLock::new();

    match POLICY.get_or_init(|| {
        let policy =
            load_tuning_v1_from_str(EMBEDDED_TUNING_POLICY_V1).map_err(|err| err.to_string())?;
        policy.validate_static().map_err(|err| err.to_string())?;
        Ok(policy)
    }) {
        Ok(policy) => Ok(policy),
        Err(err) => Err(PolicyError::PolicyLoad(err.clone())),
    }
}

fn tuning_field_names() -> Result<&'static BTreeSet<String>, PolicyError> {
    static FIELD_SET: OnceLock<Result<BTreeSet<String>, String>> = OnceLock::new();

    match FIELD_SET.get_or_init(|| {
        let value = serde_json::to_value(TuningSpec::default()).map_err(|err| err.to_string())?;
        let map = value
            .as_object()
            .ok_or_else(|| "TuningSpec must serialize as an object".to_string())?;
        Ok(map.keys().cloned().collect())
    }) {
        Ok(fields) => Ok(fields),
        Err(err) => Err(PolicyError::PolicyLoad(err.clone())),
    }
}

fn resolve_tuning_subsystem(policy: &TuningPolicyV1) -> Result<&str, String> {
    let fields = tuning_field_names().map_err(|err| err.to_string())?;

    let candidates: Vec<&str> = policy
        .parameters
        .iter()
        .filter(|(_, params)| fields.iter().all(|field| params.contains_key(field)))
        .map(|(name, _)| name.as_str())
        .collect();

    match candidates.len() {
        1 => Ok(candidates[0]),
        0 => Err("no policy subsystem matches the current tuning contract fields".to_string()),
        _ => Err(format!(
            "multiple policy subsystems match tuning contract fields: {}",
            candidates.join(", ")
        )),
    }
}

fn normalize_competitor(
    policy: &TuningPolicyV1,
    comp: &CompetitorSpec,
    era: u32,
) -> Result<CompetitorSpec, String> {
    if !comp.budget_cap.is_finite() || comp.budget_cap < 0.0 {
        return Err("budget_cap must be finite and >= 0".to_string());
    }

    let tuning_subsystem = resolve_tuning_subsystem(policy)?;
    let tuning_value = serde_json::to_value(&comp.tuning)
        .map_err(|err| format!("tuning payload must be finite and serializable: {err}"))?;

    let mut parameters = serde_json::Map::new();
    parameters.insert(tuning_subsystem.to_string(), tuning_value);

    let ctx = TuningEvalContext {
        era,
        category_levels: BTreeMap::from([(
            BUDGET_LEVEL_KEY.to_string(),
            comp.budget_cap.floor() as i64,
        )]),
        owned_upgrades: BTreeSet::new(),
    };
    let req = PlayerTuningRequest {
        parameters: JsonValue::Object(parameters),
    };

    let canonical = policy
        .canonicalize(&ctx, &req)
        .map_err(|err| err.to_string())?;
    policy
        .validate_constraints(&ctx, &canonical)
        .map_err(|err| err.to_string())?;

    let canonical_tuning = canonical
        .parameters
        .get(tuning_subsystem)
        .cloned()
        .ok_or_else(|| format!("missing canonical parameters.{tuning_subsystem} object"))?;

    let tuning: TuningSpec = serde_json::from_value(canonical_tuning)
        .map_err(|err| format!("canonical tuning shape mismatch with contract: {err}"))?;

    Ok(CompetitorSpec {
        id: comp.id.clone(),
        driver_id: comp.driver_id.clone(),
        name: comp.name.clone(),
        team_id: comp.team_id.clone(),
        is_player: comp.is_player,
        tuning,
        budget_cap: comp.budget_cap,
        stint_strategy: comp.stint_strategy.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_competitor(tuning: TuningSpec, budget_cap: f64) -> CompetitorSpec {
        CompetitorSpec {
            id: "c1".into(),
            driver_id: None,
            name: "test".into(),
            team_id: "t1".into(),
            is_player: true,
            budget_cap,
            tuning,
            stint_strategy: None,
        }
    }

    #[test]
    fn validates_valid_input() {
        let input = RaceInput {
            track_id: "spa".into(),
            laps: 10,
            competitors: vec![base_competitor(
                TuningSpec {
                    engine_points: 25.0,
                    cooling_points: 25.0,
                    aero_points: 25.0,
                    chassis_points: 25.0,
                    downforce_slider: 0.5,
                    gear_ratio_slider: 0.5,
                },
                100.0,
            )],
        };
        assert!(normalize_and_validate_race_input(&input, 1).is_ok());
    }

    #[test]
    fn rejects_over_budget_via_policy_constraint() {
        let input = RaceInput {
            track_id: "spa".into(),
            laps: 10,
            competitors: vec![base_competitor(
                TuningSpec {
                    engine_points: 25.0,
                    cooling_points: 25.0,
                    aero_points: 25.0,
                    chassis_points: 25.0,
                    downforce_slider: 0.5,
                    gear_ratio_slider: 0.5,
                },
                90.0,
            )],
        };
        assert!(normalize_and_validate_race_input(&input, 1).is_err());
    }

    #[test]
    fn quantizes_gameplay_points_and_sliders() {
        let input = RaceInput {
            track_id: "spa".into(),
            laps: 10,
            competitors: vec![base_competitor(
                TuningSpec {
                    engine_points: 20.8,
                    cooling_points: 19.4,
                    aero_points: 30.6,
                    chassis_points: 28.2,
                    downforce_slider: 0.533,
                    gear_ratio_slider: 0.497,
                },
                100.0,
            )],
        };

        let normalized = normalize_and_validate_race_input(&input, 1).expect("normalized");
        let t = &normalized.competitors[0].tuning;

        assert_eq!(t.engine_points, 21.0);
        assert_eq!(t.cooling_points, 19.0);
        assert_eq!(t.aero_points, 31.0);
        assert_eq!(t.chassis_points, 28.0);
        assert_eq!(t.downforce_slider, 0.53);
        assert_eq!(t.gear_ratio_slider, 0.5);
    }
}
