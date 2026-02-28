use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use pitgun_contract::{
    CircuitCatalogEntry, CompetitorSpec, CompetitorStintStrategy, EngineCatalogEntry, RaceInput,
    Sample, SampleValue, SignalQuality, TelemetryFrame, TuningSpec,
};
use pitgun_policy::validation::normalize_and_validate_race_input;
use pitgun_simulator::{
    ConfigProvider, DataRegistry, LapInput, LapOutput, SessionKind, Simulator, SimulatorState,
    Tuning as SimulatorTuning, default_driver_id, default_in_memory_provider,
};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

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
pub struct TelemetryEnvelope {
    pub frames: Vec<TelemetryFrame>,
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
pub struct SolverRequest {
    pub track_id: String,
    #[serde(default)]
    pub vehicle_id: Option<String>,
    #[serde(default)]
    pub track_profile: Option<SolverTrackProfile>,
    pub laps: u16,
    pub budget: f64,
    pub seed: u64,
    #[serde(default)]
    pub runs: usize,
    #[serde(default)]
    pub era: i32,
    #[serde(default)]
    pub hz: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolverResponse {
    pub baseline: TuningSpec,
    pub top_reference: TuningSpec,
    pub baseline_time_ms: u64,
    pub top_time_ms: u64,
    pub runs_used: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    pub session: SessionKind,
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
    pub session: SessionKind,
    pub standings: Vec<StandingEntry>,
    pub total_time_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRunOutput {
    pub sessions: Vec<SessionRunResult>,
}

#[derive(Debug, Clone)]
struct SimulatedCompetitor {
    competitor_id: String,
    total_time_ms: u64,
    best_lap_ms: u64,
    laps_completed: u16,
    lap_times_ms: Vec<u64>,
}

#[derive(Debug, Clone)]
struct ResolvedStintPlan {
    tire_by_lap: Vec<String>,
    pit_laps: Vec<u16>,
}

const TELEMETRY_BATCH_SIZE: usize = 64;
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

fn clamp(v: f64, lo: f64, hi: f64) -> f64 {
    v.max(lo).min(hi)
}

fn normalize_track_id(track_id: &str) -> String {
    track_id
        .chars()
        .filter(|ch| !matches!(ch, '-' | '_' | ' '))
        .flat_map(char::to_uppercase)
        .collect()
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

    for (index, stint) in strategy.stints.iter().enumerate() {
        if stint.tire_id.trim().is_empty() {
            return Err(format!("stint {index} has an empty tire_id"));
        }
        if stint.laps == 0 {
            return Err(format!("stint {index} must have at least 1 lap"));
        }
        for _ in 0..stint.laps {
            tire_by_lap.push(stint.tire_id.clone());
        }
        cumulative = cumulative.saturating_add(stint.laps);
        if index < strategy.stints.len() - 1 && cumulative > 0 && cumulative < total_laps {
            pit_laps.push(cumulative);
        }
    }

    if cumulative != total_laps {
        return Err(format!(
            "stint_strategy laps must sum to {total_laps}, got {cumulative}"
        ));
    }

    let declared_pit_laps = sanitize_pit_laps(&strategy.pit_laps, total_laps);
    if !declared_pit_laps.is_empty() && declared_pit_laps != pit_laps {
        return Err(format!(
            "stint_strategy pit_laps {:?} does not match stint boundaries {:?}",
            declared_pit_laps, pit_laps
        ));
    }

    Ok(ResolvedStintPlan { tire_by_lap, pit_laps })
}

fn simulator_tuning(t: &TuningSpec) -> SimulatorTuning {
    SimulatorTuning {
        engine_points: t.engine_points.max(0.0),
        cooling_points: t.cooling_points.max(0.0),
        aero_points: t.aero_points.max(0.0),
        chassis_points: t.chassis_points.max(0.0),
        downforce_slider: clamp(t.downforce_slider, 0.0, 1.0),
        gear_ratio_slider: clamp(t.gear_ratio_slider, 0.0, 1.0),
    }
}

fn resolve_vehicle_id(vehicle_id: Option<&str>) -> Result<&str, String> {
    let Some(vehicle_id) = vehicle_id.map(str::trim).filter(|value| !value.is_empty()) else {
        return Err("vehicle_id is required; solver no longer maps eras to simulator vehicles".to_string());
    };
    Ok(vehicle_id)
}

fn telemetry_session_id(seed: u64, track_id: &str, competitor_id: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    seed.hash(&mut hasher);
    track_id.hash(&mut hasher);
    competitor_id.hash(&mut hasher);
    let hashed = hasher.finish() & ((1u64 << 53) - 1);
    hashed.max(1)
}

fn telemetry_sample(parameter_id: u16, value: f64) -> Sample {
    Sample::new(parameter_id, SampleValue::F64(value), SignalQuality::Good)
}

fn telemetry_frames_from_lap(
    output: &LapOutput,
    lap_idx: u16,
    time_offset_s: f64,
    distance_offset_m: f64,
    time_scale: f64,
    session_id: u64,
    next_sequence: &mut u64,
    source_id: &str,
    frame_metadata: &HashMap<String, String>,
) -> Vec<TelemetryFrame> {
    let mut frames = Vec::with_capacity(output.telemetry.len());

    for frame in &output.telemetry {
        let time_s = time_offset_s + frame.time_s * time_scale;
        let timestamp_us = (time_s * 1_000_000.0).round() as i64;
        let sequence = *next_sequence;
        *next_sequence = next_sequence.saturating_add(1);

        frames.push(TelemetryFrame {
            session_id,
            sequence,
            timestamp_us,
            received_at_us: timestamp_us,
            source_id: source_id.to_string(),
            samples: vec![
                telemetry_sample(PARAM_TIME_S, time_s),
                telemetry_sample(PARAM_DISTANCE_M, frame.s_m + distance_offset_m),
                telemetry_sample(PARAM_X_M, frame.x_m),
                telemetry_sample(PARAM_Y_M, frame.y_m),
                telemetry_sample(PARAM_HEADING_RAD, frame.heading_rad),
                telemetry_sample(PARAM_SPEED_KPH, frame.speed_kph),
                telemetry_sample(PARAM_RPM, frame.rpm),
                telemetry_sample(PARAM_GEAR, frame.gear as f64),
                telemetry_sample(PARAM_THROTTLE_PCT, frame.throttle_pct),
                telemetry_sample(PARAM_BRAKE_PCT, frame.brake_pct),
                telemetry_sample(PARAM_G_LAT, frame.g_lat),
                telemetry_sample(PARAM_G_LONG, frame.g_long),
                telemetry_sample(PARAM_G_VERT, frame.g_vert),
                telemetry_sample(PARAM_ENGINE_TEMP_C, frame.engine_temp_c),
                telemetry_sample(PARAM_ENGINE_POWER_W, frame.engine_power_w),
                telemetry_sample(PARAM_TIRE_TEMP_C, frame.tire_temp_c),
                telemetry_sample(PARAM_TIRE_WEAR_PCT, frame.tire_wear_pct),
            ],
            events: Vec::new(),
            lap_number: Some(lap_idx),
            sector: None,
            lap_distance_m: Some((frame.s_m + distance_offset_m) as f32),
            metadata: frame_metadata.clone(),
        });
    }

    frames
}

fn telemetry_batches(frames: Vec<TelemetryFrame>) -> Vec<TelemetryEnvelope> {
    frames
        .chunks(TELEMETRY_BATCH_SIZE)
        .map(|chunk| TelemetryEnvelope {
            frames: chunk.to_vec(),
        })
        .collect()
}

fn simulate_competitor_session(
    provider: &dyn ConfigProvider,
    simulator: &Simulator,
    competitor: &CompetitorSpec,
    vehicle_id: &str,
    track_id: &str,
    profile_id: Option<&str>,
    laps: u16,
    hz: f64,
    stint_plan: &ResolvedStintPlan,
    pit_loss_ms: u64,
    seed: u64,
) -> Result<(SimulatedCompetitor, Vec<TelemetryFrame>), String> {
    let laps = laps.max(1);
    let mut total_time_ms = 0u64;
    let mut best_lap_ms = u64::MAX;
    let mut lap_times_ms = Vec::with_capacity(laps as usize);

    let mut state: Option<SimulatorState> = None;
    let mut telemetry_frames = Vec::new();
    let mut time_offset_s = 0.0;
    let mut distance_offset_m = 0.0;
    let mut next_sequence = 0u64;
    let vehicle = provider
        .get_vehicle(vehicle_id)
        .map_err(|err| format!("missing vehicle '{vehicle_id}': {err}"))?;
    let default_tire = vehicle.tire_id;
    let driver_id = competitor
        .driver_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(default_driver_id());
    let telemetry_session = telemetry_session_id(seed, track_id, &competitor.id);
    let source_id = format!("pitwall-sim:{}", competitor.id);

    for lap_idx in 0..laps {
        let tire_id = stint_plan
            .tire_by_lap
            .get(lap_idx as usize)
            .map(String::as_str)
            .unwrap_or(default_tire.as_str());
        let lap_output = simulator
            .simulate_lap(LapInput {
                vehicle_id: vehicle_id.to_string(),
                track_id: track_id.to_string(),
                tuning: simulator_tuning(&competitor.tuning),
                profile_id: profile_id.map(str::to_string),
                profile: None,
                driver_id: Some(driver_id.to_string()),
                tire_id: Some(tire_id.to_string()),
                initial_state: state.clone(),
                seed: Some(seed),
                lap_number: Some(lap_idx.saturating_add(1)),
                hz,
            })
            .map_err(|err| format!("simulation failed for competitor {}: {err}", competitor.id))?;

        let base_lap_ms = (lap_output.lap_time_s * 1000.0).round().max(1.0) as u64;
        let lap_number = lap_idx.saturating_add(1);
        let pit_penalty_ms = if stint_plan.pit_laps.binary_search(&lap_number).is_ok() {
            pit_loss_ms
        } else {
            0
        };
        let lap_time_ms = base_lap_ms.saturating_add(pit_penalty_ms);
        lap_times_ms.push(lap_time_ms);

        if competitor.is_player || competitor.id == "player" {
            let scale = lap_time_ms as f64 / (base_lap_ms as f64).max(1.0);
            let mut frame_metadata = HashMap::from([
                ("track_id".to_string(), track_id.to_string()),
                ("vehicle_id".to_string(), vehicle_id.to_string()),
                ("competitor_id".to_string(), competitor.id.clone()),
                ("driver_id".to_string(), driver_id.to_string()),
                ("tire_id".to_string(), tire_id.to_string()),
            ]);
            frame_metadata.insert(
                "role".to_string(),
                if competitor.is_player || competitor.id == "player" {
                    "player".to_string()
                } else {
                    "ai".to_string()
                },
            );
            telemetry_frames.extend(telemetry_frames_from_lap(
                &lap_output,
                lap_idx + 1,
                time_offset_s,
                distance_offset_m,
                scale,
                telemetry_session,
                &mut next_sequence,
                &source_id,
                &frame_metadata,
            ));
        }

        total_time_ms = total_time_ms.saturating_add(lap_time_ms);
        best_lap_ms = best_lap_ms.min(lap_time_ms);
        time_offset_s += lap_time_ms as f64 / 1000.0;
        distance_offset_m += lap_output.telemetry.last().map(|frame| frame.s_m).unwrap_or(0.0);
        let mut next_state = lap_output.final_state;
        if pit_penalty_ms > 0 {
            next_state.tire_wear = 0.0;
            next_state.tire_temp_c = 90.0;
        }
        state = Some(next_state);
    }

    Ok((
        SimulatedCompetitor {
            competitor_id: competitor.id.clone(),
            total_time_ms,
            best_lap_ms,
            laps_completed: laps,
            lap_times_ms,
        },
        telemetry_frames,
    ))
}

fn run_single_session(
    race: &RaceInput,
    vehicle_id: &str,
    pit_strategy: Option<&PitStrategyConfig>,
    track_profile: Option<&SolverTrackProfile>,
    hz: f64,
    laps: u16,
    profile_overrides: &HashMap<String, String>,
    seed: u64,
) -> Result<RaceOutput, String> {
    let mut provider = default_in_memory_provider();
    let track_id = normalize_track_id(&race.track_id);
    let base_track = provider
        .get_track(&track_id)
        .map_err(|err| format!("track '{track_id}' is not available in the simulator data pack: {err}"))?;

    if let Some(payload) = track_profile {
        let track = track_from_payload(&track_id, payload, base_track.pit_loss_ms)?;
        provider.insert_track(track);
    }

    let provider = Arc::new(provider);
    let simulator = Simulator::new(provider.clone());
    let mut rows = Vec::with_capacity(race.competitors.len());
    let mut player_frames = Vec::new();
    let mut player_pit_laps = Vec::new();
    let mut player_lap_times_ms = Vec::new();

    let pit_loss_ms = pit_strategy
        .and_then(|strategy| strategy.pit_loss_ms)
        .map(|value| value.max(1_000))
        .unwrap_or(base_track.pit_loss_ms);
    let configured_player_pit_laps = sanitize_pit_laps(
        pit_strategy
            .map(|strategy| strategy.player_pit_laps.as_slice())
            .unwrap_or(&[]),
        laps,
    );

    for competitor in &race.competitors {
        let vehicle = provider
            .get_vehicle(vehicle_id)
            .map_err(|err| format!("vehicle '{vehicle_id}' is not available in the simulator data pack: {err}"))?;
        let stint_plan = resolve_stint_plan(
            competitor,
            laps,
            &vehicle.tire_id,
            &configured_player_pit_laps,
        )?;

        let (simulated, frames) = simulate_competitor_session(
            provider.as_ref(),
            &simulator,
            competitor,
            vehicle_id,
            &track_id,
            profile_overrides.get(&competitor.id).map(String::as_str),
            laps,
            hz,
            &stint_plan,
            pit_loss_ms,
            seed,
        )?;

        if competitor.is_player || competitor.id == "player" {
            player_lap_times_ms = simulated.lap_times_ms.clone();
            player_frames = frames;
            player_pit_laps = stint_plan.pit_laps.clone();
        }

        rows.push(simulated);
    }

    rows.sort_by_key(|row| row.total_time_ms);
    let leader_time = rows.first().map(|row| row.total_time_ms).unwrap_or(0);

    let standings = rows
        .into_iter()
        .enumerate()
        .map(|(idx, row)| StandingEntry {
            competitor_id: row.competitor_id,
            position: (idx + 1) as u32,
            total_time_ms: row.total_time_ms,
            best_lap_ms: row.best_lap_ms,
            laps_completed: row.laps_completed,
            gap_to_leader_ms: row.total_time_ms.saturating_sub(leader_time),
            status: StandingStatus::Finished,
        })
        .collect::<Vec<_>>();

    Ok(RaceOutput {
        standings,
        total_time_ms: leader_time,
        player_pit_laps,
        player_lap_times_ms,
        player_batches: telemetry_batches(player_frames),
    })
}

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

    run_single_session(
        &normalized_race,
        vehicle_id,
        request.input.pit_strategy.as_ref(),
        request.input.track_profile.as_ref(),
        request.hz.unwrap_or(request.input.hz),
        normalized_race.laps,
        &request.input.competitor_profiles,
        request.seed,
    )
}

pub fn run_sessions(request: SessionRunRequest) -> Result<SessionRunOutput, String> {
    if request.sessions.is_empty() {
        return Err(
            "sessions must be provided explicitly; solver no longer injects a default weekend"
                .to_string(),
        );
    }

    let normalized_race = normalize_and_validate_race_input(
        &request.race,
        if request.era > 0 { request.era as u32 } else { 0 },
    )
    .map_err(|err| format!("invalid race input: {err}"))?;

    let vehicle_id = resolve_vehicle_id(request.vehicle_id.as_deref())?;
    let mut out = Vec::with_capacity(request.sessions.len());

    for session_cfg in &request.sessions {
        let race_output = run_single_session(
            &normalized_race,
            vehicle_id,
            request.pit_strategy.as_ref(),
            request.track_profile.as_ref(),
            request.hz,
            session_cfg.laps,
            &session_cfg.profile_overrides,
            request.seed,
        )?;

        out.push(SessionRunResult {
            session: session_cfg.session,
            standings: race_output.standings,
            total_time_ms: race_output.total_time_ms,
        });
    }

    Ok(SessionRunOutput { sessions: out })
}

pub fn solve_baseline(_: SolverRequest) -> Option<SolverResponse> {
    None
}

fn load_default_registry() -> Result<DataRegistry, String> {
    DataRegistry::load_default().map_err(|err| format!("failed to load simulator data pack: {err}"))
}

fn list_circuits() -> Result<Vec<CircuitCatalogEntry>, String> {
    let registry = load_default_registry()?;
    Ok(registry
        .tracks()
        .into_iter()
        .map(|track| CircuitCatalogEntry {
            id: track.id,
            country_code: track.country_code,
            sample_count: track.s_m.len(),
            distance_m: track.s_m.last().copied().unwrap_or(0.0),
            pit_loss_ms: track.pit_loss_ms,
        })
        .collect())
}

fn get_circuit(track_id: &str) -> Result<pitgun_simulator::TrackConfig, String> {
    let track_id = normalize_track_id(track_id);
    let registry = load_default_registry()?;
    registry
        .tracks()
        .into_iter()
        .find(|track| track.id == track_id)
        .ok_or_else(|| format!("unknown circuit '{track_id}'"))
}

fn list_engines() -> Result<Vec<EngineCatalogEntry>, String> {
    let registry = load_default_registry()?;
    Ok(registry
        .engines()
        .into_iter()
        .map(|engine| EngineCatalogEntry {
            id: engine.id,
            idle_rpm: engine.idle_rpm,
            max_rpm: engine.max_rpm,
            gear_count: engine.gear_ratios.len(),
        })
        .collect())
}

fn get_engine(engine_id: &str) -> Result<pitgun_simulator::EngineConfig, String> {
    let registry = load_default_registry()?;
    registry
        .engines()
        .into_iter()
        .find(|engine| engine.id == engine_id)
        .ok_or_else(|| format!("unknown engine '{engine_id}'"))
}

#[wasm_bindgen]
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
        Err(err) => {
            return serde_json::json!({
                "error": format!("invalid request: {err}")
            })
            .to_string();
        }
    };

