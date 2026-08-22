//! Transitional compatibility and WASM facade for the Racing Simulator.
//!
//! New Rust consumers should depend on `pitgun-racing-simulator` for race and
//! session orchestration, or `pitgun-racing-solver` for physical solving.

pub mod rng;

pub use pitgun_racing_simulator::evidence;
pub use pitgun_racing_simulator::evidence::{
    RacingHostedExecutionRequestV1, RacingHostedExecutionRequestVersion,
    RacingVerificationSubmissionV1,
};
pub use pitgun_racing_simulator::{
    AeroParams, BrowserCircuitCatalogEntry, CatalogSnapshot, ChassisParams, CircuitDetail, Driver,
    DriverCatalogEntry, DriverEffects, EngineDetail, EngineParams, PitPlan, PitStop,
    PitStrategyConfig, RaceOutput, RacingCatalogBundleV1, RacingCatalogFileV1,
    RacingCatalogResolutionError, RacingCatalogSnapshot, RacingWorkload, RacingWorkloadError,
    ResampledTelemetry, ResolveVehicleCapabilitiesRequestV1, RunRaceInput, RunRaceRequest,
    RunSimulationRequest, SessionConfig, SessionRunOutput, SessionRunRequest, SessionRunResult,
    SimConfig, SimulationRequest, SimulationResult, SimulationSolution, SolverTrackProfile,
    StandingEntry, StandingStatus, TelemetryEnvelope, TireCatalogEntry, TireParams, Track, Tuning,
    V3_POWER_UNIT_THERMAL_PROFILE_DIGEST, V3_THERMAL_FAMILY_PROFILE_DIGEST,
    V3PowerUnitThermalProfileCandidateV2, V3PowerUnitThermalResolutionV2,
    V3ThermalFamilyProfileCandidateV1, V3ThermalFamilyResolutionV1, VehicleCatalogEntry,
    VehicleParams, VehicleState, apply_driver_to_tire, apply_tuning, best_power_at_speed,
    catalog_snapshot, catalog_snapshot_with_catalog, derating_factor, driver_effects, effective_mu,
    execute_authorized_race, get_circuit, get_circuit_with_catalog, get_engine,
    get_engine_with_catalog, list_browser_circuits, list_circuits, list_drivers, list_engines,
    list_tires, list_vehicles, power_kw_from_rpm, racing_model_v1_identity,
    racing_model_v2_identity, resample_solution, resolve_vehicle_capabilities_with_catalog,
    rpm_from_speed_gear, run_race, run_race_with_catalog,
    run_race_with_catalog_and_v3_power_unit_thermal_profile,
    run_race_with_catalog_and_v3_thermal_family_profile, run_sessions, run_sessions_with_catalog,
    solve,
};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(typescript_custom_section)]
const HOSTED_RACING_TYPES: &'static str = r#"
export type PitgunSha256Digest = `sha256:${string}`;

export interface RacingHostedExecutionRequestV1 {
  schema_version: "pitgun.racing-hosted-execution/v1";
  signed_authorization: unknown;
  input: unknown;
  execution_id: string;
  wasm_artifact_digest: PitgunSha256Digest;
}

export interface RacingVerificationSubmissionV1 {
  signed_authorization: unknown;
  input: unknown;
  receipt: unknown;
  output: unknown;
  telemetry_summary: unknown;
  execution_resolution?: {
    schema_version: "pitgun.racing-execution-resolution/v1" | "pitgun.racing-execution-resolution/v2" | "pitgun.racing-execution-resolution/v3";
    catalog_release: unknown;
    simulation_pack: unknown;
    model: unknown;
    model_parameters?: unknown;
    thermal_family_profile?: unknown;
    power_unit_thermal_profile?: unknown;
    component_capability_profile?: unknown;
  };
}
"#;

#[wasm_bindgen]
pub fn run_simulation_json(input_json: String) -> String {
    pitgun_racing_simulator::run_simulation_json(input_json)
}

#[wasm_bindgen]
pub fn run_race_json(input_json: String) -> String {
    pitgun_racing_simulator::run_race_json(input_json)
}

