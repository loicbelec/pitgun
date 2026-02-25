use std::collections::HashMap;
use std::sync::Arc;

use pitgun_contract::{CompetitorSpec, RaceInput, TuningSpec, VehicleClass, resolve_vehicle_class};
use pitgun_policy::validation::normalize_and_validate_race_input;
use pitgun_simulator::{
    CompetitorProfile, ConfigProvider, LapInput, LapOutput, SessionKind, Simulator, SimulatorState,
    Tuning as SimulatorTuning, default_in_memory_provider,
};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunRaceInput {
    #[serde(flatten)]
    pub race: RaceInput,
    #[serde(default)]
    pub track_profile: Option<SolverTrackProfile>,
    #[serde(default)]
    pub era: i32,
    #[serde(default)]
    pub hz: f64,
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
}

#[derive(Debug, Clone)]
struct Candidate {
    tuning: TuningSpec,
    time_ms: u64,
}

#[derive(Debug, Clone, Copy)]
struct Shares {
    engine: f64,
    cooling: f64,
    aero: f64,
    chassis: f64,
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

fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9e3779b97f4a7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
    x ^ (x >> 31)
}

fn competitor_hash(id: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for &byte in id.as_bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn unit_noise(seed: u64, competitor_id: &str, lap_idx: u64, salt: u64) -> f64 {
    let h = splitmix64(
        seed ^ competitor_hash(competitor_id) ^ lap_idx.wrapping_mul(0x9e3779b97f4a7c15) ^ salt,
    );
    (h as f64) / (u64::MAX as f64)
}

fn resolve_hz(request: &RunRaceRequest) -> f64 {
    let input_hz = request.input.hz;
    request
        .hz
        .unwrap_or(if input_hz > 0.0 { input_hz } else { 20.0 })
}

fn resolve_era(request: &RunRaceRequest) -> i32 {
    request.era.unwrap_or(request.input.era)
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

fn resolve_vehicle_id(era: i32) -> &'static str {
    match resolve_vehicle_class(era) {
        VehicleClass::Legacy1960 => "classic_v8_1960",
        VehicleClass::GroundEffect1970 => "classic_v8_1970",
        VehicleClass::HybridModern => "modern_v6t",
        VehicleClass::ActiveAero2026 => "f1_2026",
    }
}

fn profile_for_competitor(
    competitor: &CompetitorSpec,
    session: SessionKind,
    override_id: Option<&str>,
) -> String {
    if let Some(id) = override_id {
        return id.to_string();
    }
    if competitor.is_player || competitor.id == "player" {
        return match session {
            SessionKind::Fp1 => "balanced".to_string(),
            SessionKind::Fp2 => "balanced".to_string(),
            SessionKind::Fp3 => "aggressive".to_string(),
            SessionKind::Race => "balanced".to_string(),
        };
    }

    let bucket = (competitor_hash(&competitor.id) % 3) as usize;
    match (session, bucket) {
        (SessionKind::Fp1, 0) => "conservative".to_string(),
        (SessionKind::Fp1, 1) => "balanced".to_string(),
        (SessionKind::Fp1, _) => "aggressive".to_string(),
        (SessionKind::Fp2, 0) => "balanced".to_string(),
        (SessionKind::Fp2, 1) => "conservative".to_string(),
        (SessionKind::Fp2, _) => "aggressive".to_string(),
        (SessionKind::Fp3, 0) => "aggressive".to_string(),
        (SessionKind::Fp3, 1) => "balanced".to_string(),
        (SessionKind::Fp3, _) => "conservative".to_string(),
        (SessionKind::Race, 0) => "balanced".to_string(),
        (SessionKind::Race, 1) => "conservative".to_string(),
        (SessionKind::Race, _) => "aggressive".to_string(),
    }
}

fn shares_from_tuning(tuning: &TuningSpec, budget: f64) -> Shares {
    if budget <= 1e-9 {
        return Shares {
            engine: 0.25,
            cooling: 0.25,
            aero: 0.25,
            chassis: 0.25,
        };
    }

    normalize_shares([
        tuning.engine_points / budget,
        tuning.cooling_points / budget,
        tuning.aero_points / budget,
        tuning.chassis_points / budget,
    ])
}

fn normalize_shares(values: [f64; 4]) -> Shares {
    let mut cleaned = values.map(|v| v.max(0.0));
    let sum = cleaned.iter().sum::<f64>();
    if sum <= 1e-9 {
        cleaned = [0.25; 4];
    }
    let sum = cleaned.iter().sum::<f64>().max(1e-9);
    Shares {
        engine: cleaned[0] / sum,
        cooling: cleaned[1] / sum,
        aero: cleaned[2] / sum,
        chassis: cleaned[3] / sum,
    }
}

fn to_tuning(shares: Shares, budget: f64, downforce: f64, gear_ratio: f64) -> TuningSpec {
    let safe_budget = budget.max(0.0);
    TuningSpec {
        engine_points: shares.engine * safe_budget,
        cooling_points: shares.cooling * safe_budget,
        aero_points: shares.aero * safe_budget,
        chassis_points: shares.chassis * safe_budget,
        downforce_slider: clamp(downforce, 0.0, 1.0),
        gear_ratio_slider: clamp(gear_ratio, 0.0, 1.0),
    }
}

fn sample_candidate(rng: &mut StdRng, budget: f64) -> TuningSpec {
    let shares = normalize_shares([
        rng.gen_range(0.0..1.0),
        rng.gen_range(0.0..1.0),
        rng.gen_range(0.0..1.0),
        rng.gen_range(0.0..1.0),
    ]);

    to_tuning(
        shares,
        budget,
        rng.gen_range(0.0..1.0),
        rng.gen_range(0.0..1.0),
    )
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
        events.retain(|e| e.ts_ns > 0);
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

fn default_weekend_sessions(race_laps: u16) -> Vec<SessionConfig> {
    vec![
        SessionConfig {
            session: SessionKind::Fp1,
            laps: 8,
            profile_overrides: HashMap::new(),
        },
        SessionConfig {
            session: SessionKind::Fp2,
            laps: 8,
            profile_overrides: HashMap::new(),
        },
        SessionConfig {
            session: SessionKind::Fp3,
            laps: 8,
            profile_overrides: HashMap::new(),
        },
        SessionConfig {
            session: SessionKind::Race,
            laps: race_laps.max(1),
            profile_overrides: HashMap::new(),
        },
    ]
}

fn simulate_competitor_session(
    simulator: &Simulator,
    competitor: &CompetitorSpec,
    vehicle_id: &str,
    track_id: &str,
    profile: CompetitorProfile,
    laps: u16,
    hz: f64,
    seed: u64,
    session: SessionKind,
) -> Result<(SimulatedCompetitor, Vec<SessionTelemetryEvent>), String> {
    let laps = laps.max(1);
    let mut total_time_ms = 0u64;
    let mut best_lap_ms = u64::MAX;

    let mut state: Option<SimulatorState> = None;
    let mut telemetry_events = Vec::new();
    let mut time_offset_s = 0.0;
    let mut distance_offset_m = 0.0;

    let shares = shares_from_tuning(&competitor.tuning, competitor.budget_cap.max(1.0));
    let thermal_stress = (shares.engine - shares.cooling).max(0.0);

    for lap_idx in 0..laps {
        let lap_output = simulator
            .simulate_lap(LapInput {
                vehicle_id: vehicle_id.to_string(),
                track_id: track_id.to_string(),
                tuning: simulator_tuning(&competitor.tuning),
                profile_id: None,
                profile: Some(profile.clone().for_session(session)),
                initial_state: state.clone(),
                hz,
            })
            .map_err(|err| format!("simulation failed for competitor {}: {err}", competitor.id))?;

        let lap_time_ms = (lap_output.lap_time_s * 1000.0).round().max(1.0) as u64;
        let session_scale = match session {
            SessionKind::Fp1 => 1.01,
            SessionKind::Fp2 => 1.005,
            SessionKind::Fp3 => 0.998,
            SessionKind::Race => 1.0,
        };
        let fade = 1.0 + thermal_stress * 0.0022 * lap_idx as f64;
        let jitter_unit = unit_noise(seed, &competitor.id, lap_idx as u64, 0xABCDEF01) - 0.5;
        let jitter_ms = jitter_unit * 2.0 * profile.pace_variance_ms;
        let adjusted_lap_ms = ((lap_time_ms as f64) * fade * session_scale + jitter_ms)
            .max(1.0)
            .round() as u64;

        if competitor.is_player || competitor.id == "player" {
            let sim_lap_ms = (lap_output.lap_time_s * 1000.0).max(1.0);
            let scale = adjusted_lap_ms as f64 / sim_lap_ms;
            telemetry_events.extend(telemetry_events_from_lap(
                &lap_output,
                lap_idx + 1,
                time_offset_s,
                distance_offset_m,
                scale,
            ));
        }

        total_time_ms = total_time_ms.saturating_add(adjusted_lap_ms);
        best_lap_ms = best_lap_ms.min(adjusted_lap_ms);

        time_offset_s += adjusted_lap_ms as f64 / 1000.0;
        distance_offset_m += lap_output.telemetry.last().map(|f| f.s_m).unwrap_or(0.0);
        state = Some(lap_output.final_state);
    }

    Ok((
        SimulatedCompetitor {
            competitor_id: competitor.id.clone(),
            total_time_ms,
            best_lap_ms,
            laps_completed: laps,
        },
        telemetry_events,
    ))
}

fn run_single_session(
    race: &RaceInput,
    track_profile: Option<&SolverTrackProfile>,
    era: i32,
    hz: f64,
    seed: u64,
    session: SessionKind,
    laps: u16,
    profile_overrides: &HashMap<String, String>,
) -> Result<RaceOutput, String> {
    let mut provider = default_in_memory_provider();
    let track_id = normalize_track_id(&race.track_id);

    if let Some(payload) = track_profile
        && let Some(track) = track_from_payload(&track_id, payload)
    {
        provider.insert_track(track);
    }

    let simulator = Simulator::new(Arc::new(provider.clone()));
    let vehicle_id = resolve_vehicle_id(era);

    let mut rows = Vec::with_capacity(race.competitors.len());
    let mut player_events = Vec::new();

    for competitor in &race.competitors {
        let profile_id = profile_for_competitor(
            competitor,
            session,
            profile_overrides.get(&competitor.id).map(String::as_str),
        );

        let profile = provider
            .get_profile(&profile_id)
            .unwrap_or_else(|_| CompetitorProfile::default());

        let (simulated, events) = simulate_competitor_session(
            &simulator,
            competitor,
            vehicle_id,
            &track_id,
            profile,
            laps,
            hz.max(1.0),
            seed,
            session,
        )?;

        if competitor.is_player || competitor.id == "player" {
            player_events = events;
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
        player_batches: telemetry_batches(player_events),
    })
}

pub fn run_race(request: RunRaceRequest) -> Result<RaceOutput, String> {
    if request.input.race.competitors.is_empty() {
        return Err("race requires at least one competitor".to_string());
    }

    let era = resolve_era(&request);
    let normalized_race = normalize_and_validate_race_input(
        &request.input.race,
        if era > 0 { era as u32 } else { 0 },
    )
    .map_err(|err| format!("invalid race input: {err}"))?;

    run_single_session(
        &normalized_race,
        request.input.track_profile.as_ref(),
        era,
        resolve_hz(&request),
        request.seed,
        SessionKind::Race,
        normalized_race.laps,
        &HashMap::new(),
    )
}

pub fn run_sessions(request: SessionRunRequest) -> Result<SessionRunOutput, String> {
    let normalized_race = normalize_and_validate_race_input(
        &request.race,
        if request.era > 0 {
            request.era as u32
        } else {
            0
        },
    )
    .map_err(|err| format!("invalid race input: {err}"))?;

    let sessions = if request.sessions.is_empty() {
        default_weekend_sessions(normalized_race.laps)
    } else {
        request.sessions.clone()
    };

    let mut out = Vec::with_capacity(sessions.len());
    for (idx, session_cfg) in sessions.iter().enumerate() {
        let session_seed = request
            .seed
            .wrapping_add((idx as u64).wrapping_mul(0x9e3779b97f4a7c15));

        let race_output = run_single_session(
            &normalized_race,
            request.track_profile.as_ref(),
            request.era,
            if request.hz > 0.0 { request.hz } else { 20.0 },
            session_seed,
            session_cfg.session,
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

fn evaluate_candidate(request: &SolverRequest, tuning: &TuningSpec, run_seed: u64) -> Option<u64> {
    let competitor = CompetitorSpec {
        id: "solver".to_string(),
        name: "solver".to_string(),
        team_id: "solver".to_string(),
        is_player: true,
        tuning: tuning.clone(),
        budget_cap: request.budget.max(0.0),
    };

    let race = RaceInput {
        track_id: request.track_id.clone(),
        laps: request.laps.max(1),
        competitors: vec![competitor],
    };

    let output = run_single_session(
        &race,
        request.track_profile.as_ref(),
        request.era,
        if request.hz > 0.0 { request.hz } else { 20.0 },
        run_seed,
        SessionKind::Race,
        request.laps.max(1),
        &HashMap::new(),
    )
    .ok()?;

    output.standings.first().map(|row| row.total_time_ms)
}

fn tuning_shares(t: &TuningSpec, budget: f64) -> Shares {
    shares_from_tuning(t, budget)
}

fn build_baseline(candidates: &[Candidate], budget: f64, seed: u64) -> (TuningSpec, TuningSpec) {
    let best = candidates
        .first()
        .map(|c| c.tuning.clone())
        .unwrap_or_default();

    let elite_count = candidates.len().max(1).clamp(6, 24);
    let elite = &candidates[..elite_count.min(candidates.len())];

    let mut acc_engine = 0.0;
    let mut acc_cooling = 0.0;
    let mut acc_aero = 0.0;
    let mut acc_chassis = 0.0;
    let mut acc_downforce = 0.0;
    let mut acc_gear = 0.0;

    for candidate in elite {
        let shares = tuning_shares(&candidate.tuning, budget);
        acc_engine += shares.engine;
        acc_cooling += shares.cooling;
        acc_aero += shares.aero;
        acc_chassis += shares.chassis;
        acc_downforce += candidate.tuning.downforce_slider;
        acc_gear += candidate.tuning.gear_ratio_slider;
    }

    let div = elite.len().max(1) as f64;
    let elite_shares = normalize_shares([
        acc_engine / div,
        acc_cooling / div,
        acc_aero / div,
        acc_chassis / div,
    ]);

    let neutral = Shares {
        engine: 0.25,
        cooling: 0.25,
        aero: 0.25,
        chassis: 0.25,
    };

    let blend = 0.74;
    let mixed = normalize_shares([
        elite_shares.engine * blend + neutral.engine * (1.0 - blend),
        elite_shares.cooling * blend + neutral.cooling * (1.0 - blend),
        elite_shares.aero * blend + neutral.aero * (1.0 - blend),
        elite_shares.chassis * blend + neutral.chassis * (1.0 - blend),
    ]);

    let jitter = |salt: u64, amp: f64| -> f64 {
        let value = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407)
            .wrapping_add(salt)
            >> 12;
        (((value & 1023) as f64) / 1023.0 - 0.5) * 2.0 * amp
    };

    let baseline_downforce = clamp(acc_downforce / div + jitter(11, 0.05), 0.0, 1.0);
    let baseline_gear = clamp(acc_gear / div + jitter(29, 0.05), 0.0, 1.0);
    let baseline = to_tuning(mixed, budget, baseline_downforce, baseline_gear);

    (baseline, best)
}

pub fn solve_baseline(request: SolverRequest) -> Option<SolverResponse> {
    let budget = request.budget.max(0.0);
    let runs = request.runs.clamp(24, 2048);

    let mut rng = StdRng::seed_from_u64(request.seed);
    let mut candidates = Vec::with_capacity(runs);

    for i in 0..runs {
        let tuning = sample_candidate(&mut rng, budget);
        let sim_seed = request
            .seed
            .wrapping_add((i as u64).wrapping_mul(0x9e3779b97f4a7c15));

        if let Some(time_ms) = evaluate_candidate(&request, &tuning, sim_seed) {
            candidates.push(Candidate { tuning, time_ms });
        }
    }

    if candidates.is_empty() {
        return None;
    }

    candidates.sort_by_key(|c| c.time_ms);

    let (baseline, top_reference) =
        build_baseline(&candidates, budget, request.seed ^ 0xa5a5_5a5a_1234_4321);

    let baseline_time_ms =
        evaluate_candidate(&request, &baseline, request.seed ^ 0x00ff_00ff_00ff_00ff)
            .unwrap_or(candidates[0].time_ms);

    Some(SolverResponse {
        baseline,
        top_reference,
        baseline_time_ms,
        top_time_ms: candidates[0].time_ms,
        runs_used: candidates.len(),
    })
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
            "error": "solver failed to produce a baseline"
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
) -> Option<pitgun_simulator::TrackConfig> {
    let n = payload.s.len();
    if n < 3 || payload.x.len() != n || payload.y.len() != n {
        return None;
    }
    if !payload.s.windows(2).all(|w| w[1] > w[0]) {
        return None;
    }
    if payload
        .s
        .iter()
        .chain(payload.x.iter())
        .chain(payload.y.iter())
        .any(|v| !v.is_finite())
    {
        return None;
    }

    let z = if payload.z.len() == n {
        payload.z.clone()
    } else {
        vec![0.0; n]
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

    Some(pitgun_simulator::TrackConfig {
        id: track_id.to_string(),
        s_m: payload.s.clone(),
        x_m: payload.x.clone(),
        y_m: payload.y.clone(),
        z_m: z,
        curvature_radpm: curvature,
        slope,
        heading_rad: heading,
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
                track_profile: None,
                era,
                hz: 20.0,
            },
            seed: 42,
            era: Some(era),
            hz: Some(20.0),
        }
    }

    #[test]
    fn selects_vehicle_preset_by_game_era() {
        assert_eq!(resolve_vehicle_class(1), VehicleClass::Legacy1960);
        assert_eq!(resolve_vehicle_class(2), VehicleClass::Legacy1960);
        assert_eq!(resolve_vehicle_class(3), VehicleClass::GroundEffect1970);
        assert_eq!(resolve_vehicle_class(4), VehicleClass::GroundEffect1970);
        assert_eq!(resolve_vehicle_class(5), VehicleClass::HybridModern);
        assert_eq!(resolve_vehicle_class(6), VehicleClass::ActiveAero2026);
        assert_eq!(resolve_vehicle_class(7), VehicleClass::ActiveAero2026);
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
        let mut request = sample_request(1);
        request.input.track_profile = Some(SolverTrackProfile {
            s: vec![0.0, 100.0, 200.0, 300.0],
            x: vec![0.0, 100.0, 200.0, 300.0],
            y: vec![0.0, 0.0, 0.0, 0.0],
            z: vec![0.0, 0.0, 0.0, 0.0],
        });

        let output = run_race(request).expect("race should succeed with custom track");
        let y_values: Vec<f64> = output
            .player_batches
            .iter()
            .flat_map(|batch| batch.batch.events.iter())
            .filter(|event| event.channel == "sim.y_m")
            .map(|event| event.value)
            .collect();

        assert!(!y_values.is_empty());
        let max_abs_y = y_values
            .iter()
            .fold(0.0_f64, |acc, value| acc.max(value.abs()));
        assert!(max_abs_y < 1e-6);
    }

    #[test]
    fn runs_standard_weekend_for_player_plus_nine_ai() {
        let mut competitors = Vec::new();
        competitors.push(CompetitorSpec {
            id: "player".to_string(),
            name: "Player".to_string(),
            team_id: "PLAYER".to_string(),
            is_player: true,
            tuning: TuningSpec::default(),
            budget_cap: 100.0,
        });

        for i in 0..9 {
            competitors.push(CompetitorSpec {
                id: format!("ai_{i}"),
                name: format!("AI {i}"),
                team_id: format!("T{i}"),
                is_player: false,
                tuning: TuningSpec::default(),
                budget_cap: 100.0,
            });
        }

        let output = run_sessions(SessionRunRequest {
            race: RaceInput {
                track_id: "MONZA".to_string(),
                laps: 12,
                competitors,
            },
            track_profile: None,
            sessions: vec![],
            seed: 7,
            era: 2026,
            hz: 20.0,
        })
        .expect("sessions should run");

        assert_eq!(output.sessions.len(), 4);
        for session in output.sessions {
            assert_eq!(session.standings.len(), 10);
            assert!(session.total_time_ms > 0);
        }
    }
}