    match run_race(request) {
        Ok(output) => serde_json::to_string(&output).unwrap_or_else(|err| {
            serde_json::json!({
                "error": format!("serialization error: {err}")
            })
            .to_string()
        }),
        Err(error) => serde_json::json!({ "error": error }).to_string(),
    }
}

#[wasm_bindgen]
pub fn solve_baseline_json(input_json: String) -> String {
    let parsed = serde_json::from_str::<SolverRequest>(&input_json);
    let request = match parsed {
        Ok(req) => req,
        Err(err) => {
            return serde_json::json!({
                "error": format!("invalid request: {err}")
            })
            .to_string();
        }
    };

    match solve_baseline(request) {
        Some(output) => serde_json::to_string(&output).unwrap_or_else(|err| {
            serde_json::json!({
                "error": format!("serialization error: {err}")
            })
            .to_string()
        }),
        None => serde_json::json!({
            "error": "baseline optimizer has been disabled; move this logic into a dedicated optimizer crate"
        })
        .to_string(),
    }
}

#[wasm_bindgen]
pub fn run_sessions_json(input_json: String) -> String {
    let parsed = serde_json::from_str::<SessionRunRequest>(&input_json);
    let request = match parsed {
        Ok(req) => req,
        Err(err) => {
            return serde_json::json!({
                "error": format!("invalid request: {err}")
            })
            .to_string();
        }
    };

    match run_sessions(request) {
        Ok(output) => serde_json::to_string(&output).unwrap_or_else(|err| {
            serde_json::json!({
                "error": format!("serialization error: {err}")
            })
            .to_string()
        }),
        Err(error) => serde_json::json!({ "error": error }).to_string(),
    }
}

