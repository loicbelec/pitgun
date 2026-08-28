//! Transitional compatibility and WASM facade for the Racing Simulator.
//!
//! New Rust consumers should depend on `pitgun-racing-simulator` for race and
//! session orchestration, or `pitgun-racing-solver` for physical solving.

pub mod rng;

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;

pub use pitgun_racing_simulator::evidence;
pub use pitgun_racing_simulator::evidence::{
    RacingAuthorizedApplicationResultV1, RacingAuthorizedApplicationResultVersion,
    RacingDynamicApplicationResultV1, RacingDynamicApplicationResultVersion,
    RacingDynamicExecutionRequestV1, RacingDynamicExecutionRequestVersion,
    RacingDynamicVerificationSubmissionV1, RacingDynamicVerificationSubmissionVersion,
    RacingHostedExecutionRequestV1, RacingHostedExecutionRequestVersion,
    RacingVerificationSubmissionV1,
};
pub use pitgun_racing_simulator::{
    AeroParams, AuthorizedDynamicRacingSession, BrowserCircuitCatalogEntry, CatalogSnapshot,
    ChassisParams, CircuitDetail, Driver, DriverCatalogEntry, DriverEffects, EngineDetail,
    EngineParams, PitPlan, PitStop, PitStrategyConfig, RaceOutput, RacingCatalogBundleV1,
    RacingCatalogFileV1, RacingCatalogResolutionError, RacingCatalogSnapshot,
    RacingSessionProgressV1, RacingSessionStreamBatchV1, RacingWorkload, RacingWorkloadError,
    ResampledTelemetry, ResolveVehicleCapabilitiesRequestV1, RunRaceInput, RunRaceRequest,
    RunSimulationRequest, SessionConfig, SessionRunOutput, SessionRunRequest, SessionRunResult,
    SimConfig, SimulationRequest, SimulationResult, SimulationSolution, SolverTrackProfile,
    StandingEntry, StandingStatus, TelemetryEnvelope, TireCatalogEntry, TireParams, Track, Tuning,
    V3_POWER_UNIT_THERMAL_PROFILE_DIGEST, V3_THERMAL_FAMILY_PROFILE_DIGEST,
    V3PowerUnitThermalProfileCandidateV2, V3PowerUnitThermalResolutionV2,
    V3ThermalFamilyProfileCandidateV1, V3ThermalFamilyResolutionV1, VehicleCatalogEntry,
    VehicleParams, VehicleState, apply_driver_to_tire, apply_tuning, best_power_at_speed,
    catalog_snapshot, catalog_snapshot_with_catalog, derating_factor, driver_effects, effective_mu,
    execute_authorized_dynamic_race, execute_authorized_dynamic_race_application,
    execute_authorized_race, execute_authorized_race_application, get_circuit,
    get_circuit_with_catalog, get_engine, get_engine_with_catalog, list_browser_circuits,
    list_circuits, list_drivers, list_engines, list_tires, list_vehicles, power_kw_from_rpm,
    racing_model_v1_identity, racing_model_v2_identity, resample_solution,
    resolve_vehicle_capabilities_with_catalog, rpm_from_speed_gear, run_race,
    run_race_with_catalog, run_race_with_catalog_and_v3_power_unit_thermal_profile,
    run_race_with_catalog_and_v3_thermal_family_profile, run_sessions, run_sessions_with_catalog,
    solve, start_authorized_dynamic_racing_session,
};
use serde::Serialize;
use wasm_bindgen::prelude::*;

thread_local! {
    static RACING_SESSIONS: RefCell<BTreeMap<u32, AuthorizedDynamicRacingSession>> =
        const { RefCell::new(BTreeMap::new()) };
    static NEXT_RACING_SESSION_HANDLE: Cell<u32> = const { Cell::new(1) };
}

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
    schema_version: "pitgun.racing-execution-resolution/v1" | "pitgun.racing-execution-resolution/v2" | "pitgun.racing-execution-resolution/v3" | "pitgun.racing-execution-resolution/v4";
    catalog_release: unknown;
    simulation_pack: unknown;
    model: unknown;
    model_parameters?: unknown;
    thermal_family_profile?: unknown;
    power_unit_thermal_profile?: unknown;
    component_capability_profile?: unknown;
    fuel_contract?: unknown;
  };
}

