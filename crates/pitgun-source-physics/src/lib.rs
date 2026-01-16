use anyhow::{ensure, Context, Result};
use serde::Deserialize;
use std::path::Path;

pub mod game;

pub use pitgun_contract::game::v1::{
    GamePlayerTuningV1 as PlayerTuning, GameTelemetryPointV1 as TelemetryPoint,
};

#[derive(Debug, Clone)]
pub struct VehicleParams {
    pub m: f32,
    pub rho: f32,
    pub g: f32,
    pub r_wheel: f32,
    pub mu: f32,
    pub c_rr: f32,
    pub cda_x: f32,
    pub cda_z: f32,
    pub cla_x: f32,
    pub cla_z: f32,
    pub n_idle: f32,
    pub n_max: f32,
    pub g1_total: f32,
    pub g8_total: f32,
    pub n_upshift: f32,
    pub n_downshift: f32,
    pub t_amb: f32,
    pub t_init: f32,
    pub t_soft: f32,
    pub c_th: f32,
    pub alpha_heat: f32,
    pub p_cool0: f32,
    pub k_cool: f32,
    pub beta_derate: f32,
}

impl Default for VehicleParams {
    fn default() -> Self {
        Self {
            m: 768.0,
            rho: 1.225,
            g: 9.81,
            r_wheel: 0.36,
            mu: 1.7,
            c_rr: 0.015,
            cda_x: 0.85,
            cda_z: 1.50,
            cla_x: 2.6,
            cla_z: 4.13,
            n_idle: 4000.0,
            n_max: 15000.0,
            g1_total: 14.0,
            g8_total: 4.7,
            n_upshift: 12300.0,
            n_downshift: 5500.0,
            t_amb: 35.0,
            t_init: 90.0,
            t_soft: 110.0,
            c_th: 500000.0,
            alpha_heat: 0.45,
            p_cool0: 15000.0,
            k_cool: 1100.0,
            beta_derate: 0.004,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct TrackPoint {
    pub s_m: f32,
    pub x_m: f32,
    pub y_m: f32,
    pub z_m: f32,
    pub heading_rad: f32,
    pub curvature_radpm: f32,
    pub slope_pct: f32,
}

pub fn load_track_from_csv_bytes(bytes: &[u8]) -> Result<Vec<TrackPoint>> {
    let mut rdr = csv::Reader::from_reader(bytes);
    let mut track = Vec::new();
    for result in rdr.deserialize() {
        track.push(result?);
    }
    Ok(track)
}

pub fn load_track_from_csv_path(path: impl AsRef<Path>) -> Result<Vec<TrackPoint>> {
    let path = path.as_ref();
    let mut rdr = csv::Reader::from_path(path)
        .with_context(|| format!("Failed to read track file {path:?}"))?;
    let mut track = Vec::new();
    for result in rdr.deserialize() {
        track.push(result?);
    }
    Ok(track)
}

// ============================================================================
// 2. MATH HELPERS
// ============================================================================

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn interp_1d(x_val: f32, xp: &[f32], fp: &[f32]) -> f32 {
    if x_val <= xp[0] {
        return fp[0];
    }
    if x_val >= *xp.last().unwrap() {
        return *fp.last().unwrap();
    }

    let idx = xp.partition_point(|&x| x < x_val).saturating_sub(1);
    let x0 = xp[idx];
    let x1 = xp[idx + 1];
    let t = (x_val - x0) / (x1 - x0);
    lerp(fp[idx], fp[idx + 1], t)
}

fn moving_average(data: &[f32], window: usize) -> Vec<f32> {
    if window < 2 {
        return data.to_vec();
    }
    let mut out = vec![0.0; data.len()];
    let half = window / 2;
    for (i, out_val) in out.iter_mut().enumerate() {
        let start = i.saturating_sub(half);
        let end = (i + half + 1).min(data.len());
        let slice = &data[start..end];
        let sum: f32 = slice.iter().sum();
        *out_val = sum / slice.len() as f32;
    }
    out
}

fn gradient(y: &[f32], x: &[f32]) -> Vec<f32> {
    let n = y.len();
    let mut out = vec![0.0; n];
    if n < 2 {
        return out;
    }
    out[0] = (y[1] - y[0]) / (x[1] - x[0]);
    for i in 1..n - 1 {
        out[i] = (y[i + 1] - y[i - 1]) / (x[i + 1] - x[i - 1]);
    }
    out[n - 1] = (y[n - 1] - y[n - 2]) / (x[n - 1] - x[n - 2]);
    out
}

// ============================================================================
// 3. PHYSICS CORE
// ============================================================================

fn power_curve(rpm: f32, tuning: &PlayerTuning) -> f32 {
    let rpm = rpm.clamp(0.0, 15000.0);
    let base_peak = 750_000.0;
    let engine_bonus = 1.0 + 0.20 * (tuning.engine_points as f32 / 20.0);
    let peak = base_peak * engine_bonus;

    if rpm < 4000.0 {
        0.10 * peak * (rpm / 4000.0)
    } else if rpm < 11500.0 {
        let x = (rpm - 4000.0) / (11500.0 - 4000.0);
        peak * (0.25 + 0.75 * x.powf(0.9))
    } else {
        let x = (rpm - 11500.0) / (15000.0 - 11500.0);
        peak * (1.0 - 0.18 * x)
    }
}

fn apply_tuning(base: VehicleParams, tuning: &PlayerTuning) -> VehicleParams {
    let mut p = base;
    let df = tuning.downforce_slider.clamp(0.0, 1.0);
    let gr = tuning.gear_ratio_slider.clamp(0.0, 1.0);
    let aero_k = 1.0 + 0.10 * (tuning.aero_points as f32 / 20.0);

    let drag_blend = 0.85 + 0.30 * df;
    let df_blend = 0.75 + 0.55 * df;

    p.cda_x *= aero_k * drag_blend * 0.95;
    p.cda_z *= aero_k * drag_blend * 1.05;
    p.cla_x *= aero_k * df_blend * 0.95;
    p.cla_z *= aero_k * df_blend * 1.05;

    p.mu *= 1.0 + 0.08 * (tuning.chassis_points as f32 / 20.0);

    let cool_k = 1.0 + 0.35 * (tuning.cooling_points as f32 / 20.0);
    p.p_cool0 *= cool_k;
    p.k_cool *= cool_k;

    let scale = 1.10 - 0.20 * gr;
    p.g1_total *= scale;
    p.g8_total *= scale;

    p
}

fn generate_gear_ratios(p: &VehicleParams) -> Vec<f32> {
    (0..8)
        .map(|k| p.g1_total * (p.g8_total / p.g1_total).powf(k as f32 / 7.0))
        .collect()
}

pub fn run_simulation(
    track: &[TrackPoint],
    tuning: PlayerTuning,
    hz: f32,
) -> Result<Vec<TelemetryPoint>> {
    let p = apply_tuning(VehicleParams::default(), &tuning);
    let n = track.len();
    ensure!(n >= 2, "track must contain at least 2 points");
    ensure!(hz.is_finite() && hz > 0.0, "hz must be finite and > 0");

    // --- 1. Forward Pass (Speed Profile) ---
    let mut v_fwd: Vec<f32> = vec![0.0; n];
    let mut v_corner: Vec<f32> = vec![0.0; n];
    v_fwd[0] = 30.0;

    let ds = if n > 1 {
        track[1].s_m - track[0].s_m
    } else {
        1.0
    };

    for i in 0..n {
        let k_val = track[i].curvature_radpm.abs();
        let mut v = 70.0;
        if k_val < 1e-5 {
            v = 400.0;
        } else {
            for _ in 0..5 {
                let q = 0.5 * p.rho * v * v;
                let downforce = q * p.cla_z;
                let a_lat_max = p.mu * (p.g + downforce / p.m);
                v = (a_lat_max / k_val).sqrt().max(0.1);
            }
        }
        v_corner[i] = v.min(400.0);
    }

    for i in 0..n - 1 {
        let v_curr = v_fwd[i].min(v_corner[i]);
        let mode_z = track[i].curvature_radpm.abs() > 0.001;

        let q = 0.5 * p.rho * v_curr * v_curr;
        let cda = if mode_z { p.cda_z } else { p.cda_x };
        let cla = if mode_z { p.cla_z } else { p.cla_x };

        let d_force = q * cda;
        let roll = p.c_rr * (p.m * p.g + q * cla);
        let slope = p.m * p.g * track[i].slope_pct;

        let pwr = power_curve(p.n_upshift - 1000.0, &tuning);
        let f_eng_max = pwr / v_curr.max(10.0);
        let f_drive = f_eng_max.min(p.mu * (p.m * p.g + q * cla));

        let a = (f_drive - d_force - roll - slope) / p.m;
        v_fwd[i + 1] = (v_curr.powi(2) + 2.0 * a * ds).max(0.0).sqrt();
    }

    // --- 2. Backward Pass (Braking) ---
    let mut v_bwd: Vec<f32> = v_fwd.clone();
    for i in (0..n - 1).rev() {
        let v_target = v_bwd[i + 1];
        let q = 0.5 * p.rho * v_target * v_target;
        let d_force = q * p.cda_z;
        let l_force = q * p.cla_z;
        let roll = p.c_rr * (p.m * p.g + l_force);
        let slope = p.m * p.g * track[i].slope_pct;

        let grip_avail = p.mu * (p.m * p.g + l_force);
        let f_lat_req = p.m * v_target.powi(2) * track[i].curvature_radpm.abs();

        let f_brake_max = if f_lat_req >= grip_avail {
            0.0
        } else {
            (grip_avail.powi(2) - f_lat_req.powi(2)).sqrt()
        };
        let a_decel = ((f_brake_max + d_force + roll + slope) / p.m).min(6.0 * p.g);

        let v_max_braking = (v_target.powi(2) + 2.0 * a_decel * ds).sqrt();
        if v_bwd[i] > v_max_braking {
            v_bwd[i] = v_max_braking;
        }
    }

    // --- 3. Integration ---
    let v_final: Vec<f32> = v_fwd
        .iter()
        .zip(v_bwd.iter())
        .zip(v_corner.iter())
        .map(|((f, b), c)| f.min(*b).min(*c).max(1.0))
        .collect();

    let mut t: Vec<f32> = vec![0.0; n];
    for i in 1..n {
        let v_avg = 0.5 * (v_final[i] + v_final[i - 1]);
        t[i] = t[i - 1] + ds / v_avg;
    }

    // --- 4. Resampling (Telemetry) ---
    let t_total = *t.last().unwrap();
    let num_frames = (t_total * hz).ceil() as usize;
    let dt_hz = 1.0 / hz;

    // Sources
    let s_source: Vec<f32> = track.iter().map(|p| p.s_m).collect();
    let x_source: Vec<f32> = track.iter().map(|p| p.x_m).collect();
    let y_source: Vec<f32> = track.iter().map(|p| p.y_m).collect();
    let h_source: Vec<f32> = track.iter().map(|p| p.heading_rad).collect();
    let k_source: Vec<f32> = track.iter().map(|p| p.curvature_radpm).collect();

    let mut telemetry = Vec::with_capacity(num_frames);
    let mut temp = p.t_init;
    let mut current_gear: usize = 1;
    let mut last_shift_time = -1.0;
    let ratios = generate_gear_ratios(&p);

    let mut raw_v = vec![0.0; num_frames];
    let mut raw_throttle = vec![0.0; num_frames];
    let mut raw_brake = vec![0.0; num_frames];

    for i in 0..num_frames {
        let t_curr = i as f32 * dt_hz;

        let s_curr = interp_1d(t_curr, &t, &s_source);
        let v_curr = interp_1d(t_curr, &t, &v_final);
        raw_v[i] = v_curr;

        // Gearbox
        let gear_ratio = ratios[current_gear - 1];
        let rpm_curr = (v_curr * 60.0 * gear_ratio) / (2.0 * std::f32::consts::PI * p.r_wheel);

        if (t_curr - last_shift_time) > 0.5 {
            if rpm_curr > (p.n_upshift + 200.0) && current_gear < 8 {
                current_gear += 1;
                last_shift_time = t_curr;
            } else if rpm_curr < (p.n_downshift - 200.0) && current_gear > 1 {
                current_gear -= 1;
                last_shift_time = t_curr;
            }
        }
        let gear_ratio_final = ratios[current_gear - 1];
        let rpm_final = ((v_curr * 60.0 * gear_ratio_final)
            / (2.0 * std::f32::consts::PI * p.r_wheel))
            .clamp(p.n_idle, p.n_max);

        // Pedals
        let v_prev = if i > 0 { raw_v[i - 1] } else { v_curr };
        let a_long = (v_curr - v_prev) / dt_hz;

        if a_long >= 0.0 {
            raw_brake[i] = 0.0;
            raw_throttle[i] = (a_long / 5.0).clamp(0.0, 1.0);
        } else {
            raw_throttle[i] = 0.0;
            raw_brake[i] = (-a_long / 5.0).clamp(0.0, 1.0);
        }

        // State temp
        let p_theo = power_curve(rpm_final, &tuning);
        let mut derate = 1.0;
        if temp > p.t_soft {
            derate = (1.0 - (temp - p.t_soft) * p.beta_derate).max(0.2);
        }
        let p_act = p_theo * derate;
        let heat = p.alpha_heat * p_act * raw_throttle[i];
        let cool = p.p_cool0 + p.k_cool * v_curr;
        temp += (heat - cool) / p.c_th * dt_hz;

        telemetry.push(TelemetryPoint {
            time_s: t_curr,
            s_m: s_curr,
            x_m: 0.0,
            y_m: 0.0,
            heading_rad: 0.0,
            speed_kph: v_curr * 3.6,
            rpm: rpm_final,
            gear: current_gear as i32,
            throttle_pct: 0.0,
            brake_pct: 0.0,
            g_lat: 0.0,
            g_long: 0.0,
            engine_temp_c: temp,
            engine_power_w: p_act * raw_throttle[i],
        });
    }

    // Post-Processing
    let smooth_throttle = moving_average(&raw_throttle, 15);
    let smooth_brake = moving_average(&raw_brake, 20);
    let smooth_v = moving_average(&raw_v, 5);
    let a_long_vec = gradient(
        &smooth_v,
        &telemetry.iter().map(|p| p.time_s).collect::<Vec<f32>>(),
    );

    for i in 0..num_frames {
        let s_val = telemetry[i].s_m;
        telemetry[i].x_m = interp_1d(s_val, &s_source, &x_source);
        telemetry[i].y_m = interp_1d(s_val, &s_source, &y_source);
        telemetry[i].heading_rad = interp_1d(s_val, &s_source, &h_source);

        let k_curr = interp_1d(s_val, &s_source, &k_source);
        telemetry[i].g_lat = (raw_v[i].powi(2) * k_curr) / 9.81;
        telemetry[i].g_long = a_long_vec[i] / 9.81;
        telemetry[i].throttle_pct = smooth_throttle[i] * 100.0;
        telemetry[i].brake_pct = smooth_brake[i] * 100.0;
    }

    Ok(telemetry)
}