#[wasm_bindgen]
pub fn list_circuits_json() -> String {
    match list_circuits() {
        Ok(output) => serde_json::to_string(&output).unwrap_or_else(|err| {
            serde_json::json!({
                "error": format!("serialization error: {err}")
            })
            .to_string()
        }),
        Err(error) => serde_json::json!({ "error": error }).to_string(),
    }
}

#[wasm_bindgen]
pub fn get_circuit_json(track_id: String) -> String {
    match get_circuit(&track_id) {
        Ok(output) => serde_json::to_string(&output).unwrap_or_else(|err| {
            serde_json::json!({
                "error": format!("serialization error: {err}")
            })
            .to_string()
        }),
        Err(error) => serde_json::json!({ "error": error }).to_string(),
    }
}

#[wasm_bindgen]
pub fn list_engines_json() -> String {
    match list_engines() {
        Ok(output) => serde_json::to_string(&output).unwrap_or_else(|err| {
            serde_json::json!({
                "error": format!("serialization error: {err}")
            })
            .to_string()
        }),
        Err(error) => serde_json::json!({ "error": error }).to_string(),
    }
}

#[wasm_bindgen]
pub fn get_engine_json(engine_id: String) -> String {
    match get_engine(&engine_id) {
        Ok(output) => serde_json::to_string(&output).unwrap_or_else(|err| {
            serde_json::json!({
                "error": format!("serialization error: {err}")
            })
            .to_string()
        }),
        Err(error) => serde_json::json!({ "error": error }).to_string(),
    }
}

