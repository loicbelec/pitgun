use anyhow::{ensure, Context, Result};
use serde::Deserialize;
use std::path::Path;

pub mod game;
pub mod vehicle;

pub use pitgun_contract::game::v1::{
    GamePlayerTuningV1 as PlayerTuning, GameTelemetryPointV1 as TelemetryPoint,
};

use vehicle::registry::{load_vehicle, DEFAULT_VEHICLE_ID};
use vehicle::VehicleSpec;

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

#[derive(Clone)]
struct TrackProfile {
    s: Vec<f32>,
    x: Vec<f32>,
    y: Vec<f32>,
    heading: Vec<f32>,
    curvature: Vec<f32>,
    slope: Vec<f32>,
}

impl TrackProfile {
    fn from_points(points: &[TrackPoint]) -> Result<Self> {
        ensure!(points.len() >= 2, "track must contain at least 2 points");

        let mut s = Vec::with_capacity(points.len());
        let mut x = Vec::with_capacity(points.len());
        let mut y = Vec::with_capacity(points.len());
        let mut heading = Vec::with_capacity(points.len());
        let mut curvature = Vec::with_capacity(points.len());
        let mut slope = Vec::with_capacity(points.len());

        for point in points {
            s.push(point.s_m);
            x.push(point.x_m);
            y.push(point.y_m);
            heading.push(point.heading_rad);
            curvature.push(point.curvature_radpm);
            slope.push(point.slope_pct);
        }

        Ok(Self {
            s,
            x,
            y,
            heading,
            curvature,
            slope,
        })
    }
}

struct SpeedProfile {
    s: Vec<f32>,
    t: Vec<f32>,
    v: Vec<f32>,
}

struct ResampleOutput {
    telemetry: Vec<TelemetryPoint>,
    last_temp_c: f32,
    last_time_s: f32,
    last_s_m: f32,
}

pub fn run_simulation(
    track: &[TrackPoint],
    tuning: PlayerTuning,
    hz: f32,
) -> Result<Vec<TelemetryPoint>> {
    simulate_stint(track, tuning, hz, 1)
}

pub fn simulate_stint(
    track: &[TrackPoint],
    tuning: PlayerTuning,
    hz: f32,
    laps: u32,
) -> Result<Vec<TelemetryPoint>> {
    ensure!(laps >= 1, "laps must be >= 1");
    ensure!(hz.is_finite() && hz > 0.0, "hz must be finite and > 0");

    let vehicle = load_vehicle(DEFAULT_VEHICLE_ID)
        .map_err(|err| anyhow::anyhow!("vehicle load failed: {err}"))?;
    run_stint_with_vehicle(track, tuning, hz, laps, vehicle)
}

fn run_stint_with_vehicle(
    track: &[TrackPoint],
    tuning: PlayerTuning,
    hz: f32,
    laps: u32,
    mut vehicle: VehicleSpec,
) -> Result<Vec<TelemetryPoint>> {
    let track_profile = TrackProfile::from_points(track)?;

    vehicle.apply_tuning(&tuning);
    let base_torque = vehicle.torque_curve.clone();

    let mut telemetry = Vec::new();
    let mut time_offset = 0.0;
    let mut s_offset = 0.0;

    for _ in 0..laps {
        if vehicle.t_init > vehicle.t_soft {
            let excess = vehicle.t_init - vehicle.t_soft;
            let derate = (1.0 - excess * 0.02).max(0.6);
            vehicle.torque_curve = base_torque.iter().map(|value| value * derate).collect();
        } else {
            vehicle.torque_curve.clone_from(&base_torque);
        }

        let sol = compute_speed_profile(&track_profile, &vehicle);
        let lap_output = resample_telemetry(&track_profile, &sol, &vehicle, hz);
        vehicle.t_init = lap_output.last_temp_c;

        for mut point in lap_output.telemetry {
            point.time_s += time_offset;
            point.s_m += s_offset;
            telemetry.push(point);
        }

        time_offset += lap_output.last_time_s;
        s_offset += lap_output.last_s_m;
    }

    Ok(telemetry)
}