export interface RacingAuthorizedApplicationResultV1 {
  schema_version: "pitgun.racing-authorized-application-result/v1";
  evidence: RacingVerificationSubmissionV1;
  runtime_output: unknown;
}

export interface RacingDynamicExecutionRequestV1 {
  schema_version: "pitgun.racing-dynamic-execution/v1";
  signed_authorization: unknown;
  decision_envelope: unknown;
  catalog_release: unknown;
  input: unknown;
  completed_input: unknown;
  wasm_artifact_digest: PitgunSha256Digest;
}

export interface RacingDynamicVerificationSubmissionV1 {
  schema_version: "pitgun.racing-dynamic-verification-submission/v1";
  signed_authorization: unknown;
  decision_envelope: unknown;
  input: unknown;
  completed_input: unknown;
  receipt: unknown;
  output: unknown;
  telemetry_summary: unknown;
  execution_resolution: unknown;
}

export interface RacingDynamicApplicationResultV1 {
  schema_version: "pitgun.racing-dynamic-application-result/v1";
  evidence: RacingDynamicVerificationSubmissionV1;
  runtime_output: unknown;
}

export interface RacingSessionStreamStartV1 {
  schema_version: "pitgun.racing-session-stream-start/v1";
  handle: number;
}

export interface RacingSessionStreamPullV1 {
  schema_version: "pitgun.racing-session-stream-pull/v1";
  handle: number;
  complete: boolean;
  batch: unknown;
}

export interface RacingSessionStreamCompletionV1 {
  schema_version: "pitgun.racing-session-stream-completion/v1";
  handle: number;
  result: RacingDynamicApplicationResultV1;
}

export interface RacingSessionStreamReleaseV1 {
  schema_version: "pitgun.racing-session-stream-release/v1";
  handle: number;
  released: true;
}
"#;

#[derive(Serialize)]
struct RacingSessionStreamStartV1 {
    schema_version: &'static str,
    handle: u32,
}

#[derive(Serialize)]
struct RacingSessionStreamPullV1 {
    schema_version: &'static str,
    handle: u32,
    complete: bool,
    batch: RacingSessionStreamBatchV1,
}

#[derive(Serialize)]
struct RacingSessionStreamCompletionV1 {
    schema_version: &'static str,
    handle: u32,
    result: RacingDynamicApplicationResultV1,
}

#[derive(Serialize)]
struct RacingSessionStreamReleaseV1 {
    schema_version: &'static str,
    handle: u32,
    released: bool,
}

fn session_json<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_else(|error| {
        serde_json::json!({ "error": format!("serialization failed: {error}") }).to_string()
    })
}

fn session_error(message: impl AsRef<str>) -> String {
    serde_json::json!({ "error": message.as_ref() }).to_string()
}

fn register_racing_session(session: AuthorizedDynamicRacingSession) -> Result<u32, String> {
    RACING_SESSIONS.with(|sessions| {
        NEXT_RACING_SESSION_HANDLE.with(|next| {
            let mut sessions = sessions.borrow_mut();
            let first = next.get().max(1);
            let mut handle = first;
            loop {
                if let std::collections::btree_map::Entry::Vacant(entry) = sessions.entry(handle) {
                    entry.insert(session);
                    next.set(handle.wrapping_add(1).max(1));
                    return Ok(handle);
                }
                handle = handle.wrapping_add(1).max(1);
                if handle == first {
                    return Err("Racing session handle space exhausted".to_string());
                }
            }
        })
    })
}

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
pub fn execute_authorized_race_application_json(request_json: String) -> String {
    pitgun_racing_simulator::execute_authorized_race_application_json(request_json)
}

#[wasm_bindgen]
pub fn execute_authorized_race_application_with_catalog_json(
    request_json: String,
    catalog_bundle_json: String,
) -> String {
    pitgun_racing_simulator::execute_authorized_race_application_with_catalog_json(
        request_json,
        catalog_bundle_json,
    )
}

#[wasm_bindgen]
pub fn execute_authorized_dynamic_race_json(request_json: String) -> String {
    pitgun_racing_simulator::execute_authorized_dynamic_race_json(request_json)
}

