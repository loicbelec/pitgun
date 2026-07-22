pub mod evidence;
pub mod workload;

pub use workload::{RacingWorkload, RacingWorkloadError};

use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use pitgun_contract::{Sample, SampleValue, SignalQuality, TelemetryFrame};
use pitgun_racing_contract::{
    CircuitCatalogEntry, CompetitorSpec, CompetitorStintStrategy, EngineCatalogEntry, RaceInput,
};
use pitgun_racing_policy::normalize_and_validate_race_input;
use pitgun_racing_solver::{resample_telemetry, run_simulation};
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

pub use pitgun_racing_solver::{
    AeroParams, ChassisParams, Driver, DriverEffects, EngineParams, PitPlan, PitStop,
    ResampledTelemetry, SimConfig, SimulationRequest, SimulationResult, SimulationSolution,
    TireParams, Track, Tuning, VehicleParams, VehicleState, apply_driver_to_tire, apply_tuning,
    best_power_at_speed, derating_factor, driver_effects, effective_mu, power_kw_from_rpm,
    resample_telemetry as resample_solution, rpm_from_speed_gear, run_simulation as solve,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunRaceInput {
    #[serde(flatten)]
    pub race: RaceInput,
    #[serde(default)]
    pub vehicle_id: Option<String>,
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

const EMBEDDED_FILES: &[(&str, &str)] = &[
    (
        "aero/active.json",
        include_str!("../../pitgun-simulator/data/aero/active.json"),
    ),
    (
        "aero/basic.json",
        include_str!("../../pitgun-simulator/data/aero/basic.json"),
    ),
    (
        "aero/none.json",
        include_str!("../../pitgun-simulator/data/aero/none.json"),
    ),
    (
        "chassis/default.json",
        include_str!("../../pitgun-simulator/data/chassis/default.json"),
    ),
    (
        "chassis/f1_2026.json",
        include_str!("../../pitgun-simulator/data/chassis/f1_2026.json"),
    ),
    (
        "circuits/austin.json",
        include_str!("../../pitgun-simulator/data/circuits/austin.json"),
    ),
    (
        "circuits/baku.json",
        include_str!("../../pitgun-simulator/data/circuits/baku.json"),
    ),
    (
        "circuits/barcelona.json",
        include_str!("../../pitgun-simulator/data/circuits/barcelona.json"),
    ),
    (
        "circuits/budapest.json",
        include_str!("../../pitgun-simulator/data/circuits/budapest.json"),
    ),
    (
        "circuits/default.json",
        include_str!("../../pitgun-simulator/data/circuits/default.json"),
    ),
    (
        "circuits/jeddah.json",
        include_str!("../../pitgun-simulator/data/circuits/jeddah.json"),
    ),
    (
        "circuits/las_vegas.json",
        include_str!("../../pitgun-simulator/data/circuits/las_vegas.json"),
    ),
    (
        "circuits/lusail.json",
        include_str!("../../pitgun-simulator/data/circuits/lusail.json"),
    ),
    (
        "circuits/madrid.json",
        include_str!("../../pitgun-simulator/data/circuits/madrid.json"),
    ),
    (
        "circuits/melbourne.json",
        include_str!("../../pitgun-simulator/data/circuits/melbourne.json"),
    ),
    (
        "circuits/mexico.json",
        include_str!("../../pitgun-simulator/data/circuits/mexico.json"),
    ),
    (
        "circuits/miami.json",
        include_str!("../../pitgun-simulator/data/circuits/miami.json"),
    ),
    (
        "circuits/monaco.json",
        include_str!("../../pitgun-simulator/data/circuits/monaco.json"),
    ),
    (
        "circuits/montreal.json",
        include_str!("../../pitgun-simulator/data/circuits/montreal.json"),
    ),
    (
        "circuits/monza.json",
        include_str!("../../pitgun-simulator/data/circuits/monza.json"),
    ),
    (
        "circuits/sakhir.json",
        include_str!("../../pitgun-simulator/data/circuits/sakhir.json"),
    ),
    (
        "circuits/sao_paulo.json",
        include_str!("../../pitgun-simulator/data/circuits/sao_paulo.json"),
    ),
    (
        "circuits/shanghai.json",
        include_str!("../../pitgun-simulator/data/circuits/shanghai.json"),
    ),
    (
        "circuits/silverstone.json",
        include_str!("../../pitgun-simulator/data/circuits/silverstone.json"),
    ),
    (
        "circuits/singapore.json",
        include_str!("../../pitgun-simulator/data/circuits/singapore.json"),
    ),
    (
        "circuits/spa.json",
        include_str!("../../pitgun-simulator/data/circuits/spa.json"),
    ),
    (
        "circuits/spielberg.json",
        include_str!("../../pitgun-simulator/data/circuits/spielberg.json"),
    ),
    (
        "circuits/suzuka.json",
        include_str!("../../pitgun-simulator/data/circuits/suzuka.json"),
    ),
    (
        "circuits/yas_marina.json",
        include_str!("../../pitgun-simulator/data/circuits/yas_marina.json"),
    ),
    (
        "circuits/zandvoort.json",
        include_str!("../../pitgun-simulator/data/circuits/zandvoort.json"),
    ),
    (
        "drivers/aggressive.json",
        include_str!("../../pitgun-simulator/data/drivers/aggressive.json"),
    ),
    (
        "drivers/balanced.json",
        include_str!("../../pitgun-simulator/data/drivers/balanced.json"),
    ),
    (
        "drivers/battery_voltas.json",
        include_str!("../../pitgun-simulator/data/drivers/battery_voltas.json"),
    ),
    (
        "drivers/charles_leclair.json",
        include_str!("../../pitgun-simulator/data/drivers/charles_leclair.json"),
    ),
    (
        "drivers/conservative.json",
        include_str!("../../pitgun-simulator/data/drivers/conservative.json"),
    ),
    (
        "drivers/daniel_enchantier.json",
        include_str!("../../pitgun-simulator/data/drivers/daniel_enchantier.json"),
    ),
    (
        "drivers/default.json",
        include_str!("../../pitgun-simulator/data/drivers/default.json"),
    ),
    (
        "drivers/franz_hermann.json",
        include_str!("../../pitgun-simulator/data/drivers/franz_hermann.json"),
    ),
    (
        "drivers/goat_tifi.json",
        include_str!("../../pitgun-simulator/data/drivers/goat_tifi.json"),
    ),
    (
        "drivers/isa_kadjar.json",
        include_str!("../../pitgun-simulator/data/drivers/isa_kadjar.json"),
    ),
    (
        "drivers/luis_amilton.json",
        include_str!("../../pitgun-simulator/data/drivers/luis_amilton.json"),
    ),
    (
        "drivers/pedro_gaseoso.json",
        include_str!("../../pitgun-simulator/data/drivers/pedro_gaseoso.json"),
    ),
    (
        "drivers/smooth_operator.json",
        include_str!("../../pitgun-simulator/data/drivers/smooth_operator.json"),
    ),
    (
        "engines/v6t.json",
        include_str!("../../pitgun-simulator/data/engines/v6t.json"),
    ),
    (
        "engines/v6t_hybrid.json",
        include_str!("../../pitgun-simulator/data/engines/v6t_hybrid.json"),
    ),
    (
        "engines/v8_1960.json",
        include_str!("../../pitgun-simulator/data/engines/v8_1960.json"),
    ),
    (
        "engines/v8_1970.json",
        include_str!("../../pitgun-simulator/data/engines/v8_1970.json"),
    ),
    (
        "tires/hard.json",
        include_str!("../../pitgun-simulator/data/tires/hard.json"),
    ),
    (
        "tires/medium.json",
        include_str!("../../pitgun-simulator/data/tires/medium.json"),
    ),
    (
        "tires/soft.json",
        include_str!("../../pitgun-simulator/data/tires/soft.json"),
    ),
    (
        "vehicles/classic_v8_1960.json",
        include_str!("../../pitgun-simulator/data/vehicles/classic_v8_1960.json"),
    ),
    (
        "vehicles/classic_v8_1970.json",
        include_str!("../../pitgun-simulator/data/vehicles/classic_v8_1970.json"),
    ),
    (
        "vehicles/default.json",
        include_str!("../../pitgun-simulator/data/vehicles/default.json"),
    ),
    (
        "vehicles/f1_2026.json",
        include_str!("../../pitgun-simulator/data/vehicles/f1_2026.json"),
    ),
    (
        "vehicles/modern_v6t.json",
        include_str!("../../pitgun-simulator/data/vehicles/modern_v6t.json"),
    ),
];

pub fn run_race(request: RunRaceRequest) -> Result<RaceOutput, String> {
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
    let catalog = EmbeddedCatalog::load_default()?;
    run_single_session(
        &catalog,
        &normalized_race,
        vehicle_id,
        request.input.pit_strategy.as_ref(),
        request.input.track_profile.as_ref(),
        normalized_race.laps,
        request.seed,
    )
}

pub fn run_sessions(request: SessionRunRequest) -> Result<SessionRunOutput, String> {
    if request.sessions.is_empty() {
        return Err("sessions must be provided explicitly".to_string());
    }

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
    let catalog = EmbeddedCatalog::load_default()?;
    let mut sessions = Vec::with_capacity(request.sessions.len());

    for session in &request.sessions {
        let output = run_single_session(
            &catalog,
            &normalized_race,
            vehicle_id,
            request.pit_strategy.as_ref(),
            request.track_profile.as_ref(),
            session.laps,
            request.seed,
        )?;
        sessions.push(SessionRunResult {
            session: session.session.clone(),
            standings: output.standings,
            total_time_ms: output.total_time_ms,
        });
    }

    Ok(SessionRunOutput { sessions })
}

fn run_single_session(
    catalog: &EmbeddedCatalog,
    race: &RaceInput,
    vehicle_id: &str,
    pit_strategy: Option<&PitStrategyConfig>,
    track_profile: Option<&SolverTrackProfile>,
    laps: u16,
    seed: u64,
) -> Result<RaceOutput, String> {
    let track_id = normalize_track_id(&race.track_id);
    let mut track_record = catalog.get_track(&track_id)?.clone();
    if let Some(payload) = track_profile {
        track_record = track_from_payload(&track_id, payload, track_record.pit_loss_ms)?;
    }

    let resolved_vehicle = catalog.resolve_vehicle(vehicle_id)?;
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

    let mut rows = Vec::with_capacity(race.competitors.len());
    let mut player_frames = Vec::new();
    let mut player_lap_times_ms = Vec::new();
    let mut player_resolved_pit_laps = Vec::new();

    for competitor in &race.competitors {
        let stint_plan =
            resolve_stint_plan(competitor, laps, &resolved_vehicle.1, &player_pit_laps)?;
        let driver = catalog.resolve_driver(competitor.driver_id.as_deref())?;
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
        let request = SimulationRequest {
            track: track_record.track.clone(),
            vehicle: resolved_vehicle.0.clone(),
            state: VehicleState {
                fuel_mass: 100.0,
                tire_wear: 0.0,
                tire_temp: 90.0,
                engine_temp: resolved_vehicle.0.engine.t_init,
            },
            config: sim_config,
            lap_count: laps.max(1),
            pit_plan,
            driver,
            tuning: Some(Tuning {
                engine_points: competitor.tuning.engine_points.round() as i32,
                cooling_points: competitor.tuning.cooling_points.round() as i32,
                aero_points: competitor.tuning.aero_points.round() as i32,
                chassis_points: competitor.tuning.chassis_points.round() as i32,
                downforce_slider: competitor.tuning.downforce_slider,
                gear_ratio_slider: competitor.tuning.gear_ratio_slider,
            }),
        };
        let result = run_simulation(&request)
            .map_err(|err| format!("simulation failed for competitor {}: {err}", competitor.id))?;

        let lap_times_ms = lap_times_ms(&result.lap_times_s, &stint_plan, pit_loss_ms);
        let total_time_ms = lap_times_ms.iter().copied().sum::<u64>();
        let best_lap_ms = lap_times_ms.iter().copied().min().unwrap_or(0);

        if competitor.is_player || competitor.id == "player" {
            let telemetry_hz = 5.0;
            let resampled = resample_telemetry(
                &request.track,
                &result.solution,
                &result.applied_vehicle,
                5.0,
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
    })
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn run_simulation_json(input_json: String) -> String {
    run_race_json(input_json)
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn run_race_json(input_json: String) -> String {
    let parsed = serde_json::from_str::<RunRacePayload>(&input_json);
    let request = match parsed {
        Ok(RunRacePayload::Wrapped(request)) => request,
        Ok(RunRacePayload::Bare(input)) => RunRaceRequest {
            input,
            seed: 0,
            era: None,
            hz: None,
        },
        Err(err) => return json_error(&format!("invalid request: {err}")),
    };

    match run_race(request) {
        Ok(output) => serialize_json(&output),
        Err(error) => json_error(&error),
    }
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
    Ok(CatalogSnapshot {
        circuits: list_browser_circuits()?,
        engines: list_engines()?,
        vehicles: list_vehicles()?,
        drivers: list_drivers()?,
        tires: list_tires()?,
    })
}

pub fn list_browser_circuits() -> Result<Vec<BrowserCircuitCatalogEntry>, String> {
    let catalog = EmbeddedCatalog::load_default()?;
    let mut items = catalog
        .tracks
        .values()
        .map(|track| BrowserCircuitCatalogEntry {
            id: track.id.clone(),
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
    let catalog = EmbeddedCatalog::load_default()?;
    let record = catalog.get_track(track_id)?;
    Ok(CircuitDetail {
        id: record.id.clone(),
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
    let catalog = EmbeddedCatalog::load_default()?;
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
        let mut catalog = Self::default();
        for (path, raw) in EMBEDDED_FILES {
            catalog.apply_file(path, raw)?;
        }
        if !catalog.drivers.contains_key("default") {
            catalog
                .drivers
                .insert("default".to_string(), Driver::default());
        }
        Ok(catalog)
    }

    fn apply_file(&mut self, path: &str, raw: &str) -> Result<(), String> {
        let value: Value =
            serde_json::from_str(raw).map_err(|err| format!("failed to parse '{path}': {err}"))?;
        let (category, file_name) = path
            .split_once('/')
            .ok_or_else(|| format!("invalid embedded path '{path}'"))?;
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
            _ => {}
        }
        Ok(())
    }

    fn get_track(&self, track_id: &str) -> Result<&TrackRecord, String> {
        let id = normalize_track_id(track_id);
        self.tracks
            .get(&id)
            .ok_or_else(|| format!("unknown circuit '{id}'"))
    }

    fn resolve_vehicle(&self, vehicle_id: &str) -> Result<(VehicleParams, String), String> {
        let record = self
            .vehicles
            .get(vehicle_id)
            .ok_or_else(|| format!("unknown vehicle '{vehicle_id}'"))?;
        let aero = self
            .aeros
            .get(&record.aero_id)
            .ok_or_else(|| format!("unknown aero '{}'", record.aero_id))?;
        let chassis = self
            .chassis
            .get(&record.chassis_id)
            .ok_or_else(|| format!("unknown chassis '{}'", record.chassis_id))?;
        let engine = self
            .engines
            .get(&record.engine_id)
            .ok_or_else(|| format!("unknown engine '{}'", record.engine_id))?;
        let tire = self
            .tires
            .get(&record.tire_id)
            .ok_or_else(|| format!("unknown tire '{}'", record.tire_id))?;
        Ok((
            VehicleParams {
                chassis: chassis.clone(),
                aero: aero.clone(),
                engine: engine.clone(),
                tire: tire.clone(),
            },
            record.tire_id.clone(),
        ))
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

fn normalize_track_id(track_id: &str) -> String {
    track_id
        .chars()
        .filter(|ch| !matches!(ch, '-' | '_' | ' '))
        .flat_map(char::to_uppercase)
        .collect()
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
    use pitgun_racing_contract::{CompetitorSpec, RaceInput, TuningSpec};
    use serde::Deserialize;

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

    #[test]
    fn python_monza_reference_stays_close_except_launch_override() {
        let fixture: GoldenFixture = serde_json::from_str(include_str!(
            "../../pitgun-solver/tests/golden/python_monza_f1_2026_default.json"
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
            catalog.circuits.iter().any(|entry| entry.id == "IT1922"),
            "catalog must expose IT1922"
        );
        assert!(
            catalog.engines.iter().any(|entry| entry.id == "v6t_hybrid"),
            "catalog must expose v6t_hybrid"
        );
        let monza = catalog
            .circuits
            .iter()
            .find(|entry| entry.id == "IT1922")
            .expect("catalog must expose IT1922");
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

        let request = RunRaceRequest {
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
                pit_strategy: None,
                track_profile: None,
                competitor_profiles: HashMap::new(),
                era: 2026,
                hz: 20.0,
            },
            seed: 7,
            era: Some(2026),
            hz: Some(20.0),
        };

        let output: RaceOutput = serde_json::from_str(&run_race_json(
            serde_json::to_string(&request).expect("serialize request"),
        ))
        .expect("run_race_json must return valid JSON");

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
}
