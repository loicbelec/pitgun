use pitgun_contract::{CompetitorSpec, RaceInput, TuningSpec, VehicleClass, resolve_vehicle_class};
use pitgun_engine_f1::components::aero::{ActiveAero, Aero, NoAero};
use pitgun_engine_f1::components::chassis::StandardChassis;
use pitgun_engine_f1::components::engine::{V6THybridEngine, V81960Engine, V81970Engine};
use pitgun_engine_f1::core::Tuning;
use pitgun_engine_f1::sim::{
    SimConfig, SimulationOutput, TrackProfile, run_simulation_with_tuning,
};
use pitgun_engine_f1::vehicle::Vehicle;
use pitgun_policy::validation::normalize_and_validate_race_input;
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

#[derive(Debug, Clone)]
struct Candidate {
    tuning: TuningSpec,
    time_ms: u64,
}

#[derive(Debug, Clone)]
struct SimulatedCompetitor {
    competitor_id: String,
    total_time_ms: u64,
    best_lap_ms: u64,
    laps_completed: u16,
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

fn normalize_track_id(track_id: &str) -> String {
    track_id
        .chars()
        .filter(|ch| !matches!(ch, '-' | '_' | ' '))
        .flat_map(char::to_uppercase)
        .collect()
}

#[derive(Debug, Clone, Copy)]
struct TrackTemplate {
    distance_m: f64,
    radius_x: f64,
    radius_y: f64,
    wobble_x: f64,
    wobble_y: f64,
    slope_amp_m: f64,
}

fn track_template(track_id: &str) -> TrackTemplate {
    match normalize_track_id(track_id).as_str() {
        "SPA" => TrackTemplate {
            distance_m: 7004.0,
            radius_x: 930.0,
            radius_y: 600.0,
            wobble_x: 260.0,
            wobble_y: 180.0,
            slope_amp_m: 9.0,
        },
        "MONZA" => TrackTemplate {
            distance_m: 5793.0,
            radius_x: 980.0,
            radius_y: 430.0,
            wobble_x: 120.0,
            wobble_y: 95.0,
            slope_amp_m: 2.0,
        },
        "SUZUKA" => TrackTemplate {
            distance_m: 5807.0,
            radius_x: 760.0,
            radius_y: 620.0,
            wobble_x: 290.0,
            wobble_y: 250.0,
            slope_amp_m: 5.0,
        },
        "MONTECARLO" | "MONACO" => TrackTemplate {
            distance_m: 3337.0,
            radius_x: 500.0,
            radius_y: 390.0,
            wobble_x: 170.0,
            wobble_y: 120.0,
            slope_amp_m: 6.0,
        },
        _ => TrackTemplate {
            distance_m: 5200.0,
            radius_x: 760.0,
            radius_y: 520.0,
            wobble_x: 180.0,
            wobble_y: 130.0,
            slope_amp_m: 4.0,
        },
    }
}

fn unwrap_angle(mut value: f64, reference: f64) -> f64 {
    while value - reference > std::f64::consts::PI {
        value -= 2.0 * std::f64::consts::PI;
    }
    while value - reference < -std::f64::consts::PI {
        value += 2.0 * std::f64::consts::PI;
    }
    value
}

fn build_track_profile(track_id: &str) -> TrackProfile {
    let tpl = track_template(track_id);
    let points = 420usize;
    let mut s = Vec::with_capacity(points);
    let mut x = Vec::with_capacity(points);
    let mut y = Vec::with_capacity(points);
    let mut z = Vec::with_capacity(points);

    for i in 0..points {
        let t = i as f64 / (points - 1) as f64;
        let theta = t * std::f64::consts::TAU;
        s.push(t * tpl.distance_m);
        x.push(
            tpl.radius_x * theta.cos()
                + tpl.wobble_x * (2.6 * theta).cos() * 0.55
                + tpl.wobble_x * (4.2 * theta).sin() * 0.15,
        );
        y.push(
            tpl.radius_y * theta.sin()
                + tpl.wobble_y * (1.8 * theta).sin() * 0.60
                + tpl.wobble_y * (3.3 * theta).cos() * 0.20,
        );
        z.push(
            tpl.slope_amp_m * (1.7 * theta).sin() * 0.5
                + tpl.slope_amp_m * (0.4 * theta).cos() * 0.2,
        );
    }

    let n = s.len();
    let mut heading = vec![0.0; n];
    for i in 0..n {
        let i0 = i.saturating_sub(1);
        let i1 = (i + 1).min(n - 1);
        let dx = x[i1] - x[i0];
        let dy = y[i1] - y[i0];
        heading[i] = dy.atan2(dx);
    }

    for i in 1..n {
        heading[i] = unwrap_angle(heading[i], heading[i - 1]);
    }

    let mut kappa = vec![0.0; n];
    let mut slope = vec![0.0; n];
    for i in 0..n {
        let i0 = i.saturating_sub(1);
        let i1 = (i + 1).min(n - 1);
        let ds = (s[i1] - s[i0]).max(1e-6);
        kappa[i] = (heading[i1] - heading[i0]) / ds;
        slope[i] = (z[i1] - z[i0]) / ds;
    }

    TrackProfile {
        s,
        x,
        y,
        z,
        kappa,
        slope,
        heading,
    }
}

fn is_strictly_increasing(values: &[f64]) -> bool {
    values.windows(2).all(|pair| pair[1] > pair[0])
}

fn track_profile_from_payload(payload: &SolverTrackProfile) -> Option<TrackProfile> {
    let n = payload.s.len();
    if n < 3 || payload.x.len() != n || payload.y.len() != n {
        return None;
    }
    if !is_strictly_increasing(&payload.s) {
        return None;
    }
    if payload
        .s
        .iter()
        .chain(payload.x.iter())
        .chain(payload.y.iter())
        .any(|value| !value.is_finite())
    {
        return None;
    }

    let z = if payload.z.len() == n {
        if payload.z.iter().any(|value| !value.is_finite()) {
            return None;
        }
        payload.z.clone()
    } else {
        vec![0.0; n]
    };

    let mut heading = vec![0.0; n];
    for (i, heading_i) in heading.iter_mut().enumerate().take(n) {
        let i0 = i.saturating_sub(1);
        let i1 = (i + 1).min(n - 1);
        let dx = payload.x[i1] - payload.x[i0];
        let dy = payload.y[i1] - payload.y[i0];
        *heading_i = dy.atan2(dx);
    }
    for i in 1..n {
        heading[i] = unwrap_angle(heading[i], heading[i - 1]);
    }

    let mut kappa = vec![0.0; n];
    let mut slope = vec![0.0; n];
    for i in 0..n {
        let i0 = i.saturating_sub(1);
        let i1 = (i + 1).min(n - 1);
        let ds = (payload.s[i1] - payload.s[i0]).max(1e-6);
        kappa[i] = (heading[i1] - heading[i0]) / ds;
        slope[i] = (z[i1] - z[i0]) / ds;
    }

    Some(TrackProfile {
        s: payload.s.clone(),
        x: payload.x.clone(),
        y: payload.y.clone(),
        z,
        kappa,
        slope,
        heading,
    })
}

fn resolve_track_profile(track_id: &str, payload: Option<&SolverTrackProfile>) -> TrackProfile {
    payload
        .and_then(track_profile_from_payload)
        .unwrap_or_else(|| build_track_profile(track_id))
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

fn tuning_shares(t: &TuningSpec, budget: f64) -> Shares {
    if budget <= 1e-9 {
        return Shares {
            engine: 0.25,
            cooling: 0.25,
            aero: 0.25,
            chassis: 0.25,
        };
    }

    normalize_shares([
        t.engine_points / budget,
        t.cooling_points / budget,
        t.aero_points / budget,
        t.chassis_points / budget,
    ])
}

fn to_core_tuning(spec: &TuningSpec) -> Tuning {
    Tuning {
        engine_points: spec.engine_points.max(0.0),
        cooling_points: spec.cooling_points.max(0.0),
        aero_points: spec.aero_points.max(0.0),
        chassis_points: spec.chassis_points.max(0.0),
        downforce_slider: clamp(spec.downforce_slider, 0.0, 1.0),
        gear_ratio_slider: clamp(spec.gear_ratio_slider, 0.0, 1.0),
    }
}

fn run_simulation_with_preset(
    track: &TrackProfile,
    tuning: &TuningSpec,
    sim_lap_number: usize,
    hz: f64,
    preset: VehicleClass,
) -> Option<SimulationOutput> {
    let core_tuning = to_core_tuning(tuning);
    let config = SimConfig {
        lap_number: sim_lap_number.max(1),
        hz: hz.max(1.0),
    };

    match preset {
        VehicleClass::Legacy1960 => {
            let mut vehicle = Vehicle::new(
                NoAero::new(),
                StandardChassis::new(),
                V81960Engine::default(),
            );
            run_simulation_with_tuning(track, &mut vehicle, &core_tuning, &config).ok()
        }
        VehicleClass::GroundEffect1970 => {
            let mut vehicle =
                Vehicle::new(Aero::new(), StandardChassis::new(), V81970Engine::default());
            run_simulation_with_tuning(track, &mut vehicle, &core_tuning, &config).ok()
        }
        VehicleClass::HybridModern => {
            let mut vehicle = Vehicle::new(
                Aero::new(),
                StandardChassis::new(),
                V6THybridEngine::default(),
            );
            run_simulation_with_tuning(track, &mut vehicle, &core_tuning, &config).ok()
        }
        VehicleClass::ActiveAero2026 => {
            let mut vehicle = Vehicle::new(
                ActiveAero::new(),
                StandardChassis::new(),
                V6THybridEngine::default(),
            );
            run_simulation_with_tuning(track, &mut vehicle, &core_tuning, &config).ok()
        }
    }
}

fn simulate_lap_time_s_with_preset(
    track: &TrackProfile,
    tuning: &TuningSpec,
    sim_lap_number: usize,
    hz: f64,
    preset: VehicleClass,
) -> Option<f64> {
    let output = run_simulation_with_preset(track, tuning, sim_lap_number, hz, preset)?;
    let lap_time_s = output.solution.t.last().copied()?;

    Some(lap_time_s.max(0.001))
}

fn telemetry_batches_from_simulation(output: &SimulationOutput, laps: usize) -> Vec<TelemetryEnvelope> {
    let telemetry = &output.telemetry;
    let n = [
        telemetry.time_s.len(),
        telemetry.s_m.len(),
        telemetry.x_m.len(),
        telemetry.y_m.len(),
        telemetry.heading_rad.len(),
        telemetry.speed_kph.len(),
        telemetry.rpm.len(),
        telemetry.gear.len(),
        telemetry.throttle_pct.len(),
        telemetry.brake_pct.len(),
        telemetry.g_lat.len(),
        telemetry.g_long.len(),
        telemetry.g_vert.len(),
        telemetry.engine_temp_c.len(),
        telemetry.engine_power_w.len(),
    ]
    .into_iter()
    .min()
    .unwrap_or(0);

    if n == 0 {
        return Vec::new();
    }

    let lap_count = laps.max(1);
    let lap_duration_s = telemetry.time_s[n - 1].max(0.0);
    let lap_distance_m = telemetry.s_m[n - 1].max(0.0);
    let expected_points = n.saturating_add((lap_count.saturating_sub(1)).saturating_mul(n.saturating_sub(1)));
    let mut events = Vec::with_capacity(expected_points * 15);

    for lap_idx in 0..lap_count {
        let start_i = if lap_idx == 0 { 0 } else { 1 };
        let time_offset = lap_duration_s * lap_idx as f64;
        let distance_offset = lap_distance_m * lap_idx as f64;

        for i in start_i..n {
            let time_s = telemetry.time_s[i].max(0.0) + time_offset;
            let ts_ns = (time_s * 1_000_000_000.0).round() as u64;

            events.push(SessionTelemetryEvent {
                channel: "sim.time_s".to_string(),
                ts_ns,
                value: time_s,
            });
            events.push(SessionTelemetryEvent {
                channel: "sim.s_m".to_string(),
                ts_ns,
                value: telemetry.s_m[i] + distance_offset,
            });
            events.push(SessionTelemetryEvent {
                channel: "sim.x_m".to_string(),
                ts_ns,
                value: telemetry.x_m[i],
            });
            events.push(SessionTelemetryEvent {
                channel: "sim.y_m".to_string(),
                ts_ns,
                value: telemetry.y_m[i],
            });
            events.push(SessionTelemetryEvent {
                channel: "sim.heading_rad".to_string(),
                ts_ns,
                value: telemetry.heading_rad[i],
            });
            events.push(SessionTelemetryEvent {
                channel: "sim.speed_kph".to_string(),
                ts_ns,
                value: telemetry.speed_kph[i],
            });
            events.push(SessionTelemetryEvent {
                channel: "sim.rpm".to_string(),
                ts_ns,
                value: telemetry.rpm[i],
            });
            events.push(SessionTelemetryEvent {
                channel: "sim.gear".to_string(),
                ts_ns,
                value: telemetry.gear[i] as f64,
            });
            events.push(SessionTelemetryEvent {
                channel: "sim.throttle_pct".to_string(),
                ts_ns,
                value: telemetry.throttle_pct[i],
            });
            events.push(SessionTelemetryEvent {
                channel: "sim.brake_pct".to_string(),
                ts_ns,
                value: telemetry.brake_pct[i],
            });
            events.push(SessionTelemetryEvent {
                channel: "sim.g_lat".to_string(),
                ts_ns,
                value: telemetry.g_lat[i],
            });
            events.push(SessionTelemetryEvent {
                channel: "sim.g_long".to_string(),
                ts_ns,
                value: telemetry.g_long[i],
            });
            events.push(SessionTelemetryEvent {
                channel: "sim.g_vert".to_string(),
                ts_ns,
                value: telemetry.g_vert[i],
            });
            events.push(SessionTelemetryEvent {
                channel: "sim.engine_temp_c".to_string(),
                ts_ns,
                value: telemetry.engine_temp_c[i],
            });
            events.push(SessionTelemetryEvent {
                channel: "sim.engine_power_w".to_string(),
                ts_ns,
                value: telemetry.engine_power_w[i],
            });
        }
    }

    events
        .chunks(TELEMETRY_BATCH_SIZE)
        .map(|chunk| TelemetryEnvelope {
            batch: TelemetryBatch {
                events: chunk.to_vec(),
            },
        })
        .collect()
}

fn simulate_competitor(
    track: &TrackProfile,
    competitor: &CompetitorSpec,
    laps: u16,
    hz: f64,
    seed: u64,
    era: i32,
) -> Option<SimulatedCompetitor> {
    let laps = laps.max(1);
    let preset = resolve_vehicle_class(era);
    let lap_time_s = simulate_lap_time_s_with_preset(track, &competitor.tuning, 2, hz, preset)?;
    let base_lap_ms = (lap_time_s * 1000.0).round().max(1.0) as u64;
    score_competitor_from_base_lap_ms(competitor, laps, seed, base_lap_ms)
}

fn score_competitor_from_base_lap_ms(
    competitor: &CompetitorSpec,
    laps: u16,
    seed: u64,
    base_lap_ms: u64,
) -> Option<SimulatedCompetitor> {
    let laps = laps.max(1);

    let budget = competitor.budget_cap.max(1.0);
    let engine_share = clamp(competitor.tuning.engine_points / budget, 0.0, 1.0);
    let cooling_share = clamp(competitor.tuning.cooling_points / budget, 0.0, 1.0);
    let chassis_share = clamp(competitor.tuning.chassis_points / budget, 0.0, 1.0);
    let thermal_stress = (engine_share - cooling_share).max(0.0);
    let fade_per_lap = 0.0007 + thermal_stress * 0.0013;
    let consistency = 1.15 - (cooling_share * 0.18 + chassis_share * 0.12);

    let mut total_time_ms = 0u64;
    let mut best_lap_ms = u64::MAX;

    for lap_idx in 0..laps as u64 {
        let fade = 1.0 + fade_per_lap * lap_idx as f64;
        let jitter = (unit_noise(seed, &competitor.id, lap_idx, 0x00c0ffee) - 0.5) * 2.0;
        let jitter_ms = jitter * 120.0 * consistency.max(0.65);
        let lap_ms = ((base_lap_ms as f64) * fade + jitter_ms).max(1.0).round() as u64;

        total_time_ms = total_time_ms.saturating_add(lap_ms);
        best_lap_ms = best_lap_ms.min(lap_ms);
    }

    Some(SimulatedCompetitor {
        competitor_id: competitor.id.clone(),
        total_time_ms,
        best_lap_ms: if best_lap_ms == u64::MAX {
            total_time_ms
        } else {
            best_lap_ms
        },
        laps_completed: laps,
    })
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

pub fn run_race(request: RunRaceRequest) -> Result<RaceOutput, String> {
    if request.input.race.competitors.is_empty() {
        return Err("race requires at least one competitor".to_string());
    }

    let era = resolve_era(&request);
    let normalized_contract_input = normalize_and_validate_race_input(
        &request.input.race,
        if era > 0 { era as u32 } else { 0 },
    )
    .map_err(|err| format!("invalid race input: {err}"))?;

    let track = resolve_track_profile(
        &normalized_contract_input.track_id,
        request.input.track_profile.as_ref(),
    );
    let hz = resolve_hz(&request).max(1.0);
    let preset = resolve_vehicle_class(era);

    let mut rows = Vec::with_capacity(normalized_contract_input.competitors.len());
    let mut player_batches: Vec<TelemetryEnvelope> = Vec::new();
    for competitor in &normalized_contract_input.competitors {
        let output = run_simulation_with_preset(&track, &competitor.tuning, 2, hz, preset)
            .ok_or_else(|| format!("simulation failed for competitor {}", competitor.id))?;
        let lap_time_s = output
            .solution
            .t
            .last()
            .copied()
            .ok_or_else(|| format!("simulation failed for competitor {}", competitor.id))?
            .max(0.001);
        let base_lap_ms = (lap_time_s * 1000.0).round().max(1.0) as u64;

        let result = score_competitor_from_base_lap_ms(
            competitor,
            normalized_contract_input.laps,
            request.seed,
            base_lap_ms,
        )
        .ok_or_else(|| format!("scoring failed for competitor {}", competitor.id))?;

        if competitor.is_player || competitor.id == "player" {
            player_batches = telemetry_batches_from_simulation(
                &output,
                normalized_contract_input.laps as usize,
            );
        }
        rows.push(result);
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
        player_batches,
    })
}

fn evaluate_candidate(
    track: &TrackProfile,
    laps: u16,
    seed: u64,
    budget: f64,
    tuning: &TuningSpec,
    era: i32,
    hz: f64,
) -> Option<u64> {
    let competitor = CompetitorSpec {
        id: "solver".to_string(),
        name: "solver".to_string(),
        team_id: "SOL".to_string(),
        is_player: true,
        tuning: tuning.clone(),
        budget_cap: budget,
    };

    simulate_competitor(track, &competitor, laps, hz, seed, era).map(|result| result.total_time_ms)
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

    // Keep baseline intentionally sub-optimal by blending toward neutral setup.
    let neutral_shares = Shares {
        engine: 0.25,
        cooling: 0.25,
        aero: 0.25,
        chassis: 0.25,
    };

    let blend_opt = 0.74;
    let mixed_shares = normalize_shares([
        elite_shares.engine * blend_opt + neutral_shares.engine * (1.0 - blend_opt),
        elite_shares.cooling * blend_opt + neutral_shares.cooling * (1.0 - blend_opt),
        elite_shares.aero * blend_opt + neutral_shares.aero * (1.0 - blend_opt),
        elite_shares.chassis * blend_opt + neutral_shares.chassis * (1.0 - blend_opt),
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

    let baseline = to_tuning(mixed_shares, budget, baseline_downforce, baseline_gear);

    (baseline, best)
}

pub fn solve_baseline(request: SolverRequest) -> Option<SolverResponse> {
    let budget = request.budget.max(0.0);
    let laps = request.laps.max(1);
    let runs = request.runs.clamp(24, 2048);
    let era = request.era;
    let hz = if request.hz > 0.0 { request.hz } else { 20.0 };
    let track = resolve_track_profile(&request.track_id, request.track_profile.as_ref());

    let mut rng = StdRng::seed_from_u64(request.seed);
    let mut candidates: Vec<Candidate> = Vec::with_capacity(runs);

    for i in 0..runs {
        let tuning = sample_candidate(&mut rng, budget);
        let sim_seed = request
            .seed
            .wrapping_add((i as u64).wrapping_mul(0x9e3779b97f4a7c15));
        if let Some(time_ms) = evaluate_candidate(&track, laps, sim_seed, budget, &tuning, era, hz)
        {
            candidates.push(Candidate { tuning, time_ms });
        }
    }

    if candidates.is_empty() {
        return None;
    }

    candidates.sort_by_key(|c| c.time_ms);

    let (baseline, top_reference) =
        build_baseline(&candidates, budget, request.seed ^ 0xa5a5_5a5a_1234_4321);
    let baseline_time_ms = evaluate_candidate(
        &track,
        laps,
        request.seed ^ 0x00ff_00ff_00ff_00ff,
        budget,
        &baseline,
        era,
        hz,
    )
    .unwrap_or_else(|| candidates[0].time_ms);

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
    fn selects_vehicle_preset_by_year_fallback() {
        assert_eq!(resolve_vehicle_class(1960), VehicleClass::Legacy1960);
        assert_eq!(resolve_vehicle_class(1970), VehicleClass::GroundEffect1970);
        assert_eq!(resolve_vehicle_class(2025), VehicleClass::HybridModern);
        assert_eq!(resolve_vehicle_class(2026), VehicleClass::ActiveAero2026);
    }

    #[test]
    fn race_simulation_returns_sorted_standings() {
        let output = run_race(sample_request(2026)).expect("race simulation should succeed");
        assert_eq!(output.standings.len(), 2);
        assert_eq!(output.standings[0].position, 1);
        assert_eq!(output.standings[1].position, 2);
        assert!(output.standings[0].total_time_ms <= output.standings[1].total_time_ms);
        assert_eq!(output.standings[0].laps_completed, 5);
        assert!(
            !output.player_batches.is_empty(),
            "expected player telemetry channels to be present"
        );
        assert!(
            output
                .player_batches
                .iter()
                .flat_map(|batch| batch.batch.events.iter())
                .any(|event| event.channel == "sim.speed_kph"),
            "expected speed channel in player telemetry"
        );
    }

    #[test]
    fn player_telemetry_scales_with_stint_laps() {
        let mut one_lap = sample_request(2026);
        one_lap.input.race.laps = 1;
        let one_lap_output = run_race(one_lap).expect("one-lap race simulation should succeed");
        let one_lap_speed_samples = one_lap_output
            .player_batches
            .iter()
            .flat_map(|batch| batch.batch.events.iter())
            .filter(|event| event.channel == "sim.speed_kph")
            .count();

        let mut five_laps = sample_request(2026);
        five_laps.input.race.laps = 5;
        let five_lap_output = run_race(five_laps).expect("five-lap race simulation should succeed");
        let five_lap_speed_samples = five_lap_output
            .player_batches
            .iter()
            .flat_map(|batch| batch.batch.events.iter())
            .filter(|event| event.channel == "sim.speed_kph")
            .count();

        assert!(
            five_lap_speed_samples > one_lap_speed_samples * 3,
            "expected multi-lap telemetry to produce significantly more samples"
        );
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
    fn active_aero_branch_changes_lap_time_signature() {
        let track = build_track_profile("SPA");
        let tuning = TuningSpec {
            engine_points: 12.0,
            cooling_points: 8.0,
            aero_points: 12.0,
            chassis_points: 8.0,
            downforce_slider: 0.8,
            gear_ratio_slider: 0.5,
        };

        let modern =
            simulate_lap_time_s_with_preset(&track, &tuning, 2, 20.0, VehicleClass::HybridModern)
                .expect("modern lap");
        let active =
            simulate_lap_time_s_with_preset(&track, &tuning, 2, 20.0, VehicleClass::ActiveAero2026)
                .expect("active lap");

        assert!(
            (modern - active).abs() > 1e-6,
            "expected distinct simulation branch outputs"
        );
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

        assert!(!y_values.is_empty(), "expected y telemetry samples");
        let max_abs_y = y_values
            .iter()
            .fold(0.0_f64, |acc, value| acc.max(value.abs()));
        assert!(
            max_abs_y < 1e-6,
            "expected custom track y values to stay on centerline"
        );
    }
}