fn compute_speed_profile(track: &TrackProfile, vehicle: &VehicleSpec) -> SpeedProfile {
    let n = track.s.len();
    let ds = if n > 1 { track.s[1] - track.s[0] } else { 1.0 };

    let mut v_corner: Vec<f32> = vec![400.0; n];

    for (i, corner) in v_corner.iter_mut().enumerate() {
        let k_val = track.curvature[i].abs();
        if k_val < 1e-5 {
            continue;
        }

        let mut v = 70.0;
        for _ in 0..5 {
            let q = 0.5 * vehicle.rho * v * v;
            let downforce = q * vehicle.cl_a_z;
            let a_lat_max = vehicle.mu * (vehicle.g + downforce / vehicle.mass_kg);
            v = (a_lat_max / k_val).sqrt().max(0.1);
        }
        *corner = v.min(400.0);
    }

    let mut v_fwd: Vec<f32> = vec![0.0; n];
    v_fwd[0] = 30.0 / 3.6;

    for i in 0..n - 1 {
        let v_curr = v_fwd[i].min(v_corner[i]);
        let mode_z = track.curvature[i].abs() > 0.001;

        let q = 0.5 * vehicle.rho * v_curr * v_curr;
        let cd_a = if mode_z {
            vehicle.cd_a_z
        } else {
            vehicle.cd_a_x
        };
        let cl_a = if mode_z {
            vehicle.cl_a_z
        } else {
            vehicle.cl_a_x
        };

        let f_drag = q * cd_a;
        let f_roll = vehicle.c_rr * (vehicle.mass_kg * vehicle.g + q * cl_a);
        let f_slope = vehicle.mass_kg * vehicle.g * track.slope[i];

        let (pwr_max, _rpm_max, _gear) = vehicle.max_engine_power(v_curr, vehicle.t_init);
        let f_eng_max = pwr_max * 1000.0 / v_curr.max(10.0);

        let normal_load = vehicle.mass_kg * vehicle.g + q * cl_a;
        let f_traction = vehicle.mu * normal_load;
        let f_drive = f_eng_max.min(f_traction);

        let a = (f_drive - f_drag - f_roll - f_slope) / vehicle.mass_kg;
        v_fwd[i + 1] = (v_curr.powi(2) + 2.0 * a * ds).max(0.0).sqrt();
    }

    for i in 0..n {
        v_fwd[i] = v_fwd[i].min(v_corner[i]);
    }

    let mut v_bwd = v_fwd.clone();
    for i in (0..n - 1).rev() {
        let v_target = v_bwd[i + 1];
        let q = 0.5 * vehicle.rho * v_target * v_target;
        let f_drag = q * vehicle.cd_a_z;
        let f_roll = vehicle.c_rr * (vehicle.mass_kg * vehicle.g + q * vehicle.cl_a_z);
        let f_slope = vehicle.mass_kg * vehicle.g * track.slope[i];

        let normal_load = vehicle.mass_kg * vehicle.g + q * vehicle.cl_a_z;
        let grip_total = vehicle.mu * normal_load;
        let f_lat_req = vehicle.mass_kg * v_target.powi(2) * track.curvature[i].abs();

        let f_brake_max = if f_lat_req >= grip_total {
            0.0
        } else {
            (grip_total.powi(2) - f_lat_req.powi(2)).sqrt()
        };
        let mut a_decel = (f_brake_max + f_drag + f_roll + f_slope) / vehicle.mass_kg;
        a_decel = a_decel.min(6.0 * vehicle.g);

        let v_max = (v_target.powi(2) + 2.0 * a_decel * ds).sqrt();
        if v_bwd[i] > v_max {
            v_bwd[i] = v_max;
        }
    }

    let v_final: Vec<f32> = v_fwd
        .iter()
        .zip(v_bwd.iter())
        .map(|(f, b)| f.min(*b))
        .collect();

    let mut t = vec![0.0; n];
    let v_safe: Vec<f32> = v_final.iter().map(|v| v.max(1.0)).collect();
    for i in 1..n {
        let v_avg = 0.5 * (v_safe[i] + v_safe[i - 1]);
        t[i] = t[i - 1] + ds / v_avg;
    }

    SpeedProfile {
        s: track.s.clone(),
        t,
        v: v_final,
    }
}