#[wasm_bindgen]
pub fn execute_authorized_dynamic_race_with_catalog_json(
    request_json: String,
    catalog_bundle_json: String,
) -> String {
    pitgun_racing_simulator::execute_authorized_dynamic_race_with_catalog_json(
        request_json,
        catalog_bundle_json,
    )
}

#[wasm_bindgen]
pub fn execute_authorized_dynamic_race_application_json(request_json: String) -> String {
    pitgun_racing_simulator::execute_authorized_dynamic_race_application_json(request_json)
}

#[wasm_bindgen]
pub fn execute_authorized_dynamic_race_application_with_catalog_json(
    request_json: String,
    catalog_bundle_json: String,
) -> String {
    pitgun_racing_simulator::execute_authorized_dynamic_race_application_with_catalog_json(
        request_json,
        catalog_bundle_json,
    )
}

/// Starts a pull-based Authority-authorized Racing execution.
///
/// The returned handle is process-local and never participates in evidence or
/// deterministic run identity.
#[wasm_bindgen]
pub fn start_authorized_dynamic_racing_session_with_catalog_json(
    request_json: String,
    catalog_bundle_json: String,
) -> String {
    let request = match serde_json::from_str::<RacingDynamicExecutionRequestV1>(&request_json) {
        Ok(request) => request,
        Err(error) => {
            return session_error(format!("invalid dynamic execution request: {error}"));
        }
    };
    let catalog = match RacingCatalogSnapshot::from_bundle_json(&catalog_bundle_json) {
        Ok(catalog) => catalog,
        Err(error) => return session_error(format!("invalid Racing catalog: {error}")),
    };
    let session = match start_authorized_dynamic_racing_session(request, &catalog) {
        Ok(session) => session,
        Err(error) => return session_error(error),
    };
    let handle = match register_racing_session(session) {
        Ok(handle) => handle,
        Err(error) => return session_error(error),
    };
    session_json(&RacingSessionStreamStartV1 {
        schema_version: "pitgun.racing-session-stream-start/v1",
        handle,
    })
}

/// Advances one pull-based Racing execution by one deterministic lap boundary.
#[wasm_bindgen]
pub fn pull_authorized_dynamic_racing_session_json(handle: u32) -> String {
    RACING_SESSIONS.with(|sessions| {
        let mut sessions = sessions.borrow_mut();
        let session = match sessions.get_mut(&handle) {
            Some(session) => session,
            None => return session_error(format!("unknown Racing session handle {handle}")),
        };
        if session.is_complete() {
            return session_error(format!(
                "Racing session handle {handle} completed; call complete or release"
            ));
        }
        match session.advance() {
            Ok(batch) => session_json(&RacingSessionStreamPullV1 {
                schema_version: "pitgun.racing-session-stream-pull/v1",
                handle,
                complete: session.is_complete(),
                batch,
            }),
            Err(error) => session_error(error),
        }
    })
}

/// Consumes a terminal handle and returns the unchanged hosted-verification
/// application result.
#[wasm_bindgen]
pub fn complete_authorized_dynamic_racing_session_json(handle: u32) -> String {
    RACING_SESSIONS.with(|sessions| {
        let mut sessions = sessions.borrow_mut();
        let Some(session) = sessions.get(&handle) else {
            return session_error(format!("unknown Racing session handle {handle}"));
        };
        if !session.is_complete() {
            return session_error(format!(
                "Racing session handle {handle} is not complete; pull another batch"
            ));
        }
        let session = sessions
            .remove(&handle)
            .expect("checked Racing session handle");
        match session.complete() {
            Ok(result) => session_json(&RacingSessionStreamCompletionV1 {
                schema_version: "pitgun.racing-session-stream-completion/v1",
                handle,
                result,
            }),
            Err(error) => session_error(error),
        }
    })
}

/// Releases an active or completed browser session without producing evidence.
#[wasm_bindgen]
pub fn release_authorized_dynamic_racing_session_json(handle: u32) -> String {
    RACING_SESSIONS.with(|sessions| {
        if sessions.borrow_mut().remove(&handle).is_none() {
            return session_error(format!("unknown Racing session handle {handle}"));
        }
        session_json(&RacingSessionStreamReleaseV1 {
            schema_version: "pitgun.racing-session-stream-release/v1",
            handle,
            released: true,
        })
    })
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
