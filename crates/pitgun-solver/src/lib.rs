use std::collections::HashMap;
use std::sync::Arc;

use pitgun_contract::{CompetitorSpec, RaceInput, TuningSpec};
use pitgun_policy::validation::normalize_and_validate_race_input;
use pitgun_simulator::{
    ConfigProvider, LapInput, LapOutput, SessionKind, Simulator, SimulatorState,
    Tuning as SimulatorTuning, default_in_memory_provider,
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
pub struct SessionTelemetryEvent {
    pub channel: String,
    pub ts_ns: u64,
    pub value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryBatch {
    pub events: Vec<SessionTelemetryEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryEnvelope {
    pub batch: TelemetryBatch,
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

const TELEMETRY_BATCH_SIZE: usize = 640;

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

fn telemetry_events_from_lap(
    output: &LapOutput,
    lap_idx: u16,
    time_offset_s: f64,
    distance_offset_m: f64,
    time_scale: f64,
) -> Vec<SessionTelemetryEvent> {
    let mut events = Vec::with_capacity(output.telemetry.len() * 17);

    for frame in &output.telemetry {
        let time_s = time_offset_s + frame.time_s * time_scale;
        let ts_ns = (time_s * 1_000_000_000.0).round() as u64;

        let channels = [
            ("sim.time_s", time_s),
            ("sim.s_m", frame.s_m + distance_offset_m),
            ("sim.x_m", frame.x_m),
            ("sim.y_m", frame.y_m),
            ("sim.heading_rad", frame.heading_rad),
            ("sim.speed_kph", frame.speed_kph),
            ("sim.rpm", frame.rpm),
            ("sim.gear", frame.gear as f64),
            ("sim.throttle_pct", frame.throttle_pct),
            ("sim.brake_pct", frame.brake_pct),
            ("sim.g_lat", frame.g_lat),
            ("sim.g_long", frame.g_long),
            ("sim.g_vert", frame.g_vert),
            ("sim.engine_temp_c", frame.engine_temp_c),
            ("sim.engine_power_w", frame.engine_power_w),
            ("sim.tire_temp_c", frame.tire_temp_c),
            ("sim.tire_wear_pct", frame.tire_wear_pct),
        ];

        for (channel, value) in channels {
            events.push(SessionTelemetryEvent {
                channel: channel.to_string(),
                ts_ns,
                value,
            });
        }
    }

    if lap_idx > 1 {
        events.retain(|event| event.ts_ns > 0);
    }

    events
}

fn telemetry_batches(events: Vec<SessionTelemetryEvent>) -> Vec<TelemetryEnvelope> {
    events
        .chunks(TELEMETRY_BATCH_SIZE)
        .map(|chunk| TelemetryEnvelope {
            batch: TelemetryBatch {
                events: chunk.to_vec(),
            },
        })
        .collect()
}

fn simulate_competitor_session(
    simulator: &Simulator,
    competitor: &CompetitorSpec,
    vehicle_id: &str,
    track_id: &str,
    profile_id: Option<&str>,
    laps: u16,
    hz: f64,
    pit_laps: &[u16],
    pit_loss_ms: u64,
) -> Result<(SimulatedCompetitor, Vec<SessionTelemetryEvent>), String> {
    let laps = laps.max(1);
    let mut total_time_ms = 0u64;
    let mut best_lap_ms = u64::MAX;
    let mut lap_times_ms = Vec::with_capacity(laps as usize);

    let mut state: Option<SimulatorState> = None;
    let mut telemetry_events = Vec::new();
    let mut time_offset_s = 0.0;
    let mut distance_offset_m = 0.0;

    for lap_idx in 0..laps {
        let lap_output = simulator
            .simulate_lap(LapInput {
                vehicle_id: vehicle_id.to_string(),
                track_id: track_id.to_string(),
                tuning: simulator_tuning(&competitor.tuning),
                profile_id: profile_id.map(str::to_string),
                profile: None,
                initial_state: state.clone(),
                hz,
            })
            .map_err(|err| format!("simulation failed for competitor {}: {err}", competitor.id))?;

        let base_lap_ms = (lap_output.lap_time_s * 1000.0).round().max(1.0) as u64;
        let lap_number = lap_idx.saturating_add(1);
        let pit_penalty_ms = if pit_laps.binary_search(&lap_number).is_ok() {
            pit_loss_ms
        } else {
            0
        };
        let lap_time_ms = base_lap_ms.saturating_add(pit_penalty_ms);
        lap_times_ms.push(lap_time_ms);

        if competitor.is_player || competitor.id == "player" {
            let scale = lap_time_ms as f64 / (base_lap_ms as f64).max(1.0);
            telemetry_events.extend(telemetry_events_from_lap(
                &lap_output,
                lap_idx + 1,
                time_offset_s,
                distance_offset_m,
                scale,
            ));
        }

        total_time_ms = total_time_ms.saturating_add(lap_time_ms);
        best_lap_ms = best_lap_ms.min(lap_time_ms);
        time_offset_s += lap_time_ms as f64 / 1000.0;
        distance_offset_m += lap_output.telemetry.last().map(|frame| frame.s_m).unwrap_or(0.0);
        state = Some(lap_output.final_state);
    }

    Ok((
        SimulatedCompetitor {
            competitor_id: competitor.id.clone(),
            total_time_ms,
            best_lap_ms,
            laps_completed: laps,
            lap_times_ms,
        },
        telemetry_events,
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

    let simulator = Simulator::new(Arc::new(provider));
    let mut rows = Vec::with_capacity(race.competitors.len());
    let mut player_events = Vec::new();
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
        let competitor_pit_laps = if competitor.is_player || competitor.id == "player" {
            configured_player_pit_laps.clone()
        } else {
            Vec::new()
        };

        let (simulated, events) = simulate_competitor_session(
            &simulator,
            competitor,
            vehicle_id,
            &track_id,
            profile_overrides.get(&competitor.id).map(String::as_str),
            laps,
            hz,
            competitor_pit_laps.as_slice(),
            pit_loss_ms,
        )?;

        if competitor.is_player || competitor.id == "player" {
            player_lap_times_ms = simulated.lap_times_ms.clone();
            player_events = events;
            player_pit_laps = competitor_pit_laps;
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
        player_batches: telemetry_batches(player_events),
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
                        },
                        CompetitorSpec {
                            id: "ai_1".to_string(),
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
                .flat_map(|batch| batch.batch.events.iter())
                .any(|event| event.channel == "sim.speed_kph")
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
            .flat_map(|batch| batch.batch.events.iter())
            .filter(|event| event.channel == "sim.speed_kph")
            .count();

        let mut five_laps = sample_request(2026);
        five_laps.input.race.laps = 5;
        let five_lap_output = run_race(five_laps).expect("five-lap simulation should succeed");
        let five_lap_speed_samples = five_lap_output
            .player_batches
            .iter()
            .flat_map(|batch| batch.batch.events.iter())
            .filter(|event| event.channel == "sim.speed_kph")
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
            .flat_map(|batch| batch.batch.events.iter())
            .filter(|event| event.channel == "sim.y_m")
            .map(|event| event.value)
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
        assert!(pit_player >= base_player + 20_000);
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
        assert!(with_pit_player >= baseline_player + 20_000);
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

        assert!(with_pit_player >= baseline_player + 20_000);
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
}
