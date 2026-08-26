mod catalog;
pub mod evidence;
mod fuel_contract;
mod thermal_profile;
pub mod workload;

pub use catalog::{
    RacingCatalogBundleV1, RacingCatalogFileV1, RacingCatalogResolutionError, RacingCatalogSnapshot,
};
pub use fuel_contract::{
    RACING_FUEL_CONTRACT_ID, RACING_FUEL_CONTRACT_SCHEMA, RACING_FUEL_CONTRACT_VERSION,
    RacingFuelContractV1, RacingFuelDepletionBehavior,
};
pub use thermal_profile::{
    ResolvedV3PowerUnitThermalProfileV2, ResolvedV3ThermalFamilyProfileV1,
    V3_POWER_UNIT_THERMAL_PROFILE_DIGEST, V3_POWER_UNIT_THERMAL_PROFILE_SCHEMA,
    V3_POWER_UNIT_THERMAL_PROFILE_VERSION, V3_THERMAL_FAMILY_PROFILE_DIGEST,
    V3_THERMAL_FAMILY_PROFILE_ID, V3_THERMAL_FAMILY_PROFILE_SCHEMA,
    V3_THERMAL_FAMILY_PROFILE_VERSION, V3PowerUnitThermalBindingsV2, V3PowerUnitThermalEntryV2,
    V3PowerUnitThermalProfileCandidateV2, V3PowerUnitThermalResolutionContractV2,
    V3PowerUnitThermalResolutionV2, V3ThermalFamilyBindingsV1, V3ThermalFamilyEntryV1,
    V3ThermalFamilyProfileCandidateV1, V3ThermalFamilyResolutionContractV1,
    V3ThermalFamilyResolutionV1, V3ThermalFamilySourceEvidenceV1,
};
pub use workload::{
    RacingWorkload, RacingWorkloadError, racing_model_identity_for_version,
    racing_model_v1_identity, racing_model_v2_identity, racing_model_v3_aero_candidate_identity,
    racing_model_v3_candidate_identity, racing_model_v3_component_candidate_identity,
    racing_model_v3_development_candidate_identity,
    racing_model_v3_driver_control_candidate_identity,
    racing_model_v3_driver_friction_candidate_identity,
    racing_model_v3_fidelity_candidate_identity, racing_model_v3_fuel_contract_candidate_identity,
    racing_model_v3_fuel_mass_candidate_identity, racing_model_v3_mechanical_candidate_identity,
    racing_model_v3_thermal_candidate_identity, racing_model_v3_timeline_candidate_identity,
    racing_model_v3_transmission_candidate_identity,
};

use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::hash::{Hash, Hasher};

use pitgun_contract::{
    ArtifactIdentity, AuthorizationSignatureAlgorithm, RunBundleReceiptV1, RunBundleReceiptVersion,
    RuntimeIdentity, Sample, SampleValue, SignalQuality, TelemetryFrame, canonical_json_digest,
};
use pitgun_racing_contract::{
    CircuitCatalogEntry, CompetitorSpec, CompetitorStintStrategy, ComponentCapabilityDefinitionV1,
    ComponentCapabilityProfileV1, EngineCatalogEntry, RaceInput, RacingDriverControlProfileV1,
    RacingDriverInstructionBoundaryV1, RacingDriverInstructionProfileV1,
    RacingDriverInstructionTimelineV1, RacingDriverResourceV2, RacingDriverTraitsV1,
    RacingDrivingMode, RacingModelParametersV1, RacingPresentationIndexV1,
    ResolvedVehicleCapabilitiesV1, VehicleComponentKind, VehicleComponentSelectionV1,
};
use pitgun_racing_policy::normalize_and_validate_race_input;
use pitgun_racing_solver::resample_telemetry_with_engine_thermal;
#[cfg(test)]
use pitgun_racing_solver::run_simulation;
use pitgun_runtime::execute_linked;
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

pub use pitgun_racing_solver::{
    AERO_FULL_CORNER_CURVATURE_RAD_PER_M, AERO_FULL_STRAIGHT_CURVATURE_RAD_PER_M, AeroParams,
    CORNER_CURVATURE_THRESHOLD_RAD_PER_M, ChassisParams, CircuitDescriptorsV1,
    CurvatureAeroResponse, Driver, DriverControlDiagnosticsV3, DriverControlParamsV3,
    DriverCorrectionCapacityModelV3, DriverEffects, EngineParams, EngineThermalDeratingShapeV3,
    EngineThermalParamsV3, FuelMassDiagnosticsV3, FuelMassParamsV3, MechanicalDiagnosticsV3,
    MechanicalParamsV3, PitPlan, PitStop, ResampledTelemetry, ResolvedDriverControlLapV3,
    ResolvedSimulationRequestV3, SetupResponseDiagnosticsV1, SetupResponseDiagnosticsVersion,
    SimConfig, SimulationRequest, SimulationResult, SimulationSolution, TireContactParamsV3,
    TireDegradationDiagnosticsV3, TireDegradationParamsV3, TireDiagnosticsV3, TireParams, Track,
    Tuning, TuningResponseV1, TuningResponseVersion, VehicleParams, VehicleState,
    apply_driver_to_tire, apply_tuning, apply_tuning_with_response, best_power_at_speed,
    curvature_aero_blend, derating_factor, describe_circuit, diagnose_setup_response,
    driver_effects, effective_mu, power_kw_from_rpm, resample_telemetry as resample_solution,
    rpm_from_speed_gear, run_resolved_simulation_v3 as solve_resolved_v3, run_simulation as solve,
    run_simulation_with_model_response as solve_with_model_response,
    run_simulation_with_tuning_response as solve_with_tuning_response,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunRaceInput {
    #[serde(flatten)]
    pub race: RaceInput,
    #[serde(default)]
    pub vehicle_id: Option<String>,
    /// Optional per-competitor overrides layered over `vehicle_id`.
    ///
    /// Empty historical maps are omitted so immutable legacy inputs retain
    /// their exact wire representation.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub competitor_vehicle_components: HashMap<String, VehicleComponentSelectionV1>,
    #[serde(default)]
    pub pit_strategy: Option<PitStrategyConfig>,
    #[serde(default)]
    pub track_profile: Option<SolverTrackProfile>,
    #[serde(default)]
    pub competitor_profiles: HashMap<String, String>,
    #[serde(default)]
    pub era: i32,
    #[serde(default)]
    pub hz: f64,
    /// Offline V3 experiment input. Published game calls omit it and retain
    /// the historical 100 kg initial load.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial_fuel_mass_kg: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PitStrategyConfig {
    #[serde(default)]
    pub player_pit_laps: Vec<u16>,
    #[serde(default)]
    pub pit_loss_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunRaceRequest {
    pub input: RunRaceInput,
    pub seed: u64,
    #[serde(default)]
    pub era: Option<i32>,
    #[serde(default)]
    pub hz: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum RunRacePayload {
    Wrapped(RunRaceRequest),
    Bare(RunRaceInput),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StandingEntry {
    pub competitor_id: String,
    pub position: u32,
    pub total_time_ms: u64,
    pub best_lap_ms: u64,
    pub laps_completed: u16,
    pub gap_to_leader_ms: u64,
    pub status: StandingStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StandingStatus {
    Finished,
    Dnf { reason: String },
    Dsq { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryEnvelope {
    pub frames: Vec<TelemetryFrame>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaceOutput {
    pub standings: Vec<StandingEntry>,
    pub total_time_ms: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub player_pit_laps: Vec<u16>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub player_lap_times_ms: Vec<u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub player_batches: Vec<TelemetryEnvelope>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub player_diagnostics: Option<SetupResponseDiagnosticsV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub player_tire_diagnostics_v3: Option<TireDiagnosticsV3>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub player_mechanical_diagnostics_v3: Option<MechanicalDiagnosticsV3>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub player_fuel_mass_diagnostics_v3: Option<FuelMassDiagnosticsV3>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub player_tire_degradation_diagnostics_v3: Option<TireDegradationDiagnosticsV3>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub player_thermal_family_resolution_v3: Option<V3ThermalFamilyResolutionV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub player_power_unit_thermal_resolution_v3: Option<V3PowerUnitThermalResolutionV2>,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub competitor_power_unit_thermal_resolutions_v3:
        std::collections::BTreeMap<String, V3PowerUnitThermalResolutionV2>,
    /// Exact installed components and effective controls for every competitor.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub competitor_vehicle_capabilities_v3:
        std::collections::BTreeMap<String, ResolvedVehicleCapabilitiesV1>,
    /// Driver traits, decision and resolved physical controls by competitor.
    /// Emitted only by the offline Model `0.12.0` candidate.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub competitor_driver_control_resolutions_v3:
        std::collections::BTreeMap<String, ResolvedV3DriverControlV1>,
    /// Realized sample distribution for each resolved driver control.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub competitor_driver_control_diagnostics_v3:
        std::collections::BTreeMap<String, DriverControlDiagnosticsV3>,
    /// Common default plus each predeclared mode transition resolved to the
    /// exact physical controls applied for one competitor.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub competitor_driver_instruction_schedules_v3:
        std::collections::BTreeMap<String, ResolvedV3DriverInstructionScheduleV1>,
}

/// Exact overrides for the mechanically resolved Model V3 experiment input.
///
/// Missing values retain the candidate resolver's current behavior. This
/// makes one coefficient independently screenable without copying the whole
/// resolved vehicle or changing the browser and hosted-verification contracts.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct V3MechanicalOverrides {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maximum_brake_force_n: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upshift_rpm: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub downshift_rpm: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shift_duration_s: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shift_power_fraction: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub driveline_efficiency: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fixed_drag_area_m2: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fixed_downforce_area_m2: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chassis_force_transfer_efficiency: Option<f64>,
}

impl V3MechanicalOverrides {
    fn apply_to(self, mut mechanical: MechanicalParamsV3) -> MechanicalParamsV3 {
        macro_rules! apply {
            ($field:ident) => {
                if let Some(value) = self.$field {
                    mechanical.$field = value;
                }
            };
        }
        apply!(maximum_brake_force_n);
        apply!(upshift_rpm);
        apply!(downshift_rpm);
        apply!(shift_duration_s);
        apply!(shift_power_fraction);
        apply!(driveline_efficiency);
        apply!(fixed_drag_area_m2);
        apply!(fixed_downforce_area_m2);
        apply!(chassis_force_transfer_efficiency);
        mechanical
    }
}

/// Gameplay-to-physics aerodynamic resolution for Model V3.
///
/// The setup selects one fixed downforce area. Its drag cost contains a base
/// area plus a quadratic term in the added downforce area, matching the
/// reduced-order aerodynamic polar `Cd = Cd0 + k * Cl^2`. Development points
/// improve efficiency through separate downforce gain and drag reduction.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct V3AeroResolutionParams {
    pub minimum_downforce_area_multiplier: f64,
    pub maximum_downforce_area_multiplier: f64,
    pub base_drag_area_multiplier: f64,
    pub induced_drag_factor_per_m2: f64,
    pub development_downforce_gain_at_cap: f64,
    pub development_drag_reduction_at_cap: f64,
}

impl Default for V3AeroResolutionParams {
    fn default() -> Self {
        Self {
            minimum_downforce_area_multiplier: 0.75,
            maximum_downforce_area_multiplier: 1.25,
            base_drag_area_multiplier: 0.85,
            induced_drag_factor_per_m2: 0.25,
            development_downforce_gain_at_cap: 0.08,
            development_drag_reduction_at_cap: 0.04,
        }
    }
}

impl V3AeroResolutionParams {
    pub fn validate(&self) -> Result<(), String> {
        let values = [
            self.minimum_downforce_area_multiplier,
            self.maximum_downforce_area_multiplier,
            self.base_drag_area_multiplier,
            self.induced_drag_factor_per_m2,
            self.development_downforce_gain_at_cap,
            self.development_drag_reduction_at_cap,
        ];
        if values.iter().any(|value| !value.is_finite()) {
            return Err("V3 aero-resolution coefficients must be finite".to_string());
        }
        if self.minimum_downforce_area_multiplier <= 0.0
            || self.maximum_downforce_area_multiplier <= self.minimum_downforce_area_multiplier
        {
            return Err("V3 downforce multipliers must satisfy 0 < minimum < maximum".to_string());
        }
        if self.maximum_downforce_area_multiplier > 2.0 {
            return Err("V3 maximum downforce multiplier must be <= 2".to_string());
        }
        if self.base_drag_area_multiplier <= 0.0 {
            return Err("V3 base drag multiplier must be positive".to_string());
        }
        if !(0.0..=2.0).contains(&self.induced_drag_factor_per_m2) {
            return Err("V3 induced drag factor must be in [0, 2] 1/m2".to_string());
        }
        for (name, value) in [
            (
                "development downforce gain",
                self.development_downforce_gain_at_cap,
            ),
            (
                "development drag reduction",
                self.development_drag_reduction_at_cap,
            ),
        ] {
            if !(0.0..=0.5).contains(&value) {
                return Err(format!("V3 {name} must be in [0, 0.5]"));
            }
        }
        Ok(())
    }
}

/// Gameplay development resolved into named Model V3 physical quantities.
///
/// Tire friction remains a tire property. Chassis development instead raises
/// the bounded fraction of the theoretical contact-patch force that the
/// suspension can transfer. Engine and cooling points act on torque and heat
/// rejection through separate, inspectable coefficients.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct V3DevelopmentResolutionParams {
    pub chassis_force_transfer_efficiency_without_development: f64,
    pub chassis_force_transfer_efficiency_at_cap: f64,
    pub engine_torque_gain_at_cap: f64,
    pub cooling_capacity_multiplier_without_development: f64,
    pub cooling_capacity_gain_at_cap: f64,
}

/// Relative engine-thermal coefficients exposed only to governed V3 studies.
///
/// The immutable engine resource remains the era-aware reference. Multipliers
/// retain the authored differences between engines while allowing a campaign
/// to vary heat generation, inertia, rejection and derating independently.
/// An identity-valued instance reproduces candidate 0.9 numerically.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct V3EngineThermalResolutionParams {
    /// Multiplier applied to engine thermal capacity, in J/degC.
    pub thermal_capacity_multiplier: f64,
    /// Multiplier applied to the fraction of loaded engine power converted to heat.
    pub heat_generation_multiplier: f64,
    /// Multiplier applied to speed-independent heat rejection, in W/degC.
    pub static_cooling_multiplier: f64,
    /// Multiplier applied to speed-dependent heat rejection, in W/(m/s)/degC.
    pub speed_cooling_multiplier: f64,
    /// Offset applied to the catalog soft-limit temperature, in degC.
    pub soft_limit_offset_c: f64,
    /// Multiplier applied to the catalog linear derating slope, per degC.
    pub derate_slope_multiplier: f64,
    /// Minimum fraction of engine power retained after thermal derating.
    pub minimum_power_fraction: f64,
    /// Shape used when temperature exceeds the soft-limit temperature.
    pub derating_shape: EngineThermalDeratingShapeV3,
    /// Width of the smooth derating onset, in degC; zero for the linear baseline.
    pub smooth_knee_width_c: f64,
    /// Additional fixed drag area at the cooling-development cap, in m2.
    pub cooling_drag_area_m2_at_cap: f64,
}

impl Default for V3EngineThermalResolutionParams {
    fn default() -> Self {
        Self {
            thermal_capacity_multiplier: 1.0,
            heat_generation_multiplier: 1.0,
            static_cooling_multiplier: 1.0,
            speed_cooling_multiplier: 1.0,
            soft_limit_offset_c: 0.0,
            derate_slope_multiplier: 1.0,
            minimum_power_fraction: 0.20,
            derating_shape: EngineThermalDeratingShapeV3::LinearThreshold,
            smooth_knee_width_c: 0.0,
            cooling_drag_area_m2_at_cap: 0.0,
        }
    }
}

impl V3EngineThermalResolutionParams {
    pub fn validate(&self) -> Result<(), String> {
        for (name, value) in [
            (
                "thermal capacity multiplier",
                self.thermal_capacity_multiplier,
            ),
            (
                "heat generation multiplier",
                self.heat_generation_multiplier,
            ),
            ("static cooling multiplier", self.static_cooling_multiplier),
            ("speed cooling multiplier", self.speed_cooling_multiplier),
            ("derate slope multiplier", self.derate_slope_multiplier),
        ] {
            if !value.is_finite() || !(0.1..=4.0).contains(&value) {
                return Err(format!("V3 {name} must be finite and in [0.1, 4]"));
            }
        }
        if !self.soft_limit_offset_c.is_finite()
            || !(-50.0..=50.0).contains(&self.soft_limit_offset_c)
        {
            return Err("V3 thermal soft-limit offset must be in [-50, 50] degC".to_string());
        }
        if !self.minimum_power_fraction.is_finite()
            || !(0.05..=1.0).contains(&self.minimum_power_fraction)
        {
            return Err("V3 thermal minimum-power fraction must be in [0.05, 1]".to_string());
        }
        EngineThermalParamsV3 {
            derating_shape: self.derating_shape,
            smooth_knee_width_c: self.smooth_knee_width_c,
            minimum_power_fraction: self.minimum_power_fraction,
        }
        .validate()?;
        if !self.cooling_drag_area_m2_at_cap.is_finite()
            || !(0.0..=0.5).contains(&self.cooling_drag_area_m2_at_cap)
        {
            return Err("V3 cooling drag area at cap must be in [0, 0.5] m2".to_string());
        }
        Ok(())
    }

    fn apply_to(self, engine: &mut EngineParams) {
        engine.c_th *= self.thermal_capacity_multiplier;
        engine.alpha_heat *= self.heat_generation_multiplier;
        engine.p_cool0 *= self.static_cooling_multiplier;
        engine.k_cool *= self.speed_cooling_multiplier;
        engine.t_soft += self.soft_limit_offset_c;
        engine.beta_derate *= self.derate_slope_multiplier;
    }
}

/// Gameplay gearing resolved as one physically interpretable final drive.
///
/// The normalized setup selects a target vehicle speed. At that speed, top
/// gear reaches the configured fraction of maximum engine speed. The internal
/// spacing between gearbox ratios is deliberately preserved.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct V3TransmissionResolutionParams {
    pub minimum_target_top_speed_mps: f64,
    pub maximum_target_top_speed_mps: f64,
    pub target_engine_speed_fraction: f64,
}

impl Default for V3TransmissionResolutionParams {
    fn default() -> Self {
        Self {
            minimum_target_top_speed_mps: 85.0,
            maximum_target_top_speed_mps: 105.0,
            target_engine_speed_fraction: 0.98,
        }
    }
}

impl V3TransmissionResolutionParams {
    pub fn validate(&self) -> Result<(), String> {
        let values = [
            self.minimum_target_top_speed_mps,
            self.maximum_target_top_speed_mps,
            self.target_engine_speed_fraction,
        ];
        if values.iter().any(|value| !value.is_finite()) {
            return Err("V3 transmission-resolution coefficients must be finite".to_string());
        }
        if self.minimum_target_top_speed_mps < 40.0
            || self.maximum_target_top_speed_mps <= self.minimum_target_top_speed_mps
            || self.maximum_target_top_speed_mps > 150.0
        {
            return Err(
                "V3 target top speeds must satisfy 40 <= minimum < maximum <= 150 m/s".to_string(),
            );
        }
        if !(0.5..=1.0).contains(&self.target_engine_speed_fraction) {
            return Err("V3 target engine-speed fraction must be in [0.5, 1]".to_string());
        }
        Ok(())
    }

    fn target_top_speed_mps(&self, normalized_setup: f64) -> f64 {
        self.minimum_target_top_speed_mps
            + (self.maximum_target_top_speed_mps - self.minimum_target_top_speed_mps)
                * normalized_setup.clamp(0.0, 1.0)
    }
}

impl Default for V3DevelopmentResolutionParams {
    fn default() -> Self {
        Self {
            chassis_force_transfer_efficiency_without_development: 0.97,
            chassis_force_transfer_efficiency_at_cap: 1.0,
            engine_torque_gain_at_cap: 0.06,
            cooling_capacity_multiplier_without_development: 0.75,
            cooling_capacity_gain_at_cap: 0.50,
        }
    }
}

impl V3DevelopmentResolutionParams {
    pub fn validate(&self) -> Result<(), String> {
        let values = [
            self.chassis_force_transfer_efficiency_without_development,
            self.chassis_force_transfer_efficiency_at_cap,
            self.engine_torque_gain_at_cap,
            self.cooling_capacity_multiplier_without_development,
            self.cooling_capacity_gain_at_cap,
        ];
        if values.iter().any(|value| !value.is_finite()) {
            return Err("V3 development-resolution coefficients must be finite".to_string());
        }
        if self.chassis_force_transfer_efficiency_without_development <= 0.0
            || self.chassis_force_transfer_efficiency_at_cap
                < self.chassis_force_transfer_efficiency_without_development
            || self.chassis_force_transfer_efficiency_at_cap > 1.0
        {
            return Err(
                "V3 chassis force-transfer efficiencies must satisfy 0 < base <= cap <= 1"
                    .to_string(),
            );
        }
        if !(0.0..=0.5).contains(&self.engine_torque_gain_at_cap) {
            return Err("V3 engine torque gain must be in [0, 0.5]".to_string());
        }
        if self.cooling_capacity_multiplier_without_development <= 0.0
            || !(0.0..=2.0).contains(&self.cooling_capacity_gain_at_cap)
        {
            return Err(
                "V3 cooling capacity base must be positive and gain must be in [0, 2]".to_string(),
            );
        }
        Ok(())
    }

    fn chassis_force_transfer_efficiency(&self, normalized_points: f64) -> f64 {
        self.chassis_force_transfer_efficiency_without_development
            + (self.chassis_force_transfer_efficiency_at_cap
                - self.chassis_force_transfer_efficiency_without_development)
                * normalized_points
    }
}

/// Versioned, offline-only parameter boundary for Model V3 screening.
///
/// This profile is deliberately absent from the game, WASM, Authority and
/// Verifier. Its canonical JSON digest identifies the exact parameters used by
/// local and Databricks experiments.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum V3CandidateExperimentProfileVersion {
    #[serde(rename = "pitgun.racing-v3-experiment-profile/v1")]
    V1,
    #[serde(rename = "pitgun.racing-v3-experiment-profile/v2")]
    V2,
    #[serde(rename = "pitgun.racing-v3-experiment-profile/v3")]
    V3,
    #[serde(rename = "pitgun.racing-v3-experiment-profile/v4")]
    V4,
    #[serde(rename = "pitgun.racing-v3-experiment-profile/v5")]
    V5,
    #[serde(rename = "pitgun.racing-v3-experiment-profile/v6")]
    V6,
    #[serde(rename = "pitgun.racing-v3-experiment-profile/v7")]
    V7,
    #[serde(rename = "pitgun.racing-v3-experiment-profile/v8")]
    V8,
    #[serde(rename = "pitgun.racing-v3-experiment-profile/v9")]
    V9,
    #[serde(rename = "pitgun.racing-v3-experiment-profile/v10")]
    V10,
    #[serde(rename = "pitgun.racing-v3-experiment-profile/v11")]
    V11,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct V3CandidateExperimentProfile {
    pub schema_version: V3CandidateExperimentProfileVersion,
    pub tuning_response: TuningResponseV1,
    pub tire_contact: TireContactParamsV3,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aero_resolution: Option<V3AeroResolutionParams>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub development_resolution: Option<V3DevelopmentResolutionParams>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transmission_resolution: Option<V3TransmissionResolutionParams>,
    #[serde(default)]
    pub mechanical_overrides: V3MechanicalOverrides,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub driver_control_override: Option<DriverControlParamsV3>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub driver_control_profile: Option<RacingDriverControlProfileV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fuel_mass: Option<FuelMassParamsV3>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tire_degradation: Option<TireDegradationParamsV3>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine_thermal_resolution: Option<V3EngineThermalResolutionParams>,
}

impl Default for V3CandidateExperimentProfile {
    fn default() -> Self {
        Self {
            schema_version: V3CandidateExperimentProfileVersion::V7,
            tuning_response: TuningResponseV1::default(),
            tire_contact: TireContactParamsV3::default(),
            aero_resolution: Some(V3AeroResolutionParams::default()),
            development_resolution: Some(V3DevelopmentResolutionParams::default()),
            transmission_resolution: Some(V3TransmissionResolutionParams::default()),
            mechanical_overrides: V3MechanicalOverrides::default(),
            driver_control_override: None,
            driver_control_profile: None,
            fuel_mass: Some(FuelMassParamsV3::default()),
            tire_degradation: Some(TireDegradationParamsV3::default()),
            engine_thermal_resolution: None,
        }
    }
}

impl V3CandidateExperimentProfile {
    pub fn validate(&self) -> Result<(), String> {
        self.tuning_response.validate()?;
        let resolution = match (
            self.schema_version,
            self.aero_resolution,
            self.development_resolution,
            self.transmission_resolution,
        ) {
            (V3CandidateExperimentProfileVersion::V1, None, None, None) => Ok(()),
            (V3CandidateExperimentProfileVersion::V2, Some(aero), None, None) => aero.validate(),
            (
                V3CandidateExperimentProfileVersion::V3,
                Some(aero),
                Some(development),
                None,
            ) => {
                aero.validate()?;
                development.validate()
            }
            (
                V3CandidateExperimentProfileVersion::V4
                | V3CandidateExperimentProfileVersion::V5
                | V3CandidateExperimentProfileVersion::V6
                | V3CandidateExperimentProfileVersion::V7
                | V3CandidateExperimentProfileVersion::V8
                | V3CandidateExperimentProfileVersion::V9
                | V3CandidateExperimentProfileVersion::V10
                | V3CandidateExperimentProfileVersion::V11,
                Some(aero),
                Some(development),
                Some(transmission),
            ) => {
                aero.validate()?;
                development.validate()?;
                transmission.validate()
            }
            (V3CandidateExperimentProfileVersion::V1, _, _, _) => Err(
                "V3 experiment profile V1 cannot define later resolution resources"
                    .to_string(),
            ),
            (V3CandidateExperimentProfileVersion::V2, _, _, _) => Err(
                "V3 experiment profile V2 requires aero_resolution and forbids development_resolution"
                    .to_string(),
            ),
            (V3CandidateExperimentProfileVersion::V3, _, _, _) => Err(
                "V3 experiment profile V3 requires aero and development resolution and forbids transmission resolution"
                    .to_string(),
            ),
            (V3CandidateExperimentProfileVersion::V4, _, _, _) => Err(
                "V3 experiment profile V4 requires aero, development and transmission resolution"
                    .to_string(),
            ),
            (V3CandidateExperimentProfileVersion::V5, _, _, _) => Err(
                "V3 experiment profile V5 requires aero, development and transmission resolution"
                    .to_string(),
            ),
            (V3CandidateExperimentProfileVersion::V6, _, _, _) => Err(
                "V3 experiment profile V6 requires aero, development and transmission resolution"
                    .to_string(),
            ),
            (V3CandidateExperimentProfileVersion::V7, _, _, _) => Err(
                "V3 experiment profile V7 requires aero, development and transmission resolution"
                    .to_string(),
            ),
            (V3CandidateExperimentProfileVersion::V8, _, _, _) => Err(
                "V3 experiment profile V8 requires aero, development and transmission resolution"
                    .to_string(),
            ),
            (V3CandidateExperimentProfileVersion::V9, _, _, _) => Err(
                "V3 experiment profile V9 requires aero, development and transmission resolution"
                    .to_string(),
            ),
            (V3CandidateExperimentProfileVersion::V10, _, _, _) => Err(
                "V3 experiment profile V10 requires aero, development and transmission resolution"
                    .to_string(),
            ),
            (V3CandidateExperimentProfileVersion::V11, _, _, _) => Err(
                "V3 experiment profile V11 requires aero, development and transmission resolution"
                    .to_string(),
            ),
        };
        resolution?;
        match (self.schema_version, self.fuel_mass) {
            (
                V3CandidateExperimentProfileVersion::V6
                | V3CandidateExperimentProfileVersion::V7
                | V3CandidateExperimentProfileVersion::V8
                | V3CandidateExperimentProfileVersion::V9
                | V3CandidateExperimentProfileVersion::V10
                | V3CandidateExperimentProfileVersion::V11,
                Some(fuel_mass),
            ) => fuel_mass.validate(),
            (
                V3CandidateExperimentProfileVersion::V6
                | V3CandidateExperimentProfileVersion::V7
                | V3CandidateExperimentProfileVersion::V8
                | V3CandidateExperimentProfileVersion::V9
                | V3CandidateExperimentProfileVersion::V10
                | V3CandidateExperimentProfileVersion::V11,
                None,
            ) => Err("V3 experiment profiles V6 through V11 require fuel_mass".to_string()),
            (_, None) => Ok(()),
            (_, Some(_)) => {
                Err("historical V3 experiment profiles cannot define fuel_mass".to_string())
            }
        }?;
        match (self.schema_version, self.tire_degradation) {
            (
                V3CandidateExperimentProfileVersion::V7
                | V3CandidateExperimentProfileVersion::V8
                | V3CandidateExperimentProfileVersion::V9
                | V3CandidateExperimentProfileVersion::V10
                | V3CandidateExperimentProfileVersion::V11,
                Some(tire_degradation),
            ) => tire_degradation.validate(),
            (
                V3CandidateExperimentProfileVersion::V7
                | V3CandidateExperimentProfileVersion::V8
                | V3CandidateExperimentProfileVersion::V9
                | V3CandidateExperimentProfileVersion::V10
                | V3CandidateExperimentProfileVersion::V11,
                None,
            ) => Err("V3 experiment profiles V7 through V11 require tire_degradation".to_string()),
            (_, None) => Ok(()),
            (_, Some(_)) => {
                Err("historical V3 experiment profiles cannot define tire_degradation".to_string())
            }
        }?;
        match (self.schema_version, self.engine_thermal_resolution) {
            (V3CandidateExperimentProfileVersion::V8, Some(thermal)) => thermal.validate(),
            (V3CandidateExperimentProfileVersion::V8, None) => {
                Err("V3 experiment profile V8 requires engine_thermal_resolution".to_string())
            }
            (V3CandidateExperimentProfileVersion::V9, None) => Ok(()),
            (V3CandidateExperimentProfileVersion::V9, Some(thermal)) => thermal.validate(),
            (V3CandidateExperimentProfileVersion::V10, None) => Ok(()),
            (V3CandidateExperimentProfileVersion::V10, Some(thermal)) => thermal.validate(),
            (V3CandidateExperimentProfileVersion::V11, None) => Ok(()),
            (V3CandidateExperimentProfileVersion::V11, Some(thermal)) => thermal.validate(),
            (_, None) => Ok(()),
            (_, Some(_)) => Err(
                "historical V3 experiment profiles cannot define engine_thermal_resolution"
                    .to_string(),
            ),
        }?;
        match (
            self.schema_version,
            self.driver_control_override,
            self.driver_control_profile,
        ) {
            (
                V3CandidateExperimentProfileVersion::V10 | V3CandidateExperimentProfileVersion::V11,
                None,
                Some(profile),
            ) => profile
                .validate()
                .map_err(|error| format!("invalid driver-control profile: {error}")),
            (
                V3CandidateExperimentProfileVersion::V10 | V3CandidateExperimentProfileVersion::V11,
                Some(_),
                _,
            ) => Err(
                "V3 experiment profiles V10 and V11 forbid the legacy driver_control_override"
                    .to_string(),
            ),
            (
                V3CandidateExperimentProfileVersion::V10 | V3CandidateExperimentProfileVersion::V11,
                None,
                None,
            ) => {
                Err("V3 experiment profiles V10 and V11 require driver_control_profile".to_string())
            }
            (_, _, None) => Ok(()),
            (_, _, Some(_)) => Err(
                "historical V3 experiment profiles cannot define driver_control_profile"
                    .to_string(),
            ),
        }
    }

    #[must_use]
    pub fn model_identity(&self) -> ArtifactIdentity {
        match self.schema_version {
            V3CandidateExperimentProfileVersion::V1 => {
                racing_model_v3_mechanical_candidate_identity()
            }
            V3CandidateExperimentProfileVersion::V2 => racing_model_v3_aero_candidate_identity(),
            V3CandidateExperimentProfileVersion::V3 => {
                racing_model_v3_development_candidate_identity()
            }
            V3CandidateExperimentProfileVersion::V4 => {
                racing_model_v3_transmission_candidate_identity()
            }
            V3CandidateExperimentProfileVersion::V5 => {
                racing_model_v3_fidelity_candidate_identity()
            }
            V3CandidateExperimentProfileVersion::V6 => {
                racing_model_v3_fuel_mass_candidate_identity()
            }
            V3CandidateExperimentProfileVersion::V7 => racing_model_v3_candidate_identity(),
            V3CandidateExperimentProfileVersion::V8 => racing_model_v3_thermal_candidate_identity(),
            V3CandidateExperimentProfileVersion::V9 => {
                racing_model_v3_component_candidate_identity()
            }
            V3CandidateExperimentProfileVersion::V10 => {
                racing_model_v3_driver_control_candidate_identity()
            }
            V3CandidateExperimentProfileVersion::V11 => {
                racing_model_v3_driver_friction_candidate_identity()
            }
        }
    }

    fn applies_first_stint_tire(self) -> bool {
        matches!(
            self.schema_version,
            V3CandidateExperimentProfileVersion::V5
                | V3CandidateExperimentProfileVersion::V6
                | V3CandidateExperimentProfileVersion::V7
                | V3CandidateExperimentProfileVersion::V8
                | V3CandidateExperimentProfileVersion::V9
                | V3CandidateExperimentProfileVersion::V10
                | V3CandidateExperimentProfileVersion::V11
        )
    }

    fn accepts_zero_downforce(self) -> bool {
        matches!(
            self.schema_version,
            V3CandidateExperimentProfileVersion::V5
                | V3CandidateExperimentProfileVersion::V6
                | V3CandidateExperimentProfileVersion::V7
                | V3CandidateExperimentProfileVersion::V8
                | V3CandidateExperimentProfileVersion::V9
                | V3CandidateExperimentProfileVersion::V10
                | V3CandidateExperimentProfileVersion::V11
        )
    }
}

/// Explicit offline inputs used to screen driver traits and session modes.
///
/// These maps are deliberately separate from [`RunRaceInput`]: the browser,
/// Authority and Verifier contracts do not accept candidate resources before
/// a reviewed catalog publishes them.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct V3DriverControlExperimentV1 {
    pub drivers: BTreeMap<String, RacingDriverResourceV2>,
    pub competitor_modes: BTreeMap<String, RacingDrivingMode>,
}

/// Offline predeclared timeline used to exercise the future session runtime.
///
/// The instruction profile will ultimately be catalog-owned. Keeping it
/// explicit here lets experiments bind the exact common default and limits
/// without changing a published catalog or hosted-verification identity.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct V3DriverInstructionExperimentV1 {
    pub drivers: BTreeMap<String, RacingDriverResourceV2>,
    pub instruction_profile: RacingDriverInstructionProfileV1,
    pub timeline: RacingDriverInstructionTimelineV1,
}

/// One mode state and its exact physical interpretation from this lap onward.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ResolvedV3DriverInstructionTransitionV1 {
    pub sequence: Option<u32>,
    pub effective_at: RacingDriverInstructionBoundaryV1,
    pub mode: RacingDrivingMode,
    pub physical: ResolvedV3DriverControlV1,
}

