//! Explicit native candidate only. A preparation condition is not an in-race thermostat.
//! Source provenance and era applicability live in the separately versioned experiment.
use crate::tire_events::TireWindowPolicyV2;
use pitgun_contract::{Digest, canonical_json_digest};
use pitgun_racing_solver::{ResolvedSimulationRequestV3, ResolvedSimulationSessionV3};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TirePreparationVersionV1 {
    #[serde(rename = "pitgun.racing-tire-preparation/v1")]
    V1,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TirePreparationPolicyV1 {
    pub schema_version: TirePreparationVersionV1,
    pub mount_surface_temperature_c: f64,
    pub mount_carcass_temperature_c: f64,
    /// Prescribed outer-tire limit, not a measured carcass temperature.
    pub maximum_external_preheat_temperature_c: f64,
    pub window: TireWindowPolicyV2,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TirePreparationReceiptV1 {
    pub schema_version: TirePreparationVersionV1,
    pub policy_digest: Digest,
    pub source_input_digest: Digest,
    pub prepared_input_digest: Digest,
}
impl TirePreparationPolicyV1 {
    pub fn validate(&self) -> Result<(), String> {
        self.window.validate()?;
        if ![
            self.mount_surface_temperature_c,
            self.mount_carcass_temperature_c,
            self.maximum_external_preheat_temperature_c,
        ]
        .iter()
        .all(|v| v.is_finite() && (0.0..=150.0).contains(v))
            || self.mount_surface_temperature_c > self.maximum_external_preheat_temperature_c
        {
            return Err("invalid tire preparation temperature or external preheat limit".into());
        }
        Ok(())
    }
}

/// Produces a new complete request atomically. Never infers temperatures from a prior
/// diagnostic cold start or resets unchanged tires at a normal lap boundary.
pub fn prepare_tires_v1(
    input: &ResolvedSimulationRequestV3,
    policy: &TirePreparationPolicyV1,
) -> Result<(ResolvedSimulationRequestV3, TirePreparationReceiptV1), String> {
    policy.validate()?;
    ResolvedSimulationSessionV3::new(input.clone())?;
    if input.thermal_physics_v2.is_none() {
        return Err("tire preparation requires explicit thermal V2".into());
    }
    policy.window.bounds(&input.vehicle.tire)?;
    for stop in &input.pit_plan.stops {
        policy.window.bounds(&stop.tire)?;
    }
    let mut prepared = input.clone();
    prepared.state.tire_temp = policy.mount_surface_temperature_c;
    prepared.config.pit_tire_temp = Some(policy.mount_surface_temperature_c);
    let thermal = prepared.thermal_physics_v2.as_mut().unwrap();
    thermal.initial_carcass_temperature_c = policy.mount_carcass_temperature_c;
    thermal.pit_carcass_temperature_c = policy.mount_carcass_temperature_c;
    ResolvedSimulationSessionV3::new(prepared.clone())?;
    let receipt = TirePreparationReceiptV1 {
        schema_version: TirePreparationVersionV1::V1,
        policy_digest: canonical_json_digest(policy).map_err(|e| e.to_string())?,
        source_input_digest: canonical_json_digest(input).map_err(|e| e.to_string())?,
        prepared_input_digest: canonical_json_digest(&prepared).map_err(|e| e.to_string())?,
    };
    Ok((prepared, receipt))
}