#[wasm_bindgen]
pub fn run_race_with_catalog_json(input_json: String, catalog_bundle_json: String) -> String {
    pitgun_racing_simulator::run_race_with_catalog_json(input_json, catalog_bundle_json)
}

#[wasm_bindgen]
pub fn resolve_vehicle_capabilities_json_from_bundle(
    request_json: String,
    catalog_bundle_json: String,
) -> String {
    pitgun_racing_simulator::resolve_vehicle_capabilities_json_from_bundle(
        request_json,
        catalog_bundle_json,
    )
}

#[wasm_bindgen]
pub fn run_race_with_catalog_and_v3_thermal_family_profile_json(
    input_json: String,
    catalog_bundle_json: String,
    candidate_json: String,
) -> String {
    pitgun_racing_simulator::run_race_with_catalog_and_v3_thermal_family_profile_json(
        input_json,
        catalog_bundle_json,
        candidate_json,
    )
}

#[wasm_bindgen]
pub fn run_race_with_catalog_and_v3_power_unit_thermal_profile_json(
    input_json: String,
    catalog_bundle_json: String,
    candidate_json: String,
) -> String {
    pitgun_racing_simulator::run_race_with_catalog_and_v3_power_unit_thermal_profile_json(
        input_json,
        catalog_bundle_json,
        candidate_json,
    )
}

#[wasm_bindgen]
pub fn execute_authorized_race_json(request_json: String) -> String {
    pitgun_racing_simulator::execute_authorized_race_json(request_json)
}

#[wasm_bindgen]
pub fn execute_authorized_race_with_catalog_json(
    request_json: String,
    catalog_bundle_json: String,
) -> String {
    pitgun_racing_simulator::execute_authorized_race_with_catalog_json(
        request_json,
        catalog_bundle_json,
    )
}

#[wasm_bindgen]
pub fn run_sessions_json(input_json: String) -> String {
    pitgun_racing_simulator::run_sessions_json(input_json)
}

#[wasm_bindgen]
pub fn run_sessions_with_catalog_json(input_json: String, catalog_bundle_json: String) -> String {
    pitgun_racing_simulator::run_sessions_with_catalog_json(input_json, catalog_bundle_json)
}

#[wasm_bindgen]
pub fn solve_baseline_json(input_json: String) -> String {
    pitgun_racing_simulator::solve_baseline_json(input_json)
}

#[wasm_bindgen]
pub fn catalog_json() -> String {
    pitgun_racing_simulator::catalog_json()
}

#[wasm_bindgen]
pub fn catalog_json_from_bundle(catalog_bundle_json: String) -> String {
    pitgun_racing_simulator::catalog_json_from_bundle(catalog_bundle_json)
}

#[wasm_bindgen]
pub fn list_circuits_json() -> String {
    pitgun_racing_simulator::list_circuits_json()
}

#[wasm_bindgen]
pub fn get_circuit_json(track_id: String) -> String {
    pitgun_racing_simulator::get_circuit_json(track_id)
}

#[wasm_bindgen]
pub fn get_circuit_json_from_bundle(track_id: String, catalog_bundle_json: String) -> String {
    pitgun_racing_simulator::get_circuit_json_from_bundle(track_id, catalog_bundle_json)
}

#[wasm_bindgen]
pub fn list_engines_json() -> String {
    pitgun_racing_simulator::list_engines_json()
}

#[wasm_bindgen]
pub fn get_engine_json(engine_id: String) -> String {
    pitgun_racing_simulator::get_engine_json(engine_id)
}

#[wasm_bindgen]
pub fn get_engine_json_from_bundle(engine_id: String, catalog_bundle_json: String) -> String {
    pitgun_racing_simulator::get_engine_json_from_bundle(engine_id, catalog_bundle_json)
}

#[wasm_bindgen]
pub fn list_drivers_json() -> String {
    pitgun_racing_simulator::list_drivers_json()
}

#[wasm_bindgen]
pub fn list_vehicles_json() -> String {
    pitgun_racing_simulator::list_vehicles_json()
}

#[wasm_bindgen]
pub fn list_tires_json() -> String {
    pitgun_racing_simulator::list_tires_json()
}