/// Canonical lineage for one competitor, including the common lap-zero mode.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ResolvedV3DriverInstructionScheduleV1 {
    pub transitions: Vec<ResolvedV3DriverInstructionTransitionV1>,
}

/// Explainable physical controls resolved for one competitor.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ResolvedV3DriverControlV1 {
    pub driver_id: String,
    pub driver_traits: RacingDriverTraitsV1,
    pub driving_mode: RacingDrivingMode,
    pub requested_commitment: f64,
    pub cornering_utilization: f64,
    pub braking_utilization: f64,
    pub traction_utilization: f64,
    pub control_error_amplitude: f64,
    pub correction_workload_multiplier: f64,
}

impl ResolvedV3DriverControlV1 {
    #[must_use]
    pub const fn physical_controls(&self) -> DriverControlParamsV3 {
        DriverControlParamsV3 {
            cornering_utilization: self.cornering_utilization,
            braking_utilization: self.braking_utilization,
            traction_utilization: self.traction_utilization,
            control_error: self.control_error_amplitude,
        }
    }
}

/// Resolves persistent driver ability and an explicit session decision to
/// physical Solver inputs. No value is sampled or silently clamped.
pub fn resolve_v3_driver_control_v1(
    driver: &RacingDriverResourceV2,
    mode: RacingDrivingMode,
    profile: &RacingDriverControlProfileV1,
) -> Result<ResolvedV3DriverControlV1, String> {
    driver
        .validate()
        .map_err(|error| format!("invalid Racing driver resource: {error}"))?;
    profile
        .validate()
        .map_err(|error| format!("invalid Racing driver-control profile: {error}"))?;

    let commitment = profile.commitment_for(mode);
    let utilization = |response: pitgun_racing_contract::RacingDriverUtilizationResponseV1| {
        response.floor + response.span * driver.traits.limit_exploitation * commitment
    };
    let control_error_amplitude = profile.base_control_error
        + profile.commitment_error_gain
            * commitment.powf(profile.commitment_error_exponent)
            * (1.0 - driver.traits.consistency);
    let correction_workload_multiplier = 1.0
        + profile.correction_workload_gain
            * commitment
            * control_error_amplitude
            * (1.0 - driver.traits.tire_management);

    Ok(ResolvedV3DriverControlV1 {
        driver_id: driver.id.clone(),
        driver_traits: driver.traits,
        driving_mode: mode,
        requested_commitment: commitment,
        cornering_utilization: utilization(profile.cornering),
        braking_utilization: utilization(profile.braking),
        traction_utilization: utilization(profile.traction),
        control_error_amplitude,
        correction_workload_multiplier,
    })
}

impl V3DriverControlExperimentV1 {
    fn resolve_competitor(
        &self,
        competitor: &CompetitorSpec,
        profile: &RacingDriverControlProfileV1,
    ) -> Result<ResolvedV3DriverControlV1, String> {
        let driver = driver_resource_for_competitor(&self.drivers, competitor)?;
        let mode = self
            .competitor_modes
            .get(&competitor.id)
            .copied()
            .ok_or_else(|| format!("missing driving mode for competitor {}", competitor.id))?;
        resolve_v3_driver_control_v1(driver, mode, profile)
    }
}

fn driver_resource_for_competitor<'a>(
    drivers: &'a BTreeMap<String, RacingDriverResourceV2>,
    competitor: &CompetitorSpec,
) -> Result<&'a RacingDriverResourceV2, String> {
    let driver_id = competitor.driver_id.as_deref().ok_or_else(|| {
        format!(
            "V3 driver-control candidate requires driver_id for competitor {}",
            competitor.id
        )
    })?;
    let driver = drivers.get(driver_id).ok_or_else(|| {
        format!(
            "missing V2 driver resource {driver_id:?} for competitor {}",
            competitor.id
        )
    })?;
    if driver.id != driver_id {
        return Err(format!(
            "V2 driver resource key {driver_id:?} does not match embedded id {:?}",
            driver.id
        ));
    }
    Ok(driver)
}

impl V3DriverInstructionExperimentV1 {
    fn validate_for_session(
        &self,
        race: &RaceInput,
        lap_count: u16,
        segment_count: usize,
    ) -> Result<(), String> {
        let competitor_ids = race
            .competitors
            .iter()
            .map(|competitor| competitor.id.clone())
            .collect::<BTreeSet<_>>();
        let segment_count = u32::try_from(segment_count)
            .map_err(|_| "resolved track has too many instruction boundaries".to_string())?;
        self.timeline
            .validate_for_session(
                &self.instruction_profile,
                &competitor_ids,
                lap_count.max(1),
                segment_count,
            )
            .map_err(|error| format!("invalid driver-instruction timeline: {error}"))?;
        for competitor in &race.competitors {
            driver_resource_for_competitor(&self.drivers, competitor)?;
        }
        Ok(())
    }

    fn resolve_competitor(
        &self,
        competitor: &CompetitorSpec,
        control_profile: &RacingDriverControlProfileV1,
    ) -> Result<ResolvedV3DriverInstructionScheduleV1, String> {
        let driver = driver_resource_for_competitor(&self.drivers, competitor)?;
        let baseline = resolve_v3_driver_control_v1(
            driver,
            self.instruction_profile.default_mode,
            control_profile,
        )?;
        let mut transitions = vec![ResolvedV3DriverInstructionTransitionV1 {
            sequence: None,
            effective_at: RacingDriverInstructionBoundaryV1 {
                lap_index: 0,
                segment_index: 0,
            },
            mode: self.instruction_profile.default_mode,
            physical: baseline,
        }];
        for event in self
            .timeline
            .events
            .iter()
            .filter(|event| event.competitor_id == competitor.id)
        {
            transitions.push(ResolvedV3DriverInstructionTransitionV1 {
                sequence: Some(event.sequence),
                effective_at: event.effective_at,
                mode: event.mode,
                physical: resolve_v3_driver_control_v1(driver, event.mode, control_profile)?,
            });
        }
        Ok(ResolvedV3DriverInstructionScheduleV1 { transitions })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolverTrackProfile {
    pub s: Vec<f64>,
    pub x: Vec<f64>,
    pub y: Vec<f64>,
    #[serde(default)]
    pub z: Vec<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    pub session: String,
    pub laps: u16,
    #[serde(default)]
    pub profile_overrides: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRunRequest {
    pub race: RaceInput,
    #[serde(default)]
    pub vehicle_id: Option<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub competitor_vehicle_components: HashMap<String, VehicleComponentSelectionV1>,
    #[serde(default)]
    pub pit_strategy: Option<PitStrategyConfig>,
    #[serde(default)]
    pub track_profile: Option<SolverTrackProfile>,
    #[serde(default)]
    pub sessions: Vec<SessionConfig>,
    pub seed: u64,
    #[serde(default)]
    pub era: i32,
    #[serde(default)]
    pub hz: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRunResult {
    pub session: String,
    pub standings: Vec<StandingEntry>,
    pub total_time_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRunOutput {
    pub sessions: Vec<SessionRunResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogSnapshot {
    pub circuits: Vec<BrowserCircuitCatalogEntry>,
    pub engines: Vec<EngineCatalogEntry>,
    pub vehicles: Vec<VehicleCatalogEntry>,
    pub drivers: Vec<DriverCatalogEntry>,
    pub tires: Vec<TireCatalogEntry>,
    /// Governed component capabilities. Historical catalogs omit this field.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub components: Vec<ComponentCapabilityDefinitionV1>,
    /// Exact catalog resource that authored `components`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub component_capability_profile: Option<ArtifactIdentity>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BrowserCircuitCatalogEntry {
    pub id: String,
    pub display_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub country_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub laps: Option<u16>,
    pub sample_count: usize,
    pub distance_m: f64,
    pub pit_loss_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DriverCatalogEntry {
    pub id: String,
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VehicleCatalogEntry {
    pub id: String,
    pub engine_id: String,
    pub default_tire_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TireCatalogEntry {
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolveVehicleCapabilitiesRequestV1 {
    pub vehicle_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub components: Option<VehicleComponentSelectionV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitDetail {
    pub id: String,
    pub display_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub country_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub laps: Option<u16>,
    pub s_m: Vec<f64>,
    pub x_m: Vec<f64>,
    pub y_m: Vec<f64>,
    pub z_m: Vec<f64>,
    pub curvature_radpm: Vec<f64>,
    pub slope: Vec<f64>,
    pub heading_rad: Vec<f64>,
    pub pit_loss_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineDetail {
    pub id: String,
    pub rpm_samples: Vec<f64>,
    pub torque_samples: Vec<f64>,
    pub gear_ratios: Vec<f64>,
    pub idle_rpm: f64,
    pub max_rpm: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunSimulationRequest {
    #[serde(flatten)]
    pub race: RunRaceRequest,
}

#[derive(Debug, Clone)]
struct VehicleRecord {
    aero_id: String,
    chassis_id: String,
    engine_id: String,
    tire_id: String,
}

#[derive(Debug, Clone)]
struct TrackRecord {
    id: String,
    browser_id: String,
    display_name: String,
    country_code: Option<String>,
    laps: Option<u16>,
    track: Track,
    pit_loss_ms: u64,
}

#[derive(Debug, Clone, Default)]
struct EmbeddedCatalog {
    aeros: HashMap<String, AeroParams>,
    chassis: HashMap<String, ChassisParams>,
    engines: HashMap<String, EngineParams>,
    tires: HashMap<String, TireParams>,
    tracks: HashMap<String, TrackRecord>,
    vehicles: HashMap<String, VehicleRecord>,
    drivers: HashMap<String, Driver>,
    component_capability_profile: Option<ComponentCapabilityProfileV1>,
}

#[derive(Debug, Clone)]
struct SimulatedCompetitor {
    competitor_id: String,
    total_time_ms: u64,
    best_lap_ms: u64,
    laps_completed: u16,
}

#[derive(Debug, Clone)]
struct ResolvedStintPlan {
    tire_by_lap: Vec<String>,
    pit_laps: Vec<u16>,
}

const TELEMETRY_BATCH_SIZE: usize = 64;
const DEFAULT_PIT_LOSS_MS: u64 = 22_000;

const PARAM_TIME_S: u16 = 5000;
const PARAM_DISTANCE_M: u16 = 5001;
const PARAM_X_M: u16 = 5002;
const PARAM_Y_M: u16 = 5003;
const PARAM_HEADING_RAD: u16 = 5004;
const PARAM_SPEED_KPH: u16 = 5005;
const PARAM_RPM: u16 = 5006;
const PARAM_GEAR: u16 = 5007;
const PARAM_THROTTLE_PCT: u16 = 5008;
const PARAM_BRAKE_PCT: u16 = 5009;
const PARAM_G_LAT: u16 = 5010;
const PARAM_G_LONG: u16 = 5011;
const PARAM_G_VERT: u16 = 5012;
const PARAM_ENGINE_TEMP_C: u16 = 5013;
const PARAM_ENGINE_POWER_W: u16 = 5014;
const PARAM_TIRE_TEMP_C: u16 = 5015;
const PARAM_TIRE_WEAR_PCT: u16 = 5016;

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../generated/racing_catalog_v1.rs"
));
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../generated/racing_catalog_model_v2.rs"
));
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../generated/racing_catalog_model_v3_thermal.rs"
));
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../generated/racing_catalog_model_v3_component.rs"
));
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../generated/racing_catalog_model_v3_timeline.rs"
));
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../generated/racing_catalog_model_v3_fuel_contract.rs"
));
const PRESENTATION_INDEX: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../catalogs/racing/v1.0.0/presentation/index.json"
));

pub fn run_race(request: RunRaceRequest) -> Result<RaceOutput, String> {
    let catalog = RacingCatalogSnapshot::embedded()
        .map_err(|error| format!("invalid embedded Racing catalog: {error}"))?;
    run_race_with_catalog(request, &catalog)
}

/// Runs one race against a fully validated immutable catalog snapshot.
pub fn run_race_with_catalog(
    request: RunRaceRequest,
    snapshot: &RacingCatalogSnapshot,
) -> Result<RaceOutput, String> {
    let tuning_response = resolve_catalog_tuning_response(snapshot, None)?;
    run_race_with_catalog_and_tuning_response(request, snapshot, &tuning_response)
}

pub(crate) fn resolve_catalog_tuning_response(
    snapshot: &RacingCatalogSnapshot,
    model: Option<&ArtifactIdentity>,
) -> Result<TuningResponseV1, String> {
    let Some(parameters) = snapshot.model_parameters() else {
        return Ok(TuningResponseV1::default());
    };
    if let Some(model) = model {
        parameters
            .validate_for_model(model)
            .map_err(|error| format!("invalid catalog model parameters: {error}"))?;
    }
    tuning_response_from_model_parameters(parameters)
}

fn tuning_response_from_model_parameters(
    parameters: &RacingModelParametersV1,
) -> Result<TuningResponseV1, String> {
    parameters
        .validate()
        .map_err(|error| format!("invalid catalog model parameters: {error}"))?;
    let development = parameters.development_resolution;
    let setup = parameters.setup_response;
    let aerodynamic = parameters.aerodynamic_state_response;
    let response = TuningResponseV1 {
        schema_version: TuningResponseVersion::V1,
        development_points_cap: development.points_cap_per_axis,
        aero_development_gain: development.aerodynamic_area_gain_at_cap,
        drag_base: setup.drag_area_base_multiplier,
        drag_slider_gain: setup.drag_area_slider_gain,
        downforce_base: setup.downforce_area_base_multiplier,
        downforce_slider_gain: setup.downforce_area_slider_gain,
        straight_aero_scale: aerodynamic.straight_multiplier,
        corner_aero_scale: aerodynamic.corner_multiplier,
        chassis_grip_development_gain: development.chassis_grip_gain_at_cap,
        cooling_base: development.cooling_base_multiplier,
        cooling_development_gain: development.cooling_gain_at_cap,
        engine_torque_development_gain: development.engine_torque_gain_at_cap,
        gear_ratio_base: setup.gear_ratio_base_multiplier,
        gear_ratio_slider_reduction: setup.gear_ratio_slider_reduction,
    };
    response
        .validate()
        .map_err(|error| format!("invalid resolved tuning response: {error}"))?;
    Ok(response)
}

/// Resolves transitional gameplay controls to the physical vehicle accepted by
/// the offline V3 candidate.
///
/// The formulas intentionally reproduce the reviewed Model V2 compatibility
/// response while moving their execution to the Simulator side of the V3
/// boundary. They are not the final Model V3 parameter semantics.
pub fn resolve_v3_physical_vehicle(
    vehicle: &VehicleParams,
    tuning: &Tuning,
    response: &TuningResponseV1,
) -> Result<VehicleParams, String> {
    response
        .validate()
        .map_err(|error| format!("invalid V3 physical resolution response: {error}"))?;

    let points_cap = response.development_points_cap;
    let aero_points = (tuning.aero_points as f64).clamp(0.0, points_cap);
    let chassis_points = (tuning.chassis_points as f64).clamp(0.0, points_cap);
    let cooling_points = (tuning.cooling_points as f64).clamp(0.0, points_cap);
    let engine_points = (tuning.engine_points as f64).clamp(0.0, points_cap);
    let downforce = tuning.downforce_slider.clamp(0.0, 1.0);
    let gearing = tuning.gear_ratio_slider.clamp(0.0, 1.0);

    let aero_gain = 1.0 + response.aero_development_gain * (aero_points / points_cap);
    let drag_multiplier = response.drag_base + response.drag_slider_gain * downforce;
    let downforce_multiplier = response.downforce_base + response.downforce_slider_gain * downforce;
    let aero = AeroParams {
        cd_a_x: vehicle.aero.cd_a_x * aero_gain * drag_multiplier * response.straight_aero_scale,
        cd_a_z: vehicle.aero.cd_a_z * aero_gain * drag_multiplier * response.corner_aero_scale,
        cl_a_x: vehicle.aero.cl_a_x
            * aero_gain
            * downforce_multiplier
            * response.straight_aero_scale,
        cl_a_z: vehicle.aero.cl_a_z * aero_gain * downforce_multiplier * response.corner_aero_scale,
    };

    let grip_multiplier =
        1.0 + response.chassis_grip_development_gain * (chassis_points / points_cap);
    let chassis = ChassisParams {
        mass_empty: vehicle.chassis.mass_empty,
        r_wheel: vehicle.chassis.r_wheel,
        mu0: vehicle.chassis.mu0 * grip_multiplier,
        c_rr: vehicle.chassis.c_rr,
        rho: vehicle.chassis.rho,
        g: vehicle.chassis.g,
    };

    let cooling_multiplier =
        response.cooling_base + response.cooling_development_gain * (cooling_points / points_cap);
    let torque_multiplier =
        1.0 + response.engine_torque_development_gain * (engine_points / points_cap);
    let gear_multiplier = response.gear_ratio_base - response.gear_ratio_slider_reduction * gearing;
    let engine = EngineParams {
        n_rpm: vehicle.engine.n_rpm.clone(),
        trq: vehicle
            .engine
            .trq
            .iter()
            .map(|torque| torque * torque_multiplier)
            .collect(),
        gear_ratios: vehicle
            .engine
            .gear_ratios
            .iter()
            .map(|ratio| ratio * gear_multiplier)
            .collect(),
        n_upshift: vehicle.engine.n_upshift,
        n_downshift: vehicle.engine.n_downshift,
        n_idle: vehicle.engine.n_idle,
        n_max: vehicle.engine.n_max,
        t_amb: vehicle.engine.t_amb,
        t_init: vehicle.engine.t_init,
        c_th: vehicle.engine.c_th,
        alpha_heat: vehicle.engine.alpha_heat,
        p_cool0: vehicle.engine.p_cool0 * cooling_multiplier,
        k_cool: vehicle.engine.k_cool * cooling_multiplier,
        t_soft: vehicle.engine.t_soft,
        beta_derate: vehicle.engine.beta_derate,
        fuel_burn_kg_per_s: vehicle.engine.fuel_burn_kg_per_s,
    };

    Ok(VehicleParams {
        chassis,
        aero,
        engine,
        tire: vehicle.tire.clone(),
    })
}

/// Resolves the current V3 experiment profile to one fixed aerodynamic state.
///
/// Profile V1 retains the immutable 0.3.0 transitional mapping. Profile V2
/// replaces its aerodynamic resolution. Profile V3 also replaces chassis,
/// cooling and engine development. Profile V4 resolves the gearing slider as
/// an explicit target-speed final drive. Profile V5 retains body drag while
/// accepting vehicles with no aerodynamic downforce.
pub fn resolve_v3_physical_vehicle_with_profile(
    vehicle: &VehicleParams,
    tuning: &Tuning,
    profile: &V3CandidateExperimentProfile,
) -> Result<VehicleParams, String> {
    profile
        .validate()
        .map_err(|error| format!("invalid V3 experiment profile: {error}"))?;
    let mut resolved = resolve_v3_physical_vehicle(vehicle, tuning, &profile.tuning_response)?;
    if let Some(development_resolution) = profile.development_resolution {
        let points_cap = profile.tuning_response.development_points_cap;
        let engine_development = (tuning.engine_points as f64).clamp(0.0, points_cap) / points_cap;
        let cooling_development =
            (tuning.cooling_points as f64).clamp(0.0, points_cap) / points_cap;

        // Tire friction is deliberately restored to its catalog value. Model
        // V3 resolves chassis progression through MechanicalParamsV3 instead.
        resolved.chassis.mu0 = vehicle.chassis.mu0;

        let torque_multiplier =
            1.0 + development_resolution.engine_torque_gain_at_cap * engine_development;
        resolved.engine.trq = vehicle
            .engine
            .trq
            .iter()
            .map(|torque| torque * torque_multiplier)
            .collect();

        let cooling_multiplier = development_resolution
            .cooling_capacity_multiplier_without_development
            + development_resolution.cooling_capacity_gain_at_cap * cooling_development;
        resolved.engine.p_cool0 = vehicle.engine.p_cool0 * cooling_multiplier;
        resolved.engine.k_cool = vehicle.engine.k_cool * cooling_multiplier;
    }
    if let Some(engine_thermal_resolution) = profile.engine_thermal_resolution {
        engine_thermal_resolution.apply_to(&mut resolved.engine);
    }
    let Some(aero_resolution) = profile.aero_resolution else {
        return Ok(resolved);
    };

    let reference_drag_area_m2 = 0.5 * (vehicle.aero.cd_a_x + vehicle.aero.cd_a_z);
    let reference_downforce_area_m2 = 0.5 * (vehicle.aero.cl_a_x + vehicle.aero.cl_a_z);
    let invalid_downforce = !reference_downforce_area_m2.is_finite()
        || if profile.accepts_zero_downforce() {
            reference_downforce_area_m2 < 0.0
        } else {
            reference_downforce_area_m2 <= 0.0
        };
    if !reference_drag_area_m2.is_finite() || reference_drag_area_m2 <= 0.0 || invalid_downforce {
        return Err(
            "V3 aero resolution requires positive drag and supported finite downforce areas"
                .to_string(),
        );
    }

    let setup = tuning.downforce_slider.clamp(0.0, 1.0);
    let points_cap = profile.tuning_response.development_points_cap;
    let development = (tuning.aero_points as f64).clamp(0.0, points_cap) / points_cap;
    let setup_downforce_multiplier = aero_resolution.minimum_downforce_area_multiplier
        + (aero_resolution.maximum_downforce_area_multiplier
            - aero_resolution.minimum_downforce_area_multiplier)
            * setup;
    let setup_downforce_area_m2 = reference_downforce_area_m2 * setup_downforce_multiplier;
    let fixed_downforce_area_m2 = setup_downforce_area_m2
        * (1.0 + aero_resolution.development_downforce_gain_at_cap * development);
    let minimum_downforce_area_m2 =
        reference_downforce_area_m2 * aero_resolution.minimum_downforce_area_multiplier;
    let added_downforce_area_m2 = setup_downforce_area_m2 - minimum_downforce_area_m2;
    let mut fixed_drag_area_m2 = (reference_drag_area_m2
        * aero_resolution.base_drag_area_multiplier
        + aero_resolution.induced_drag_factor_per_m2 * added_downforce_area_m2.powi(2))
        * (1.0 - aero_resolution.development_drag_reduction_at_cap * development);
    if let Some(engine_thermal_resolution) = profile.engine_thermal_resolution {
        let cooling_development =
            (tuning.cooling_points as f64).clamp(0.0, points_cap) / points_cap;
        fixed_drag_area_m2 +=
            engine_thermal_resolution.cooling_drag_area_m2_at_cap * cooling_development;
    }

    let invalid_fixed_downforce = !fixed_downforce_area_m2.is_finite()
        || if profile.accepts_zero_downforce() {
            fixed_downforce_area_m2 < 0.0
        } else {
            fixed_downforce_area_m2 <= 0.0
        };
    if !fixed_drag_area_m2.is_finite() || fixed_drag_area_m2 <= 0.0 || invalid_fixed_downforce {
        return Err("V3 aero resolution produced invalid fixed areas".to_string());
    }
    resolved.aero = AeroParams {
        cd_a_x: fixed_drag_area_m2,
        cd_a_z: fixed_drag_area_m2,
        cl_a_x: fixed_downforce_area_m2,
        cl_a_z: fixed_downforce_area_m2,
    };
    if let Some(transmission_resolution) = profile.transmission_resolution {
        resolved.engine.gear_ratios = resolve_v3_transmission_gear_ratios(
            vehicle,
            tuning.gear_ratio_slider,
            &transmission_resolution,
        )?;
    }
    Ok(resolved)
}

