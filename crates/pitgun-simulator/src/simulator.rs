use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::drivers::{
    apply_driver_to_tire, default_driver_id, deterministic_lap_delta_ms, driver_effects,
};
use crate::errors::SimulatorError;
use crate::models::{AeroConfig, ChassisConfig, EngineConfig, TireConfig, TrackConfig};
use crate::profiles::CompetitorProfile;
use crate::provider::ConfigProvider;
use crate::state::SimulatorState;
use crate::telemetry::TelemetryFrame;
use crate::tuning::Tuning;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LapInput {
    pub vehicle_id: String,
    pub track_id: String,
    #[serde(default)]
    pub tuning: Tuning,
    #[serde(default)]
    pub profile_id: Option<String>,
    #[serde(default)]
    pub profile: Option<CompetitorProfile>,
    #[serde(default)]
    pub driver_id: Option<String>,
    #[serde(default)]
    pub tire_id: Option<String>,
    #[serde(default)]
    pub initial_state: Option<SimulatorState>,
    #[serde(default)]
    pub seed: Option<u64>,
    #[serde(default)]
    pub lap_number: Option<u16>,
    #[serde(default = "default_hz")]
    pub hz: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LapOutput {
    pub lap_time_s: f64,
    pub average_speed_kph: f64,
    pub fuel_used_kg: f64,
    pub final_state: SimulatorState,
    pub telemetry: Vec<TelemetryFrame>,
    pub max_engine_temp_c: f64,
    pub max_tire_temp_c: f64,
}

#[derive(Clone)]
pub struct Simulator {
    provider: Arc<dyn ConfigProvider>,
}

impl Simulator {
    pub fn new(provider: Arc<dyn ConfigProvider>) -> Self {
        Self { provider }
    }

    pub fn simulate_lap(&self, input: LapInput) -> Result<LapOutput, SimulatorError> {
        let vehicle = self.provider.get_vehicle(&input.vehicle_id)?;
        let aero = self.provider.get_aero(&vehicle.aero_id)?;
        let chassis = self.provider.get_chassis(&vehicle.chassis_id)?;
        let engine = self.provider.get_engine(&vehicle.engine_id)?;
        let tire_id = input
            .tire_id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .or_else(|| {
                input
                    .profile
                    .as_ref()
                    .map(|p| p.tire_id.as_str())
                    .filter(|id| !id.trim().is_empty())
            })
            .unwrap_or(vehicle.tire_id.as_str());
        let base_tire = self.provider.get_tire(tire_id)?;
        let driver_id = input
            .driver_id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .unwrap_or(default_driver_id());
        let driver = self.provider.get_driver(driver_id)?;
        let effects = driver_effects(&driver);
        let tire = apply_driver_to_tire(&base_tire, &effects);
        let track = self.provider.get_track(&input.track_id)?;

        track.validate()?;
        aero.validate()?;
        chassis.validate()?;
        engine.validate()?;
        driver.validate()?;
        tire.validate()?;

        let profile = match (input.profile, input.profile_id) {
            (Some(profile), _) => profile,
            (None, Some(profile_id)) => self.provider.get_profile(&profile_id)?,
            (None, None) => self.provider.get_profile("balanced")?,
        };

        let tuning = input.tuning.clamped();
        let resolved = apply_tuning(aero, chassis, engine, tire, &tuning, &profile);
        let initial_state = input.initial_state.unwrap_or_else(|| SimulatorState {
            engine_temp_c: resolved.engine.thermal.initial_temp_c,
            ..SimulatorState::default()
        });
        let initial_fuel_mass_kg = initial_state.fuel_mass_kg.max(0.0);

        let mut sim = run_single_lap(
            &track,
            &resolved,
            &profile,
            initial_state,
            input.lap_number.unwrap_or(1).max(1),
            if input.hz > 0.0 {
                input.hz
            } else {
                default_hz()
            },
        )?;
        let lap_number = input.lap_number.unwrap_or(1).max(1);
        let lap_delta_ms = deterministic_lap_delta_ms(&effects, &driver.id, input.seed, lap_number);
        if lap_delta_ms != 0 {
            apply_lap_delta(&mut sim, lap_delta_ms);
        }
        let fuel_used_kg =
            (resolved.engine.fuel_burn_kg_per_s * sim.lap_time_s).min(initial_fuel_mass_kg);
        sim.fuel_used_kg = fuel_used_kg;
        sim.final_state.fuel_mass_kg = (initial_fuel_mass_kg - fuel_used_kg).max(0.0);
        Ok(sim)
    }
}

fn apply_lap_delta(output: &mut LapOutput, lap_delta_ms: i32) {
    let adjusted_lap_time_s = (output.lap_time_s + lap_delta_ms as f64 / 1000.0).max(0.1);
    let scale = adjusted_lap_time_s / output.lap_time_s.max(1e-6);
    output.lap_time_s = adjusted_lap_time_s;
    for frame in &mut output.telemetry {
        frame.time_s *= scale;
    }
}

fn default_hz() -> f64 {
    20.0
}

#[derive(Debug, Clone)]
struct ResolvedVehicle {
    aero: AeroConfig,
    chassis: ChassisConfig,
    engine: EngineConfig,
    tire: TireConfig,
}

fn apply_tuning(
    mut aero: AeroConfig,
    mut chassis: ChassisConfig,
    mut engine: EngineConfig,
    tire: TireConfig,
    tuning: &Tuning,
    profile: &CompetitorProfile,
) -> ResolvedVehicle {
    let aero_pts = (tuning.aero_points / 40.0).clamp(0.0, 1.0);
    let chassis_pts = (tuning.chassis_points / 40.0).clamp(0.0, 1.0);
    let cooling_pts = (tuning.cooling_points / 40.0).clamp(0.0, 1.0);
    let engine_pts = (tuning.engine_points / 40.0).clamp(0.0, 1.0);

    let downforce_slider = (tuning.downforce_slider + profile.downforce_bias).clamp(0.0, 1.0);
    let gear_slider = (tuning.gear_ratio_slider + profile.gear_ratio_bias).clamp(0.0, 1.0);

    let aero_k = 1.0 + 0.10 * aero_pts;
    let drag_blend = 0.85 + 0.30 * downforce_slider;
    let downforce_blend = 0.75 + 0.55 * downforce_slider;
    aero.cd_a_straight *= aero_k * drag_blend * 0.95;
    aero.cd_a_corner *= aero_k * drag_blend * 1.05;
    aero.cl_a_straight *= aero_k * downforce_blend * 0.95;
    aero.cl_a_corner *= aero_k * downforce_blend * 1.05;

    chassis.mu0 *= 1.0 + 0.08 * chassis_pts;

    let cooling_mult = 0.75 + 0.50 * cooling_pts;
    engine.thermal.cooling_base_w *= cooling_mult;
    engine.thermal.cooling_speed_w_per_ms *= cooling_mult;

    let power_mult = 1.0 + 0.02 * engine_pts;
    for torque in &mut engine.torque_samples {
        *torque *= power_mult;
    }

    let gear_scale = 1.10 - 0.20 * gear_slider;
    for ratio in &mut engine.gear_ratios {
        *ratio *= gear_scale;
    }

    ResolvedVehicle {
        aero,
        chassis,
        engine,
        tire,
    }
}

fn run_single_lap(
    track: &TrackConfig,
    vehicle: &ResolvedVehicle,
    profile: &CompetitorProfile,
    initial_state: SimulatorState,
    lap_number: u16,
    hz: f64,
) -> Result<LapOutput, SimulatorError> {
    let n = track.s_m.len();
    if n < 3 {
        return Err(SimulatorError::InvalidInput(
            "track must have at least 3 samples".to_string(),
        ));
    }

    let max_speed = 120.0;
    let mass = (vehicle.chassis.mass_empty_kg + initial_state.fuel_mass_kg).max(200.0);
    let rho = vehicle.chassis.air_density;
    let g = vehicle.chassis.gravity;

    let mut v_corner = vec![max_speed; n];
    for (i, speed_limit) in v_corner.iter_mut().enumerate() {
        let k = track.curvature_radpm[i].abs();
        if k < 1e-6 {
            continue;
        }

        let mut v = 70.0;
        for _ in 0..4 {
            let downforce = 0.5 * rho * v * v * vehicle.aero.cl_a_corner;
            let mu = effective_mu(
                vehicle.chassis.mu0,
                initial_state.tire_wear,
                initial_state.tire_temp_c,
                &vehicle.tire,
            );
            let a_lat = mu * (g + downforce / mass);
            v = (a_lat / k).max(0.1).sqrt();
        }

        *speed_limit = v.min(max_speed);
    }

    let mut v_bwd = v_corner.clone();
    for i in (0..(n - 1)).rev() {
        let ds = (track.s_m[i + 1] - track.s_m[i]).max(1e-3);
        let v_target = v_bwd[i + 1].max(1.0);

        let q = 0.5 * rho * v_target * v_target;
        let drag = q * vehicle.aero.cd_a_corner;
        let downforce = q * vehicle.aero.cl_a_corner;
        let f_roll = vehicle.chassis.rolling_resistance * (mass * g + downforce);
        let f_slope = mass * g * track.slope[i];

        let mu = effective_mu(
            vehicle.chassis.mu0,
            initial_state.tire_wear,
            initial_state.tire_temp_c,
            &vehicle.tire,
        );

        let normal = mass * g + downforce;
        let grip = mu * normal;
        let f_lat = mass * v_target * v_target * track.curvature_radpm[i].abs();
        let f_brake = if f_lat >= grip {
            0.0
        } else {
            (grip * grip - f_lat * f_lat).sqrt()
        };

        let a_decel = ((f_brake + drag + f_roll + f_slope) / mass).clamp(0.0, 6.0 * g);
        let v_max = (v_target * v_target + 2.0 * a_decel * ds).sqrt();
        if v_bwd[i] > v_max {
            v_bwd[i] = v_max;
        }
    }

    let mut v_fwd = vec![0.0; n];
    let mut engine_temp = vec![0.0; n];
    let mut tire_temp = vec![0.0; n];
    let mut tire_wear = vec![0.0; n];
    let mut power_kw = vec![0.0; n];
    let mut gear = vec![1u8; n];
    let start_speed =
        if initial_state.exit_speed_mps.is_finite() && initial_state.exit_speed_mps > 0.0 {
            initial_state.exit_speed_mps
        } else {
            30.0
        };
    let start_gear = initial_state
        .exit_gear
        .clamp(1, vehicle.engine.gear_ratios.len() as u8);
    v_fwd[n - 1] = start_speed.min(v_bwd[n - 1]);
    engine_temp[n - 1] = initial_state.engine_temp_c;
    tire_temp[n - 1] = initial_state.tire_temp_c;
    tire_wear[n - 1] = initial_state.tire_wear;
    gear[n - 1] = start_gear;

    v_fwd[0] = v_fwd[n - 1];
    engine_temp[0] = engine_temp[n - 1];
    tire_temp[0] = tire_temp[n - 1];
    tire_wear[0] = tire_wear[n - 1];
    gear[0] = gear[n - 1];

    let power_mult = profile.power_multiplier();
    let heat_mult = profile.heat_multiplier();
    let wear_mult = profile.tire_wear_multiplier();
    for i in 0..(n - 1) {
        let ds = (track.s_m[i + 1] - track.s_m[i]).max(1e-3);
        let v = v_fwd[i].min(v_bwd[i]).max(1.0);
        let dt = ds / v;

        let (mut pwr_kw, rpm, best_gear) =
            best_power_at_speed(v, &vehicle.engine, &vehicle.chassis);
        pwr_kw *= power_mult;
        pwr_kw *= derating_factor(engine_temp[i], &vehicle.engine);

        if v_fwd[i] >= v_bwd[i] {
            power_kw[i] = 0.0;
            v_fwd[i + 1] = v_bwd[i];
            gear[i] = best_gear;
        } else {
            let corner_mode = track.curvature_radpm[i].abs() > 0.001;
            let (cd, cl) = if corner_mode {
                (vehicle.aero.cd_a_corner, vehicle.aero.cl_a_corner)
            } else {
                (vehicle.aero.cd_a_straight, vehicle.aero.cl_a_straight)
            };

            let q = 0.5 * rho * v * v;
            let drag = q * cd;
            let downforce = q * cl;
            let f_roll = vehicle.chassis.rolling_resistance * (mass * g + downforce);
            let f_slope = mass * g * track.slope[i];

            let mu = effective_mu(
                vehicle.chassis.mu0,
                tire_wear[i],
                tire_temp[i],
                &vehicle.tire,
            );
            let normal = mass * g + downforce;
            let f_eng_max = 1000.0 * pwr_kw / v.max(10.0);
            let f_drive = f_eng_max.min(mu * normal);

            power_kw[i] = if f_eng_max > 0.0 {
                pwr_kw * (f_drive / f_eng_max)
            } else {
                0.0
            };

            let f_net = f_drive - drag - f_roll - f_slope;
            let a = f_net / mass;
            v_fwd[i + 1] = (v * v + 2.0 * a * ds).max(0.0).sqrt();
            gear[i] = best_gear;
        }

        let heat = 1000.0 * vehicle.engine.thermal.heat_alpha * power_kw[i] * heat_mult;
        let cool = (vehicle.engine.thermal.cooling_base_w
            + vehicle.engine.thermal.cooling_speed_w_per_ms * v)
            * (engine_temp[i] - vehicle.engine.thermal.ambient_temp_c);
        engine_temp[i + 1] = engine_temp[i]
            + ((heat - cool) / vehicle.engine.thermal.capacity_j_per_c.max(1.0)) * dt;

        let a_long = (v_fwd[i + 1] * v_fwd[i + 1] - v * v) / (2.0 * ds).max(1e-3);
        let a_lat = v * v * track.curvature_radpm[i];
        let load_metric = a_long * a_long + a_lat * a_lat;
        let tire_heat = vehicle.tire.heat_k * load_metric;
        let tire_cool =
            vehicle.tire.cool_k * v * (tire_temp[i] - vehicle.engine.thermal.ambient_temp_c);

        tire_temp[i + 1] = (tire_temp[i] + (tire_heat - tire_cool) * dt).max(0.0);

        let wear_rate =
            (vehicle.tire.wear_per_s + vehicle.tire.wear_load_k * load_metric) * wear_mult;
        tire_wear[i + 1] = (tire_wear[i] + wear_rate * dt).min(1.0);

        let _ = rpm;
    }

    let v_final: Vec<f64> = v_fwd
        .iter()
        .zip(v_bwd.iter())
        .map(|(fwd, bwd)| fwd.min(*bwd).max(0.5))
        .collect();

    let t = cumulative_time(&track.s_m, &v_final);
    let lap_time_s = t
        .last()
        .copied()
        .ok_or_else(|| SimulatorError::InvalidInput("failed to compute lap time".to_string()))?;

    let telemetry = build_telemetry(
        track,
        vehicle,
        hz,
        lap_number,
        &t,
        &v_final,
        &power_kw,
        &engine_temp,
        &tire_temp,
        &tire_wear,
        &gear,
    );

    let avg_speed_mps = if lap_time_s > 0.0 {
        track.s_m[n - 1] / lap_time_s
    } else {
        0.0
    };

    let final_state = SimulatorState {
        fuel_mass_kg: initial_state.fuel_mass_kg.max(0.0),
        tire_wear: tire_wear[n - 1],
        tire_temp_c: tire_temp[n - 1],
        engine_temp_c: engine_temp[n - 1],
        exit_speed_mps: v_final[n - 1],
        exit_gear: gear[n - 1].clamp(1, vehicle.engine.gear_ratios.len() as u8),
    };

    let max_engine_temp_c = engine_temp
        .iter()
        .copied()
        .reduce(f64::max)
        .unwrap_or(initial_state.engine_temp_c);
    let max_tire_temp_c = tire_temp
        .iter()
        .copied()
        .reduce(f64::max)
        .unwrap_or(initial_state.tire_temp_c);

    Ok(LapOutput {
        lap_time_s,
        average_speed_kph: avg_speed_mps * 3.6,
        fuel_used_kg: 0.0,
        final_state,
        telemetry,
        max_engine_temp_c,
        max_tire_temp_c,
    })
}

#[allow(clippy::too_many_arguments)]
fn build_telemetry(
    track: &TrackConfig,
    vehicle: &ResolvedVehicle,
    hz: f64,
    lap_number: u16,
    t_grid: &[f64],
    v_grid: &[f64],
    power_grid_kw: &[f64],
    engine_temp_grid: &[f64],
    tire_temp_grid: &[f64],
    tire_wear_grid: &[f64],
    gear_grid: &[u8],
) -> Vec<TelemetryFrame> {
    if t_grid.is_empty() {
        return Vec::new();
    }

    let t_end = *t_grid.last().unwrap_or(&0.0);
    if t_end <= 0.0 {
        return Vec::new();
    }

    let slope_grad = gradient_wrt(&track.s_m, &track.slope);
    let mut frames = Vec::new();

    let mut prev_t = 0.0;
    let mut prev_v = interp_linear(0.0, t_grid, v_grid);

    let dt = 1.0 / hz.max(1.0);
    let mut ts = 0.0;
    while ts <= t_end + 1e-9 {
        let s = interp_linear(ts, t_grid, &track.s_m);
        let v = interp_linear(ts, t_grid, v_grid).max(0.0);
        let p_kw = interp_linear(ts, t_grid, power_grid_kw).max(0.0);
        let eng_temp = interp_linear(ts, t_grid, engine_temp_grid);
        let tire_temp = interp_linear(ts, t_grid, tire_temp_grid);
        let tire_wear = interp_linear(ts, t_grid, tire_wear_grid).clamp(0.0, 1.0);
        let tire_mu = effective_mu(vehicle.chassis.mu0, tire_wear, tire_temp, &vehicle.tire);
        let gear_raw = interp_linear(ts, t_grid, &u8_to_f64(gear_grid));
        let gear = gear_raw
            .round()
            .clamp(1.0, vehicle.engine.gear_ratios.len() as f64) as u8;

        let x = interp_linear(s, &track.s_m, &track.x_m);
        let y = interp_linear(s, &track.s_m, &track.y_m);
        let heading = interp_linear(s, &track.s_m, &track.heading_rad);
        let kappa = interp_linear(s, &track.s_m, &track.curvature_radpm);
        let slope_g = interp_linear(s, &track.s_m, &slope_grad);

        let a_long = if ts > prev_t {
            (v - prev_v) / (ts - prev_t)
        } else {
            0.0
        };
        let g_lat = (v * v * kappa) / vehicle.chassis.gravity;
        let g_long = a_long / vehicle.chassis.gravity;
        let g_vert = (v * v * slope_g) / vehicle.chassis.gravity;

        let gear_ratio = vehicle.engine.gear_ratios[gear as usize - 1];
        let rpm = rpm_from_speed_gear(v, gear_ratio, vehicle.chassis.wheel_radius_m);
        let p_theoretical_kw = power_kw_from_rpm(rpm, &vehicle.engine);

        let throttle_pct = if p_theoretical_kw > 1e-9 {
            ((p_kw / p_theoretical_kw).clamp(0.0, 1.2)) * 100.0
        } else {
            0.0
        };
        let brake_pct = if throttle_pct < 1.0 && a_long < -0.2 {
            (a_long.abs() / 8.0).clamp(0.0, 1.0) * 100.0
        } else {
            0.0
        };

        frames.push(TelemetryFrame {
            time_s: ts,
            s_m: s,
            x_m: x,
            y_m: y,
            heading_rad: heading,
            speed_kph: v * 3.6,
            rpm,
            gear,
            throttle_pct,
            brake_pct,
            g_lat,
            g_long,
            g_vert,
            engine_temp_c: eng_temp,
            engine_power_w: p_kw * 1000.0,
            tire_temp_c: tire_temp,
            tire_wear_pct: tire_wear * 100.0,
            tire_mu: Some(tire_mu),
            n_lap: Some(lap_number),
        });

        prev_t = ts;
        prev_v = v;
        ts += dt;
    }

    if frames
        .last()
        .map(|f| (f.time_s - t_end).abs() > 1e-6)
        .unwrap_or(true)
    {
        let s = *track.s_m.last().unwrap_or(&0.0);
        let v = *v_grid.last().unwrap_or(&0.0);
        let p_kw = *power_grid_kw.last().unwrap_or(&0.0);
        let eng_temp = *engine_temp_grid.last().unwrap_or(&0.0);
        let tire_temp = *tire_temp_grid.last().unwrap_or(&0.0);
        let tire_wear = *tire_wear_grid.last().unwrap_or(&0.0);
        let tire_mu = effective_mu(vehicle.chassis.mu0, tire_wear, tire_temp, &vehicle.tire);
        let gear = *gear_grid.last().unwrap_or(&1);
        let gear_ratio = vehicle.engine.gear_ratios[gear as usize - 1];
        let rpm = rpm_from_speed_gear(v, gear_ratio, vehicle.chassis.wheel_radius_m);

        frames.push(TelemetryFrame {
            time_s: t_end,
            s_m: s,
            x_m: *track.x_m.last().unwrap_or(&0.0),
            y_m: *track.y_m.last().unwrap_or(&0.0),
            heading_rad: *track.heading_rad.last().unwrap_or(&0.0),
            speed_kph: v * 3.6,
            rpm,
            gear,
            throttle_pct: 0.0,
            brake_pct: 0.0,
            g_lat: 0.0,
            g_long: 0.0,
            g_vert: 0.0,
            engine_temp_c: eng_temp,
            engine_power_w: p_kw * 1000.0,
            tire_temp_c: tire_temp,
            tire_wear_pct: tire_wear * 100.0,
            tire_mu: Some(tire_mu),
            n_lap: Some(lap_number),
        });
    }

    frames
}

fn cumulative_time(s: &[f64], v: &[f64]) -> Vec<f64> {
    let n = s.len().min(v.len());
    let mut t = vec![0.0; n];
    for i in 1..n {
        let ds = (s[i] - s[i - 1]).max(1e-3);
        let v_avg = (v[i] + v[i - 1]).max(1.0) * 0.5;
        t[i] = t[i - 1] + ds / v_avg;
    }
    t
}

fn gradient_wrt(x: &[f64], y: &[f64]) -> Vec<f64> {
    let n = x.len().min(y.len());
    let mut out = vec![0.0; n];
    for i in 0..n {
        let i0 = i.saturating_sub(1);
        let i1 = (i + 1).min(n - 1);
        let dx = (x[i1] - x[i0]).abs().max(1e-6);
        out[i] = (y[i1] - y[i0]) / dx;
    }
    out
}

fn u8_to_f64(values: &[u8]) -> Vec<f64> {
    values.iter().map(|v| *v as f64).collect()
}

fn effective_mu(mu0: f64, tire_wear: f64, tire_temp: f64, tire: &TireConfig) -> f64 {
    let wear_k = (1.0 - tire.wear_grip_k * tire_wear).max(tire.wear_min);
    let temp_z = (tire_temp - tire.temp_opt_c) / tire.temp_sigma_c.max(1e-3);
    let temp_k = (-temp_z * temp_z).exp().max(tire.temp_min_k);
    mu0 * tire.mu_scale * wear_k * temp_k
}

fn derating_factor(temp_c: f64, engine: &EngineConfig) -> f64 {
    if temp_c <= engine.thermal.soft_temp_c {
        return 1.0;
    }
    let loss = (temp_c - engine.thermal.soft_temp_c) * engine.thermal.derate_per_c;
    (1.0 - loss).max(0.2)
}

fn best_power_at_speed(
    speed_mps: f64,
    engine: &EngineConfig,
    chassis: &ChassisConfig,
) -> (f64, f64, u8) {
    let mut pmax = 0.0;
    let mut rpm_best = engine.idle_rpm;
    let mut gear_best = 1u8;

    for (idx, ratio) in engine.gear_ratios.iter().enumerate() {
        let rpm = rpm_from_speed_gear(speed_mps, *ratio, chassis.wheel_radius_m);
        let p = power_kw_from_rpm(rpm, engine);
        if p > pmax {
            pmax = p;
            rpm_best = rpm;
            gear_best = (idx + 1) as u8;
        }
    }

    (pmax, rpm_best, gear_best)
}

fn rpm_from_speed_gear(speed_mps: f64, gear_ratio: f64, wheel_radius_m: f64) -> f64 {
    if gear_ratio <= 0.0 || wheel_radius_m <= 0.0 {
        return 0.0;
    }
    speed_mps * 60.0 * gear_ratio / (std::f64::consts::TAU * wheel_radius_m)
}

fn power_kw_from_rpm(rpm: f64, engine: &EngineConfig) -> f64 {
    if rpm < engine.idle_rpm || rpm > engine.max_rpm {
        return 0.0;
    }

    let tq = interp_linear(rpm, &engine.rpm_samples, &engine.torque_samples);
    tq * rpm * std::f64::consts::PI / 30.0
}

fn interp_linear(x: f64, xs: &[f64], ys: &[f64]) -> f64 {
    let n = xs.len().min(ys.len());
    if n == 0 {
        return 0.0;
    }
    if n == 1 {
        return ys[0];
    }

    if x <= xs[0] {
        return ys[0];
    }
    if x >= xs[n - 1] {
        return ys[n - 1];
    }

    let mut lo = 0usize;
    let mut hi = n - 1;
    while hi - lo > 1 {
        let mid = (lo + hi) / 2;
        if xs[mid] <= x {
            lo = mid;
        } else {
            hi = mid;
        }
    }

    let x0 = xs[lo];
    let x1 = xs[hi];
    if (x1 - x0).abs() < 1e-12 {
        return ys[lo];
    }
    let a = (x - x0) / (x1 - x0);
    ys[lo] + (ys[hi] - ys[lo]) * a
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::default_in_memory_provider;

    #[test]
    fn lap_simulation_produces_finite_values() {
        let provider = Arc::new(default_in_memory_provider());
        let simulator = Simulator::new(provider);

        let output = simulator
            .simulate_lap(LapInput {
                vehicle_id: "f1_2026".to_string(),
                track_id: "SPA".to_string(),
                tuning: Tuning {
                    engine_points: 12.0,
                    cooling_points: 9.0,
                    aero_points: 12.0,
                    chassis_points: 7.0,
                    downforce_slider: 0.6,
                    gear_ratio_slider: 0.5,
                },
                profile_id: Some("balanced".to_string()),
                profile: None,
                driver_id: None,
                tire_id: None,
                initial_state: None,
                seed: None,
                lap_number: None,
                hz: 20.0,
            })
            .expect("lap simulation");

        assert!(output.lap_time_s > 20.0);
        assert!(output.average_speed_kph > 20.0);
        assert!(output.telemetry.len() > 50);
        assert!(output.telemetry.iter().all(|f| f.speed_kph.is_finite()));
        assert!(output.telemetry.iter().all(|f| f.tire_mu.is_some()));
        assert!(output.telemetry.iter().all(|f| f.n_lap == Some(1)));
        assert!(output.fuel_used_kg > 0.0);
        assert!(output.final_state.fuel_mass_kg < 100.0);
    }

    #[test]
    fn aggressive_profile_is_not_identical_to_conservative() {
        let provider = Arc::new(default_in_memory_provider());
        let simulator = Simulator::new(provider.clone());

        let aggressive = simulator
            .simulate_lap(LapInput {
                vehicle_id: "f1_2026".to_string(),
                track_id: "MONZA".to_string(),
                tuning: Tuning::default(),
                profile_id: Some("aggressive".to_string()),
                profile: None,
                driver_id: None,
                tire_id: None,
                initial_state: None,
                seed: None,
                lap_number: None,
                hz: 20.0,
            })
            .expect("aggressive run");

        let conservative = simulator
            .simulate_lap(LapInput {
                vehicle_id: "f1_2026".to_string(),
                track_id: "MONZA".to_string(),
                tuning: Tuning::default(),
                profile_id: Some("conservative".to_string()),
                profile: None,
                driver_id: None,
                tire_id: None,
                initial_state: None,
                seed: None,
                lap_number: None,
                hz: 20.0,
            })
            .expect("conservative run");

        assert!(
            (aggressive.lap_time_s - conservative.lap_time_s).abs() > 1e-4,
            "profile branches should affect lap time"
        );
    }

    #[test]
    fn carried_exit_speed_affects_next_lap_entry_speed() {
        let provider = Arc::new(default_in_memory_provider());
        let simulator = Simulator::new(provider);

        let fresh = simulator
            .simulate_lap(LapInput {
                vehicle_id: "f1_2026".to_string(),
                track_id: "MONZA".to_string(),
                tuning: Tuning::default(),
                profile_id: Some("balanced".to_string()),
                profile: None,
                driver_id: None,
                tire_id: None,
                initial_state: Some(SimulatorState {
                    exit_speed_mps: 0.0,
                    exit_gear: 1,
                    ..SimulatorState::default()
                }),
                seed: None,
                lap_number: None,
                hz: 20.0,
            })
            .expect("fresh lap");

        let carried = simulator
            .simulate_lap(LapInput {
                vehicle_id: "f1_2026".to_string(),
                track_id: "MONZA".to_string(),
                tuning: Tuning::default(),
                profile_id: Some("balanced".to_string()),
                profile: None,
                driver_id: None,
                tire_id: None,
                initial_state: Some(SimulatorState {
                    exit_speed_mps: 80.0,
                    exit_gear: 6,
                    ..SimulatorState::default()
                }),
                seed: None,
                lap_number: None,
                hz: 20.0,
            })
            .expect("carried lap");

        let fresh_entry_speed = fresh
            .telemetry
            .first()
            .map(|frame| frame.speed_kph)
            .unwrap_or(0.0);
        let carried_entry_speed = carried
            .telemetry
            .first()
            .map(|frame| frame.speed_kph)
            .unwrap_or(0.0);

        assert!(
            carried_entry_speed > fresh_entry_speed + 20.0,
            "expected carried lap to start faster: fresh={fresh_entry_speed:.2} carried={carried_entry_speed:.2}"
        );
        assert!(carried.final_state.exit_speed_mps > 0.0);
        assert!(carried.final_state.exit_gear >= 1);
    }
}