fn resample_telemetry(
    track: &TrackProfile,
    sol: &SpeedProfile,
    vehicle: &VehicleSpec,
    hz: f32,
) -> ResampleOutput {
    let t_end = sol.t.last().copied().unwrap_or(0.0);
    let dt_frame = 1.0 / hz;

    let mut t = Vec::new();
    let mut cursor = 0.0;
    while cursor < t_end {
        t.push(cursor);
        cursor += dt_frame;
    }

    if t.is_empty() {
        return ResampleOutput {
            telemetry: Vec::new(),
            last_temp_c: vehicle.t_init,
            last_time_s: 0.0,
            last_s_m: 0.0,
        };
    }

    let s_t = interp_series(&t, &sol.t, &sol.s);
    let v_t = interp_series(&t, &sol.t, &sol.v);

    let x_t = interp_series(&s_t, &track.s, &track.x);
    let y_t = interp_series(&s_t, &track.s, &track.y);
    let heading_t = interp_series(&s_t, &track.s, &track.heading);
    let k_t = interp_series(&s_t, &track.s, &track.curvature);

    let a_long = gradient(&v_t, &t);

    let mut telemetry = Vec::with_capacity(t.len());
    let mut current_temp = vehicle.t_init;
    let mut current_gear: usize = 1;
    let mut last_shift = -1.0f32;

    for i in 0..t.len() {
        let v = v_t[i];
        let al = a_long[i];

        let mut gear_ratio = vehicle.gear_ratios[current_gear - 1];
        let r_curr = v * 60.0 * gear_ratio / (2.0 * std::f32::consts::PI * vehicle.wheel_radius_m);

        if (t[i] - last_shift) > 0.2 {
            if r_curr > vehicle.n_upshift && current_gear < vehicle.gear_count as usize {
                current_gear += 1;
                last_shift = t[i];
            } else if r_curr < vehicle.n_downshift && current_gear > 1 {
                current_gear -= 1;
                last_shift = t[i];
            }
        }

        gear_ratio = vehicle.gear_ratios[current_gear - 1];
        let r_final = v * 60.0 * gear_ratio / (2.0 * std::f32::consts::PI * vehicle.wheel_radius_m);
        let rpm = r_final.clamp(vehicle.n_idle, vehicle.n_max);

        let (thr, brk) = if al >= 0.0 {
            ((al / 5.0).clamp(0.0, 1.0), 0.0)
        } else {
            (0.0, (-al / 10.0).clamp(0.0, 1.0))
        };

        let p_avail = vehicle.power_kw_from_rpm(rpm) * 1000.0;
        let p_out = p_avail * thr;

        let heat_in = vehicle.alpha_heat * p_out;
        let heat_out = vehicle.p_cool0 + vehicle.k_cool * v;
        let delta_t = (heat_in - heat_out) / vehicle.c_th * dt_frame;
        current_temp += delta_t;

        let g_lat = (v * v * k_t[i]) / 9.81;
        let g_long = al / 9.81;

        telemetry.push(TelemetryPoint {
            time_s: t[i],
            s_m: s_t[i],
            x_m: x_t[i],
            y_m: y_t[i],
            heading_rad: heading_t[i],
            speed_kph: v * 3.6,
            rpm,
            gear: current_gear as i32,
            throttle_pct: thr * 100.0,
            brake_pct: brk * 100.0,
            g_lat,
            g_long,
            engine_temp_c: current_temp,
            engine_power_w: p_out,
        });
    }

    let last_time_s = *t.last().unwrap_or(&0.0);
    let last_s_m = *s_t.last().unwrap_or(&0.0);

    ResampleOutput {
        telemetry,
        last_temp_c: current_temp,
        last_time_s,
        last_s_m,
    }
}

fn interp_series(x: &[f32], xp: &[f32], fp: &[f32]) -> Vec<f32> {
    let mut out = Vec::with_capacity(x.len());
    let mut idx = 0usize;

    for &xq in x {
        if xq <= xp[0] {
            out.push(fp[0]);
            continue;
        }
        if xq >= *xp.last().unwrap() {
            out.push(*fp.last().unwrap());
            continue;
        }
        while idx + 1 < xp.len() && xp[idx + 1] < xq {
            idx += 1;
        }
        let x0 = xp[idx];
        let x1 = xp[idx + 1];
        let y0 = fp[idx];
        let y1 = fp[idx + 1];
        let t = (xq - x0) / (x1 - x0);
        out.push(y0 + (y1 - y0) * t);
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