/// Resolves one normalized gearbox control as an explicit final drive.
///
/// The selected target speed is reached in top gear at a configured fraction
/// of maximum engine speed. Multiplying all source ratios by one factor keeps
/// their internal spacing unchanged.
pub fn resolve_v3_transmission_gear_ratios(
    vehicle: &VehicleParams,
    normalized_setup: f64,
    parameters: &V3TransmissionResolutionParams,
) -> Result<Vec<f64>, String> {
    parameters.validate()?;
    if !vehicle.chassis.r_wheel.is_finite() || vehicle.chassis.r_wheel <= 0.0 {
        return Err("V3 transmission resolution requires a positive wheel radius".to_string());
    }
    if !vehicle.engine.n_max.is_finite() || vehicle.engine.n_max <= 0.0 {
        return Err("V3 transmission resolution requires a positive maximum RPM".to_string());
    }
    if vehicle.engine.gear_ratios.is_empty()
        || vehicle
            .engine
            .gear_ratios
            .iter()
            .any(|ratio| !ratio.is_finite() || *ratio <= 0.0)
    {
        return Err("V3 transmission resolution requires positive finite gear ratios".to_string());
    }

    let target_speed_mps = parameters.target_top_speed_mps(normalized_setup);
    let target_rpm = parameters.target_engine_speed_fraction * vehicle.engine.n_max;
    let required_top_ratio =
        target_rpm * std::f64::consts::TAU * vehicle.chassis.r_wheel / (target_speed_mps * 60.0);
    let source_top_ratio = *vehicle
        .engine
        .gear_ratios
        .last()
        .expect("non-empty ratios checked above");
    let final_drive_multiplier = required_top_ratio / source_top_ratio;
    if !final_drive_multiplier.is_finite() || final_drive_multiplier <= 0.0 {
        return Err("V3 transmission resolution produced an invalid final drive".to_string());
    }
    Ok(vehicle
        .engine
        .gear_ratios
        .iter()
        .map(|ratio| ratio * final_drive_multiplier)
        .collect())
}

fn theoretical_top_speed_at_max_rpm_kph(vehicle: &VehicleParams) -> Result<f64, String> {
    let top_ratio = vehicle
        .engine
        .gear_ratios
        .last()
        .copied()
        .ok_or_else(|| "V3 transmission diagnostics require a top gear".to_string())?;
    if !top_ratio.is_finite()
        || top_ratio <= 0.0
        || !vehicle.engine.n_max.is_finite()
        || vehicle.engine.n_max <= 0.0
        || !vehicle.chassis.r_wheel.is_finite()
        || vehicle.chassis.r_wheel <= 0.0
    {
        return Err("V3 transmission diagnostics require positive finite inputs".to_string());
    }
    Ok(
        vehicle.engine.n_max * std::f64::consts::TAU * vehicle.chassis.r_wheel / (60.0 * top_ratio)
            * 3.6,
    )
}

/// Resolves the named Model V3 mechanical envelope after vehicle resolution.
///
/// The aerodynamic areas come from the resolved vehicle. Profile V3 additionally
/// maps chassis development to a bounded force-transfer efficiency without
/// changing tire friction. Older profiles retain an efficiency of one and are
/// therefore exactly replayable.
pub fn resolve_v3_mechanical_params(
    physical_vehicle: &VehicleParams,
    tuning: &Tuning,
    profile: &V3CandidateExperimentProfile,
) -> Result<MechanicalParamsV3, String> {
    profile
        .validate()
        .map_err(|error| format!("invalid V3 experiment profile: {error}"))?;
    let mut mechanical = MechanicalParamsV3 {
        fixed_drag_area_m2: 0.5 * (physical_vehicle.aero.cd_a_x + physical_vehicle.aero.cd_a_z),
        fixed_downforce_area_m2: 0.5
            * (physical_vehicle.aero.cl_a_x + physical_vehicle.aero.cl_a_z),
        ..MechanicalParamsV3::default()
    };
    if let Some(development_resolution) = profile.development_resolution {
        let points_cap = profile.tuning_response.development_points_cap;
        let chassis_development =
            (tuning.chassis_points as f64).clamp(0.0, points_cap) / points_cap;
        mechanical.chassis_force_transfer_efficiency =
            development_resolution.chassis_force_transfer_efficiency(chassis_development);
    }
    Ok(profile.mechanical_overrides.apply_to(mechanical))
}

fn resolve_v3_driver_control(driver: &Driver) -> DriverControlParamsV3 {
    let aggressiveness = driver.aggressiveness.clamp(0.0, 1.0);
    DriverControlParamsV3 {
        cornering_utilization: 0.94 + 0.05 * aggressiveness,
        braking_utilization: 0.93 + 0.06 * aggressiveness,
        traction_utilization: 0.95 + 0.04 * aggressiveness,
        control_error: 0.04 - 0.03 * aggressiveness,
    }
}

/// Runs one offline calibration race with an explicit physical tuning response.
///
/// This Rust-only boundary exists for governed experiments. Production, WASM,
/// catalog, and player contracts resolve either their immutable catalog
/// resource or the compiled historical compatibility default.
pub fn run_race_with_catalog_and_tuning_response(
    request: RunRaceRequest,
    snapshot: &RacingCatalogSnapshot,
    tuning_response: &TuningResponseV1,
) -> Result<RaceOutput, String> {
    run_race_with_catalog_and_model_response(
        request,
        snapshot,
        tuning_response,
        CurvatureAeroResponse::LegacyBinary,
    )
}

/// Runs one governed offline experiment with an explicit parameter resource.
///
/// The candidate must target the exact model selected by the caller and the
/// catalog must authorize that same model. This boundary never mutates a
/// catalog release or the public `LATEST` pointer.
pub fn run_race_with_catalog_and_model_parameters(
    request: RunRaceRequest,
    snapshot: &RacingCatalogSnapshot,
    model: &ArtifactIdentity,
    parameters: &RacingModelParametersV1,
    curvature_response: CurvatureAeroResponse,
) -> Result<RaceOutput, String> {
    snapshot
        .manifest()
        .compatibility
        .validate_for(model, pitgun_contract::ContractVersion::V1)
        .map_err(|error| format!("Racing model/catalog incompatibility: {error}"))?;
    parameters
        .validate_for_model(model)
        .map_err(|error| format!("invalid explicit model parameters: {error}"))?;
    let tuning_response = tuning_response_from_model_parameters(parameters)?;
    run_race_with_catalog_and_model_response(
        request,
        snapshot,
        &tuning_response,
        curvature_response,
    )
}

/// Runs an offline model experiment with explicit tuning and curvature responses.
///
/// This boundary is intentionally Rust-only. Player, production, and WASM
/// calls continue to execute the published legacy model.
pub fn run_race_with_catalog_and_model_response(
    request: RunRaceRequest,
    snapshot: &RacingCatalogSnapshot,
    tuning_response: &TuningResponseV1,
    curvature_response: CurvatureAeroResponse,
) -> Result<RaceOutput, String> {
    reject_component_selections_for_model(
        &request.input.competitor_vehicle_components,
        "published historical Racing model",
    )?;
    tuning_response
        .validate()
        .map_err(|error| format!("invalid tuning response: {error}"))?;
    if request.input.race.competitors.is_empty() {
        return Err("race requires at least one competitor".to_string());
    }

    let validation_era = request.era.unwrap_or(request.input.era);
    let normalized_race = normalize_and_validate_race_input(
        &request.input.race,
        if validation_era > 0 {
            validation_era as u32
        } else {
            0
        },
    )
    .map_err(|err| format!("invalid race input: {err}"))?;

    let vehicle_id = resolve_vehicle_id(request.input.vehicle_id.as_deref())?;
    let catalog = EmbeddedCatalog::from_snapshot(snapshot)?;
    run_single_session(
        &catalog,
        &normalized_race,
        vehicle_id,
        &request.input.competitor_vehicle_components,
        SessionExecution {
            pit_strategy: request.input.pit_strategy.as_ref(),
            track_profile: request.input.track_profile.as_ref(),
            laps: normalized_race.laps,
            seed: request.seed,
            initial_fuel_mass_kg: 100.0,
            model: SessionPhysicalModel::Historical {
                tuning_response,
                curvature_response,
            },
        },
    )
}

/// Runs the offline Racing Game Model V3 vertical slice.
///
/// The Simulator resolves gameplay tuning to physical vehicle parameters, then
/// invokes the V3 Solver boundary that cannot accept development points or
/// setup sliders. This candidate deliberately bypasses production model/catalog
/// selection and therefore cannot be authorized or used by the browser.
pub fn run_race_with_catalog_and_v3_candidate(
    request: RunRaceRequest,
    snapshot: &RacingCatalogSnapshot,
    tuning_response: &TuningResponseV1,
) -> Result<RaceOutput, String> {
    let profile = V3CandidateExperimentProfile {
        tuning_response: *tuning_response,
        ..V3CandidateExperimentProfile::default()
    };
    run_race_with_catalog_and_v3_profile(request, snapshot, &profile)
}

/// Runs one governed offline Model V3 experiment with explicit parameters.
///
/// The profile is not part of any published workload contract. It exists so a
/// local campaign and its Databricks replay can execute the same canonical
/// parameter document against the same immutable candidate binary.
pub fn run_race_with_catalog_and_v3_profile(
    request: RunRaceRequest,
    snapshot: &RacingCatalogSnapshot,
    profile: &V3CandidateExperimentProfile,
) -> Result<RaceOutput, String> {
    profile
        .validate()
        .map_err(|error| format!("invalid V3 candidate experiment profile: {error}"))?;
    if request.input.race.competitors.is_empty() {
        return Err("race requires at least one competitor".to_string());
    }

    let validation_era = request.era.unwrap_or(request.input.era);
    let normalized_race = normalize_and_validate_race_input(
        &request.input.race,
        if validation_era > 0 {
            validation_era as u32
        } else {
            0
        },
    )
    .map_err(|err| format!("invalid race input: {err}"))?;
    let vehicle_id = resolve_vehicle_id(request.input.vehicle_id.as_deref())?;
    let catalog = EmbeddedCatalog::from_snapshot(snapshot)?;
    let initial_fuel_mass_kg = request.input.initial_fuel_mass_kg.unwrap_or(100.0);
    if !initial_fuel_mass_kg.is_finite() || !(1.0..=200.0).contains(&initial_fuel_mass_kg) {
        return Err("V3 initial fuel mass must be in [1, 200] kg".to_string());
    }

    run_single_session(
        &catalog,
        &normalized_race,
        vehicle_id,
        &request.input.competitor_vehicle_components,
        SessionExecution {
            pit_strategy: request.input.pit_strategy.as_ref(),
            track_profile: request.input.track_profile.as_ref(),
            laps: normalized_race.laps,
            seed: request.seed,
            initial_fuel_mass_kg,
            model: SessionPhysicalModel::V3Candidate {
                profile,
                power_unit_thermal_profile: None,
                driver_experiment: None,
            },
        },
    )
}

/// Runs the reviewed Model V3 thermal candidate selected by exact vehicle id.
///
/// The candidate can only be constructed from its exact content-addressed JSON
/// bytes. This integration boundary is shared by native Rust and WASM but is
/// not selected by any published catalog or hosted-verification workload yet.
pub fn run_race_with_catalog_and_v3_thermal_family_profile(
    request: RunRaceRequest,
    snapshot: &RacingCatalogSnapshot,
    candidate: &V3ThermalFamilyProfileCandidateV1,
) -> Result<RaceOutput, String> {
    let vehicle_id = resolve_vehicle_id(request.input.vehicle_id.as_deref())?;
    reject_component_selections_for_model(
        &request.input.competitor_vehicle_components,
        "published Model V3 thermal candidate",
    )?;
    let resolved = candidate.resolve_vehicle(vehicle_id)?;
    let profile = V3CandidateExperimentProfile {
        schema_version: V3CandidateExperimentProfileVersion::V8,
        engine_thermal_resolution: Some(resolved.engine_thermal_resolution),
        ..V3CandidateExperimentProfile::default()
    };
    let mut output = run_race_with_catalog_and_v3_profile(request, snapshot, &profile)?;
    output.player_thermal_family_resolution_v3 = Some(resolved.evidence);
    Ok(output)
}

/// Runs Model V3 with thermal coefficients selected independently for every
/// competitor from the exact power unit installed by component composition.
///
/// This is a candidate boundary: catalog 1.5.0 and Model 0.10.0 keep their
/// historical vehicle-bound semantics unchanged.
pub fn run_race_with_catalog_and_v3_power_unit_thermal_profile(
    request: RunRaceRequest,
    snapshot: &RacingCatalogSnapshot,
    candidate: &V3PowerUnitThermalProfileCandidateV2,
) -> Result<RaceOutput, String> {
    candidate.validate()?;
    let profile = V3CandidateExperimentProfile {
        schema_version: V3CandidateExperimentProfileVersion::V9,
        engine_thermal_resolution: None,
        ..V3CandidateExperimentProfile::default()
    };
    run_race_with_catalog_and_v3_component_profile(request, snapshot, &profile, candidate, None)
}

/// Runs the offline Model `0.12.0` driver-control candidate.
///
/// The exact component and thermal resolution from `0.11.0` is retained. The
/// candidate adds explicit V2 driver resources and one deterministic mode per
/// competitor without widening the published browser contract.
pub fn run_race_with_catalog_and_v3_driver_control_profile(
    request: RunRaceRequest,
    snapshot: &RacingCatalogSnapshot,
    profile: &V3CandidateExperimentProfile,
    thermal_candidate: &V3PowerUnitThermalProfileCandidateV2,
    driver_experiment: &V3DriverControlExperimentV1,
) -> Result<RaceOutput, String> {
    run_race_with_catalog_and_v3_component_profile(
        request,
        snapshot,
        profile,
        thermal_candidate,
        Some(V3DriverExperiment::Static(driver_experiment)),
    )
}

/// Runs a predeclared deterministic instruction timeline through the same
/// physical driver-control equations as the static screening candidate.
///
/// This remains an offline boundary. It proves schedule execution before the
/// timeline becomes part of a published catalog and Hosted Verification.
pub fn run_race_with_catalog_and_v3_driver_instruction_profile(
    request: RunRaceRequest,
    snapshot: &RacingCatalogSnapshot,
    profile: &V3CandidateExperimentProfile,
    thermal_candidate: &V3PowerUnitThermalProfileCandidateV2,
    instruction_experiment: &V3DriverInstructionExperimentV1,
) -> Result<RaceOutput, String> {
    run_race_with_catalog_and_v3_component_profile(
        request,
        snapshot,
        profile,
        thermal_candidate,
        Some(V3DriverExperiment::Instructions(instruction_experiment)),
    )
}

/// Runs the catalog-governed Model V3 timeline candidate.
///
/// Release 1.8 owns every physical driver resource, the driver-control
/// coefficients and the instruction profile. The caller supplies only the
/// canonical applied timeline; an empty timeline executes the common catalog
/// default. Selecting a missing or legacy-only driver fails before solving.
pub fn run_race_with_catalog_and_v3_timeline_candidate(
    request: RunRaceRequest,
    snapshot: &RacingCatalogSnapshot,
    timeline: RacingDriverInstructionTimelineV1,
) -> Result<RaceOutput, String> {
    let model = racing_model_v3_timeline_candidate_identity();
    snapshot
        .manifest()
        .compatibility
        .validate_for(&model, pitgun_contract::ContractVersion::V1)
        .map_err(|error| format!("timeline model/catalog incompatibility: {error}"))?;
    let thermal_candidate = snapshot.power_unit_thermal_profile().ok_or_else(|| {
        "timeline candidate catalog has no power-unit thermal profile".to_string()
    })?;
    let driver_control_profile = snapshot.driver_control_profile().copied().ok_or_else(|| {
        "timeline candidate catalog has no physical driver-control profile".to_string()
    })?;
    let instruction_profile = snapshot
        .driver_instruction_profile()
        .copied()
        .ok_or_else(|| {
            "timeline candidate catalog has no driver-instruction profile".to_string()
        })?;
    if snapshot.drivers_v2().is_empty() {
        return Err("timeline candidate catalog has no V2 driver resources".to_string());
    }
    let profile = V3CandidateExperimentProfile {
        schema_version: V3CandidateExperimentProfileVersion::V11,
        driver_control_profile: Some(driver_control_profile),
        ..V3CandidateExperimentProfile::default()
    };
    let experiment = V3DriverInstructionExperimentV1 {
        drivers: snapshot.drivers_v2().clone(),
        instruction_profile,
        timeline,
    };
    run_race_with_catalog_and_v3_driver_instruction_profile(
        request,
        snapshot,
        &profile,
        thermal_candidate,
        &experiment,
    )
}

/// Runs the catalog-governed Model V3 fuel-contract candidate.
///
/// Unlike offline experiments, this published-shaped workload rejects a
/// caller-provided initial load. Both load and consumption coefficients are
/// resolved from the immutable catalog so a browser cannot silently obtain a
/// mass or endurance advantage over hosted replay.
pub fn run_race_with_catalog_and_v3_fuel_contract_candidate(
    mut request: RunRaceRequest,
    snapshot: &RacingCatalogSnapshot,
    timeline: RacingDriverInstructionTimelineV1,
) -> Result<RaceOutput, String> {
    let model = racing_model_v3_fuel_contract_candidate_identity();
    snapshot
        .manifest()
        .compatibility
        .validate_for(&model, pitgun_contract::ContractVersion::V1)
        .map_err(|error| format!("fuel-contract model/catalog incompatibility: {error}"))?;
    if request.input.initial_fuel_mass_kg.is_some() {
        return Err(
            "published fuel-contract workload forbids an initial-fuel override".to_string(),
        );
    }
    let fuel_contract = snapshot
        .fuel_contract()
        .ok_or_else(|| "fuel-contract candidate catalog has no fuel contract".to_string())?;
    let thermal_candidate = snapshot.power_unit_thermal_profile().ok_or_else(|| {
        "fuel-contract candidate catalog has no power-unit thermal profile".to_string()
    })?;
    let driver_control_profile = snapshot.driver_control_profile().copied().ok_or_else(|| {
        "fuel-contract candidate catalog has no physical driver-control profile".to_string()
    })?;
    let instruction_profile = snapshot
        .driver_instruction_profile()
        .copied()
        .ok_or_else(|| {
            "fuel-contract candidate catalog has no driver-instruction profile".to_string()
        })?;
    if snapshot.drivers_v2().is_empty() {
        return Err("fuel-contract candidate catalog has no V2 driver resources".to_string());
    }

    request.input.initial_fuel_mass_kg = Some(fuel_contract.default_initial_fuel_mass_kg);
    let profile = V3CandidateExperimentProfile {
        schema_version: V3CandidateExperimentProfileVersion::V11,
        driver_control_profile: Some(driver_control_profile),
        fuel_mass: Some(fuel_contract.consumption),
        ..V3CandidateExperimentProfile::default()
    };
    let experiment = V3DriverInstructionExperimentV1 {
        drivers: snapshot.drivers_v2().clone(),
        instruction_profile,
        timeline,
    };
    let output = run_race_with_catalog_and_v3_driver_instruction_profile(
        request,
        snapshot,
        &profile,
        thermal_candidate,
        &experiment,
    )?;
    let diagnostics = output
        .player_fuel_mass_diagnostics_v3
        .as_ref()
        .ok_or_else(|| "fuel-contract execution produced no player fuel diagnostics".to_string())?;
    if diagnostics.final_fuel_mass_kg + 1e-9 < fuel_contract.minimum_finish_reserve_kg {
        return Err(format!(
            "fuel-contract finish reserve violated: required {:.6} kg, available {:.6} kg",
            fuel_contract.minimum_finish_reserve_kg, diagnostics.final_fuel_mass_kg
        ));
    }
    Ok(output)
}

fn run_race_with_catalog_and_v3_component_profile(
    request: RunRaceRequest,
    snapshot: &RacingCatalogSnapshot,
    profile: &V3CandidateExperimentProfile,
    candidate: &V3PowerUnitThermalProfileCandidateV2,
    driver_experiment: Option<V3DriverExperiment<'_>>,
) -> Result<RaceOutput, String> {
    profile
        .validate()
        .map_err(|error| format!("invalid V3 component experiment profile: {error}"))?;
    match (profile.schema_version, driver_experiment) {
        (V3CandidateExperimentProfileVersion::V9, None)
        | (
            V3CandidateExperimentProfileVersion::V10 | V3CandidateExperimentProfileVersion::V11,
            Some(_),
        ) => {}
        (V3CandidateExperimentProfileVersion::V9, Some(_)) => {
            return Err(
                "V3 experiment profile V9 cannot define driver-control experiment inputs"
                    .to_string(),
            );
        }
        (
            V3CandidateExperimentProfileVersion::V10 | V3CandidateExperimentProfileVersion::V11,
            None,
        ) => {
            return Err(
                "V3 experiment profiles V10 and V11 require driver-control experiment inputs"
                    .to_string(),
            );
        }
        _ => {
            return Err(
                "power-unit thermal selection requires V3 experiment profile V9, V10 or V11"
                    .to_string(),
            );
        }
    }
    if profile.engine_thermal_resolution.is_some() {
        return Err(
            "power-unit thermal selection rejects a pre-resolved thermal profile".to_string(),
        );
    }
    if request.input.race.competitors.is_empty() {
        return Err("race requires at least one competitor".to_string());
    }
    let validation_era = request.era.unwrap_or(request.input.era);
    let normalized_race = normalize_and_validate_race_input(
        &request.input.race,
        if validation_era > 0 {
            validation_era as u32
        } else {
            0
        },
    )
    .map_err(|err| format!("invalid race input: {err}"))?;
    let vehicle_id = resolve_vehicle_id(request.input.vehicle_id.as_deref())?;
    let catalog = EmbeddedCatalog::from_snapshot(snapshot)?;
    let initial_fuel_mass_kg = request.input.initial_fuel_mass_kg.unwrap_or(100.0);
    if !initial_fuel_mass_kg.is_finite() || !(1.0..=200.0).contains(&initial_fuel_mass_kg) {
        return Err("V3 initial fuel mass must be in [1, 200] kg".to_string());
    }
    run_single_session(
        &catalog,
        &normalized_race,
        vehicle_id,
        &request.input.competitor_vehicle_components,
        SessionExecution {
            pit_strategy: request.input.pit_strategy.as_ref(),
            track_profile: request.input.track_profile.as_ref(),
            laps: normalized_race.laps,
            seed: request.seed,
            initial_fuel_mass_kg,
            model: SessionPhysicalModel::V3Candidate {
                profile,
                power_unit_thermal_profile: Some(candidate),
                driver_experiment,
            },
        },
    )
}

pub fn run_sessions(request: SessionRunRequest) -> Result<SessionRunOutput, String> {
    let catalog = RacingCatalogSnapshot::embedded()
        .map_err(|error| format!("invalid embedded Racing catalog: {error}"))?;
    run_sessions_with_catalog(request, &catalog)
}

/// Runs a session sequence against one pinned validated catalog snapshot.
pub fn run_sessions_with_catalog(
    request: SessionRunRequest,
    snapshot: &RacingCatalogSnapshot,
) -> Result<SessionRunOutput, String> {
    if request.sessions.is_empty() {
        return Err("sessions must be provided explicitly".to_string());
    }
    reject_component_selections_for_model(
        &request.competitor_vehicle_components,
        "published historical Racing session model",
    )?;

    let normalized_race = normalize_and_validate_race_input(
        &request.race,
        if request.era > 0 {
            request.era as u32
        } else {
            0
        },
    )
    .map_err(|err| format!("invalid race input: {err}"))?;
    let vehicle_id = resolve_vehicle_id(request.vehicle_id.as_deref())?;
    let catalog = EmbeddedCatalog::from_snapshot(snapshot)?;
    let mut sessions = Vec::with_capacity(request.sessions.len());
    let tuning_response = resolve_catalog_tuning_response(snapshot, None)?;

    for session in &request.sessions {
        let output = run_single_session(
            &catalog,
            &normalized_race,
            vehicle_id,
            &request.competitor_vehicle_components,
            SessionExecution {
                pit_strategy: request.pit_strategy.as_ref(),
                track_profile: request.track_profile.as_ref(),
                laps: session.laps,
                seed: request.seed,
                initial_fuel_mass_kg: 100.0,
                model: SessionPhysicalModel::Historical {
                    tuning_response: &tuning_response,
                    curvature_response: CurvatureAeroResponse::LegacyBinary,
                },
            },
        )?;
        sessions.push(SessionRunResult {
            session: session.session.clone(),
            standings: output.standings,
            total_time_ms: output.total_time_ms,
        });
    }

    Ok(SessionRunOutput { sessions })
}

struct SessionExecution<'a> {
    pit_strategy: Option<&'a PitStrategyConfig>,
    track_profile: Option<&'a SolverTrackProfile>,
    laps: u16,
    seed: u64,
    initial_fuel_mass_kg: f64,
    model: SessionPhysicalModel<'a>,
}

#[derive(Clone, Copy)]
enum V3DriverExperiment<'a> {
    Static(&'a V3DriverControlExperimentV1),
    Instructions(&'a V3DriverInstructionExperimentV1),
}

#[derive(Clone, Copy)]
enum SessionPhysicalModel<'a> {
    Historical {
        tuning_response: &'a TuningResponseV1,
        curvature_response: CurvatureAeroResponse,
    },
    V3Candidate {
        profile: &'a V3CandidateExperimentProfile,
        power_unit_thermal_profile: Option<&'a V3PowerUnitThermalProfileCandidateV2>,
        driver_experiment: Option<V3DriverExperiment<'a>>,
    },
}

