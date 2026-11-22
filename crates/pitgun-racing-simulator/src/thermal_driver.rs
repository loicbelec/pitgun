//! Native-only candidate: explicit V2 ability + fixed commitment + accounted costs.
//! No existing workload, catalog, browser or Authority entry selects this policy.
use crate::{RacingCatalogSnapshot, ResolvedV3DriverControlV1, resolve_v3_driver_control_v1};
use pitgun_contract::{Digest, canonical_json_digest};
use pitgun_racing_contract::{
    RacingDriverControlProfileV1, RacingDriverResourceV2, RacingDrivingMode,
};
use pitgun_racing_solver::{
    DriverCorrectionCapacityModelV3, ResolvedSimulationRequestV3, ResolvedSimulationSessionV3,
    driver_lapses::{DriverLapseParamsV1, DriverLapseVersion},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThermalDriverPolicyVersion {
    #[serde(rename = "pitgun.thermal-driver-policy/v1")]
    V1,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThermalDriverPolicyV1 {
    pub schema_version: ThermalDriverPolicyVersion,
    pub controls: RacingDriverControlProfileV1,
    pub base_hazard_per_100km: f64,
    pub commitment_hazard_gain_per_100km: f64,
    pub commitment_hazard_exponent: f64,
    pub exposure_cell_m: f64,
    pub recovery_distance_m: f64,
    pub minimum_engine_request_fraction: f64,
}
impl ThermalDriverPolicyV1 {
    /// Provisional reduced game hypothesis, not calibrated human accident data.
    pub fn from_catalog(snapshot: &RacingCatalogSnapshot) -> Result<Self, String> {
        Ok(Self {
            schema_version: ThermalDriverPolicyVersion::V1,
            controls: *snapshot
                .driver_control_profile()
                .ok_or("missing driver control profile")?,
            base_hazard_per_100km: 0.02,
            commitment_hazard_gain_per_100km: 40.0,
            commitment_hazard_exponent: 4.0,
            exposure_cell_m: 500.0,
            recovery_distance_m: 250.0,
            minimum_engine_request_fraction: 0.15,
        })
    }
    pub fn validate(&self) -> Result<(), String> {
        self.controls.validate().map_err(|e| e.to_string())?;
        if !self.base_hazard_per_100km.is_finite()
            || !(0.0..=10.0).contains(&self.base_hazard_per_100km)
            || !self.commitment_hazard_gain_per_100km.is_finite()
            || !(0.0..=500.0).contains(&self.commitment_hazard_gain_per_100km)
            || !self.commitment_hazard_exponent.is_finite()
            || !(1.0..=8.0).contains(&self.commitment_hazard_exponent)
        {
            return Err("thermal driver: invalid hazard response".into());
        }
        DriverLapseParamsV1 {
            schema_version: DriverLapseVersion::V1,
            noise_stream_id: "validation".into(),
            hazard_per_100km: self.base_hazard_per_100km + self.commitment_hazard_gain_per_100km,
            exposure_cell_m: self.exposure_cell_m,
            recovery_distance_m: self.recovery_distance_m,
            minimum_engine_request_fraction: self.minimum_engine_request_fraction,
        }
        .validate()
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThermalDriverResolutionV1 {
    pub policy_digest: Digest,
    pub driver_resource_digest: Digest,
    pub controls: ResolvedV3DriverControlV1,
    pub lapse_profile: DriverLapseParamsV1,
    pub correction_model: DriverCorrectionCapacityModelV3,
}

/// Explicit trait ablations may use the same driver/noise ID with changed traits.
/// Such offline resources are separately hashed; this is not a catalog admission.
pub fn resolve_thermal_driver_resource_v1(
    driver: &RacingDriverResourceV2,
    mode: RacingDrivingMode,
    policy: &ThermalDriverPolicyV1,
    noise_stream_id: &str,
) -> Result<ThermalDriverResolutionV1, String> {
    policy.validate()?;
    let controls = resolve_v3_driver_control_v1(driver, mode, &policy.controls)?;
    let c = controls.requested_commitment;
    let hazard = policy.base_hazard_per_100km * c * c
        + policy.commitment_hazard_gain_per_100km
            * c.powf(policy.commitment_hazard_exponent)
            * (1.0 - driver.traits.consistency).powi(2);
    let lapse_profile = DriverLapseParamsV1 {
        schema_version: DriverLapseVersion::V1,
        noise_stream_id: noise_stream_id.into(),
        hazard_per_100km: hazard,
        exposure_cell_m: policy.exposure_cell_m,
        recovery_distance_m: policy.recovery_distance_m,
        minimum_engine_request_fraction: policy.minimum_engine_request_fraction,
    };
    lapse_profile.validate()?;
    Ok(ThermalDriverResolutionV1 {
        policy_digest: canonical_json_digest(policy).map_err(|e| e.to_string())?,
        driver_resource_digest: canonical_json_digest(driver).map_err(|e| e.to_string())?,
        controls,
        lapse_profile,
        correction_model: DriverCorrectionCapacityModelV3::EnergyAccountedV1,
    })
}

pub fn resolve_thermal_driver_v1(
    snapshot: &RacingCatalogSnapshot,
    driver_id: &str,
    mode: RacingDrivingMode,
    policy: &ThermalDriverPolicyV1,
    noise_stream_id: &str,
) -> Result<ThermalDriverResolutionV1, String> {
    let resource = snapshot.drivers_v2().get(driver_id).ok_or_else(|| {
        format!("thermal driver: unknown V2 driver {driver_id}; legacy fallback forbidden")
    })?;
    resolve_thermal_driver_resource_v1(resource, mode, policy, noise_stream_id)
}

/// Atomic native-only application. Keep the original historical resolution untouched.
pub fn apply_thermal_driver_v1(
    input: &mut ResolvedSimulationRequestV3,
    resolution: &ThermalDriverResolutionV1,
) -> Result<(), String> {
    if input.thermal_physics_v2.is_none() || input.longitudinal_energy.is_none() {
        return Err("thermal driver requires the explicit energy/thermal candidate".into());
    }
    let mut next = input.clone();
    next.driver.id = resolution.controls.driver_id.clone();
    next.driver.display_name = resolution.controls.driver_id.clone();
    // Compatibility metadata only: V3 physics uses the resolved controls below.
    next.driver.aggressiveness = resolution.controls.requested_commitment;
    next.driver_control = resolution.controls.physical_controls();
    next.driver_correction_workload_multiplier =
        Some(resolution.controls.correction_workload_multiplier);
    next.driver_correction_capacity_model = Some(resolution.correction_model);
    next.driver_lapses_v1 = Some(resolution.lapse_profile.clone());
    ResolvedSimulationSessionV3::new(next.clone())?;
    *input = next;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_traits_replace_legacy_fallback_and_modes_raise_error_cost_and_risk() {
        let snapshot = RacingCatalogSnapshot::embedded_model_v3_timeline().unwrap();
        let policy = ThermalDriverPolicyV1::from_catalog(&snapshot).unwrap();
        let resolve = |id, mode| {
            resolve_thermal_driver_v1(&snapshot, id, mode, &policy, "same-actor-noise").unwrap()
        };
        let manage = resolve("balanced_reference", RacingDrivingMode::Manage);
        let balanced = resolve("balanced_reference", RacingDrivingMode::Balanced);
        let attack = resolve("balanced_reference", RacingDrivingMode::Attack);
        for (a, b) in [(&manage, &balanced), (&balanced, &attack)] {
            assert!(a.controls.cornering_utilization < b.controls.cornering_utilization);
            assert!(a.controls.control_error_amplitude < b.controls.control_error_amplitude);
            assert!(
                a.controls.correction_workload_multiplier
                    < b.controls.correction_workload_multiplier
            );
            assert!(a.lapse_profile.hazard_per_100km < b.lapse_profile.hazard_per_100km);
            assert_eq!(
                a.lapse_profile.noise_stream_id,
                b.lapse_profile.noise_stream_id
            );
        }
        assert_eq!(attack.controls.driver_id, "balanced_reference");
        assert!(
            resolve("limit_specialist", RacingDrivingMode::Attack)
                .controls
                .cornering_utilization
                > attack.controls.cornering_utilization
        );
        assert!(
            resolve("smooth_operator", RacingDrivingMode::Attack)
                .controls
                .control_error_amplitude
                < attack.controls.control_error_amplitude
        );
        assert!(
            resolve_thermal_driver_v1(
                &snapshot,
                "default",
                RacingDrivingMode::Balanced,
                &policy,
                "same-actor-noise"
            )
            .is_err()
        );
    }
    #[test]
    fn one_trait_pairs_keep_ability_mode_and_noise_independent() {
        let snapshot = RacingCatalogSnapshot::embedded_model_v3_timeline().unwrap();
        let policy = ThermalDriverPolicyV1::from_catalog(&snapshot).unwrap();
        let mut driver = snapshot.drivers_v2()["balanced_reference"].clone();
        let reference =
            resolve_thermal_driver_resource_v1(&driver, RacingDrivingMode::Attack, &policy, "pair")
                .unwrap();
        driver.traits.tire_management = 1.0;
        let managed =
            resolve_thermal_driver_resource_v1(&driver, RacingDrivingMode::Attack, &policy, "pair")
                .unwrap();
        assert_eq!(
            managed.controls.control_error_amplitude,
            reference.controls.control_error_amplitude
        );
        assert_eq!(managed.lapse_profile, reference.lapse_profile);
        assert_eq!(managed.controls.correction_workload_multiplier, 1.0);
        driver.traits.consistency = 1.0;
        let consistent =
            resolve_thermal_driver_resource_v1(&driver, RacingDrivingMode::Attack, &policy, "pair")
                .unwrap();
        assert!(
            consistent.controls.control_error_amplitude
                < reference.controls.control_error_amplitude
        );
        assert!(
            consistent.lapse_profile.hazard_per_100km < reference.lapse_profile.hazard_per_100km
        );
        assert_ne!(
            consistent.driver_resource_digest,
            reference.driver_resource_digest
        );
    }
}