fn track_from_payload(
    track_id: &str,
    payload: &SolverTrackProfile,
    pit_loss_ms: u64,
) -> Result<pitgun_simulator::TrackConfig, String> {
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
    if payload
        .s
        .iter()
        .chain(payload.x.iter())
        .chain(payload.y.iter())
        .any(|value| !value.is_finite())
    {
        return Err("track_profile s/x/y values must be finite".to_string());
    }

    let z = if payload.z.is_empty() {
        vec![0.0; n]
    } else if payload.z.len() == n {
        if payload.z.iter().any(|value| !value.is_finite()) {
            return Err("track_profile z values must be finite".to_string());
        }
        payload.z.clone()
    } else {
        return Err("track_profile z must be empty or match the length of s".to_string());
    };

    let mut heading = vec![0.0; n];
    for i in 0..n {
        let i0 = i.saturating_sub(1);
        let i1 = (i + 1).min(n - 1);
        let dx = payload.x[i1] - payload.x[i0];
        let dy = payload.y[i1] - payload.y[i0];
        heading[i] = dy.atan2(dx);
    }

    for i in 1..n {
        heading[i] = unwrap_angle(heading[i], heading[i - 1]);
    }

    let mut curvature = vec![0.0; n];
    let mut slope = vec![0.0; n];
    for i in 0..n {
        let i0 = i.saturating_sub(1);
        let i1 = (i + 1).min(n - 1);
        let ds = (payload.s[i1] - payload.s[i0]).max(1e-6);
        curvature[i] = (heading[i1] - heading[i0]) / ds;
        slope[i] = (z[i1] - z[i0]) / ds;
    }

    Ok(pitgun_simulator::TrackConfig {
        id: track_id.to_string(),
        country_code: None,
        s_m: payload.s.clone(),
        x_m: payload.x.clone(),
        y_m: payload.y.clone(),
        z_m: z,
        curvature_radpm: curvature,
        slope,
        heading_rad: heading,
        pit_loss_ms,
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_request(era: i32) -> RunRaceRequest {
        RunRaceRequest {
            input: RunRaceInput {
                race: RaceInput {
                    track_id: "SPA".to_string(),
                    laps: 5,
                    competitors: vec![
                        CompetitorSpec {
                            id: "player".to_string(),
                            driver_id: Some("default".to_string()),
                            name: "Player".to_string(),
                            team_id: "PLAYER_TEAM".to_string(),
                            is_player: true,
                            tuning: TuningSpec {
                                engine_points: 10.0,
                                cooling_points: 8.0,
                                aero_points: 10.0,
                                chassis_points: 8.0,
                                downforce_slider: 0.55,
                                gear_ratio_slider: 0.50,
                            },
                            budget_cap: 36.0,
                            stint_strategy: None,
                        },
                        CompetitorSpec {
                            id: "ai_1".to_string(),
                            driver_id: Some("smooth_operator".to_string()),
                            name: "AI 1".to_string(),
                            team_id: "TEAM_1".to_string(),
                            is_player: false,
                            tuning: TuningSpec {
                                engine_points: 9.0,
                                cooling_points: 9.0,
                                aero_points: 9.0,
                                chassis_points: 9.0,
                                downforce_slider: 0.52,
                                gear_ratio_slider: 0.51,
                            },
                            budget_cap: 36.0,
                            stint_strategy: None,
                        },
                    ],
                },
                vehicle_id: Some("f1_2026".to_string()),
                pit_strategy: None,
                track_profile: None,
                competitor_profiles: HashMap::new(),
                era,
                hz: 20.0,
            },
            seed: 42,
            era: Some(era),
            hz: Some(20.0),
        }
    }

    #[test]
    fn requires_explicit_vehicle_id() {
        let mut request = sample_request(2026);
        request.input.vehicle_id = None;

        let err = run_race(request).expect_err("vehicle id should be required");
        assert!(err.contains("vehicle_id is required"));
    }

    #[test]
    fn race_simulation_returns_sorted_standings() {
        let output = run_race(sample_request(2026)).expect("race simulation should succeed");
        assert_eq!(output.standings.len(), 2);
        assert_eq!(output.standings[0].position, 1);
        assert_eq!(output.standings[1].position, 2);
        assert!(output.standings[0].total_time_ms <= output.standings[1].total_time_ms);
        assert_eq!(output.standings[0].laps_completed, 5);
        assert!(!output.player_batches.is_empty());
        assert!(
            output
                .player_batches
                .iter()
                .flat_map(|batch| batch.frames.iter())
                .flat_map(|frame| frame.samples.iter())
                .any(|sample| sample.parameter_id == PARAM_SPEED_KPH)
        );
    }

    #[test]
    fn player_telemetry_scales_with_stint_laps() {
        let mut one_lap = sample_request(2026);
        one_lap.input.race.laps = 1;
        let one_lap_output = run_race(one_lap).expect("one-lap simulation should succeed");
        let one_lap_speed_samples = one_lap_output
            .player_batches
            .iter()
            .flat_map(|batch| batch.frames.iter())
            .flat_map(|frame| frame.samples.iter())
            .filter(|sample| sample.parameter_id == PARAM_SPEED_KPH)
            .count();

        let mut five_laps = sample_request(2026);
        five_laps.input.race.laps = 5;
        let five_lap_output = run_race(five_laps).expect("five-lap simulation should succeed");
        let five_lap_speed_samples = five_lap_output
            .player_batches
            .iter()
            .flat_map(|batch| batch.frames.iter())
            .flat_map(|frame| frame.samples.iter())
            .filter(|sample| sample.parameter_id == PARAM_SPEED_KPH)
            .count();

        assert!(five_lap_speed_samples > one_lap_speed_samples * 3);
    }

    #[test]
    fn rejects_over_budget_input_via_policy_validation() {
        let mut request = sample_request(2026);
        request.input.race.competitors[0].budget_cap = 20.0;

        let err = run_race(request)
            .expect_err("race must be rejected when policy budget constraint fails");
        assert!(err.contains("invalid race input"));
    }

    #[test]
    fn uses_custom_track_profile_for_player_telemetry() {
        let mut request = sample_request(2026);
        request.input.track_profile = Some(SolverTrackProfile {
            s: vec![0.0, 100.0, 200.0, 300.0],
            x: vec![0.0, 100.0, 200.0, 300.0],
            y: vec![0.0, 0.0, 0.0, 0.0],
            z: vec![0.0, 0.0, 0.0, 0.0],
        });

        let output = run_race(request).expect("race should succeed with custom track");
        let y_values = output
            .player_batches
            .iter()
            .flat_map(|batch| batch.frames.iter())
            .flat_map(|frame| frame.samples.iter())
            .filter(|sample| sample.parameter_id == PARAM_Y_M)
            .filter_map(|sample| sample.value.as_f64())
            .collect::<Vec<_>>();

        assert!(!y_values.is_empty());
        let max_abs_y = y_values
            .iter()
            .fold(0.0_f64, |acc, value| acc.max(value.abs()));
        assert!(max_abs_y < 1e-6);
    }

    #[test]
    fn applies_player_pit_strategy_penalty_and_reports_laps() {
        let mut base = sample_request(2026);
        base.input.race.laps = 10;
        let base_output = run_race(base.clone()).expect("baseline race should succeed");
        let base_player = base_output
            .standings
            .iter()
            .find(|entry| entry.competitor_id == "player")
            .expect("player standing must exist")
            .total_time_ms;

        base.input.pit_strategy = Some(PitStrategyConfig {
            player_pit_laps: vec![5],
            pit_loss_ms: Some(22_000),
        });
        let pit_output = run_race(base).expect("pit strategy race should succeed");
        let pit_player = pit_output
            .standings
            .iter()
            .find(|entry| entry.competitor_id == "player")
            .expect("player standing must exist")
            .total_time_ms;

        assert_eq!(pit_output.player_pit_laps, vec![5]);
        assert_eq!(pit_output.player_lap_times_ms.len(), base_output.player_lap_times_ms.len());
        assert!(pit_output.player_lap_times_ms[4] >= base_output.player_lap_times_ms[4] + 20_000);
        assert!(pit_player > base_player);
    }

    #[test]
    fn applies_player_pit_strategy_in_fp_sessions() {
        let mut race = sample_request(2026).input.race;
        race.laps = 10;

        let baseline = run_single_session(
            &race,
            "f1_2026",
            None,
            None,
            20.0,
            race.laps,
            &HashMap::new(),
            42,
        )
        .expect("baseline fp session should succeed");

        let with_pit = run_single_session(
            &race,
            "f1_2026",
            Some(&PitStrategyConfig {
                player_pit_laps: vec![5],
                pit_loss_ms: Some(22_000),
            }),
            None,
            20.0,
            race.laps,
            &HashMap::new(),
            42,
        )
        .expect("fp session with pit strategy should succeed");

        let baseline_player = baseline
            .standings
            .iter()
            .find(|entry| entry.competitor_id == "player")
            .expect("player standing must exist")
            .total_time_ms;
        let with_pit_player = with_pit
            .standings
            .iter()
            .find(|entry| entry.competitor_id == "player")
            .expect("player standing must exist")
            .total_time_ms;

        assert_eq!(with_pit.player_pit_laps, vec![5]);
        assert_eq!(with_pit.player_lap_times_ms.len(), race.laps as usize);
        assert!(with_pit.player_lap_times_ms[4] >= baseline.player_lap_times_ms[4] + 20_000);
        assert!(with_pit_player > baseline_player);
    }

    #[test]
    fn run_sessions_honors_pit_strategy() {
        let mut race = sample_request(2026).input.race;
        race.laps = 10;

        let baseline = run_sessions(SessionRunRequest {
            race: race.clone(),
            vehicle_id: Some("f1_2026".to_string()),
            pit_strategy: None,
            track_profile: None,
            sessions: vec![SessionConfig {
                session: SessionKind::Race,
                laps: race.laps,
                profile_overrides: HashMap::new(),
            }],
            seed: 1,
            era: 2026,
            hz: 20.0,
        })
        .expect("baseline sessions should run");

        let with_pit = run_sessions(SessionRunRequest {
            race,
            vehicle_id: Some("f1_2026".to_string()),
            pit_strategy: Some(PitStrategyConfig {
                player_pit_laps: vec![5],
                pit_loss_ms: Some(22_000),
            }),
            track_profile: None,
            sessions: vec![SessionConfig {
                session: SessionKind::Race,
                laps: 10,
                profile_overrides: HashMap::new(),
            }],
            seed: 1,
            era: 2026,
            hz: 20.0,
        })
        .expect("pit sessions should run");

        let baseline_player = baseline.sessions[0]
            .standings
            .iter()
            .find(|entry| entry.competitor_id == "player")
            .expect("player standing must exist")
            .total_time_ms;
        let with_pit_player = with_pit.sessions[0]
            .standings
            .iter()
            .find(|entry| entry.competitor_id == "player")
            .expect("player standing must exist")
            .total_time_ms;

        assert!(with_pit_player > baseline_player);
    }

    #[test]
    fn rejects_empty_session_list() {
        let err = run_sessions(SessionRunRequest {
            race: sample_request(2026).input.race,
            vehicle_id: Some("f1_2026".to_string()),
            pit_strategy: None,
            track_profile: None,
            sessions: vec![],
            seed: 7,
            era: 2026,
            hz: 20.0,
        })
        .expect_err("sessions should be required");

        assert!(err.contains("sessions must be provided explicitly"));
    }

    #[test]
    fn explicit_stint_strategy_changes_tires_and_pit_laps() {
        let mut baseline_request = sample_request(2026);
        baseline_request.input.race.laps = 10;
        let mut request = baseline_request.clone();
        request.input.race.competitors[0].stint_strategy = Some(CompetitorStintStrategy {
            stints: vec![
                pitgun_contract::RaceStint {
                    tire_id: "hard".to_string(),
                    laps: 6,
                },
                pitgun_contract::RaceStint {
                    tire_id: "medium".to_string(),
                    laps: 4,
                },
            ],
            pit_laps: vec![6],
        });

        let baseline = run_race(baseline_request).expect("baseline race should succeed");
        let output = run_race(request).expect("stint strategy race should succeed");

        assert_eq!(output.player_pit_laps, vec![6]);

        let baseline_player = baseline
            .standings
            .iter()
            .find(|entry| entry.competitor_id == "player")
            .expect("baseline player standing must exist")
            .total_time_ms;
        let player = output
            .standings
            .iter()
            .find(|entry| entry.competitor_id == "player")
            .expect("player standing must exist")
            .total_time_ms;

        assert_ne!(player, baseline_player);
    }

    #[test]
    fn catalog_exports_include_circuits_and_engines() {
        let circuits = list_circuits().expect("circuit catalog");
        let engines = list_engines().expect("engine catalog");

        assert!(circuits.len() >= 4);
        assert!(circuits.iter().any(|entry| entry.id == "SPA"));
        assert!(circuits.iter().any(|entry| entry.id == "SPA" && entry.country_code.as_deref() == Some("BE")));
        assert!(engines.iter().any(|entry| entry.id == "v6t_hybrid"));

        let spa = get_circuit("spa").expect("spa circuit");
        let v6t = get_engine("v6t_hybrid").expect("hybrid engine");

        assert!(spa.s_m.len() > 100);
        assert_eq!(spa.country_code.as_deref(), Some("BE"));
        assert_eq!(v6t.gear_ratios.len(), 8);
    }
}