fn run_single_session(
    catalog: &EmbeddedCatalog,
    race: &RaceInput,
    vehicle_id: &str,
    competitor_vehicle_components: &HashMap<String, VehicleComponentSelectionV1>,
    execution: SessionExecution<'_>,
) -> Result<RaceOutput, String> {
    let SessionExecution {
        pit_strategy,
        track_profile,
        laps,
        seed,
        initial_fuel_mass_kg,
        model,
    } = execution;
    let track_id = normalize_track_id(&race.track_id);
    let mut track_record = catalog.get_track(&track_id)?.clone();
    if let Some(payload) = track_profile {
        track_record = track_from_payload(&track_id, payload, track_record.pit_loss_ms)?;
    }

    validate_component_selection_subjects(race, competitor_vehicle_components)?;
    let pit_loss_ms = pit_strategy
        .and_then(|value| value.pit_loss_ms)
        .map(|value| value.max(1_000))
        .unwrap_or(track_record.pit_loss_ms);
    let player_pit_laps = sanitize_pit_laps(
        pit_strategy
            .map(|value| value.player_pit_laps.as_slice())
            .unwrap_or(&[]),
        laps,
    );
    if let SessionPhysicalModel::V3Candidate {
        driver_experiment: Some(V3DriverExperiment::Instructions(experiment)),
        ..
    } = model
    {
        experiment.validate_for_session(race, laps, track_record.track.s.len())?;
    }

    let mut rows = Vec::with_capacity(race.competitors.len());
    let mut player_frames = Vec::new();
    let mut player_lap_times_ms = Vec::new();
    let mut player_resolved_pit_laps = Vec::new();
    let mut player_diagnostics = None;
    let mut player_tire_diagnostics_v3 = None;
    let mut player_mechanical_diagnostics_v3 = None;
    let mut player_fuel_mass_diagnostics_v3 = None;
    let mut player_tire_degradation_diagnostics_v3 = None;
    let mut player_power_unit_thermal_resolution_v3 = None;
    let mut competitor_power_unit_thermal_resolutions_v3 = std::collections::BTreeMap::new();
    let mut competitor_vehicle_capabilities_v3 = std::collections::BTreeMap::new();
    let mut competitor_driver_control_resolutions_v3 = std::collections::BTreeMap::new();
    let mut competitor_driver_control_diagnostics_v3 = std::collections::BTreeMap::new();
    let mut competitor_driver_instruction_schedules_v3 = std::collections::BTreeMap::new();

    for competitor in &race.competitors {
        let component_selection = competitor_vehicle_components.get(&competitor.id);
        if let Some(capabilities) =
            catalog.resolve_vehicle_capabilities(vehicle_id, component_selection)?
        {
            competitor_vehicle_capabilities_v3.insert(competitor.id.clone(), capabilities);
        }
        let resolved_vehicle =
            catalog.resolve_vehicle_with_components(vehicle_id, component_selection)?;
        let effective_v3_profile = match model {
            SessionPhysicalModel::Historical { .. } => None,
            SessionPhysicalModel::V3Candidate {
                profile,
                power_unit_thermal_profile,
                driver_experiment: _,
            } => {
                let mut resolved_profile = *profile;
                if let Some(candidate) = power_unit_thermal_profile {
                    let power_unit_id = catalog
                        .resolve_engine_id_with_components(vehicle_id, component_selection)?;
                    let thermal = candidate.resolve_power_unit(power_unit_id)?;
                    resolved_profile.engine_thermal_resolution =
                        Some(thermal.engine_thermal_resolution);
                    competitor_power_unit_thermal_resolutions_v3
                        .insert(competitor.id.clone(), thermal.evidence.clone());
                    if competitor.is_player || competitor.id == "player" {
                        player_power_unit_thermal_resolution_v3 = Some(thermal.evidence);
                    }
                }
                resolved_profile.validate().map_err(|error| {
                    format!(
                        "cannot resolve V3 profile for competitor {}: {error}",
                        competitor.id
                    )
                })?;
                Some(resolved_profile)
            }
        };
        let stint_plan =
            resolve_stint_plan(competitor, laps, &resolved_vehicle.1, &player_pit_laps)?;
        let initial_vehicle = match effective_v3_profile {
            Some(profile) if profile.applies_first_stint_tire() => {
                let tire_id = stint_plan
                    .tire_by_lap
                    .first()
                    .ok_or_else(|| "resolved stint plan has no initial tire".to_string())?;
                let mut vehicle = resolved_vehicle.0.clone();
                vehicle.tire = catalog.resolve_tire(tire_id)?;
                vehicle
            }
            _ => resolved_vehicle.0.clone(),
        };
        let (driver_control_resolution, driver_instruction_schedule) =
            match (effective_v3_profile.as_ref(), model) {
                (
                    Some(profile),
                    SessionPhysicalModel::V3Candidate {
                        driver_experiment: Some(V3DriverExperiment::Static(experiment)),
                        ..
                    },
                ) if matches!(
                    profile.schema_version,
                    V3CandidateExperimentProfileVersion::V10
                        | V3CandidateExperimentProfileVersion::V11
                ) =>
                {
                    let control_profile =
                        profile.driver_control_profile.as_ref().ok_or_else(|| {
                            "V3 driver-control experiment profile has no resolved profile"
                                .to_string()
                        })?;
                    (
                        Some(experiment.resolve_competitor(competitor, control_profile)?),
                        None,
                    )
                }
                (
                    Some(profile),
                    SessionPhysicalModel::V3Candidate {
                        driver_experiment: Some(V3DriverExperiment::Instructions(experiment)),
                        ..
                    },
                ) if matches!(
                    profile.schema_version,
                    V3CandidateExperimentProfileVersion::V10
                        | V3CandidateExperimentProfileVersion::V11
                ) =>
                {
                    let control_profile =
                        profile.driver_control_profile.as_ref().ok_or_else(|| {
                            "V3 driver-control experiment profile has no resolved profile"
                                .to_string()
                        })?;
                    let schedule = experiment.resolve_competitor(competitor, control_profile)?;
                    let baseline = schedule
                        .transitions
                        .first()
                        .expect("resolved instruction schedule always has a baseline")
                        .physical
                        .clone();
                    (Some(baseline), Some(schedule))
                }
                _ => (None, None),
            };
        if let Some(resolution) = driver_control_resolution.as_ref() {
            competitor_driver_control_resolutions_v3
                .insert(competitor.id.clone(), resolution.clone());
        }
        if let Some(schedule) = driver_instruction_schedule.as_ref() {
            competitor_driver_instruction_schedules_v3
                .insert(competitor.id.clone(), schedule.clone());
        }
        let mut driver = catalog.resolve_driver(competitor.driver_id.as_deref())?;
        if driver_control_resolution.is_some() {
            driver.id = competitor
                .driver_id
                .clone()
                .expect("validated driver-control candidate driver id");
        }
        let sim_config = SimConfig {
            ds: track_record
                .track
                .s
                .windows(2)
                .next()
                .map(|window| window[1] - window[0])
                .unwrap_or(1.0),
            max_speed: 400.0,
            pit_time_penalty_s: pit_loss_ms as f64 / 1000.0,
            pit_tire_temp: None,
            tire_temp_amb: 35.0,
            sim_seed: seed,
        };

        let pit_plan = build_pit_plan(catalog, &stint_plan)?;
        let tuning = Tuning {
            engine_points: competitor.tuning.engine_points.round() as i32,
            cooling_points: competitor.tuning.cooling_points.round() as i32,
            aero_points: competitor.tuning.aero_points.round() as i32,
            chassis_points: competitor.tuning.chassis_points.round() as i32,
            downforce_slider: competitor.tuning.downforce_slider,
            gear_ratio_slider: competitor.tuning.gear_ratio_slider,
        };
        let request = SimulationRequest {
            track: track_record.track.clone(),
            vehicle: initial_vehicle.clone(),
            state: VehicleState {
                fuel_mass: initial_fuel_mass_kg,
                tire_wear: 0.0,
                tire_temp: 90.0,
                engine_temp: initial_vehicle.engine.t_init,
            },
            config: sim_config,
            lap_count: laps.max(1),
            pit_plan,
            driver,
            tuning: Some(tuning.clone()),
        };
        let mut result = match (model, effective_v3_profile.as_ref()) {
            (
                SessionPhysicalModel::Historical {
                    tuning_response,
                    curvature_response,
                },
                None,
            ) => solve_with_model_response(&request, tuning_response, curvature_response),
            (SessionPhysicalModel::V3Candidate { .. }, Some(profile)) => {
                let physical_vehicle =
                    resolve_v3_physical_vehicle_with_profile(&request.vehicle, &tuning, profile)
                        .map_err(|error| {
                            format!(
                                "cannot resolve V3 physical vehicle for competitor {}: {error}",
                                competitor.id
                            )
                        })?;
                let mechanical = resolve_v3_mechanical_params(&physical_vehicle, &tuning, profile)
                    .map_err(|error| {
                        format!(
                            "cannot resolve V3 mechanical envelope for competitor {}: {error}",
                            competitor.id
                        )
                    })?;
                let driver_control = driver_control_resolution
                    .as_ref()
                    .map(ResolvedV3DriverControlV1::physical_controls)
                    .or(profile.driver_control_override)
                    .unwrap_or_else(|| resolve_v3_driver_control(&request.driver));
                solve_resolved_v3(&ResolvedSimulationRequestV3 {
                    track: request.track.clone(),
                    vehicle: physical_vehicle,
                    state: request.state.clone(),
                    config: request.config.clone(),
                    lap_count: request.lap_count,
                    pit_plan: request.pit_plan.clone(),
                    driver: request.driver.clone(),
                    tire_contact: profile.tire_contact,
                    mechanical,
                    driver_control,
                    driver_correction_workload_multiplier: driver_control_resolution
                        .as_ref()
                        .map(|resolution| resolution.correction_workload_multiplier),
                    driver_control_schedule: driver_instruction_schedule
                        .as_ref()
                        .map(|schedule| {
                            schedule
                                .transitions
                                .iter()
                                .skip(1)
                                .map(|transition| ResolvedDriverControlLapV3 {
                                    lap_index: transition.effective_at.lap_index,
                                    driver_control: transition.physical.physical_controls(),
                                    correction_workload_multiplier: Some(
                                        transition.physical.correction_workload_multiplier,
                                    ),
                                })
                                .collect()
                        })
                        .unwrap_or_default(),
                    driver_correction_capacity_model: (profile.schema_version
                        == V3CandidateExperimentProfileVersion::V11)
                        .then_some(DriverCorrectionCapacityModelV3::FrictionBudgetV1),
                    fuel_mass: profile.fuel_mass,
                    tire_degradation: profile.tire_degradation,
                    engine_thermal: profile.engine_thermal_resolution.map(|thermal| {
                        EngineThermalParamsV3 {
                            derating_shape: thermal.derating_shape,
                            smooth_knee_width_c: thermal.smooth_knee_width_c,
                            minimum_power_fraction: thermal.minimum_power_fraction,
                        }
                    }),
                })
            }
            _ => unreachable!("session model and resolved V3 profile must agree"),
        }
        .map_err(|err| format!("simulation failed for competitor {}: {err}", competitor.id))?;

        if let Some(diagnostics) = result.driver_control_diagnostics_v3 {
            competitor_driver_control_diagnostics_v3.insert(competitor.id.clone(), diagnostics);
        }

        if effective_v3_profile.is_some_and(|profile| profile.transmission_resolution.is_some())
            && let Some(diagnostics) = result.mechanical_diagnostics_v3.as_mut()
        {
            diagnostics.theoretical_top_speed_at_max_rpm_kph = Some(
                theoretical_top_speed_at_max_rpm_kph(&result.applied_vehicle)?,
            );
        }

        let lap_times_ms = lap_times_ms(&result.lap_times_s, &stint_plan, pit_loss_ms);
        let total_time_ms = lap_times_ms.iter().copied().sum::<u64>();
        let best_lap_ms = lap_times_ms.iter().copied().min().unwrap_or(0);

        if competitor.is_player || competitor.id == "player" {
            let telemetry_hz = 5.0;
            let engine_thermal = match effective_v3_profile {
                Some(profile) => {
                    profile
                        .engine_thermal_resolution
                        .map(|thermal| EngineThermalParamsV3 {
                            derating_shape: thermal.derating_shape,
                            smooth_knee_width_c: thermal.smooth_knee_width_c,
                            minimum_power_fraction: thermal.minimum_power_fraction,
                        })
                }
                None => None,
            };
            let resampled = resample_telemetry_with_engine_thermal(
                &request.track,
                &result.solution,
                &result.applied_vehicle,
                5.0,
                engine_thermal,
            )
            .map_err(|err| format!("telemetry resampling failed: {err}"))?;
            player_frames = gateway_frames_from_resampled(
                &resampled,
                telemetry_session_id(seed, &track_id, &competitor.id),
                &format!("pitwall-sim:{}", competitor.id),
                &telemetry_metadata(
                    &track_id,
                    vehicle_id,
                    &competitor.id,
                    &request.driver.id,
                    &stint_plan,
                    telemetry_hz,
                ),
            );
            player_lap_times_ms = lap_times_ms.clone();
            player_resolved_pit_laps = stint_plan.pit_laps.clone();
            player_diagnostics = Some(result.diagnostics);
            player_tire_diagnostics_v3 = result.tire_diagnostics_v3;
            player_mechanical_diagnostics_v3 = result.mechanical_diagnostics_v3;
            player_fuel_mass_diagnostics_v3 = result.fuel_mass_diagnostics_v3;
            player_tire_degradation_diagnostics_v3 = result.tire_degradation_diagnostics_v3;
        }

        rows.push(SimulatedCompetitor {
            competitor_id: competitor.id.clone(),
            total_time_ms,
            best_lap_ms,
            laps_completed: laps.max(1),
        });
    }

    rows.sort_by_key(|row| row.total_time_ms);
    let leader = rows.first().map(|row| row.total_time_ms).unwrap_or(0);
    let standings = rows
        .iter()
        .enumerate()
        .map(|(idx, row)| StandingEntry {
            competitor_id: row.competitor_id.clone(),
            position: (idx + 1) as u32,
            total_time_ms: row.total_time_ms,
            best_lap_ms: row.best_lap_ms,
            laps_completed: row.laps_completed,
            gap_to_leader_ms: row.total_time_ms.saturating_sub(leader),
            status: StandingStatus::Finished,
        })
        .collect::<Vec<_>>();

    Ok(RaceOutput {
        standings,
        total_time_ms: leader,
        player_pit_laps: player_resolved_pit_laps,
        player_lap_times_ms,
        player_batches: telemetry_batches(player_frames),
        player_diagnostics,
        player_tire_diagnostics_v3,
        player_mechanical_diagnostics_v3,
        player_fuel_mass_diagnostics_v3,
        player_tire_degradation_diagnostics_v3,
        player_thermal_family_resolution_v3: None,
        player_power_unit_thermal_resolution_v3,
        competitor_power_unit_thermal_resolutions_v3,
        competitor_vehicle_capabilities_v3,
        competitor_driver_control_resolutions_v3,
        competitor_driver_control_diagnostics_v3,
        competitor_driver_instruction_schedules_v3,
    })
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn run_simulation_json(input_json: String) -> String {
    run_race_json(input_json)
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn run_race_json(input_json: String) -> String {
    let request = match parse_run_race_request(&input_json) {
        Ok(request) => request,
        Err(error) => return json_error(&error),
    };

    match run_race(request) {
        Ok(output) => serialize_json(&output),
        Err(error) => json_error(&error),
    }
}

/// Browser facade for a race using caller-fetched immutable catalog bytes.
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn run_race_with_catalog_json(input_json: String, catalog_bundle_json: String) -> String {
    let request = match parse_run_race_request(&input_json) {
        Ok(request) => request,
        Err(error) => return json_error(&error),
    };
    let catalog = match RacingCatalogSnapshot::from_bundle_json(&catalog_bundle_json) {
        Ok(catalog) => catalog,
        Err(error) => {
            return json_error(&format!("invalid Racing catalog: {error}"));
        }
    };

    match run_race_with_catalog(request, &catalog) {
        Ok(output) => serialize_json(&output),
        Err(error) => json_error(&error),
    }
}

/// Browser facade for the reviewed family-specific Model V3 thermal candidate.
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn run_race_with_catalog_and_v3_thermal_family_profile_json(
    input_json: String,
    catalog_bundle_json: String,
    candidate_json: String,
) -> String {
    let request = match parse_run_race_request(&input_json) {
        Ok(request) => request,
        Err(error) => return json_error(&error),
    };
    let catalog = match RacingCatalogSnapshot::from_bundle_json(&catalog_bundle_json) {
        Ok(catalog) => catalog,
        Err(error) => return json_error(&format!("invalid Racing catalog: {error}")),
    };
    let candidate = match V3ThermalFamilyProfileCandidateV1::from_exact_json(&candidate_json) {
        Ok(candidate) => candidate,
        Err(error) => return json_error(&error),
    };

    match run_race_with_catalog_and_v3_thermal_family_profile(request, &catalog, &candidate) {
        Ok(output) => serialize_json(&output),
        Err(error) => json_error(&error),
    }
}

/// Browser facade for per-competitor power-unit thermal selection.
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn run_race_with_catalog_and_v3_power_unit_thermal_profile_json(
    input_json: String,
    catalog_bundle_json: String,
    candidate_json: String,
) -> String {
    let request = match parse_run_race_request(&input_json) {
        Ok(request) => request,
        Err(error) => return json_error(&error),
    };
    let catalog = match RacingCatalogSnapshot::from_bundle_json(&catalog_bundle_json) {
        Ok(catalog) => catalog,
        Err(error) => return json_error(&format!("invalid Racing catalog: {error}")),
    };
    let candidate = match V3PowerUnitThermalProfileCandidateV2::from_exact_json(&candidate_json) {
        Ok(candidate) => candidate,
        Err(error) => return json_error(&error),
    };

    match run_race_with_catalog_and_v3_power_unit_thermal_profile(request, &catalog, &candidate) {
        Ok(output) => serialize_json(&output),
        Err(error) => json_error(&error),
    }
}

/// Executes one signed Racing contract through the embedded immutable catalog.
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn execute_authorized_race_json(request_json: String) -> String {
    let request =
        match serde_json::from_str::<evidence::RacingHostedExecutionRequestV1>(&request_json) {
            Ok(request) => request,
            Err(error) => return json_error(&format!("invalid hosted execution request: {error}")),
        };
    let catalog = match RacingCatalogSnapshot::embedded() {
        Ok(catalog) => catalog,
        Err(error) => return json_error(&format!("invalid embedded Racing catalog: {error}")),
    };

    match execute_authorized_race(request, &catalog) {
        Ok(submission) => serialize_json(&submission),
        Err(error) => json_error(&error),
    }
}

/// Executes one signed Racing contract and exposes its application projection.
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn execute_authorized_race_application_json(request_json: String) -> String {
    let request =
        match serde_json::from_str::<evidence::RacingHostedExecutionRequestV1>(&request_json) {
            Ok(request) => request,
            Err(error) => return json_error(&format!("invalid hosted execution request: {error}")),
        };
    let catalog = match RacingCatalogSnapshot::embedded() {
        Ok(catalog) => catalog,
        Err(error) => return json_error(&format!("invalid embedded Racing catalog: {error}")),
    };

    match execute_authorized_race_application(request, &catalog) {
        Ok(result) => serialize_json(&result),
        Err(error) => json_error(&error),
    }
}

/// Executes one signed Racing contract through caller-fetched immutable catalog bytes.
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn execute_authorized_race_with_catalog_json(
    request_json: String,
    catalog_bundle_json: String,
) -> String {
    let request =
        match serde_json::from_str::<evidence::RacingHostedExecutionRequestV1>(&request_json) {
            Ok(request) => request,
            Err(error) => return json_error(&format!("invalid hosted execution request: {error}")),
        };
    let catalog = match RacingCatalogSnapshot::from_bundle_json(&catalog_bundle_json) {
        Ok(catalog) => catalog,
        Err(error) => return json_error(&format!("invalid Racing catalog: {error}")),
    };

    match execute_authorized_race(request, &catalog) {
        Ok(submission) => serialize_json(&submission),
        Err(error) => json_error(&error),
    }
}

/// Executes one signed Racing contract against caller-fetched catalog bytes
/// and exposes complete local runtime data beside the unchanged evidence.
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn execute_authorized_race_application_with_catalog_json(
    request_json: String,
    catalog_bundle_json: String,
) -> String {
    let request =
        match serde_json::from_str::<evidence::RacingHostedExecutionRequestV1>(&request_json) {
            Ok(request) => request,
            Err(error) => return json_error(&format!("invalid hosted execution request: {error}")),
        };
    let catalog = match RacingCatalogSnapshot::from_bundle_json(&catalog_bundle_json) {
        Ok(catalog) => catalog,
        Err(error) => return json_error(&format!("invalid Racing catalog: {error}")),
    };

    match execute_authorized_race_application(request, &catalog) {
        Ok(result) => serialize_json(&result),
        Err(error) => json_error(&error),
    }
}

/// Produces the complete hosted-verification evidence for one browser attempt.
pub fn execute_authorized_race(
    request: evidence::RacingHostedExecutionRequestV1,
    catalog: &RacingCatalogSnapshot,
) -> Result<evidence::RacingVerificationSubmissionV1, String> {
    execute_authorized_race_application(request, catalog).map(|result| result.evidence)
}

/// Produces verifier evidence and complete application data from one linked run.
pub fn execute_authorized_race_application(
    request: evidence::RacingHostedExecutionRequestV1,
    catalog: &RacingCatalogSnapshot,
) -> Result<evidence::RacingAuthorizedApplicationResultV1, String> {
    request
        .signed_authorization
        .authorization
        .validate_integrity()
        .map_err(|error| format!("invalid signed authorization: {error}"))?;
    let contract = &request.signed_authorization.authorization.contract;
    catalog
        .manifest()
        .validate_for_run(contract)
        .map_err(|error| format!("authorized catalog is unavailable: {error}"))?;

    let workload = racing_workload_for(&contract.model, catalog)?;
    let execution = execute_linked(&workload, contract, request.input.clone())
        .map_err(|error| format!("authorized Racing execution failed: {error}"))?;
    let runtime = RuntimeIdentity {
        engine: "pitgun-wasm"
            .parse()
            .expect("static WASM runtime engine identifier"),
        engine_version: env!("CARGO_PKG_VERSION")
            .parse()
            .expect("crate version is semantic"),
        target: "wasm32-unknown-unknown"
            .parse()
            .expect("static WASM target identifier"),
        artifact_digest: request.wasm_artifact_digest,
    };
    let receipt = execution
        .evidence
        .execution_receipt(contract, request.execution_id, runtime)
        .map_err(|error| format!("cannot create Racing execution receipt: {error}"))?;
    let execution_resolution =
        evidence::RacingExecutionResolutionV1::from_catalog(catalog, &contract.model);

    let evidence = evidence::RacingVerificationSubmissionV1 {
        signed_authorization: request.signed_authorization,
        input: request.input,
        receipt: RunBundleReceiptV1 {
            schema_version: RunBundleReceiptVersion::V1,
            receipt,
        },
        output: execution.evidence.output,
        telemetry_summary: execution.evidence.telemetry_summary,
        execution_resolution,
    };

    Ok(evidence::RacingAuthorizedApplicationResultV1 {
        schema_version: evidence::RacingAuthorizedApplicationResultVersion::V1,
        evidence,
        runtime_output: execution.output,
    })
}

/// Executes one dynamic Racing attempt through the embedded timeline catalog.
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn execute_authorized_dynamic_race_json(request_json: String) -> String {
    let request =
        match serde_json::from_str::<evidence::RacingDynamicExecutionRequestV1>(&request_json) {
            Ok(request) => request,
            Err(error) => {
                return json_error(&format!("invalid dynamic execution request: {error}"));
            }
        };
    let catalog = match RacingCatalogSnapshot::embedded_model_v3_timeline() {
        Ok(catalog) => catalog,
        Err(error) => {
            return json_error(&format!(
                "invalid embedded Racing timeline catalog: {error}"
            ));
        }
    };

    match execute_authorized_dynamic_race(request, &catalog) {
        Ok(submission) => serialize_json(&submission),
        Err(error) => json_error(&error),
    }
}

/// Executes one dynamic attempt and retains complete local runtime telemetry.
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn execute_authorized_dynamic_race_application_json(request_json: String) -> String {
    let request =
        match serde_json::from_str::<evidence::RacingDynamicExecutionRequestV1>(&request_json) {
            Ok(request) => request,
            Err(error) => {
                return json_error(&format!("invalid dynamic execution request: {error}"));
            }
        };
    let catalog = match RacingCatalogSnapshot::embedded_model_v3_timeline() {
        Ok(catalog) => catalog,
        Err(error) => {
            return json_error(&format!(
                "invalid embedded Racing timeline catalog: {error}"
            ));
        }
    };

    match execute_authorized_dynamic_race_application(request, &catalog) {
        Ok(result) => serialize_json(&result),
        Err(error) => json_error(&error),
    }
}

/// Executes one dynamic attempt against caller-fetched immutable catalog bytes.
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn execute_authorized_dynamic_race_with_catalog_json(
    request_json: String,
    catalog_bundle_json: String,
) -> String {
    let request =
        match serde_json::from_str::<evidence::RacingDynamicExecutionRequestV1>(&request_json) {
            Ok(request) => request,
            Err(error) => {
                return json_error(&format!("invalid dynamic execution request: {error}"));
            }
        };
    let catalog = match RacingCatalogSnapshot::from_bundle_json(&catalog_bundle_json) {
        Ok(catalog) => catalog,
        Err(error) => return json_error(&format!("invalid Racing catalog: {error}")),
    };

    match execute_authorized_dynamic_race(request, &catalog) {
        Ok(submission) => serialize_json(&submission),
        Err(error) => json_error(&error),
    }
}

/// Executes one dynamic attempt against caller-fetched catalog bytes and
/// retains complete local runtime telemetry.
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn execute_authorized_dynamic_race_application_with_catalog_json(
    request_json: String,
    catalog_bundle_json: String,
) -> String {
    let request =
        match serde_json::from_str::<evidence::RacingDynamicExecutionRequestV1>(&request_json) {
            Ok(request) => request,
            Err(error) => {
                return json_error(&format!("invalid dynamic execution request: {error}"));
            }
        };
    let catalog = match RacingCatalogSnapshot::from_bundle_json(&catalog_bundle_json) {
        Ok(catalog) => catalog,
        Err(error) => return json_error(&format!("invalid Racing catalog: {error}")),
    };

    match execute_authorized_dynamic_race_application(request, &catalog) {
        Ok(result) => serialize_json(&result),
        Err(error) => json_error(&error),
    }
}

/// Produces compact verifier evidence for one completed dynamic Racing attempt.
pub fn execute_authorized_dynamic_race(
    request: evidence::RacingDynamicExecutionRequestV1,
    catalog: &RacingCatalogSnapshot,
) -> Result<evidence::RacingDynamicVerificationSubmissionV1, String> {
    execute_authorized_dynamic_race_application(request, catalog).map(|result| result.evidence)
}

/// Validates and executes one predeclared driver-instruction history.
///
/// Cryptographic HMAC verification remains at the trusted Verifier boundary:
/// browser runtimes receive no Authority secret. This boundary nevertheless
/// rejects malformed signature shapes and validates every signed identity before
/// physical execution starts.
pub fn execute_authorized_dynamic_race_application(
    request: evidence::RacingDynamicExecutionRequestV1,
    catalog: &RacingCatalogSnapshot,
) -> Result<evidence::RacingDynamicApplicationResultV1, String> {
    let signed = &request.signed_authorization;
    signed
        .authorization
        .validate_integrity()
        .map_err(|error| format!("invalid signed attempt authorization: {error}"))?;
    validate_attempt_signature_shape(signed.algorithm, &signed.signature)?;

    let initial_contract = &signed.authorization.initial_contract;
    let timeline_model = racing_model_v3_timeline_candidate_identity();
    let fuel_contract_model = racing_model_v3_fuel_contract_candidate_identity();
    if initial_contract.model != timeline_model && initial_contract.model != fuel_contract_model {
        return Err(format!(
            "dynamic Racing execution requires model {} or {}, got {}@{} {}",
            timeline_model.version,
            fuel_contract_model.version,
            initial_contract.model.id,
            initial_contract.model.version,
            initial_contract.model.digest,
        ));
    }
    catalog
        .manifest()
        .validate_for_run(initial_contract)
        .map_err(|error| format!("authorized dynamic catalog is unavailable: {error}"))?;
    if catalog.release_identity() != &request.catalog_release {
        return Err(format!(
            "dynamic Racing catalog release mismatch: expected {}@{} {}, got {}@{} {}",
            request.catalog_release.id,
            request.catalog_release.version,
            request.catalog_release.manifest_digest,
            catalog.release_identity().id,
            catalog.release_identity().version,
            catalog.release_identity().manifest_digest,
        ));
    }

    let actual_input_digest = canonical_json_digest(&request.input)
        .map_err(|error| format!("cannot canonicalize dynamic Racing input: {error}"))?;
    if actual_input_digest != initial_contract.input.digest {
        return Err(format!(
            "dynamic Racing input digest mismatch: expected {}, got {}",
            initial_contract.input.digest, actual_input_digest,
        ));
    }

    let instruction_profile_identity = catalog
        .driver_instruction_profile_identity()
        .ok_or_else(|| "dynamic Racing catalog has no instruction-profile identity".to_string())?;
    let instruction_profile = catalog
        .driver_instruction_profile()
        .ok_or_else(|| "dynamic Racing catalog has no instruction profile".to_string())?;
    let final_contract = request
        .decision_envelope
        .final_contract_for_attempt(
            &signed.authorization,
            &request.completed_input,
            instruction_profile_identity,
            instruction_profile,
        )
        .map_err(|error| format!("invalid completed dynamic Racing input: {error}"))?;

    let run_request = RunRaceRequest {
        input: request.input.clone(),
        seed: initial_contract.random.seed.get(),
        era: Some(request.input.era),
        hz: Some(request.input.hz),
    };
    let timeline = request
        .completed_input
        .driver_instructions
        .applied_timeline
        .clone();
    let output = if initial_contract.model == fuel_contract_model {
        run_race_with_catalog_and_v3_fuel_contract_candidate(run_request, catalog, timeline)
    } else {
        run_race_with_catalog_and_v3_timeline_candidate(run_request, catalog, timeline)
    }
    .map_err(|error| format!("authorized dynamic Racing execution failed: {error}"))?;
    let run_evidence = evidence::RacingRunEvidenceV1::from_race_output(&output)
        .map_err(|error| format!("cannot project dynamic Racing evidence: {error}"))?;
    let runtime = RuntimeIdentity {
        engine: "pitgun-wasm"
            .parse()
            .expect("static WASM runtime engine identifier"),
        engine_version: env!("CARGO_PKG_VERSION")
            .parse()
            .expect("crate version is semantic"),
        target: "wasm32-unknown-unknown"
            .parse()
            .expect("static WASM target identifier"),
        artifact_digest: request.wasm_artifact_digest,
    };
    let receipt = run_evidence
        .execution_receipt(&final_contract, signed.authorization.execution_id, runtime)
        .map_err(|error| format!("cannot create dynamic Racing receipt: {error}"))?;
    signed
        .authorization
        .validate_completed_receipt(&final_contract, &receipt)
        .map_err(|error| format!("invalid completed dynamic Racing receipt: {error}"))?;
    let execution_resolution =
        evidence::RacingExecutionResolutionV1::from_catalog(catalog, &initial_contract.model)
            .ok_or_else(|| {
                "dynamic Racing catalog has no explicit execution lineage".to_string()
            })?;

    let evidence = evidence::RacingDynamicVerificationSubmissionV1 {
        schema_version: evidence::RacingDynamicVerificationSubmissionVersion::V1,
        signed_authorization: request.signed_authorization,
        decision_envelope: request.decision_envelope,
        input: request.input,
        completed_input: request.completed_input,
        receipt: RunBundleReceiptV1 {
            schema_version: RunBundleReceiptVersion::V1,
            receipt,
        },
        output: run_evidence.output,
        telemetry_summary: run_evidence.telemetry_summary,
        execution_resolution,
    };

    Ok(evidence::RacingDynamicApplicationResultV1 {
        schema_version: evidence::RacingDynamicApplicationResultVersion::V1,
        evidence,
        runtime_output: output,
    })
}

fn validate_attempt_signature_shape(
    algorithm: AuthorizationSignatureAlgorithm,
    signature: &str,
) -> Result<(), String> {
    if algorithm != AuthorizationSignatureAlgorithm::HmacSha256 {
        return Err("unsupported dynamic authorization signature algorithm".to_string());
    }
    if signature.len() != 64
        || !signature
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(
            "invalid dynamic authorization signature shape: expected 64 lowercase hex characters"
                .to_string(),
        );
    }
    Ok(())
}

fn racing_workload_for(
    model: &ArtifactIdentity,
    catalog: &RacingCatalogSnapshot,
) -> Result<RacingWorkload, String> {
    RacingWorkload::for_model(model, catalog.clone())
}

fn parse_run_race_request(input_json: &str) -> Result<RunRaceRequest, String> {
    Ok(match serde_json::from_str::<RunRacePayload>(input_json) {
        Ok(RunRacePayload::Wrapped(request)) => request,
        Ok(RunRacePayload::Bare(input)) => RunRaceRequest {
            input,
            seed: 0,
            era: None,
            hz: None,
        },
        Err(error) => {
            return Err(format!("invalid request: {error}"));
        }
    })
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn run_sessions_json(input_json: String) -> String {
    let request = match serde_json::from_str::<SessionRunRequest>(&input_json) {
        Ok(value) => value,
        Err(err) => return json_error(&format!("invalid request: {err}")),
    };

    match run_sessions(request) {
        Ok(output) => serialize_json(&output),
        Err(error) => json_error(&error),
    }
}

/// Browser facade for sessions using one caller-fetched immutable catalog.
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn run_sessions_with_catalog_json(input_json: String, catalog_bundle_json: String) -> String {
    let request = match serde_json::from_str::<SessionRunRequest>(&input_json) {
        Ok(value) => value,
        Err(error) => {
            return json_error(&format!("invalid request: {error}"));
        }
    };
    let catalog = match RacingCatalogSnapshot::from_bundle_json(&catalog_bundle_json) {
        Ok(catalog) => catalog,
        Err(error) => {
            return json_error(&format!("invalid Racing catalog: {error}"));
        }
    };

    match run_sessions_with_catalog(request, &catalog) {
        Ok(output) => serialize_json(&output),
        Err(error) => json_error(&error),
    }
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn solve_baseline_json(_: String) -> String {
    json_error("baseline optimizer has been disabled")
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn catalog_json() -> String {
    match catalog_snapshot() {
        Ok(output) => serialize_json(&output),
        Err(error) => json_error(&error),
    }
}

/// Browser facade exposing presentation derived from caller-fetched bytes.
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn catalog_json_from_bundle(catalog_bundle_json: String) -> String {
    let catalog = match RacingCatalogSnapshot::from_bundle_json(&catalog_bundle_json) {
        Ok(catalog) => catalog,
        Err(error) => {
            return json_error(&format!("invalid Racing catalog: {error}"));
        }
    };
    match catalog_snapshot_with_catalog(&catalog) {
        Ok(output) => serialize_json(&output),
        Err(error) => json_error(&error),
    }
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn list_circuits_json() -> String {
    match list_browser_circuits() {
        Ok(output) => serialize_json(&output),
        Err(error) => json_error(&error),
    }
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn get_circuit_json(track_id: String) -> String {
    match get_circuit(&track_id) {
        Ok(output) => serialize_json(&output),
        Err(error) => json_error(&error),
    }
}

/// Browser facade exposing one circuit from caller-fetched catalog bytes.
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn get_circuit_json_from_bundle(track_id: String, catalog_bundle_json: String) -> String {
    let catalog = match RacingCatalogSnapshot::from_bundle_json(&catalog_bundle_json) {
        Ok(catalog) => catalog,
        Err(error) => {
            return json_error(&format!("invalid Racing catalog: {error}"));
        }
    };
    match get_circuit_with_catalog(&catalog, &track_id) {
        Ok(output) => serialize_json(&output),
        Err(error) => json_error(&error),
    }
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn list_engines_json() -> String {
    match list_engines() {
        Ok(output) => serialize_json(&output),
        Err(error) => json_error(&error),
    }
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn get_engine_json(engine_id: String) -> String {
    match get_engine(&engine_id) {
        Ok(output) => serialize_json(&output),
        Err(error) => json_error(&error),
    }
}

/// Browser facade exposing one engine from caller-fetched catalog bytes.
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn get_engine_json_from_bundle(engine_id: String, catalog_bundle_json: String) -> String {
    let catalog = match RacingCatalogSnapshot::from_bundle_json(&catalog_bundle_json) {
        Ok(catalog) => catalog,
        Err(error) => {
            return json_error(&format!("invalid Racing catalog: {error}"));
        }
    };
    match get_engine_with_catalog(&catalog, &engine_id) {
        Ok(output) => serialize_json(&output),
        Err(error) => json_error(&error),
    }
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn list_drivers_json() -> String {
    match list_drivers() {
        Ok(output) => serialize_json(&output),
        Err(error) => json_error(&error),
    }
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn list_vehicles_json() -> String {
    match list_vehicles() {
        Ok(output) => serialize_json(&output),
        Err(error) => json_error(&error),
    }
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn list_tires_json() -> String {
    match list_tires() {
        Ok(output) => serialize_json(&output),
        Err(error) => json_error(&error),
    }
}

pub fn catalog_snapshot() -> Result<CatalogSnapshot, String> {
    let catalog = RacingCatalogSnapshot::embedded()
        .map_err(|error| format!("invalid embedded Racing catalog: {error}"))?;
    catalog_snapshot_with_catalog(&catalog)
}

/// Builds application-facing data from one validated immutable snapshot.
pub fn catalog_snapshot_with_catalog(
    snapshot: &RacingCatalogSnapshot,
) -> Result<CatalogSnapshot, String> {
    let catalog = EmbeddedCatalog::from_snapshot(snapshot)?;

    let mut circuits = catalog
        .tracks
        .values()
        .map(|track| BrowserCircuitCatalogEntry {
            id: track.browser_id.clone(),
            display_name: track.display_name.clone(),
            country_code: track.country_code.clone(),
            laps: track.laps,
            sample_count: track.track.s.len(),
            distance_m: track.track.s.last().copied().unwrap_or(0.0),
            pit_loss_ms: track.pit_loss_ms,
        })
        .collect::<Vec<_>>();
    circuits.sort_by(|left, right| left.id.cmp(&right.id));

    let mut engines = catalog
        .engines
        .iter()
        .map(|(id, engine)| EngineCatalogEntry {
            id: id.clone(),
            idle_rpm: engine.n_idle,
            max_rpm: engine.n_max,
            gear_count: engine.gear_ratios.len(),
        })
        .collect::<Vec<_>>();
    engines.sort_by(|left, right| left.id.cmp(&right.id));

    let mut vehicles = catalog
        .vehicles
        .iter()
        .map(|(id, vehicle)| VehicleCatalogEntry {
            id: id.clone(),
            engine_id: vehicle.engine_id.clone(),
            default_tire_id: vehicle.tire_id.clone(),
        })
        .collect::<Vec<_>>();
    vehicles.sort_by(|left, right| left.id.cmp(&right.id));

    let mut drivers = catalog
        .drivers
        .values()
        .filter(|driver| driver.id != "default")
        .map(|driver| DriverCatalogEntry {
            id: driver.id.clone(),
            display_name: driver.display_name.clone(),
        })
        .collect::<Vec<_>>();
    drivers.sort_by(|left, right| left.id.cmp(&right.id));

    let mut tires = catalog
        .tires
        .keys()
        .map(|id| TireCatalogEntry { id: id.clone() })
        .collect::<Vec<_>>();
    tires.sort_by(|left, right| left.id.cmp(&right.id));

    Ok(CatalogSnapshot {
        circuits,
        engines,
        vehicles,
        drivers,
        tires,
        components: snapshot
            .component_capability_profile()
            .map(|profile| profile.components.clone())
            .unwrap_or_default(),
        component_capability_profile: snapshot.component_capability_profile_identity().cloned(),
    })
}

/// Resolves the exact installed component identities and effective controls.
pub fn resolve_vehicle_capabilities_with_catalog(
    snapshot: &RacingCatalogSnapshot,
    vehicle_id: &str,
    selection: Option<&VehicleComponentSelectionV1>,
) -> Result<ResolvedVehicleCapabilitiesV1, String> {
    EmbeddedCatalog::from_snapshot(snapshot)?
        .resolve_vehicle_capabilities(vehicle_id, selection)?
        .ok_or_else(|| "Racing catalog has no component-capability profile".to_string())
}

/// Browser facade for resolving controls from an immutable catalog bundle.
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn resolve_vehicle_capabilities_json_from_bundle(
    request_json: String,
    catalog_bundle_json: String,
) -> String {
    let request: ResolveVehicleCapabilitiesRequestV1 = match serde_json::from_str(&request_json) {
        Ok(request) => request,
        Err(error) => return json_error(&format!("invalid capability request: {error}")),
    };
    let snapshot = match RacingCatalogSnapshot::from_bundle_json(&catalog_bundle_json) {
        Ok(snapshot) => snapshot,
        Err(error) => return json_error(&format!("invalid Racing catalog: {error}")),
    };
    match resolve_vehicle_capabilities_with_catalog(
        &snapshot,
        &request.vehicle_id,
        request.components.as_ref(),
    ) {
        Ok(resolved) => serialize_json(&resolved),
        Err(error) => json_error(&error),
    }
}

pub fn list_browser_circuits() -> Result<Vec<BrowserCircuitCatalogEntry>, String> {
    let catalog = EmbeddedCatalog::load_default()?;
    let mut items = catalog
        .tracks
        .values()
        .map(|track| BrowserCircuitCatalogEntry {
            id: track.browser_id.clone(),
            display_name: track.display_name.clone(),
            country_code: track.country_code.clone(),
            laps: track.laps,
            sample_count: track.track.s.len(),
            distance_m: track.track.s.last().copied().unwrap_or(0.0),
            pit_loss_ms: track.pit_loss_ms,
        })
        .collect::<Vec<_>>();
    items.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(items)
}

pub fn list_circuits() -> Result<Vec<CircuitCatalogEntry>, String> {
    let catalog = EmbeddedCatalog::load_default()?;
    let mut items = catalog
        .tracks
        .values()
        .map(|track| CircuitCatalogEntry {
            id: track.id.clone(),
            country_code: track.country_code.clone(),
            sample_count: track.track.s.len(),
            distance_m: track.track.s.last().copied().unwrap_or(0.0),
            pit_loss_ms: track.pit_loss_ms,
        })
        .collect::<Vec<_>>();
    items.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(items)
}

pub fn get_circuit(track_id: &str) -> Result<CircuitDetail, String> {
    let snapshot = RacingCatalogSnapshot::embedded()
        .map_err(|error| format!("invalid embedded Racing catalog: {error}"))?;
    get_circuit_with_catalog(&snapshot, track_id)
}

/// Resolves one circuit detail from a validated immutable snapshot.
pub fn get_circuit_with_catalog(
    snapshot: &RacingCatalogSnapshot,
    track_id: &str,
) -> Result<CircuitDetail, String> {
    let catalog = EmbeddedCatalog::from_snapshot(snapshot)?;
    let record = catalog.get_track(track_id)?;
    Ok(CircuitDetail {
        id: record.browser_id.clone(),
        display_name: record.display_name.clone(),
        country_code: record.country_code.clone(),
        laps: record.laps,
        s_m: record.track.s.clone(),
        x_m: record.track.x.clone(),
        y_m: record.track.y.clone(),
        z_m: record.track.z.clone(),
        curvature_radpm: record.track.kappa.clone(),
        slope: record.track.slope.clone(),
        heading_rad: record.track.heading.clone(),
        pit_loss_ms: record.pit_loss_ms,
    })
}

pub fn list_engines() -> Result<Vec<EngineCatalogEntry>, String> {
    let catalog = EmbeddedCatalog::load_default()?;
    let mut items = catalog
        .engines
        .iter()
        .map(|(id, engine)| EngineCatalogEntry {
            id: id.clone(),
            idle_rpm: engine.n_idle,
            max_rpm: engine.n_max,
            gear_count: engine.gear_ratios.len(),
        })
        .collect::<Vec<_>>();
    items.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(items)
}

pub fn get_engine(engine_id: &str) -> Result<EngineDetail, String> {
    let snapshot = RacingCatalogSnapshot::embedded()
        .map_err(|error| format!("invalid embedded Racing catalog: {error}"))?;
    get_engine_with_catalog(&snapshot, engine_id)
}

/// Resolves one engine detail from a validated immutable snapshot.
pub fn get_engine_with_catalog(
    snapshot: &RacingCatalogSnapshot,
    engine_id: &str,
) -> Result<EngineDetail, String> {
    let catalog = EmbeddedCatalog::from_snapshot(snapshot)?;
    let engine = catalog
        .engines
        .get(engine_id)
        .ok_or_else(|| format!("unknown engine '{engine_id}'"))?;
    Ok(EngineDetail {
        id: engine_id.to_string(),
        rpm_samples: engine.n_rpm.clone(),
        torque_samples: engine.trq.clone(),
        gear_ratios: engine.gear_ratios.clone(),
        idle_rpm: engine.n_idle,
        max_rpm: engine.n_max,
    })
}

pub fn list_drivers() -> Result<Vec<DriverCatalogEntry>, String> {
    let catalog = EmbeddedCatalog::load_default()?;
    let mut items = catalog
        .drivers
        .values()
        .filter(|driver| driver.id != "default")
        .map(|driver| DriverCatalogEntry {
            id: driver.id.clone(),
            display_name: driver.display_name.clone(),
        })
        .collect::<Vec<_>>();
    items.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(items)
}

pub fn list_vehicles() -> Result<Vec<VehicleCatalogEntry>, String> {
    let catalog = EmbeddedCatalog::load_default()?;
    let mut items = catalog
        .vehicles
        .iter()
        .map(|(id, vehicle)| VehicleCatalogEntry {
            id: id.clone(),
            engine_id: vehicle.engine_id.clone(),
            default_tire_id: vehicle.tire_id.clone(),
        })
        .collect::<Vec<_>>();
    items.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(items)
}

pub fn list_tires() -> Result<Vec<TireCatalogEntry>, String> {
    let catalog = EmbeddedCatalog::load_default()?;
    let mut items = catalog
        .tires
        .keys()
        .map(|id| TireCatalogEntry { id: id.clone() })
        .collect::<Vec<_>>();
    items.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(items)
}

impl EmbeddedCatalog {
    fn load_default() -> Result<Self, String> {
        let snapshot = RacingCatalogSnapshot::embedded()
            .map_err(|error| format!("invalid embedded Racing catalog: {error}"))?;
        Self::from_snapshot(&snapshot)
    }

    fn from_snapshot(snapshot: &RacingCatalogSnapshot) -> Result<Self, String> {
        let mut catalog = Self {
            component_capability_profile: snapshot.component_capability_profile().cloned(),
            ..Self::default()
        };
        for (path, raw) in snapshot.resources() {
            let relative_path = path
                .as_str()
                .strip_prefix("simulation/")
                .ok_or_else(|| format!("Racing resource is outside simulation/: {path}"))?;
            catalog.apply_file(relative_path, raw)?;
        }
        catalog.apply_presentation(snapshot.presentation_index().clone())?;
        catalog.validate()?;
        Ok(catalog)
    }

    fn apply_file(&mut self, path: &str, raw: &[u8]) -> Result<(), String> {
        let value: Value = serde_json::from_slice(raw)
            .map_err(|err| format!("failed to parse '{path}': {err}"))?;
        let (category, file_name) = path
            .split_once('/')
            .ok_or_else(|| format!("invalid embedded path '{path}'"))?;
        if file_name.contains('/') || !file_name.ends_with(".json") {
            return Err(format!("invalid Racing simulation resource path '{path}'"));
        }
        let stem = file_name.trim_end_matches(".json");

        match category {
            "aero" => {
                self.aeros.insert(stem.to_string(), parse_aero(&value)?);
            }
            "chassis" => {
                self.chassis
                    .insert(stem.to_string(), parse_chassis(&value)?);
            }
            "engines" => {
                self.engines.insert(stem.to_string(), parse_engine(&value)?);
            }
            "tires" => {
                self.tires.insert(stem.to_string(), parse_tire(&value)?);
            }
            "vehicles" => {
                self.vehicles
                    .insert(stem.to_string(), parse_vehicle(stem, &value)?);
            }
            "drivers" => {
                if let Some(driver) = parse_driver(stem, &value)? {
                    self.drivers.insert(driver.id.clone(), driver);
                }
            }
            "circuits" => {
                let track = parse_track(stem, &value)?;
                self.tracks.insert(track.id.clone(), track);
            }
            "policies" => {
                if !matches!(
                    value.get("schema_version").and_then(Value::as_str),
                    Some("pitgun.racing-opponent-policy/v1")
                        | Some("pitgun.racing-opponent-policy/v2")
                ) {
                    return Err(format!("unsupported Racing opponent policy in '{path}'"));
                }
                // Opponent policies influence canonical race-input composition,
                // not the physical solver. Browser/application adapters consume
                // them before submitting the complete competitor field.
            }
            "model-parameters" => {
                let parameters: RacingModelParametersV1 =
                    serde_json::from_value(value).map_err(|error| {
                        format!("invalid Racing model parameters in '{path}': {error}")
                    })?;
                parameters.validate().map_err(|error| {
                    format!("invalid Racing model parameters in '{path}': {error}")
                })?;
                // The validated snapshot owns this resource. The physical
                // catalog only acknowledges its category here so it cannot be
                // mistaken for a vehicle component.
            }
            "thermal-profiles" => {
                // The validated snapshot owns and resolves this exact candidate.
                // The physical component catalog only acknowledges the category.
            }
            "component-capabilities" => {
                // Parsed and coverage-validated by `RacingCatalogSnapshot`.
            }
            "driver-instructions" => {
                // Parsed and identity-validated by `RacingCatalogSnapshot`.
                // Execution receives the resolved profile explicitly.
            }
            "driver-control" | "drivers-v2" => {
                // Parsed and identity-validated by `RacingCatalogSnapshot`.
                // A future timeline-enabled workload consumes this complete package.
            }
            "fuel-contract" => {
                // Parsed and identity-validated by `RacingCatalogSnapshot`.
                // Published-shaped V3 workloads consume the resolved contract.
            }
            _ => {
                return Err(format!(
                    "unsupported Racing simulation resource category '{category}'"
                ));
            }
        }
        Ok(())
    }

    fn apply_presentation(
        &mut self,
        presentation: RacingPresentationIndexV1,
    ) -> Result<(), String> {
        if presentation.circuits.len() != self.tracks.len() {
            return Err(format!(
                "presentation index contains {} circuits for {} simulation resources",
                presentation.circuits.len(),
                self.tracks.len()
            ));
        }
        if presentation.drivers.len() != self.drivers.len() {
            return Err(format!(
                "presentation index contains {} drivers for {} simulation resources",
                presentation.drivers.len(),
                self.drivers.len()
            ));
        }

        for entry in presentation.circuits {
            let browser_id = normalize_track_id(&entry.id);
            let track = self
                .tracks
                .values_mut()
                .find(|track| track.browser_id == browser_id)
                .ok_or_else(|| {
                    format!(
                        "presentation circuit '{}' has no simulation resource",
                        entry.source_id
                    )
                })?;
            if track.id != normalize_track_id(&entry.model_id) {
                return Err(format!(
                    "presentation circuit '{}' expects model '{}', found '{}'",
                    entry.source_id, entry.model_id, track.id
                ));
            }
            track.browser_id = entry.id;
            track.display_name = entry.display_name;
            track.country_code = entry.country_code;
            track.laps = entry.laps;
        }

        for entry in presentation.drivers {
            let driver = self.drivers.get_mut(&entry.id).ok_or_else(|| {
                format!(
                    "presentation driver '{}' has no simulation resource",
                    entry.id
                )
            })?;
            driver.display_name = entry.display_name;
        }
        Ok(())
    }

    fn validate(&self) -> Result<(), String> {
        if !self.drivers.contains_key("default") {
            return Err("Racing catalog is missing the default driver".to_string());
        }
        for (id, vehicle) in &self.vehicles {
            for (kind, reference, exists) in [
                (
                    "aero",
                    vehicle.aero_id.as_str(),
                    self.aeros.contains_key(&vehicle.aero_id),
                ),
                (
                    "chassis",
                    vehicle.chassis_id.as_str(),
                    self.chassis.contains_key(&vehicle.chassis_id),
                ),
                (
                    "engine",
                    vehicle.engine_id.as_str(),
                    self.engines.contains_key(&vehicle.engine_id),
                ),
                (
                    "tire",
                    vehicle.tire_id.as_str(),
                    self.tires.contains_key(&vehicle.tire_id),
                ),
            ] {
                if !exists {
                    return Err(format!(
                        "vehicle '{id}' references unknown {kind} '{reference}'"
                    ));
                }
            }
        }
        Ok(())
    }

    fn get_track(&self, track_id: &str) -> Result<&TrackRecord, String> {
        let id = normalize_track_id(track_id);
        self.tracks
            .get(&id)
            .or_else(|| self.tracks.values().find(|track| track.browser_id == id))
            .ok_or_else(|| format!("unknown circuit '{id}'"))
    }

    #[cfg(test)]
    fn resolve_vehicle(&self, vehicle_id: &str) -> Result<(VehicleParams, String), String> {
        self.resolve_vehicle_with_components(vehicle_id, None)
    }

    fn resolve_vehicle_with_components(
        &self,
        vehicle_id: &str,
        selection: Option<&VehicleComponentSelectionV1>,
    ) -> Result<(VehicleParams, String), String> {
        let installed = self.resolve_installed_components(vehicle_id, selection)?;
        let aero_id = installed
            .get(&VehicleComponentKind::AerodynamicPackage)
            .expect("resolved aerodynamic package");
        let chassis_id = installed
            .get(&VehicleComponentKind::Chassis)
            .expect("resolved chassis");
        let engine_id = installed
            .get(&VehicleComponentKind::PowerUnit)
            .expect("resolved power unit");
        let tire_id = installed
            .get(&VehicleComponentKind::TireSpecification)
            .expect("resolved tire specification");
        let aero = self
            .aeros
            .get(aero_id)
            .expect("installed aero was validated");
        let chassis = self
            .chassis
            .get(chassis_id)
            .expect("installed chassis was validated");
        let engine = self
            .engines
            .get(engine_id)
            .expect("installed engine was validated");
        let tire = self
            .tires
            .get(tire_id)
            .expect("installed tire was validated");
        Ok((
            VehicleParams {
                chassis: chassis.clone(),
                aero: aero.clone(),
                engine: engine.clone(),
                tire: tire.clone(),
            },
            tire_id.clone(),
        ))
    }

    fn resolve_installed_components(
        &self,
        vehicle_id: &str,
        selection: Option<&VehicleComponentSelectionV1>,
    ) -> Result<std::collections::BTreeMap<VehicleComponentKind, String>, String> {
        let record = self
            .vehicles
            .get(vehicle_id)
            .ok_or_else(|| format!("unknown vehicle '{vehicle_id}'"))?;
        if let Some(selection) = selection {
            validate_component_selection(selection)?;
        }
        let aero_id = selection
            .and_then(|value| value.aero_id.as_deref())
            .unwrap_or(&record.aero_id);
        let chassis_id = selection
            .and_then(|value| value.chassis_id.as_deref())
            .unwrap_or(&record.chassis_id);
        let engine_id = selection
            .and_then(|value| value.engine_id.as_deref())
            .unwrap_or(&record.engine_id);
        let tire_id = selection
            .and_then(|value| value.tire_id.as_deref())
            .unwrap_or(&record.tire_id);
        if !self.aeros.contains_key(aero_id) {
            return Err(format!("unknown aero '{aero_id}'"));
        }
        if !self.chassis.contains_key(chassis_id) {
            return Err(format!("unknown chassis '{chassis_id}'"));
        }
        if !self.engines.contains_key(engine_id) {
            return Err(format!("unknown engine '{engine_id}'"));
        }
        if !self.tires.contains_key(tire_id) {
            return Err(format!("unknown tire '{tire_id}'"));
        }
        Ok(std::collections::BTreeMap::from([
            (
                VehicleComponentKind::AerodynamicPackage,
                aero_id.to_string(),
            ),
            (VehicleComponentKind::Chassis, chassis_id.to_string()),
            (VehicleComponentKind::PowerUnit, engine_id.to_string()),
            (VehicleComponentKind::TireSpecification, tire_id.to_string()),
        ]))
    }

    fn resolve_vehicle_capabilities(
        &self,
        vehicle_id: &str,
        selection: Option<&VehicleComponentSelectionV1>,
    ) -> Result<Option<ResolvedVehicleCapabilitiesV1>, String> {
        let Some(profile) = &self.component_capability_profile else {
            return Ok(None);
        };
        let installed = self.resolve_installed_components(vehicle_id, selection)?;
        profile
            .resolve(vehicle_id, installed)
            .map(Some)
            .map_err(|error| format!("cannot resolve vehicle capabilities: {error}"))
    }

    fn resolve_engine_id_with_components<'a>(
        &'a self,
        vehicle_id: &str,
        selection: Option<&'a VehicleComponentSelectionV1>,
    ) -> Result<&'a str, String> {
        let record = self
            .vehicles
            .get(vehicle_id)
            .ok_or_else(|| format!("unknown vehicle '{vehicle_id}'"))?;
        if let Some(selection) = selection {
            validate_component_selection(selection)?;
        }
        let engine_id = selection
            .and_then(|value| value.engine_id.as_deref())
            .unwrap_or(&record.engine_id);
        if !self.engines.contains_key(engine_id) {
            return Err(format!("unknown engine '{engine_id}'"));
        }
        Ok(engine_id)
    }

    fn resolve_tire(&self, tire_id: &str) -> Result<TireParams, String> {
        self.tires
            .get(tire_id)
            .cloned()
            .ok_or_else(|| format!("unknown tire '{tire_id}'"))
    }

    fn resolve_driver(&self, driver_id: Option<&str>) -> Result<Driver, String> {
        let requested = driver_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("default");
        Ok(self
            .drivers
            .get(requested)
            .cloned()
            .or_else(|| self.drivers.get("default").cloned())
            .unwrap_or_default())
    }
}

fn sanitize_pit_laps(raw_laps: &[u16], total_laps: u16) -> Vec<u16> {
    if total_laps <= 1 {
        return Vec::new();
    }

    let mut cleaned = raw_laps
        .iter()
        .copied()
        .filter(|lap| *lap > 0 && *lap < total_laps)
        .collect::<Vec<_>>();
    cleaned.sort_unstable();
    cleaned.dedup();
    cleaned
}

fn resolve_stint_plan(
    competitor: &CompetitorSpec,
    total_laps: u16,
    default_tire_id: &str,
    legacy_player_pit_laps: &[u16],
) -> Result<ResolvedStintPlan, String> {
    if let Some(strategy) = &competitor.stint_strategy {
        return resolve_explicit_stint_plan(strategy, total_laps);
    }

    let pit_laps = if competitor.is_player || competitor.id == "player" {
        sanitize_pit_laps(legacy_player_pit_laps, total_laps)
    } else {
        Vec::new()
    };

    Ok(ResolvedStintPlan {
        tire_by_lap: vec![default_tire_id.to_string(); total_laps as usize],
        pit_laps,
    })
}

fn resolve_explicit_stint_plan(
    strategy: &CompetitorStintStrategy,
    total_laps: u16,
) -> Result<ResolvedStintPlan, String> {
    if strategy.stints.is_empty() {
        return Err("stint_strategy requires at least one stint".to_string());
    }

    let mut tire_by_lap = Vec::with_capacity(total_laps as usize);
    let mut pit_laps = Vec::new();
    let mut cumulative = 0u16;

    for (idx, stint) in strategy.stints.iter().enumerate() {
        if stint.tire_id.trim().is_empty() {
            return Err(format!("stint {idx} has an empty tire_id"));
        }
        if stint.laps == 0 {
            return Err(format!("stint {idx} must have at least 1 lap"));
        }
        for _ in 0..stint.laps {
            tire_by_lap.push(stint.tire_id.clone());
        }
        cumulative = cumulative.saturating_add(stint.laps);
        if idx < strategy.stints.len() - 1 && cumulative > 0 && cumulative < total_laps {
            pit_laps.push(cumulative);
        }
    }

    if cumulative != total_laps {
        return Err(format!(
            "stint_strategy laps must sum to {total_laps}, got {cumulative}"
        ));
    }

    let declared = sanitize_pit_laps(&strategy.pit_laps, total_laps);
    if !declared.is_empty() && declared != pit_laps {
        return Err(format!(
            "stint_strategy pit_laps {:?} does not match stint boundaries {:?}",
            declared, pit_laps
        ));
    }

    Ok(ResolvedStintPlan {
        tire_by_lap,
        pit_laps,
    })
}

fn build_pit_plan(
    catalog: &EmbeddedCatalog,
    stint_plan: &ResolvedStintPlan,
) -> Result<PitPlan, String> {
    let mut stops = Vec::new();
    for lap in &stint_plan.pit_laps {
        let tire_id = stint_plan
            .tire_by_lap
            .get(*lap as usize)
            .ok_or_else(|| format!("missing tire assignment for lap {}", *lap + 1))?;
        let tire = catalog
            .tires
            .get(tire_id)
            .ok_or_else(|| format!("unknown tire '{tire_id}'"))?;
        stops.push(PitStop {
            lap: *lap,
            tire: tire.clone(),
        });
    }
    Ok(PitPlan { stops })
}

fn lap_times_ms(lap_times_s: &[f64], stint_plan: &ResolvedStintPlan, pit_loss_ms: u64) -> Vec<u64> {
    lap_times_s
        .iter()
        .enumerate()
        .map(|(idx, lap_time)| {
            let lap_number = idx as u16 + 1;
            let base = (lap_time * 1000.0).round().max(1.0) as u64;
            if stint_plan.pit_laps.binary_search(&lap_number).is_ok() {
                base.saturating_add(pit_loss_ms)
            } else {
                base
            }
        })
        .collect()
}

fn telemetry_batches(frames: Vec<TelemetryFrame>) -> Vec<TelemetryEnvelope> {
    frames
        .chunks(TELEMETRY_BATCH_SIZE)
        .map(|chunk| TelemetryEnvelope {
            frames: chunk.to_vec(),
        })
        .collect()
}

fn telemetry_session_id(seed: u64, track_id: &str, competitor_id: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    seed.hash(&mut hasher);
    track_id.hash(&mut hasher);
    competitor_id.hash(&mut hasher);
    (hasher.finish() & ((1u64 << 53) - 1)).max(1)
}

fn telemetry_metadata(
    track_id: &str,
    vehicle_id: &str,
    competitor_id: &str,
    driver_id: &str,
    stint_plan: &ResolvedStintPlan,
    telemetry_hz: f64,
) -> HashMap<String, String> {
    let mut metadata = HashMap::from([
        ("track_id".to_string(), track_id.to_string()),
        ("vehicle_id".to_string(), vehicle_id.to_string()),
        ("competitor_id".to_string(), competitor_id.to_string()),
        ("driver_id".to_string(), driver_id.to_string()),
        (
            "role".to_string(),
            if competitor_id == "player" {
                "player".to_string()
            } else {
                "ai".to_string()
            },
        ),
        ("sampling_hz".to_string(), telemetry_hz.to_string()),
    ]);
    if let Some(last_tire) = stint_plan.tire_by_lap.last() {
        metadata.insert("tire_id".to_string(), last_tire.clone());
    }
    metadata
}

fn gateway_sample(parameter_id: u16, value: f64) -> Sample {
    Sample::new(parameter_id, SampleValue::F64(value), SignalQuality::Good)
}

fn gateway_frames_from_resampled(
    telemetry: &ResampledTelemetry,
    session_id: u64,
    source_id: &str,
    metadata: &HashMap<String, String>,
) -> Vec<TelemetryFrame> {
    let lap_numbers = telemetry
        .n_lap
        .clone()
        .unwrap_or_else(|| vec![0; telemetry.time_s.len()]);
    let tire_temp = telemetry
        .tire_temp_c
        .clone()
        .unwrap_or_else(|| vec![0.0; telemetry.time_s.len()]);
    let tire_wear = telemetry
        .tire_wear_pct
        .clone()
        .unwrap_or_else(|| vec![0.0; telemetry.time_s.len()]);

    let mut frames = Vec::with_capacity(telemetry.time_s.len());
    for idx in 0..telemetry.time_s.len() {
        let timestamp_us = (telemetry.time_s[idx] * 1_000_000.0).round() as i64;
        frames.push(TelemetryFrame {
            session_id,
            sequence: idx as u64,
            timestamp_us,
            received_at_us: timestamp_us,
            source_id: source_id.to_string(),
            samples: vec![
                gateway_sample(PARAM_TIME_S, telemetry.time_s[idx]),
                gateway_sample(PARAM_DISTANCE_M, telemetry.s_m[idx]),
                gateway_sample(PARAM_X_M, telemetry.x_m[idx]),
                gateway_sample(PARAM_Y_M, telemetry.y_m[idx]),
                gateway_sample(PARAM_HEADING_RAD, telemetry.heading_rad[idx]),
                gateway_sample(PARAM_SPEED_KPH, telemetry.speed_kph[idx]),
                gateway_sample(PARAM_RPM, telemetry.rpm[idx]),
                gateway_sample(PARAM_GEAR, telemetry.gear[idx] as f64),
                gateway_sample(PARAM_THROTTLE_PCT, telemetry.throttle_pct[idx]),
                gateway_sample(PARAM_BRAKE_PCT, telemetry.brake_pct[idx]),
                gateway_sample(PARAM_G_LAT, telemetry.g_lat[idx]),
                gateway_sample(PARAM_G_LONG, telemetry.g_long[idx]),
                gateway_sample(PARAM_G_VERT, telemetry.g_vert[idx]),
                gateway_sample(PARAM_ENGINE_TEMP_C, telemetry.engine_temp_c[idx]),
                gateway_sample(PARAM_ENGINE_POWER_W, telemetry.engine_power_w[idx]),
                gateway_sample(PARAM_TIRE_TEMP_C, tire_temp[idx]),
                gateway_sample(PARAM_TIRE_WEAR_PCT, tire_wear[idx]),
            ],
            events: Vec::new(),
            cycle_index: Some(lap_numbers[idx]),
            segment_index: None,
            progress_m: Some(telemetry.s_m[idx] as f32),
            metadata: metadata.clone(),
        });
    }
    frames
}

fn resolve_vehicle_id(vehicle_id: Option<&str>) -> Result<&str, String> {
    vehicle_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "vehicle_id is required".to_string())
}

fn validate_component_selection_subjects(
    race: &RaceInput,
    selections: &HashMap<String, VehicleComponentSelectionV1>,
) -> Result<(), String> {
    for competitor_id in selections.keys() {
        if !race
            .competitors
            .iter()
            .any(|competitor| competitor.id == *competitor_id)
        {
            return Err(format!(
                "vehicle component selection references unknown competitor '{competitor_id}'"
            ));
        }
    }
    Ok(())
}

fn reject_component_selections_for_model(
    selections: &HashMap<String, VehicleComponentSelectionV1>,
    model: &str,
) -> Result<(), String> {
    if selections.is_empty() {
        return Ok(());
    }
    Err(format!(
        "{model} does not support vehicle component selection; select a component-aware model and catalog"
    ))
}

fn validate_component_selection(selection: &VehicleComponentSelectionV1) -> Result<(), String> {
    let components = [
        ("aero", selection.aero_id.as_deref()),
        ("chassis", selection.chassis_id.as_deref()),
        ("engine", selection.engine_id.as_deref()),
        ("tire", selection.tire_id.as_deref()),
    ];
    if components.iter().all(|(_, value)| value.is_none()) {
        return Err("vehicle component selection must override at least one component".to_string());
    }
    for (kind, value) in components {
        if let Some(value) = value
            && (value.is_empty() || value != value.trim())
        {
            return Err(format!(
                "vehicle component selection has non-canonical {kind} id '{value}'"
            ));
        }
    }
    Ok(())
}

fn normalize_track_id(track_id: &str) -> String {
    track_id
        .chars()
        .filter(|ch| !matches!(ch, '-' | '_' | ' '))
        .flat_map(char::to_uppercase)
        .collect()
}

fn browser_track_id(file_stem: &str) -> String {
    let slug = file_stem.split('-').next().unwrap_or(file_stem);
    normalize_track_id(slug)
}

fn track_from_payload(
    track_id: &str,
    payload: &SolverTrackProfile,
    pit_loss_ms: u64,
) -> Result<TrackRecord, String> {
    let n = payload.s.len();
    if n < 3 {
        return Err("track_profile requires at least 3 samples".to_string());
    }
    if payload.x.len() != n || payload.y.len() != n {
        return Err("track_profile s/x/y vectors must have the same length".to_string());
    }
    if !payload.s.windows(2).all(|window| window[1] > window[0]) {
        return Err("track_profile s values must be strictly increasing".to_string());
    }

    let z = if payload.z.is_empty() {
        vec![0.0; n]
    } else if payload.z.len() == n {
        payload.z.clone()
    } else {
        return Err("track_profile z must be empty or match the length of s".to_string());
    };

    let heading = derive_heading(&payload.x, &payload.y);
    let curvature = derive_curvature(&payload.s, &heading);
    let slope = derive_gradient(&payload.s, &z);

    Ok(TrackRecord {
        id: normalize_track_id(track_id),
        browser_id: normalize_track_id(track_id),
        display_name: normalize_track_id(track_id),
        country_code: None,
        laps: None,
        track: Track {
            s: payload.s.clone(),
            x: payload.x.clone(),
            y: payload.y.clone(),
            z,
            kappa: curvature,
            slope,
            heading,
        },
        pit_loss_ms,
    })
}

fn parse_aero(value: &Value) -> Result<AeroParams, String> {
    Ok(AeroParams {
        cd_a_x: read_required_f64(value, &["cdA_x", "cd_a_straight"])?,
        cd_a_z: read_required_f64(value, &["cdA_z", "cd_a_corner"])?,
        cl_a_x: read_required_f64(value, &["clA_x", "cl_a_straight"])?,
        cl_a_z: read_required_f64(value, &["clA_z", "cl_a_corner"])?,
    })
}

fn parse_chassis(value: &Value) -> Result<ChassisParams, String> {
    Ok(ChassisParams {
        mass_empty: read_required_f64(value, &["mass_empty", "mass_empty_kg"])?,
        r_wheel: read_required_f64(value, &["r_wheel", "wheel_radius_m"])?,
        mu0: read_required_f64(value, &["mu0"])?,
        c_rr: read_required_f64(value, &["c_rr", "rolling_resistance"])?,
        rho: read_required_f64(value, &["rho", "air_density"])?,
        g: read_optional_f64(value, &["g", "gravity"]).unwrap_or(9.81),
    })
}

fn parse_tire(value: &Value) -> Result<TireParams, String> {
    Ok(TireParams {
        mu_scale: read_required_f64(value, &["mu_scale"])?,
        wear_per_s: read_required_f64(value, &["wear_per_s"])?,
        wear_load_k: read_required_f64(value, &["wear_load_k"])?,
        wear_grip_k: read_required_f64(value, &["wear_grip_k"])?,
        wear_min: read_required_f64(value, &["wear_min"])?,
        temp_opt: read_required_f64(value, &["temp_opt", "temp_opt_c"])?,
        temp_sigma: read_required_f64(value, &["temp_sigma", "temp_sigma_c"])?,
        temp_min_k: read_required_f64(value, &["temp_min_k"])?,
        heat_k: read_required_f64(value, &["heat_k"])?,
        cool_k: read_required_f64(value, &["cool_k"])?,
    })
}

fn parse_vehicle(stem: &str, value: &Value) -> Result<VehicleRecord, String> {
    let _ = stem;
    Ok(VehicleRecord {
        aero_id: read_required_string(value, &["aero", "aero_id"])?,
        chassis_id: read_required_string(value, &["chassis", "chassis_id"])?,
        engine_id: read_required_string(value, &["engine", "engine_id"])?,
        tire_id: read_optional_string(value, &["tire", "tire_id"])
            .unwrap_or_else(|| "medium".to_string()),
    })
}

fn parse_driver(stem: &str, value: &Value) -> Result<Option<Driver>, String> {
    let Some(aggressiveness) = read_optional_f64(value, &["aggressiveness"]) else {
        return Ok(None);
    };

    Ok(Some(Driver {
        id: read_optional_string(value, &["id"]).unwrap_or_else(|| stem.to_string()),
        display_name: read_optional_string(value, &["display_name"])
            .unwrap_or_else(|| stem.to_string()),
        aggressiveness,
    }))
}

fn parse_engine(value: &Value) -> Result<EngineParams, String> {
    let n_rpm = build_series(
        value
            .get("n_rpm")
            .ok_or_else(|| "engine is missing n_rpm".to_string())?,
    )?;
    let trq = build_torque(
        value
            .get("trq_segments")
            .and_then(Value::as_array)
            .ok_or_else(|| "engine is missing trq_segments".to_string())?,
    )?;
    let gearbox = value
        .get("gearbox")
        .ok_or_else(|| "engine is missing gearbox".to_string())?;
    let g1_total = read_required_f64(gearbox, &["g1_total"])?;
    let g_last_total = read_required_f64(gearbox, &["g_last_total"])?;
    let gear_count = read_required_u64(gearbox, &["gear_count"])? as usize;
    let gear_ratios = build_gear_ratios(g1_total, g_last_total, gear_count.max(2));
    let thermal = value
        .get("thermal")
        .ok_or_else(|| "engine is missing thermal".to_string())?;

    Ok(EngineParams {
        n_rpm,
        trq,
        gear_ratios,
        n_upshift: read_optional_f64(value, &["n_upshift"]).unwrap_or(0.0),
        n_downshift: read_optional_f64(value, &["n_downshift"]).unwrap_or(0.0),
        n_idle: read_required_f64(value, &["n_idle"])?,
        n_max: read_required_f64(value, &["n_max"])?,
        t_amb: read_required_f64(thermal, &["t_amb"])?,
        t_init: read_required_f64(thermal, &["t_init"])?,
        c_th: read_required_f64(thermal, &["c_th"])?,
        alpha_heat: read_required_f64(thermal, &["alpha_heat"])?,
        p_cool0: read_required_f64(thermal, &["p_cool0"])?,
        k_cool: read_required_f64(thermal, &["k_cool"])?,
        t_soft: read_required_f64(thermal, &["t_soft"])?,
        beta_derate: read_required_f64(thermal, &["beta_derate"])?,
        fuel_burn_kg_per_s: read_optional_f64(value, &["fuel_burn_kg_per_s"]).unwrap_or(0.02),
    })
}

fn parse_track(stem: &str, value: &Value) -> Result<TrackRecord, String> {
    if value.get("distance_m").is_some() {
        return parse_compact_track(stem, value);
    }

    let data = value.get("data").unwrap_or(value);
    let id = normalize_track_id(
        read_optional_string(value.get("meta").unwrap_or(value), &["id"])
            .unwrap_or_else(|| stem.to_string())
            .as_str(),
    );
    let s = read_vec(data, "s_m")?;
    let x = read_vec(data, "x_m")?;
    let y = read_vec(data, "y_m")?;
    let z = read_vec(data, "z_m")?;
    let heading = if data.get("heading_rad").is_some() {
        read_vec(data, "heading_rad")?
    } else {
        derive_heading(&x, &y)
    };
    let curvature = if data.get("curvature_radpm").is_some() {
        read_vec(data, "curvature_radpm")?
    } else {
        derive_curvature(&s, &heading)
    };
    let slope = if data.get("slope_pct").is_some() {
        read_vec(data, "slope_pct")?
    } else if data.get("slope").is_some() {
        read_vec(data, "slope")?
    } else {
        derive_gradient(&s, &z)
    };

    Ok(TrackRecord {
        id,
        browser_id: browser_track_id(stem),
        display_name: derive_track_display_name(value).unwrap_or_else(|| stem.to_string()),
        country_code: derive_track_country_code(value),
        laps: derive_track_laps(value),
        track: Track {
            s,
            x,
            y,
            z,
            kappa: curvature,
            slope,
            heading,
        },
        pit_loss_ms: read_optional_u64(value, &["pit_loss_ms"]).unwrap_or(DEFAULT_PIT_LOSS_MS),
    })
}

fn parse_compact_track(stem: &str, value: &Value) -> Result<TrackRecord, String> {
    let id = normalize_track_id(
        read_optional_string(value, &["id"])
            .unwrap_or_else(|| stem.to_string())
            .as_str(),
    );
    let distance_m = read_required_f64(value, &["distance_m"])?;
    let radius_x = read_required_f64(value, &["radius_x"])?;
    let radius_y = read_required_f64(value, &["radius_y"])?;
    let wobble_x = read_required_f64(value, &["wobble_x"])?;
    let wobble_y = read_required_f64(value, &["wobble_y"])?;
    let slope_amp_m = read_required_f64(value, &["slope_amp_m"])?;
    let pit_loss_ms = read_optional_u64(value, &["pit_loss_ms"]).unwrap_or(DEFAULT_PIT_LOSS_MS);

    let points = 420usize;
    let mut s = Vec::with_capacity(points);
    let mut x = Vec::with_capacity(points);
    let mut y = Vec::with_capacity(points);
    let mut z = Vec::with_capacity(points);
    for i in 0..points {
        let t = i as f64 / (points - 1) as f64;
        let theta = t * std::f64::consts::TAU;
        s.push(t * distance_m);
        x.push(
            radius_x * theta.cos()
                + wobble_x * (2.6 * theta).cos() * 0.55
                + wobble_x * (4.2 * theta).sin() * 0.15,
        );
        y.push(
            radius_y * theta.sin()
                + wobble_y * (1.8 * theta).sin() * 0.60
                + wobble_y * (3.3 * theta).cos() * 0.20,
        );
        z.push(slope_amp_m * (1.7 * theta).sin() * 0.5 + slope_amp_m * (0.4 * theta).cos() * 0.2);
    }

    let heading = derive_heading(&x, &y);
    let curvature = derive_curvature(&s, &heading);
    let slope = derive_gradient(&s, &z);

    Ok(TrackRecord {
        id,
        browser_id: browser_track_id(stem),
        display_name: read_optional_string(value, &["name"])
            .unwrap_or_else(|| stem.replace('_', " ")),
        country_code: None,
        laps: read_optional_u16(value, &["laps"]),
        track: Track {
            s,
            x,
            y,
            z,
            kappa: curvature,
            slope,
            heading,
        },
        pit_loss_ms,
    })
}

fn derive_track_country_code(value: &Value) -> Option<String> {
    let meta = value.get("meta")?;
    let raw_id = read_optional_string(meta, &["id"])?;
    let prefix = raw_id.split('-').next()?.trim().to_uppercase();
    if prefix.len() == 2 && prefix.chars().all(|ch| ch.is_ascii_alphabetic()) {
        Some(prefix)
    } else {
        None
    }
}

fn derive_track_display_name(value: &Value) -> Option<String> {
    let meta = value.get("meta")?;
    read_optional_string(meta, &["Name", "name", "Location", "location"])
}

fn derive_track_laps(value: &Value) -> Option<u16> {
    let meta = value.get("meta")?;
    read_optional_u16(meta, &["laps"])
}

fn read_required_f64(value: &Value, keys: &[&str]) -> Result<f64, String> {
    read_optional_f64(value, keys)
        .ok_or_else(|| format!("missing numeric field '{}'", keys.join("' or '")))
}

fn read_optional_f64(value: &Value, keys: &[&str]) -> Option<f64> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_f64))
}

fn read_required_u64(value: &Value, keys: &[&str]) -> Result<u64, String> {
    read_optional_u64(value, keys)
        .ok_or_else(|| format!("missing integer field '{}'", keys.join("' or '")))
}

fn read_optional_u64(value: &Value, keys: &[&str]) -> Option<u64> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_u64))
}

fn read_optional_u16(value: &Value, keys: &[&str]) -> Option<u16> {
    read_optional_u64(value, keys).and_then(|value| u16::try_from(value).ok())
}

fn read_required_string(value: &Value, keys: &[&str]) -> Result<String, String> {
    read_optional_string(value, keys)
        .ok_or_else(|| format!("missing string field '{}'", keys.join("' or '")))
}

fn read_optional_string(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str).map(str::to_string))
}

