//! Transitional compatibility and WASM facade for the Racing Simulator.
//!
//! New Rust consumers should depend on `pitgun-racing-simulator` for race and
//! session orchestration, or `pitgun-racing-solver` for physical solving.

pub mod rng;

pub use pitgun_racing_simulator::evidence;
pub use pitgun_racing_simulator::{
    AeroParams, BrowserCircuitCatalogEntry, CatalogSnapshot, ChassisParams, CircuitDetail, Driver,
    DriverCatalogEntry, DriverEffects, EngineDetail, EngineParams, PitPlan, PitStop,
    PitStrategyConfig, RaceOutput, RacingWorkload, RacingWorkloadError, ResampledTelemetry,
    RunRaceInput, RunRaceRequest, RunSimulationRequest, SessionConfig, SessionRunOutput,
    SessionRunRequest, SessionRunResult, SimConfig, SimulationRequest, SimulationResult,
    SimulationSolution, SolverTrackProfile, StandingEntry, StandingStatus, TelemetryEnvelope,
    TireCatalogEntry, TireParams, Track, Tuning, VehicleCatalogEntry, VehicleParams, VehicleState,
    apply_driver_to_tire, apply_tuning, best_power_at_speed, catalog_snapshot, derating_factor,
    driver_effects, effective_mu, get_circuit, get_engine, list_browser_circuits, list_circuits,
    list_drivers, list_engines, list_tires, list_vehicles, power_kw_from_rpm, resample_solution,
    rpm_from_speed_gear, run_race, run_sessions, solve,
};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run_simulation_json(input_json: String) -> String {
    pitgun_racing_simulator::run_simulation_json(input_json)
}

#[wasm_bindgen]
pub fn run_race_json(input_json: String) -> String {
    pitgun_racing_simulator::run_race_json(input_json)
}

#[wasm_bindgen]
pub fn run_sessions_json(input_json: String) -> String {
    pitgun_racing_simulator::run_sessions_json(input_json)
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
pub fn list_circuits_json() -> String {
    pitgun_racing_simulator::list_circuits_json()
}

#[wasm_bindgen]
pub fn get_circuit_json(track_id: String) -> String {
    pitgun_racing_simulator::get_circuit_json(track_id)
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