fn read_vec(value: &Value, key: &str) -> Result<Vec<f64>, String> {
    let arr = value
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("missing array key '{key}'"))?;
    let mut out = Vec::with_capacity(arr.len());
    for item in arr {
        let Some(num) = item.as_f64() else {
            return Err(format!("non-numeric value in array '{key}'"));
        };
        out.push(num);
    }
    Ok(out)
}

fn build_series(value: &Value) -> Result<Vec<f64>, String> {
    if let Some(items) = value.as_array() {
        let mut out = Vec::with_capacity(items.len());
        for item in items {
            let Some(num) = item.as_f64() else {
                return Err("n_rpm must contain only numeric values".to_string());
            };
            out.push(num);
        }
        return Ok(out);
    }

    let start = read_required_f64(value, &["start"])?;
    let end = read_required_f64(value, &["end"])?;
    let step = read_required_f64(value, &["step"])?;
    if step <= 0.0 || end < start {
        return Ok(vec![start]);
    }

    let mut out = Vec::new();
    let mut current = start;
    while current <= end + step * 0.5 {
        out.push(current);
        current += step;
    }
    Ok(out)
}

fn build_torque(segments: &[Value]) -> Result<Vec<f64>, String> {
    let mut out = Vec::new();
    for segment in segments {
        let kind = read_required_string(segment, &["type"])?;
        match kind.as_str() {
            "linspace" => {
                let start = read_required_f64(segment, &["start"])?;
                let end = read_required_f64(segment, &["end"])?;
                let num = read_required_u64(segment, &["num"])? as usize;
                if num == 0 {
                    continue;
                }
                if num == 1 {
                    out.push(start);
                    continue;
                }
                for idx in 0..num {
                    let a = idx as f64 / (num - 1) as f64;
                    out.push(start + (end - start) * a);
                }
            }
            "list" => {
                let values = segment
                    .get("values")
                    .and_then(Value::as_array)
                    .ok_or_else(|| "list torque segment is missing values".to_string())?;
                for item in values {
                    let Some(num) = item.as_f64() else {
                        return Err(
                            "list torque segment must contain only numeric values".to_string()
                        );
                    };
                    out.push(num);
                }
            }
            other => return Err(format!("unknown torque segment type '{other}'")),
        }
    }
    Ok(out)
}

fn build_gear_ratios(g1_total: f64, g_last_total: f64, gear_count: usize) -> Vec<f64> {
    if gear_count <= 1 {
        return vec![g1_total];
    }
    let mut out = Vec::with_capacity(gear_count);
    for gear in 0..gear_count {
        let a = gear as f64 / (gear_count - 1) as f64;
        out.push(g1_total * (g_last_total / g1_total).powf(a));
    }
    out
}

fn derive_heading(x: &[f64], y: &[f64]) -> Vec<f64> {
    let n = x.len().min(y.len());
    let mut heading = vec![0.0; n];
    for (i, value) in heading.iter_mut().enumerate() {
        let i0 = i.saturating_sub(1);
        let i1 = (i + 1).min(n - 1);
        let dx = x[i1] - x[i0];
        let dy = y[i1] - y[i0];
        *value = dy.atan2(dx);
    }
    for i in 1..n {
        heading[i] = unwrap_angle(heading[i], heading[i - 1]);
    }
    heading
}

fn derive_curvature(s: &[f64], heading: &[f64]) -> Vec<f64> {
    let n = s.len().min(heading.len());
    let mut out = vec![0.0; n];
    for (i, value) in out.iter_mut().enumerate() {
        let i0 = i.saturating_sub(1);
        let i1 = (i + 1).min(n - 1);
        let ds = (s[i1] - s[i0]).abs().max(1e-6);
        *value = (heading[i1] - heading[i0]) / ds;
    }
    out
}

fn derive_gradient(s: &[f64], values: &[f64]) -> Vec<f64> {
    let n = s.len().min(values.len());
    let mut out = vec![0.0; n];
    for (i, value) in out.iter_mut().enumerate() {
        let i0 = i.saturating_sub(1);
        let i1 = (i + 1).min(n - 1);
        let ds = (s[i1] - s[i0]).abs().max(1e-6);
        *value = (values[i1] - values[i0]) / ds;
    }
    out
}

fn unwrap_angle(mut value: f64, reference: f64) -> f64 {
    while value - reference > std::f64::consts::PI {
        value -= std::f64::consts::TAU;
    }
    while value - reference < -std::f64::consts::PI {
        value += std::f64::consts::TAU;
    }
    value
}

fn serialize_json<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value)
        .unwrap_or_else(|err| json_error(&format!("serialization error: {err}")))
}

fn json_error(message: &str) -> String {
    serde_json::json!({ "error": message }).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(not(target_arch = "wasm32"))]
    use pitgun_racing_contract::RacingModelParametersPurpose;
    use pitgun_racing_contract::{
        CompetitorSpec, RaceInput, RaceStint, RacingDriverControlProfileVersion,
        RacingDriverInstructionBoundaryGranularityV1, RacingDriverInstructionEventV1,
        RacingDriverInstructionProfileVersion, RacingDriverInstructionTimelineVersion,
        RacingDriverResourceVersion, RacingDriverUtilizationResponseV1,
        RacingDrivingModeCommitmentsV1, TuningSpec, VehicleComponentSelectionVersion,
    };
    use pitgun_runtime::LinkedWorkload;
    use serde::Deserialize;

    #[test]
    fn hosted_workload_selection_requires_exact_model_and_catalog_identities() {
        let model_v1_catalog = RacingCatalogSnapshot::embedded().expect("model V1 catalog");
        let model_v2_catalog =
            RacingCatalogSnapshot::embedded_model_v2().expect("model V2 catalog");
        let model_v3_catalog =
            RacingCatalogSnapshot::embedded_model_v3_thermal().expect("model V3 thermal catalog");
        let component_model_catalog = RacingCatalogSnapshot::embedded_model_v3_component()
            .expect("component-composed Model V3 catalog");
        let timeline_model_catalog = RacingCatalogSnapshot::embedded_model_v3_timeline()
            .expect("timeline-enabled Model V3 catalog");
        let fuel_contract_model_catalog = RacingCatalogSnapshot::embedded_model_v3_fuel_contract()
            .expect("fuel-contract Model V3 catalog");

        let selected = racing_workload_for(&racing_model_v2_identity(), &model_v2_catalog)
            .expect("exact model V2 selection");
        assert_eq!(selected.model_identity(), &racing_model_v2_identity());
        assert!(
            racing_workload_for(&racing_model_v1_identity(), &model_v2_catalog).is_err(),
            "model V1 must not run against the model V2 catalog"
        );
        assert!(
            racing_workload_for(&racing_model_v2_identity(), &model_v1_catalog).is_err(),
            "model V2 must not run against the model V1 catalog"
        );
        assert!(
            racing_workload_for(&racing_model_v3_candidate_identity(), &model_v2_catalog).is_err(),
            "the offline V3 candidate must not run through a production V2 catalog"
        );
        let selected_v3 = racing_workload_for(
            &racing_model_v3_thermal_candidate_identity(),
            &model_v3_catalog,
        )
        .expect("exact Model V3 thermal selection");
        assert_eq!(
            selected_v3.model_identity(),
            &racing_model_v3_thermal_candidate_identity()
        );
        assert!(
            racing_workload_for(&racing_model_v2_identity(), &model_v3_catalog).is_err(),
            "model V2 must not run against the V3 thermal catalog"
        );
        let selected_component = racing_workload_for(
            &racing_model_v3_component_candidate_identity(),
            &component_model_catalog,
        )
        .expect("exact component Model V3 selection");
        assert_eq!(
            selected_component.model_identity(),
            &racing_model_v3_component_candidate_identity()
        );
        let selected_timeline = racing_workload_for(
            &racing_model_v3_timeline_candidate_identity(),
            &timeline_model_catalog,
        )
        .expect("exact timeline Model V3 selection");
        assert_eq!(
            selected_timeline.model_identity(),
            &racing_model_v3_timeline_candidate_identity()
        );
        let selected_fuel_contract = racing_workload_for(
            &racing_model_v3_fuel_contract_candidate_identity(),
            &fuel_contract_model_catalog,
        )
        .expect("exact fuel-contract Model V3 selection");
        assert_eq!(
            selected_fuel_contract.model_identity(),
            &racing_model_v3_fuel_contract_candidate_identity()
        );
        assert!(
            racing_workload_for(
                &racing_model_v3_component_candidate_identity(),
                &timeline_model_catalog,
            )
            .is_err(),
            "component Model 0.11 must not run against timeline Catalog 1.8"
        );
        assert!(
            racing_workload_for(
                &racing_model_v3_timeline_candidate_identity(),
                &component_model_catalog,
            )
            .is_err(),
            "timeline Model 0.14 must not run against component Catalog 1.6"
        );
        assert!(
            racing_workload_for(
                &racing_model_v3_timeline_candidate_identity(),
                &fuel_contract_model_catalog,
            )
            .is_err(),
            "timeline Model 0.14 must not run against fuel-contract Catalog 1.9"
        );

        let mut forged = racing_model_v2_identity();
        forged.digest = pitgun_contract::Digest::from_bytes(b"forged model V2");
        assert!(
            racing_workload_for(&forged, &model_v2_catalog).is_err(),
            "a known version with an unknown digest must fail closed"
        );
    }

    #[derive(Debug, Deserialize)]
    struct GoldenFixture {
        track_id: String,
        vehicle_id: String,
        driver_id: String,
        lap_count: u16,
        config: GoldenConfig,
        initial_state: VehicleState,
        expected: GoldenExpected,
    }

    #[derive(Debug, Deserialize)]
    struct GoldenConfig {
        ds: f64,
        max_speed: f64,
        pit_time_penalty_s: f64,
        pit_tire_temp: Option<f64>,
        tire_temp_amb: f64,
        sim_seed: u64,
    }

    #[derive(Debug, Deserialize)]
    struct GoldenExpected {
        total_time_s: f64,
        sample_count: usize,
        speed_tail: Vec<f64>,
        gear_tail: Vec<u8>,
        final_state: VehicleState,
    }

    fn approx_eq(actual: f64, expected: f64, tolerance: f64, label: &str) {
        let delta = (actual - expected).abs();
        assert!(
            delta <= tolerance,
            "{label} mismatch: expected {expected:.12}, got {actual:.12}, delta {delta:.12}, tolerance {tolerance:.12}"
        );
    }

    fn approx_slice_eq(actual: &[f64], expected: &[f64], tolerance: f64, label: &str) {
        assert_eq!(
            actual.len(),
            expected.len(),
            "{label} length mismatch: expected {}, got {}",
            expected.len(),
            actual.len()
        );
        for (idx, (actual, expected)) in actual.iter().zip(expected.iter()).enumerate() {
            approx_eq(*actual, *expected, tolerance, &format!("{label}[{idx}]"));
        }
    }

    fn one_lap_request() -> RunRaceRequest {
        RunRaceRequest {
            input: RunRaceInput {
                race: RaceInput {
                    track_id: "it-1922".to_string(),
                    laps: 1,
                    competitors: vec![CompetitorSpec {
                        id: "player".to_string(),
                        driver_id: Some("default".to_string()),
                        name: "Player".to_string(),
                        team_id: "team".to_string(),
                        is_player: true,
                        tuning: TuningSpec {
                            engine_points: 25.0,
                            cooling_points: 25.0,
                            aero_points: 25.0,
                            chassis_points: 25.0,
                            downforce_slider: 0.5,
                            gear_ratio_slider: 0.5,
                        },
                        budget_cap: 100.0,
                        stint_strategy: None,
                    }],
                },
                vehicle_id: Some("f1_2026".to_string()),
                competitor_vehicle_components: HashMap::new(),
                pit_strategy: None,
                track_profile: None,
                competitor_profiles: HashMap::new(),
                era: 2026,
                hz: 20.0,
                initial_fuel_mass_kg: None,
            },
            seed: 7,
            era: Some(2026),
            hz: Some(20.0),
        }
    }

    #[test]
    fn timeline_candidate_uses_catalog_drivers_and_common_default_fail_closed() {
        let snapshot = RacingCatalogSnapshot::embedded_model_v3_timeline()
            .expect("timeline candidate catalog");
        let mut request = one_lap_request();
        request.input.race.competitors[0].driver_id = Some("balanced_reference".to_string());
        let empty_timeline = RacingDriverInstructionTimelineV1 {
            schema_version: RacingDriverInstructionTimelineVersion::V1,
            events: Vec::new(),
        };

        let output = run_race_with_catalog_and_v3_timeline_candidate(
            request.clone(),
            &snapshot,
            empty_timeline.clone(),
        )
        .expect("catalog-governed timeline candidate");
        let schedule = output
            .competitor_driver_instruction_schedules_v3
            .get("player")
            .expect("player driver schedule");
        assert_eq!(schedule.transitions.len(), 1);
        assert_eq!(schedule.transitions[0].sequence, None);
        assert_eq!(schedule.transitions[0].mode, RacingDrivingMode::Balanced);
        assert_eq!(
            schedule.transitions[0].physical.driver_id,
            "balanced_reference"
        );

        request.input.race.competitors[0].driver_id = Some("default".to_string());
        let error =
            run_race_with_catalog_and_v3_timeline_candidate(request, &snapshot, empty_timeline)
                .expect_err("legacy-only driver must fail closed");
        assert!(error.contains("missing V2 driver resource \"default\""));
    }

    #[test]
    fn fuel_contract_candidate_is_catalog_governed_and_deterministic() {
        let snapshot = RacingCatalogSnapshot::embedded_model_v3_fuel_contract()
            .expect("fuel-contract candidate catalog");
        let mut request = one_lap_request();
        request.input.race.competitors[0].driver_id = Some("balanced_reference".to_string());
        let timeline = RacingDriverInstructionTimelineV1 {
            schema_version: RacingDriverInstructionTimelineVersion::V1,
            events: Vec::new(),
        };

        let first = run_race_with_catalog_and_v3_fuel_contract_candidate(
            request.clone(),
            &snapshot,
            timeline.clone(),
        )
        .expect("first fuel-contract execution");
        let second = run_race_with_catalog_and_v3_fuel_contract_candidate(
            request.clone(),
            &snapshot,
            timeline.clone(),
        )
        .expect("second fuel-contract execution");
        assert_eq!(
            serde_json::to_value(&first).expect("first JSON"),
            serde_json::to_value(&second).expect("second JSON")
        );
        let diagnostics = first
            .player_fuel_mass_diagnostics_v3
            .expect("fuel diagnostics");
        assert_eq!(diagnostics.initial_fuel_mass_kg, 110.0);
        assert!(diagnostics.final_fuel_mass_kg >= 1.0);

        request.input.initial_fuel_mass_kg = Some(200.0);
        let error =
            run_race_with_catalog_and_v3_fuel_contract_candidate(request, &snapshot, timeline)
                .expect_err("client fuel override must fail closed");
        assert!(error.contains("forbids an initial-fuel override"));
    }

    fn component_selection(
        aero_id: Option<&str>,
        chassis_id: Option<&str>,
        engine_id: Option<&str>,
        tire_id: Option<&str>,
    ) -> VehicleComponentSelectionV1 {
        VehicleComponentSelectionV1 {
            schema_version: VehicleComponentSelectionVersion::V1,
            aero_id: aero_id.map(str::to_string),
            chassis_id: chassis_id.map(str::to_string),
            engine_id: engine_id.map(str::to_string),
            tire_id: tire_id.map(str::to_string),
        }
    }

    fn driver_control_profile_v10() -> V3CandidateExperimentProfile {
        V3CandidateExperimentProfile {
            schema_version: V3CandidateExperimentProfileVersion::V10,
            driver_control_profile: Some(RacingDriverControlProfileV1 {
                schema_version: RacingDriverControlProfileVersion::V1,
                mode_commitments: RacingDrivingModeCommitmentsV1 {
                    manage: 0.60,
                    balanced: 0.80,
                    attack: 1.00,
                },
                cornering: RacingDriverUtilizationResponseV1 {
                    floor: 0.80,
                    span: 0.20,
                },
                braking: RacingDriverUtilizationResponseV1 {
                    floor: 0.78,
                    span: 0.22,
                },
                traction: RacingDriverUtilizationResponseV1 {
                    floor: 0.82,
                    span: 0.18,
                },
                base_control_error: 0.005,
                commitment_error_gain: 0.08,
                commitment_error_exponent: 2.0,
                correction_workload_gain: 2.0,
            }),
            ..V3CandidateExperimentProfile::default()
        }
    }

    fn driver_friction_profile_v11() -> V3CandidateExperimentProfile {
        V3CandidateExperimentProfile {
            schema_version: V3CandidateExperimentProfileVersion::V11,
            ..driver_control_profile_v10()
        }
    }

    fn driver_resource(consistency: f64, tire_management: f64) -> RacingDriverResourceV2 {
        RacingDriverResourceV2 {
            schema_version: RacingDriverResourceVersion::V2,
            id: "default".to_string(),
            traits: RacingDriverTraitsV1 {
                limit_exploitation: 0.85,
                consistency,
                tire_management,
            },
        }
    }

    #[test]
    fn historical_race_input_omits_empty_component_selection_map() {
        let value = serde_json::to_value(one_lap_request().input).expect("race input");
        assert!(value.get("competitor_vehicle_components").is_none());
    }

    #[test]
    fn vehicle_components_override_independent_physical_resources() {
        let catalog = EmbeddedCatalog::load_default().expect("catalog");
        let (legacy, _) = catalog
            .resolve_vehicle("classic_v8_1960")
            .expect("legacy vehicle");
        let (aero_upgrade, _) = catalog
            .resolve_vehicle_with_components(
                "classic_v8_1960",
                Some(&component_selection(Some("basic"), None, None, None)),
            )
            .expect("independent aero upgrade");
        let (engine_upgrade, _) = catalog
            .resolve_vehicle_with_components(
                "classic_v8_1960",
                Some(&component_selection(None, None, Some("v8_1970"), None)),
            )
            .expect("independent engine upgrade");

        assert_eq!(legacy.aero.cl_a_x, 0.0);
        assert!(aero_upgrade.aero.cl_a_x > 0.0);
        assert_eq!(aero_upgrade.engine.n_rpm, legacy.engine.n_rpm);
        assert_eq!(engine_upgrade.aero, legacy.aero);
        assert_ne!(engine_upgrade.engine.n_rpm, legacy.engine.n_rpm);
    }

    #[test]
    fn vehicle_component_selection_rejects_unknown_and_redundant_overrides() {
        let catalog = EmbeddedCatalog::load_default().expect("catalog");
        assert!(
            catalog
                .resolve_vehicle_with_components(
                    "classic_v8_1960",
                    Some(&component_selection(Some("unknown"), None, None, None)),
                )
                .expect_err("unknown aero")
                .contains("unknown aero")
        );
        assert!(
            catalog
                .resolve_vehicle_with_components(
                    "classic_v8_1960",
                    Some(&component_selection(None, None, None, None)),
                )
                .expect_err("empty selection")
                .contains("must override at least one component")
        );
    }

    #[test]
    fn component_catalog_exposes_effective_controls_and_unavailable_reasons() {
        let snapshot = RacingCatalogSnapshot::embedded_model_v3_component()
            .expect("component-composed catalog");
        let historical =
            resolve_vehicle_capabilities_with_catalog(&snapshot, "classic_v8_1960", None)
                .expect("historical capabilities");
        assert!(
            historical
                .supported_capabilities
                .contains(&pitgun_racing_contract::VehicleCapability::AdjustableGearRatio)
        );
        let unavailable_downforce = historical
            .unavailable_capabilities
            .iter()
            .find(|entry| {
                entry.capability == pitgun_racing_contract::VehicleCapability::AdjustableDownforce
            })
            .expect("unavailable downforce explanation");
        assert_eq!(unavailable_downforce.installed_component_id, "none");

        let upgraded = resolve_vehicle_capabilities_with_catalog(
            &snapshot,
            "classic_v8_1960",
            Some(&component_selection(Some("basic"), None, None, None)),
        )
        .expect("upgraded capabilities");
        assert!(
            upgraded
                .supported_capabilities
                .contains(&pitgun_racing_contract::VehicleCapability::AdjustableDownforce)
        );
        assert!(
            upgraded
                .unavailable_capabilities
                .iter()
                .any(|entry| entry.capability
                    == pitgun_racing_contract::VehicleCapability::EnergyDeployment),
            "future energy controls must not be advertised before their equations exist"
        );

        let browser_snapshot = catalog_snapshot_with_catalog(&snapshot).expect("browser snapshot");
        assert_eq!(browser_snapshot.components.len(), 12);
        assert!(browser_snapshot.component_capability_profile.is_some());
        let historical_snapshot = catalog_snapshot().expect("historical browser snapshot");
        assert!(historical_snapshot.components.is_empty());
        assert!(historical_snapshot.component_capability_profile.is_none());
    }

    #[test]
    fn capability_resolution_has_native_and_browser_bundle_parity() {
        let snapshot = RacingCatalogSnapshot::embedded_model_v3_component()
            .expect("component-composed catalog");
        let request = ResolveVehicleCapabilitiesRequestV1 {
            vehicle_id: "f1_2026".to_string(),
            components: None,
        };
        let native = resolve_vehicle_capabilities_with_catalog(&snapshot, "f1_2026", None)
            .expect("native resolution");
        let browser_json = resolve_vehicle_capabilities_json_from_bundle(
            serde_json::to_string(&request).expect("request JSON"),
            serde_json::to_string(&snapshot.to_bundle().expect("catalog bundle"))
                .expect("bundle JSON"),
        );
        let browser: ResolvedVehicleCapabilitiesV1 =
            serde_json::from_str(&browser_json).expect("browser resolution");
        assert_eq!(browser, native);
    }

    #[test]
    fn component_model_output_records_each_competitor_composition() {
        let snapshot = RacingCatalogSnapshot::embedded_model_v3_component()
            .expect("component-composed catalog");
        let output = run_race_with_catalog_and_v3_power_unit_thermal_profile(
            one_lap_request(),
            &snapshot,
            snapshot
                .power_unit_thermal_profile()
                .expect("power-unit thermal profile"),
        )
        .expect("component-composed race");
        let player = output
            .competitor_vehicle_capabilities_v3
            .get("player")
            .expect("player component lineage");
        assert_eq!(player.baseline_vehicle_id, "f1_2026");
        assert_eq!(player.components.len(), 4);
    }

    #[test]
    fn one_race_accepts_distinct_component_selections_for_each_competitor() {
        let snapshot = RacingCatalogSnapshot::embedded().expect("catalog");
        let mut request = one_lap_request();
        let mut rival = request.input.race.competitors[0].clone();
        rival.id = "rival".to_string();
        rival.name = "Rival".to_string();
        rival.is_player = false;
        request.input.race.competitors.push(rival);
        request.input.competitor_vehicle_components.insert(
            "player".to_string(),
            component_selection(Some("none"), None, None, None),
        );
        request.input.competitor_vehicle_components.insert(
            "rival".to_string(),
            component_selection(None, None, Some("v8_1970"), None),
        );

        let output = run_race_with_catalog_and_v3_profile(
            request,
            &snapshot,
            &V3CandidateExperimentProfile::default(),
        )
        .expect("race with competitor-specific components");
        assert_eq!(output.standings.len(), 2);
    }

    #[test]
    fn race_rejects_component_selection_for_unknown_competitor() {
        let snapshot = RacingCatalogSnapshot::embedded().expect("catalog");
        let mut request = one_lap_request();
        request.input.competitor_vehicle_components.insert(
            "ghost".to_string(),
            component_selection(Some("none"), None, None, None),
        );

        let error = run_race_with_catalog_and_v3_profile(
            request,
            &snapshot,
            &V3CandidateExperimentProfile::default(),
        )
        .expect_err("unknown competitor selection must fail");
        assert!(error.contains("unknown competitor 'ghost'"));
    }

    #[test]
    fn published_historical_model_rejects_new_component_semantics() {
        let snapshot = RacingCatalogSnapshot::embedded().expect("catalog");
        let mut request = one_lap_request();
        request.input.competitor_vehicle_components.insert(
            "player".to_string(),
            component_selection(Some("none"), None, None, None),
        );

        let error = run_race_with_catalog(request, &snapshot)
            .expect_err("published model identity must keep historical semantics");
        assert!(error.contains("does not support vehicle component selection"));
    }

    #[test]
    fn thermal_profile_rejects_power_unit_overrides_until_power_unit_binding_exists() {
        let snapshot = RacingCatalogSnapshot::embedded_model_v3_thermal().expect("V3 catalog");
        let candidate = snapshot
            .thermal_family_profile()
            .expect("reviewed thermal family");
        let mut request = one_lap_request();
        request.input.competitor_vehicle_components.insert(
            "player".to_string(),
            component_selection(None, None, Some("v8_1970"), None),
        );

        let error =
            run_race_with_catalog_and_v3_thermal_family_profile(request, &snapshot, candidate)
                .expect_err("vehicle-bound thermal profile must fail closed");
        assert!(error.contains("does not support vehicle component selection"));
    }

    #[test]
    fn python_monza_reference_stays_close_except_launch_override() {
        let fixture: GoldenFixture = serde_json::from_str(include_str!(
            "../tests/golden/python_monza_f1_2026_default.json"
        ))
        .expect("golden fixture");

        let catalog = EmbeddedCatalog::load_default().expect("catalog");
        let track = catalog
            .get_track(&fixture.track_id)
            .expect("track")
            .track
            .clone();
        let (vehicle, _) = catalog
            .resolve_vehicle(&fixture.vehicle_id)
            .expect("vehicle");
        let driver = catalog
            .resolve_driver(Some(&fixture.driver_id))
            .expect("driver");

        let request = SimulationRequest {
            track,
            vehicle,
            state: fixture.initial_state.clone(),
            config: SimConfig {
                ds: fixture.config.ds,
                max_speed: fixture.config.max_speed,
                pit_time_penalty_s: fixture.config.pit_time_penalty_s,
                pit_tire_temp: fixture.config.pit_tire_temp,
                tire_temp_amb: fixture.config.tire_temp_amb,
                sim_seed: fixture.config.sim_seed,
            },
            lap_count: fixture.lap_count,
            pit_plan: PitPlan::default(),
            driver,
            tuning: None,
        };

        let result = run_simulation(&request).expect("simulation result");
        let solution = &result.solution;

        assert_eq!(
            solution.s.len(),
            fixture.expected.sample_count,
            "sample_count mismatch"
        );
        approx_eq(solution.v[0], 0.0, 0.001, "speed_head[0]");
        approx_slice_eq(
            &solution.v[solution.v.len() - fixture.expected.speed_tail.len()..],
            &fixture.expected.speed_tail,
            2.000,
            "speed_tail",
        );
        assert_eq!(
            &solution.gear[solution.gear.len() - fixture.expected.gear_tail.len()..],
            fixture.expected.gear_tail.as_slice(),
            "gear_tail mismatch"
        );
        approx_eq(
            result.final_state.fuel_mass,
            fixture.expected.final_state.fuel_mass,
            0.100,
            "final_state.fuel_mass",
        );
        approx_eq(
            result.final_state.tire_wear,
            fixture.expected.final_state.tire_wear,
            0.001,
            "final_state.tire_wear",
        );
        approx_eq(
            result.final_state.tire_temp,
            fixture.expected.final_state.tire_temp,
            5.0,
            "final_state.tire_temp",
        );
        approx_eq(
            result.final_state.engine_temp,
            fixture.expected.final_state.engine_temp,
            15.0,
            "final_state.engine_temp",
        );
        // The current kernel is close to the Python reference but is not yet
        // bit-exact because NumPy PCG64, some data mappings, and the
        // intentionally overridden standing-start launch model still differ.
        approx_eq(
            result.total_time_s,
            fixture.expected.total_time_s,
            6.000,
            "total_time_s",
        );
    }

    #[test]
    fn json_exports_smoke_with_player_telemetry_at_5hz() {
        let catalog: CatalogSnapshot =
            serde_json::from_str(&catalog_json()).expect("catalog_json must return valid JSON");
        assert!(
            catalog.engines.iter().any(|entry| entry.id == "v6t_hybrid"),
            "catalog must expose v6t_hybrid"
        );
        let monza = catalog
            .circuits
            .iter()
            .find(|entry| entry.id == "MONZA")
            .expect("browser catalog must preserve the MONZA identifier");
        assert_eq!(monza.display_name, "Autodromo Nazionale Monza");
        assert_eq!(monza.country_code.as_deref(), Some("IT"));
        assert_eq!(monza.laps, Some(53));
        assert!(catalog.vehicles.iter().any(|entry| {
            entry.id == "f1_2026"
                && entry.engine_id == "v6t_hybrid"
                && entry.default_tire_id == "medium"
        }));
        assert!(catalog.tires.iter().any(|entry| entry.id == "medium"));

        for browser_catalog_json in [list_drivers_json(), list_vehicles_json(), list_tires_json()] {
            let value: serde_json::Value =
                serde_json::from_str(&browser_catalog_json).expect("browser catalog JSON");
            assert!(value.is_array(), "browser catalog export must be an array");
        }

        let request = one_lap_request();

        let output: RaceOutput = serde_json::from_str(&run_race_json(
            serde_json::to_string(&request).expect("serialize request"),
        ))
        .expect("run_race_json must return valid JSON");

        let snapshot = RacingCatalogSnapshot::embedded().expect("embedded catalog");
        let implicit_default =
            run_race_with_catalog(request.clone(), &snapshot).expect("implicit default response");
        let explicit_default = run_race_with_catalog_and_tuning_response(
            request,
            &snapshot,
            &TuningResponseV1::default(),
        )
        .expect("explicit default response");
        assert_eq!(
            serde_json::to_value(implicit_default).expect("implicit JSON"),
            serde_json::to_value(explicit_default).expect("explicit JSON"),
            "the experimental boundary must preserve the production default"
        );

        assert!(
            !output.player_batches.is_empty(),
            "player telemetry batches must be present"
        );

        let frames = output
            .player_batches
            .iter()
            .flat_map(|batch| batch.frames.iter())
            .collect::<Vec<_>>();
        assert!(frames.len() >= 2, "expected at least two 5 Hz frames");
        assert_eq!(frames[0].samples.len(), 17, "gateway sample count changed");
        assert_eq!(
            frames[0].metadata.get("sampling_hz").map(String::as_str),
            Some("5"),
            "player telemetry must advertise 5 Hz"
        );
        assert_eq!(
            frames[1].timestamp_us - frames[0].timestamp_us,
            200_000,
            "5 Hz telemetry must be sampled every 200 ms"
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn catalog_backed_model_parameters_preserve_model_v2_output() {
        let root =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../catalogs/racing/v1.4.0");
        let snapshot =
            RacingCatalogSnapshot::from_release_dir(root).expect("catalog-backed model parameters");
        let resolved =
            resolve_catalog_tuning_response(&snapshot, Some(&racing_model_v2_identity()))
                .expect("resolved model parameters");
        assert_eq!(resolved, TuningResponseV1::default());

        let request = one_lap_request();
        let automatic =
            run_race_with_catalog(request.clone(), &snapshot).expect("automatic catalog output");
        let historical_default = run_race_with_catalog_and_tuning_response(
            request.clone(),
            &snapshot,
            &TuningResponseV1::default(),
        )
        .expect("historical default output");
        assert_eq!(
            pitgun_contract::canonical_json_bytes(&automatic).expect("canonical automatic JSON"),
            pitgun_contract::canonical_json_bytes(&historical_default)
                .expect("canonical historical JSON"),
            "automatic catalog resolution must preserve the historical response"
        );

        let catalog_backed = run_race_with_catalog_and_model_response(
            request.clone(),
            &snapshot,
            &resolved,
            CurvatureAeroResponse::ContinuousV1,
        )
        .expect("catalog-backed output");
        let compatibility = run_race_with_catalog_and_model_response(
            request,
            &snapshot,
            &TuningResponseV1::default(),
            CurvatureAeroResponse::ContinuousV1,
        )
        .expect("compiled compatibility output");
        let mut offline_candidate = snapshot
            .model_parameters()
            .expect("catalog model parameters")
            .clone();
        offline_candidate.purpose = RacingModelParametersPurpose::ModelV2OfflineCandidate;
        let explicit_resource = run_race_with_catalog_and_model_parameters(
            one_lap_request(),
            &snapshot,
            &racing_model_v2_identity(),
            &offline_candidate,
            CurvatureAeroResponse::ContinuousV1,
        )
        .expect("explicit resource output");
        assert_eq!(
            snapshot
                .model_parameters()
                .expect("catalog resource")
                .purpose,
            RacingModelParametersPurpose::ModelV2Compatibility,
            "offline screening must not mutate the catalog-backed default"
        );

        assert_eq!(
            pitgun_contract::canonical_json_bytes(&catalog_backed)
                .expect("canonical catalog-backed JSON"),
            pitgun_contract::canonical_json_bytes(&compatibility)
                .expect("canonical compatibility JSON"),
            "catalog storage must not change Model V2 output"
        );
        assert_eq!(
            pitgun_contract::canonical_json_bytes(&catalog_backed)
                .expect("canonical catalog-backed JSON"),
            pitgun_contract::canonical_json_bytes(&explicit_resource)
                .expect("canonical explicit-resource JSON"),
            "offline resource injection must use the same validated semantics"
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn v3_candidate_resolves_gameplay_before_the_solver_and_is_deterministic() {
        let root =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../catalogs/racing/v1.4.0");
        let snapshot = RacingCatalogSnapshot::from_release_dir(root).expect("catalog snapshot");
        let request = one_lap_request();

        let first = run_race_with_catalog_and_v3_candidate(
            request.clone(),
            &snapshot,
            &TuningResponseV1::default(),
        )
        .expect("first V3 candidate race");
        let second = run_race_with_catalog_and_v3_candidate(
            request.clone(),
            &snapshot,
            &TuningResponseV1::default(),
        )
        .expect("second V3 candidate race");
        let model_v2 = run_race_with_catalog_and_model_response(
            request,
            &snapshot,
            &TuningResponseV1::default(),
            CurvatureAeroResponse::ContinuousV1,
        )
        .expect("Model V2 race");

        assert_eq!(
            pitgun_contract::canonical_json_bytes(&first).expect("first canonical V3 output"),
            pitgun_contract::canonical_json_bytes(&second).expect("second canonical V3 output"),
        );
        assert_ne!(
            first.total_time_ms, model_v2.total_time_ms,
            "the aggregate contact patch is the first intentionally distinct V3 physics slice",
        );
        assert_ne!(
            racing_model_v3_candidate_identity(),
            racing_model_v2_identity(),
            "even an equivalent uniform-grid fixture must retain distinct model lineage",
        );
    }

    #[test]
    fn v3_experiment_profile_applies_exact_mechanical_overrides() {
        let snapshot = RacingCatalogSnapshot::embedded_model_v2().expect("catalog snapshot");
        let baseline = run_race_with_catalog_and_v3_profile(
            one_lap_request(),
            &snapshot,
            &V3CandidateExperimentProfile::default(),
        )
        .expect("baseline V3 profile");
        let profile = V3CandidateExperimentProfile {
            mechanical_overrides: V3MechanicalOverrides {
                maximum_brake_force_n: Some(10_000.0),
                ..V3MechanicalOverrides::default()
            },
            ..V3CandidateExperimentProfile::default()
        };
        let constrained =
            run_race_with_catalog_and_v3_profile(one_lap_request(), &snapshot, &profile)
                .expect("constrained V3 profile");

        assert_eq!(
            constrained
                .player_mechanical_diagnostics_v3
                .expect("mechanical diagnostics")
                .maximum_brake_force_n,
            10_000.0,
        );
        assert_ne!(baseline.total_time_ms, constrained.total_time_ms);
        assert!(baseline.player_tire_diagnostics_v3.is_some());
        assert!(
            baseline
                .player_mechanical_diagnostics_v3
                .expect("baseline mechanical diagnostics")
                .theoretical_top_speed_at_max_rpm_kph
                .is_some_and(|speed| speed.is_finite() && speed > 0.0)
        );
    }

    #[test]
    fn v3_transitional_resolution_happens_in_the_simulator() {
        let snapshot = RacingCatalogSnapshot::embedded_model_v2().expect("catalog snapshot");
        let catalog = EmbeddedCatalog::from_snapshot(&snapshot).expect("resolved catalog");
        let (base_vehicle, _) = catalog.resolve_vehicle("f1_2026").expect("base vehicle");
        let tuning = Tuning {
            engine_points: 8,
            cooling_points: 5,
            aero_points: 4,
            chassis_points: 3,
            downforce_slider: 0.65,
            gear_ratio_slider: 0.35,
        };

        let simulator_resolved =
            resolve_v3_physical_vehicle(&base_vehicle, &tuning, &TuningResponseV1::default())
                .expect("Simulator V3 resolution");
        let historical_transform =
            apply_tuning_with_response(&base_vehicle, &tuning, &TuningResponseV1::default())
                .expect("historical compatibility transform");

        assert_eq!(simulator_resolved, historical_transform);
    }

    #[test]
    fn v3_aero_resolution_creates_a_nonlinear_setup_cost_and_development_efficiency() {
        let snapshot = RacingCatalogSnapshot::embedded_model_v2().expect("catalog snapshot");
        let catalog = EmbeddedCatalog::from_snapshot(&snapshot).expect("resolved catalog");
        let (base_vehicle, _) = catalog.resolve_vehicle("f1_2026").expect("base vehicle");
        let profile = V3CandidateExperimentProfile::default();
        let tuning = |downforce_slider, aero_points| Tuning {
            engine_points: 0,
            cooling_points: 0,
            aero_points,
            chassis_points: 0,
            downforce_slider,
            gear_ratio_slider: 0.5,
        };

        let low =
            resolve_v3_physical_vehicle_with_profile(&base_vehicle, &tuning(0.2, 0), &profile)
                .expect("low-downforce vehicle");
        let middle =
            resolve_v3_physical_vehicle_with_profile(&base_vehicle, &tuning(0.5, 0), &profile)
                .expect("middle-downforce vehicle");
        let high =
            resolve_v3_physical_vehicle_with_profile(&base_vehicle, &tuning(0.8, 0), &profile)
                .expect("high-downforce vehicle");

        assert!(low.aero.cl_a_x < middle.aero.cl_a_x);
        assert!(middle.aero.cl_a_x < high.aero.cl_a_x);
        assert!(low.aero.cd_a_x < middle.aero.cd_a_x);
        assert!(middle.aero.cd_a_x < high.aero.cd_a_x);
        assert!(
            high.aero.cd_a_x - middle.aero.cd_a_x > middle.aero.cd_a_x - low.aero.cd_a_x,
            "the induced-drag cost must grow quadratically with added downforce",
        );

        let developed =
            resolve_v3_physical_vehicle_with_profile(&base_vehicle, &tuning(0.5, 20), &profile)
                .expect("aerodynamically developed vehicle");
        assert!(developed.aero.cl_a_x > middle.aero.cl_a_x);
        assert!(
            developed.aero.cl_a_x / developed.aero.cd_a_x > middle.aero.cl_a_x / middle.aero.cd_a_x,
            "aero development must improve the resolved ClA/CdA ratio",
        );
    }

    #[test]
    fn v3_development_keeps_tire_friction_separate_from_chassis_efficiency() {
        let catalog = EmbeddedCatalog::load_default().expect("catalog");
        let (base_vehicle, _) = catalog.resolve_vehicle("f1_2026").expect("base vehicle");
        let profile = V3CandidateExperimentProfile::default();
        let tuning = |chassis_points, engine_points, cooling_points| Tuning {
            engine_points,
            cooling_points,
            aero_points: 0,
            chassis_points,
            downforce_slider: 0.5,
            gear_ratio_slider: 0.5,
        };

        let undeveloped_tuning = tuning(0, 0, 0);
        let developed_tuning = tuning(20, 20, 20);
        let undeveloped_vehicle =
            resolve_v3_physical_vehicle_with_profile(&base_vehicle, &undeveloped_tuning, &profile)
                .expect("undeveloped physical vehicle");
        let developed_vehicle =
            resolve_v3_physical_vehicle_with_profile(&base_vehicle, &developed_tuning, &profile)
                .expect("developed physical vehicle");

        assert_eq!(undeveloped_vehicle.chassis.mu0, base_vehicle.chassis.mu0);
        assert_eq!(developed_vehicle.chassis.mu0, base_vehicle.chassis.mu0);
        assert!(developed_vehicle.engine.trq[2] > undeveloped_vehicle.engine.trq[2]);
        assert!(developed_vehicle.engine.k_cool > undeveloped_vehicle.engine.k_cool);

        let undeveloped_mechanical =
            resolve_v3_mechanical_params(&undeveloped_vehicle, &undeveloped_tuning, &profile)
                .expect("undeveloped mechanical envelope");
        let developed_mechanical =
            resolve_v3_mechanical_params(&developed_vehicle, &developed_tuning, &profile)
                .expect("developed mechanical envelope");
        assert!(
            undeveloped_mechanical.chassis_force_transfer_efficiency
                < developed_mechanical.chassis_force_transfer_efficiency
        );
        assert_eq!(developed_mechanical.chassis_force_transfer_efficiency, 1.0);
    }

    #[test]
    fn v3_transmission_resolves_target_speed_without_changing_gear_spacing() {
        let catalog = EmbeddedCatalog::load_default().expect("catalog");
        let (vehicle, _) = catalog.resolve_vehicle("f1_2026").expect("vehicle");
        let parameters = V3TransmissionResolutionParams::default();

        let short = resolve_v3_transmission_gear_ratios(&vehicle, 0.0, &parameters)
            .expect("short final drive");
        let long = resolve_v3_transmission_gear_ratios(&vehicle, 1.0, &parameters)
            .expect("long final drive");

        assert!(short.last().expect("short top gear") > long.last().expect("long top gear"));
        for index in 0..(vehicle.engine.gear_ratios.len() - 1) {
            let source_spacing =
                vehicle.engine.gear_ratios[index] / vehicle.engine.gear_ratios[index + 1];
            assert!((short[index] / short[index + 1] - source_spacing).abs() < 1e-12);
            assert!((long[index] / long[index + 1] - source_spacing).abs() < 1e-12);
        }

        for (ratios, target_speed_mps) in [
            (&short, parameters.minimum_target_top_speed_mps),
            (&long, parameters.maximum_target_top_speed_mps),
        ] {
            let rpm = rpm_from_speed_gear(
                target_speed_mps,
                *ratios.last().expect("resolved top gear"),
                &vehicle.chassis,
            );
            assert!(
                (rpm - parameters.target_engine_speed_fraction * vehicle.engine.n_max).abs() < 1e-9
            );
        }
    }

    #[test]
    fn v3_transmission_parameters_fail_closed() {
        for parameters in [
            V3TransmissionResolutionParams {
                minimum_target_top_speed_mps: f64::NAN,
                ..V3TransmissionResolutionParams::default()
            },
            V3TransmissionResolutionParams {
                minimum_target_top_speed_mps: 105.0,
                maximum_target_top_speed_mps: 85.0,
                ..V3TransmissionResolutionParams::default()
            },
            V3TransmissionResolutionParams {
                target_engine_speed_fraction: 1.01,
                ..V3TransmissionResolutionParams::default()
            },
        ] {
            assert!(parameters.validate().is_err());
        }
    }

    #[test]
    fn v3_fidelity_profile_preserves_drag_without_inventing_legacy_downforce() {
        let catalog = EmbeddedCatalog::load_default().expect("catalog");
        let (vehicle, _) = catalog
            .resolve_vehicle("classic_v8_1960")
            .expect("legacy vehicle");
        assert!(vehicle.aero.cd_a_x > 0.0);
        assert_eq!(vehicle.aero.cl_a_x, 0.0);

        let profile = V3CandidateExperimentProfile::default();
        let low = resolve_v3_physical_vehicle_with_profile(
            &vehicle,
            &Tuning {
                downforce_slider: 0.0,
                ..Tuning::default()
            },
            &profile,
        )
        .expect("low-downforce legacy vehicle");
        let high = resolve_v3_physical_vehicle_with_profile(
            &vehicle,
            &Tuning {
                downforce_slider: 1.0,
                ..Tuning::default()
            },
            &profile,
        )
        .expect("high-downforce legacy vehicle");

        assert!(low.aero.cd_a_x > 0.0, "body drag must remain physical");
        assert_eq!(low.aero.cl_a_x, 0.0, "the resolver must not invent aero");
        assert_eq!(low.aero, high.aero, "the downforce setup must be inert");

        let historical_profile = V3CandidateExperimentProfile {
            schema_version: V3CandidateExperimentProfileVersion::V4,
            fuel_mass: None,
            tire_degradation: None,
            ..profile
        };
        assert!(
            resolve_v3_physical_vehicle_with_profile(
                &vehicle,
                &Tuning::default(),
                &historical_profile,
            )
            .is_err(),
            "candidate 0.6 semantics must remain immutable"
        );
    }

    #[test]
    fn v3_fidelity_profile_applies_the_declared_first_stint_tire() {
        fn request_with_tire(tire_id: &str) -> RunRaceRequest {
            let mut request = one_lap_request();
            request.input.race.laps = 6;
            request.input.race.competitors[0].stint_strategy = Some(CompetitorStintStrategy {
                stints: vec![pitgun_racing_contract::RaceStint {
                    tire_id: tire_id.to_string(),
                    laps: 6,
                }],
                pit_laps: Vec::new(),
            });
            request
        }

        let snapshot = RacingCatalogSnapshot::embedded().expect("catalog");
        let profile = V3CandidateExperimentProfile::default();
        let soft =
            run_race_with_catalog_and_v3_profile(request_with_tire("soft"), &snapshot, &profile)
                .expect("soft first stint");
        let hard =
            run_race_with_catalog_and_v3_profile(request_with_tire("hard"), &snapshot, &profile)
                .expect("hard first stint");
        assert_ne!(soft.total_time_ms, hard.total_time_ms);
        assert_ne!(
            soft.player_tire_diagnostics_v3,
            hard.player_tire_diagnostics_v3
        );

        let historical_profile = V3CandidateExperimentProfile {
            schema_version: V3CandidateExperimentProfileVersion::V4,
            fuel_mass: None,
            tire_degradation: None,
            ..profile
        };
        let historical_soft = run_race_with_catalog_and_v3_profile(
            request_with_tire("soft"),
            &snapshot,
            &historical_profile,
        )
        .expect("historical soft declaration");
        let historical_hard = run_race_with_catalog_and_v3_profile(
            request_with_tire("hard"),
            &snapshot,
            &historical_profile,
        )
        .expect("historical hard declaration");
        assert_eq!(historical_soft.total_time_ms, historical_hard.total_time_ms);
        assert_eq!(
            historical_soft.player_lap_times_ms,
            historical_hard.player_lap_times_ms
        );
        assert_eq!(
            historical_soft.player_tire_diagnostics_v3, historical_hard.player_tire_diagnostics_v3,
            "candidate 0.6 must retain its historical first-stint physics"
        );
    }

    #[test]
    fn v3_fuel_mass_profile_exposes_power_based_mass_lineage() {
        fn request_with_fuel(fuel_mass_kg: f64) -> RunRaceRequest {
            let mut request = one_lap_request();
            request.input.race.laps = 8;
            request.input.initial_fuel_mass_kg = Some(fuel_mass_kg);
            request
        }

        let snapshot = RacingCatalogSnapshot::embedded().expect("catalog");
        let profile = V3CandidateExperimentProfile::default();
        let light =
            run_race_with_catalog_and_v3_profile(request_with_fuel(40.0), &snapshot, &profile)
                .expect("light fuel load");
        let heavy =
            run_race_with_catalog_and_v3_profile(request_with_fuel(100.0), &snapshot, &profile)
                .expect("heavy fuel load");

        let diagnostics = light
            .player_fuel_mass_diagnostics_v3
            .expect("fuel diagnostics");
        assert_eq!(diagnostics.initial_fuel_mass_kg, 40.0);
        assert_eq!(diagnostics.fuel_mass_after_lap_kg.len(), 8);
        assert!(diagnostics.final_fuel_mass_kg >= 0.0);
        assert!(diagnostics.final_fuel_mass_kg < diagnostics.initial_fuel_mass_kg);
        assert!(
            diagnostics
                .fuel_mass_after_lap_kg
                .windows(2)
                .all(|window| window[1] <= window[0])
        );
        let expected_consumption = diagnostics.engine_output_work_kj / 3_600.0
            * profile
                .fuel_mass
                .expect("fuel parameters")
                .brake_specific_fuel_consumption_kg_per_kwh
            + profile
                .fuel_mass
                .expect("fuel parameters")
                .idle_fuel_flow_kg_per_s
                * (light.total_time_ms as f64 / 1_000.0);
        approx_eq(
            diagnostics.fuel_consumed_kg,
            expected_consumption,
            0.001,
            "power-based fuel consumption",
        );
        assert!(light.total_time_ms < heavy.total_time_ms);
        assert!(
            diagnostics.maximum_total_vehicle_mass_kg
                < heavy
                    .player_fuel_mass_diagnostics_v3
                    .expect("heavy fuel diagnostics")
                    .maximum_total_vehicle_mass_kg
        );

        let error =
            run_race_with_catalog_and_v3_profile(request_with_fuel(0.0), &snapshot, &profile)
                .expect_err("an empty fuel load must fail closed");
        assert!(error.contains("initial fuel mass"));
    }

    #[test]
    fn v3_tire_degradation_profile_uses_compound_parameters_and_preserves_v6() {
        fn request_with_tire(tire_id: &str) -> RunRaceRequest {
            let mut request = one_lap_request();
            request.input.race.laps = 12;
            request.input.initial_fuel_mass_kg = Some(100.0);
            request.input.race.competitors[0].stint_strategy = Some(CompetitorStintStrategy {
                stints: vec![pitgun_racing_contract::RaceStint {
                    tire_id: tire_id.to_string(),
                    laps: 12,
                }],
                pit_laps: Vec::new(),
            });
            request
        }

        let snapshot = RacingCatalogSnapshot::embedded().expect("catalog");
        let profile = V3CandidateExperimentProfile::default();
        let soft =
            run_race_with_catalog_and_v3_profile(request_with_tire("soft"), &snapshot, &profile)
                .expect("soft degradation run");
        let hard =
            run_race_with_catalog_and_v3_profile(request_with_tire("hard"), &snapshot, &profile)
                .expect("hard degradation run");
        let soft_diagnostics = soft
            .player_tire_degradation_diagnostics_v3
            .expect("soft degradation diagnostics");
        let hard_diagnostics = hard
            .player_tire_degradation_diagnostics_v3
            .expect("hard degradation diagnostics");

        assert_eq!(soft_diagnostics.wear_before_service_after_lap.len(), 12);
        assert_eq!(hard_diagnostics.wear_before_service_after_lap.len(), 12);
        assert!(
            soft_diagnostics.requested_baseline_wear_fraction
                > hard_diagnostics.requested_baseline_wear_fraction
        );
        assert!(
            soft_diagnostics
                .wear_before_service_after_lap
                .last()
                .expect("soft final wear")
                > hard_diagnostics
                    .wear_before_service_after_lap
                    .last()
                    .expect("hard final wear")
        );

        let historical_profile = V3CandidateExperimentProfile {
            schema_version: V3CandidateExperimentProfileVersion::V6,
            tire_degradation: None,
            ..profile
        };
        let historical = run_race_with_catalog_and_v3_profile(
            request_with_tire("soft"),
            &snapshot,
            &historical_profile,
        )
        .expect("historical V6 tire run");
        assert!(historical.player_tire_degradation_diagnostics_v3.is_none());
        assert_eq!(
            historical_profile.model_identity(),
            racing_model_v3_fuel_mass_candidate_identity()
        );
    }

    #[test]
    fn v3_profile_versions_bind_exact_candidate_semantics() {
        let mechanical_profile = V3CandidateExperimentProfile {
            schema_version: V3CandidateExperimentProfileVersion::V1,
            aero_resolution: None,
            development_resolution: None,
            transmission_resolution: None,
            fuel_mass: None,
            tire_degradation: None,
            ..V3CandidateExperimentProfile::default()
        };
        assert_eq!(
            mechanical_profile.model_identity(),
            racing_model_v3_mechanical_candidate_identity(),
        );
        assert!(mechanical_profile.validate().is_ok());

        let aero_profile = V3CandidateExperimentProfile {
            schema_version: V3CandidateExperimentProfileVersion::V2,
            development_resolution: None,
            transmission_resolution: None,
            fuel_mass: None,
            tire_degradation: None,
            ..V3CandidateExperimentProfile::default()
        };
        assert_eq!(
            aero_profile.model_identity(),
            racing_model_v3_aero_candidate_identity()
        );
        assert!(aero_profile.validate().is_ok());

        let development_profile = V3CandidateExperimentProfile {
            schema_version: V3CandidateExperimentProfileVersion::V3,
            transmission_resolution: None,
            fuel_mass: None,
            tire_degradation: None,
            ..V3CandidateExperimentProfile::default()
        };
        assert_eq!(
            development_profile.model_identity(),
            racing_model_v3_development_candidate_identity()
        );
        assert!(development_profile.validate().is_ok());

        let transmission_profile = V3CandidateExperimentProfile {
            schema_version: V3CandidateExperimentProfileVersion::V4,
            fuel_mass: None,
            tire_degradation: None,
            ..V3CandidateExperimentProfile::default()
        };
        assert_eq!(
            transmission_profile.model_identity(),
            racing_model_v3_transmission_candidate_identity()
        );
        assert!(transmission_profile.validate().is_ok());

        let fidelity_profile = V3CandidateExperimentProfile {
            schema_version: V3CandidateExperimentProfileVersion::V5,
            fuel_mass: None,
            tire_degradation: None,
            ..V3CandidateExperimentProfile::default()
        };
        assert_eq!(
            fidelity_profile.model_identity(),
            racing_model_v3_fidelity_candidate_identity()
        );
        assert!(fidelity_profile.validate().is_ok());

        let fuel_mass_profile = V3CandidateExperimentProfile {
            schema_version: V3CandidateExperimentProfileVersion::V6,
            tire_degradation: None,
            ..V3CandidateExperimentProfile::default()
        };
        assert_eq!(
            fuel_mass_profile.model_identity(),
            racing_model_v3_fuel_mass_candidate_identity()
        );
        assert!(fuel_mass_profile.validate().is_ok());

        let tire_degradation_profile = V3CandidateExperimentProfile::default();
        assert_eq!(
            tire_degradation_profile.model_identity(),
            racing_model_v3_candidate_identity()
        );
        assert!(tire_degradation_profile.validate().is_ok());

        let thermal_profile = V3CandidateExperimentProfile {
            schema_version: V3CandidateExperimentProfileVersion::V8,
            engine_thermal_resolution: Some(V3EngineThermalResolutionParams::default()),
            ..V3CandidateExperimentProfile::default()
        };
        assert_eq!(
            thermal_profile.model_identity(),
            racing_model_v3_thermal_candidate_identity()
        );
        assert!(thermal_profile.validate().is_ok());

        let snapshot = RacingCatalogSnapshot::embedded().expect("catalog");
        let baseline = run_race_with_catalog_and_v3_profile(
            one_lap_request(),
            &snapshot,
            &tire_degradation_profile,
        )
        .expect("0.9 baseline");
        let explicit =
            run_race_with_catalog_and_v3_profile(one_lap_request(), &snapshot, &thermal_profile)
                .expect("0.10 identity thermal profile");
        assert_eq!(explicit.total_time_ms, baseline.total_time_ms);
        assert_eq!(
            explicit.player_mechanical_diagnostics_v3, baseline.player_mechanical_diagnostics_v3,
            "identity-valued V8 thermal parameters must reproduce candidate 0.9"
        );

        let embedded = EmbeddedCatalog::from_snapshot(&snapshot).expect("resolved catalog");
        let (vehicle, _) = embedded.resolve_vehicle("f1_2026").expect("vehicle");
        let tuning = Tuning {
            cooling_points: 20,
            ..Tuning::default()
        };
        let identity_vehicle =
            resolve_v3_physical_vehicle_with_profile(&vehicle, &tuning, &thermal_profile)
                .expect("identity thermal vehicle");
        let mut costly_profile = thermal_profile;
        costly_profile
            .engine_thermal_resolution
            .as_mut()
            .expect("thermal parameters")
            .cooling_drag_area_m2_at_cap = 0.10;
        let costly_vehicle =
            resolve_v3_physical_vehicle_with_profile(&vehicle, &tuning, &costly_profile)
                .expect("cooling-cost vehicle");
        approx_eq(
            costly_vehicle.aero.cd_a_x - identity_vehicle.aero.cd_a_x,
            0.10,
            1e-12,
            "cooling drag at cap",
        );

        let mut malformed_thermal = thermal_profile;
        malformed_thermal
            .engine_thermal_resolution
            .as_mut()
            .expect("thermal parameters")
            .thermal_capacity_multiplier = 0.0;
        assert!(malformed_thermal.validate().is_err());

        let malformed = V3CandidateExperimentProfile {
            schema_version: V3CandidateExperimentProfileVersion::V2,
            aero_resolution: None,
            development_resolution: None,
            fuel_mass: None,
            tire_degradation: None,
            ..V3CandidateExperimentProfile::default()
        };
        assert!(malformed.validate().is_err());
    }

    #[test]
    fn catalog_detail_views_use_the_selected_snapshot() {
        let snapshot = RacingCatalogSnapshot::embedded().expect("catalog");
        let circuit = get_circuit_with_catalog(&snapshot, "MONZA").expect("circuit");
        let engine = get_engine_with_catalog(&snapshot, "v6t_hybrid").expect("engine");

        assert_eq!(circuit.id, "MONZA");
        assert!(circuit.s_m.len() > 100);
        assert_eq!(engine.id, "v6t_hybrid");
        assert!(!engine.rpm_samples.is_empty());
    }

    #[test]
    fn driver_traits_and_modes_resolve_to_explainable_physical_controls() {
        let profile = driver_control_profile_v10()
            .driver_control_profile
            .expect("driver-control profile");
        let driver = driver_resource(0.70, 0.55);

        let manage = resolve_v3_driver_control_v1(&driver, RacingDrivingMode::Manage, &profile)
            .expect("manage resolution");
        let attack = resolve_v3_driver_control_v1(&driver, RacingDrivingMode::Attack, &profile)
            .expect("attack resolution");

        assert!(attack.requested_commitment > manage.requested_commitment);
        assert!(attack.cornering_utilization > manage.cornering_utilization);
        assert!(attack.braking_utilization > manage.braking_utilization);
        assert!(attack.traction_utilization > manage.traction_utilization);
        assert!(attack.control_error_amplitude > manage.control_error_amplitude);
        assert!(
            attack.correction_workload_multiplier > manage.correction_workload_multiplier,
            "attack must pay a physical correction-workload cost"
        );

        let consistent = driver_resource(0.95, 0.55);
        let consistent_attack =
            resolve_v3_driver_control_v1(&consistent, RacingDrivingMode::Attack, &profile)
                .expect("consistent attack resolution");
        assert_eq!(
            consistent_attack.cornering_utilization, attack.cornering_utilization,
            "consistency changes error, not the requested force envelope"
        );
        assert!(consistent_attack.control_error_amplitude < attack.control_error_amplitude);

        let tire_manager = driver_resource(0.70, 0.95);
        let managed_attack =
            resolve_v3_driver_control_v1(&tire_manager, RacingDrivingMode::Attack, &profile)
                .expect("tire-manager attack resolution");
        assert_eq!(
            managed_attack.control_error_amplitude,
            attack.control_error_amplitude
        );
        assert!(
            managed_attack.correction_workload_multiplier < attack.correction_workload_multiplier
        );
    }

    #[test]
    fn driver_control_candidate_is_offline_deterministic_and_emits_lineage() {
        let snapshot = RacingCatalogSnapshot::embedded_model_v3_component()
            .expect("component-composed catalog");
        let thermal = snapshot
            .power_unit_thermal_profile()
            .expect("power-unit thermal profile");
        let profile = driver_control_profile_v10();
        let experiment = V3DriverControlExperimentV1 {
            drivers: BTreeMap::from([("default".to_string(), driver_resource(0.70, 0.55))]),
            competitor_modes: BTreeMap::from([("player".to_string(), RacingDrivingMode::Attack)]),
        };

        assert_eq!(
            profile.model_identity(),
            racing_model_v3_driver_control_candidate_identity()
        );
        assert!(
            racing_model_identity_for_version("0.12.0").is_err(),
            "the screening candidate must not be selectable by hosted verification"
        );

        let first = run_race_with_catalog_and_v3_driver_control_profile(
            one_lap_request(),
            &snapshot,
            &profile,
            thermal,
            &experiment,
        )
        .expect("first driver-control candidate run");
        let second = run_race_with_catalog_and_v3_driver_control_profile(
            one_lap_request(),
            &snapshot,
            &profile,
            thermal,
            &experiment,
        )
        .expect("second driver-control candidate run");

        assert_eq!(
            serde_json::to_value(&first).expect("first JSON"),
            serde_json::to_value(&second).expect("second JSON"),
            "identical driver-control inputs must be bit-for-bit deterministic"
        );
        let resolution = first
            .competitor_driver_control_resolutions_v3
            .get("player")
            .expect("driver-control lineage");
        assert_eq!(resolution.driver_id, "default");
        assert_eq!(resolution.driving_mode, RacingDrivingMode::Attack);
        assert!(resolution.correction_workload_multiplier > 1.0);
        let realized = first
            .competitor_driver_control_diagnostics_v3
            .get("player")
            .expect("realized driver-control diagnostics");
        assert_eq!(
            realized.correction_force_capacity_fraction, None,
            "candidate 0.12 must retain its reviewed diagnostics"
        );
        assert_eq!(
            realized.cornering.requested_limit,
            resolution.cornering_utilization
        );
        assert!(realized.cornering.minimum_realized < realized.cornering.maximum_realized);
        assert!(
            realized.cornering.mean_realized <= realized.cornering.requested_limit,
            "deterministic error may only reduce requested utilization"
        );
        let tire = first.player_tire_diagnostics_v3.expect("tire diagnostics");
        assert!(
            tire.correction_contact_workload_mj
                .expect("correction workload")
                > 0.0
        );
        assert!(tire.correction_generated_heat_kj.expect("correction heat") > 0.0);
        assert!(
            first
                .player_tire_degradation_diagnostics_v3
                .expect("wear diagnostics")
                .requested_correction_wear_fraction
                .expect("correction wear")
                > 0.0
        );

        let missing_mode = V3DriverControlExperimentV1 {
            drivers: experiment.drivers.clone(),
            competitor_modes: BTreeMap::new(),
        };
        assert!(
            run_race_with_catalog_and_v3_driver_control_profile(
                one_lap_request(),
                &snapshot,
                &profile,
                thermal,
                &missing_mode,
            )
            .expect_err("missing mode must fail closed")
            .contains("missing driving mode")
        );
    }

    #[test]
    fn predeclared_driver_instruction_timeline_is_deterministic_and_physical() {
        let snapshot = RacingCatalogSnapshot::embedded_model_v3_component()
            .expect("component-composed catalog");
        let thermal = snapshot
            .power_unit_thermal_profile()
            .expect("power-unit thermal profile");
        let profile = driver_friction_profile_v11();
        let mut request = one_lap_request();
        request.input.race.laps = 5;
        request.input.race.competitors[0].stint_strategy = Some(CompetitorStintStrategy {
            stints: vec![
                RaceStint {
                    tire_id: "soft".to_string(),
                    laps: 3,
                },
                RaceStint {
                    tire_id: "hard".to_string(),
                    laps: 2,
                },
            ],
            pit_laps: vec![3],
        });
        let mut ai = request.input.race.competitors[0].clone();
        ai.id = "ai-01".to_string();
        ai.name = "AI 01".to_string();
        ai.team_id = "ai-team".to_string();
        ai.is_player = false;
        request.input.race.competitors.push(ai);
        let instruction_profile = RacingDriverInstructionProfileV1 {
            schema_version: RacingDriverInstructionProfileVersion::V1,
            default_mode: RacingDrivingMode::Balanced,
            boundary_granularity: RacingDriverInstructionBoundaryGranularityV1::LapStart,
            max_events_per_session: 8,
        };
        let experiment = V3DriverInstructionExperimentV1 {
            drivers: BTreeMap::from([("default".to_string(), driver_resource(0.70, 0.55))]),
            instruction_profile,
            timeline: RacingDriverInstructionTimelineV1 {
                schema_version: RacingDriverInstructionTimelineVersion::V1,
                events: vec![
                    RacingDriverInstructionEventV1 {
                        sequence: 0,
                        competitor_id: "ai-01".to_string(),
                        effective_at: RacingDriverInstructionBoundaryV1 {
                            lap_index: 1,
                            segment_index: 0,
                        },
                        mode: RacingDrivingMode::Manage,
                    },
                    RacingDriverInstructionEventV1 {
                        sequence: 1,
                        competitor_id: "player".to_string(),
                        effective_at: RacingDriverInstructionBoundaryV1 {
                            lap_index: 1,
                            segment_index: 0,
                        },
                        mode: RacingDrivingMode::Attack,
                    },
                    RacingDriverInstructionEventV1 {
                        sequence: 2,
                        competitor_id: "player".to_string(),
                        effective_at: RacingDriverInstructionBoundaryV1 {
                            lap_index: 3,
                            segment_index: 0,
                        },
                        mode: RacingDrivingMode::Manage,
                    },
                ],
            },
        };

        let first = run_race_with_catalog_and_v3_driver_instruction_profile(
            request.clone(),
            &snapshot,
            &profile,
            thermal,
            &experiment,
        )
        .expect("first scheduled driver run");
        let second = run_race_with_catalog_and_v3_driver_instruction_profile(
            request.clone(),
            &snapshot,
            &profile,
            thermal,
            &experiment,
        )
        .expect("second scheduled driver run");
        assert_eq!(
            serde_json::to_value(&first).expect("first scheduled JSON"),
            serde_json::to_value(&second).expect("second scheduled JSON")
        );

        let lineage = first
            .competitor_driver_instruction_schedules_v3
            .get("player")
            .expect("player instruction lineage");
        assert_eq!(lineage.transitions.len(), 3);
        assert_eq!(lineage.transitions[0].sequence, None);
        assert_eq!(lineage.transitions[0].mode, RacingDrivingMode::Balanced);
        assert_eq!(lineage.transitions[1].sequence, Some(1));
        assert_eq!(lineage.transitions[1].mode, RacingDrivingMode::Attack);
        assert_eq!(lineage.transitions[2].sequence, Some(2));
        assert_eq!(lineage.transitions[2].mode, RacingDrivingMode::Manage);
        let ai_lineage = first
            .competitor_driver_instruction_schedules_v3
            .get("ai-01")
            .expect("AI instruction lineage");
        assert_eq!(ai_lineage.transitions.len(), 2);
        assert_eq!(ai_lineage.transitions[0].mode, RacingDrivingMode::Balanced);
        assert_eq!(ai_lineage.transitions[1].mode, RacingDrivingMode::Manage);

        let empty = V3DriverInstructionExperimentV1 {
            drivers: experiment.drivers.clone(),
            instruction_profile,
            timeline: RacingDriverInstructionTimelineV1 {
                schema_version: RacingDriverInstructionTimelineVersion::V1,
                events: Vec::new(),
            },
        };
        let common_default = run_race_with_catalog_and_v3_driver_instruction_profile(
            request.clone(),
            &snapshot,
            &profile,
            thermal,
            &empty,
        )
        .expect("common-default run");
        let static_balanced = run_race_with_catalog_and_v3_driver_control_profile(
            request,
            &snapshot,
            &profile,
            thermal,
            &V3DriverControlExperimentV1 {
                drivers: experiment.drivers.clone(),
                competitor_modes: BTreeMap::from([
                    ("ai-01".to_string(), RacingDrivingMode::Balanced),
                    ("player".to_string(), RacingDrivingMode::Balanced),
                ]),
            },
        )
        .expect("static balanced run");
        assert_eq!(
            serde_json::to_value(&common_default.standings).expect("default standings JSON"),
            serde_json::to_value(&static_balanced.standings).expect("static standings JSON")
        );
        assert_eq!(
            common_default.player_lap_times_ms,
            static_balanced.player_lap_times_ms
        );
        assert_eq!(
            serde_json::to_value(&common_default.player_batches).expect("default telemetry JSON"),
            serde_json::to_value(&static_balanced.player_batches).expect("static telemetry JSON")
        );
        assert_ne!(
            first.player_lap_times_ms,
            common_default.player_lap_times_ms
        );

        let invalid = V3DriverInstructionExperimentV1 {
            drivers: experiment.drivers,
            instruction_profile,
            timeline: RacingDriverInstructionTimelineV1 {
                schema_version: RacingDriverInstructionTimelineVersion::V1,
                events: vec![RacingDriverInstructionEventV1 {
                    sequence: 0,
                    competitor_id: "ghost".to_string(),
                    effective_at: RacingDriverInstructionBoundaryV1 {
                        lap_index: 1,
                        segment_index: 0,
                    },
                    mode: RacingDrivingMode::Attack,
                }],
            },
        };
        assert!(
            run_race_with_catalog_and_v3_driver_instruction_profile(
                one_lap_request(),
                &snapshot,
                &profile,
                thermal,
                &invalid,
            )
            .expect_err("unknown instruction competitor must fail closed")
            .contains("targets unknown competitor")
        );
    }

    #[test]
    fn driver_friction_candidate_is_offline_deterministic_and_reserves_force_capacity() {
        let snapshot = RacingCatalogSnapshot::embedded_model_v3_component()
            .expect("component-composed catalog");
        let thermal = snapshot
            .power_unit_thermal_profile()
            .expect("power-unit thermal profile");
        let profile = driver_friction_profile_v11();
        let experiment = V3DriverControlExperimentV1 {
            drivers: BTreeMap::from([("default".to_string(), driver_resource(0.70, 0.55))]),
            competitor_modes: BTreeMap::from([("player".to_string(), RacingDrivingMode::Attack)]),
        };

        assert_eq!(
            profile.model_identity(),
            racing_model_v3_driver_friction_candidate_identity()
        );
        assert!(
            racing_model_identity_for_version("0.13.0").is_err(),
            "the friction-budget candidate must not be selectable by hosted verification"
        );

        let first = run_race_with_catalog_and_v3_driver_control_profile(
            one_lap_request(),
            &snapshot,
            &profile,
            thermal,
            &experiment,
        )
        .expect("first driver-friction candidate run");
        let second = run_race_with_catalog_and_v3_driver_control_profile(
            one_lap_request(),
            &snapshot,
            &profile,
            thermal,
            &experiment,
        )
        .expect("second driver-friction candidate run");

        assert_eq!(
            serde_json::to_value(&first).expect("first JSON"),
            serde_json::to_value(&second).expect("second JSON"),
            "identical driver-friction inputs must be bit-for-bit deterministic"
        );
        let realized = first
            .competitor_driver_control_diagnostics_v3
            .get("player")
            .expect("realized driver-control diagnostics");
        let capacity = realized
            .correction_force_capacity_fraction
            .expect("friction-budget capacity fraction");
        assert!(
            0.0 < capacity && capacity < 1.0,
            "correction workload must reserve a bounded part of tire-force capacity"
        );
        let resolution = first
            .competitor_driver_control_resolutions_v3
            .get("player")
            .expect("driver-control lineage");
        approx_eq(
            capacity,
            1.0 / resolution.correction_workload_multiplier.sqrt(),
            1e-12,
            "correction force capacity",
        );
        assert!(realized.cornering.requested_limit < resolution.cornering_utilization);
        assert!(realized.braking.requested_limit < resolution.braking_utilization);
        assert!(realized.traction.requested_limit < resolution.traction_utilization);
    }
}
