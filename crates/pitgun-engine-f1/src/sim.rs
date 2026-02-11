use std::error::Error;
use std::fmt::{Display, Formatter};

use crate::core::Tuning;
use crate::vehicle::Vehicle;
use crate::{core::Aero, core::Chassis, core::Engine};

#[derive(Debug, Clone)]
pub struct TrackProfile {
    pub s: Vec<f64>,
    pub x: Vec<f64>,
    pub y: Vec<f64>,
    pub z: Vec<f64>,
    pub kappa: Vec<f64>,
    pub slope: Vec<f64>,
    pub heading: Vec<f64>,
}

#[derive(Debug, Clone)]
pub struct SimConfig {
    pub lap_number: usize,
    pub hz: f64,
}

impl Default for SimConfig {
    fn default() -> Self {
        Self {
            lap_number: 2,
            hz: 60.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SpeedSolution {
    pub s: Vec<f64>,
    pub t: Vec<f64>,
    pub v: Vec<f64>,
    pub v_corner: Vec<f64>,
    pub power_kw: Vec<f64>,
    pub temp_c: Vec<f64>,
    pub gear: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct TelemetryData {
    pub time_s: Vec<f64>,
    pub s_m: Vec<f64>,
    pub x_m: Vec<f64>,
    pub y_m: Vec<f64>,
    pub heading_rad: Vec<f64>,
    pub speed_kph: Vec<f64>,
    pub rpm: Vec<f64>,
    pub gear: Vec<i32>,
    pub throttle_pct: Vec<f64>,
    pub brake_pct: Vec<f64>,
    pub g_lat: Vec<f64>,
    pub g_long: Vec<f64>,
    pub g_vert: Vec<f64>,
    pub engine_temp_c: Vec<f64>,
    pub engine_power_w: Vec<f64>,
}

#[derive(Debug, Clone)]
pub struct SimulationOutput {
    pub solution: SpeedSolution,
    pub telemetry: TelemetryData,
}

#[derive(Debug)]
pub enum SimError {
    InvalidTrack(&'static str),
    InvalidConfig(&'static str),
}

impl Display for SimError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidTrack(msg) => write!(f, "invalid track: {msg}"),
            Self::InvalidConfig(msg) => write!(f, "invalid sim config: {msg}"),
        }
    }
}

impl Error for SimError {}

pub fn run_simulation_with_tuning<A: Aero, C: Chassis, E: Engine>(
    track: &TrackProfile,
    vehicle: &mut Vehicle<A, C, E>,
    tuning: &Tuning,
    config: &SimConfig,
) -> Result<SimulationOutput, SimError> {
    // Cumulative tuning model is preserved; call once per run.
    vehicle.apply_tuning(tuning);
    run_simulation(track, vehicle, config)
}

pub fn run_simulation<A: Aero, C: Chassis, E: Engine>(
    track: &TrackProfile,
    vehicle: &Vehicle<A, C, E>,
    config: &SimConfig,
) -> Result<SimulationOutput, SimError> {
    if config.lap_number == 0 {
        return Err(SimError::InvalidConfig("lap_number must be >= 1"));
    }
    if config.hz <= 0.0 {
        return Err(SimError::InvalidConfig("hz must be > 0"));
    }
    validate_track(track)?;

    let solution = compute_speed_profile(track, vehicle, config.lap_number)?;
    let telemetry = resample_telemetry(track, &solution, vehicle, config.hz)?;

    Ok(SimulationOutput {
        solution,
        telemetry,
    })
}

pub fn compute_speed_profile<A: Aero, C: Chassis, E: Engine>(
    track: &TrackProfile,
    vehicle: &Vehicle<A, C, E>,
    lap_number: usize,
) -> Result<SpeedSolution, SimError> {
    validate_track(track)?;
    if lap_number == 0 {
        return Err(SimError::InvalidConfig("lap_number must be >= 1"));
    }

    let n = track.s.len();
    let ds = track.s[1] - track.s[0];
    if ds <= 0.0 {
        return Err(SimError::InvalidTrack("s must be strictly increasing"));
    }

    let m = vehicle.chassis.mass();
    let rho = vehicle.chassis.air_density();
    let g = vehicle.chassis.gravity();
    let mu = vehicle.chassis.friction_mu();
    let c_rr = vehicle.chassis.rolling_resistance();
    let (cda_x, cla_x) = vehicle.aero.coeffs_straight();
    let (cda_z, cla_z) = vehicle.aero.coeffs_corner();

    let slope_change = gradient_uniform(&track.slope);

    let mut v_corner = vec![120.0; n];
    for i in 0..n {
        let k_val = track.kappa[i].abs();
        if k_val < 1e-5 {
            v_corner[i] = 400.0;
            continue;
        }
        let mut v = 70.0;
        for _ in 0..5 {
            let q = 0.5 * rho * v * v;
            let downforce = q * cla_z;
            let a_vert = v * v * slope_change[i] / (ds * ds);
            let a_lat_max = mu * (g + a_vert + downforce / m);
            v = (a_lat_max / k_val).max(1e-1).sqrt();
        }
        v_corner[i] = v.min(400.0);
    }

    let mut v_bwd = v_corner.clone();
    for i in (0..(n - 1)).rev() {
        let v_target = v_bwd[i + 1];

        let q = 0.5 * rho * v_target * v_target;
        let drag = q * cda_z;
        let lift = q * cla_z;

        let f_drag = drag;
        let f_roll = c_rr * (m * g + lift);
        let f_slope = m * g * track.slope[i];

        let a_vert = v_target * v_target * slope_change[i] / (ds * ds);
        let normal_load = m * (g + a_vert) + lift;
        let grip_avail = mu * normal_load;
        let f_lat_req = m * (v_target * v_target) * track.kappa[i].abs();

        let f_brake_max = if f_lat_req >= grip_avail {
            0.0
        } else {
            (grip_avail * grip_avail - f_lat_req * f_lat_req).sqrt()
        };

        let f_decel_avail = f_brake_max + f_drag + f_roll + f_slope;
        let a_decel = (f_decel_avail / m).min(6.0 * g);

        let v_max_braking = (v_target * v_target + 2.0 * a_decel * ds).sqrt();
        if v_bwd[i] > v_max_braking {
            v_bwd[i] = v_max_braking;
        }
    }

    let mut v_fwd: Vec<f64> = vec![0.0; n];
    v_fwd[n - 1] = 30.0;
    let mut temp: Vec<f64> = vec![0.0; n];
    temp[n - 1] = vehicle.engine.thermal_init_c();
    let mut gear = vec![1u8; n];
    let mut power_kw: Vec<f64> = vec![0.0; n];

    for _lap in 0..lap_number {
        v_fwd[0] = v_fwd[n - 1];
        temp[0] = temp[n - 1];
        gear[0] = gear[n - 1];

        for i in 0..(n - 1) {
            let v = v_fwd[i].min(v_bwd[i]).max(1e-6);
            let dt = ds / v;
            let (pwr, _rpm, best_gear) = vehicle.max_engine_power(v, temp[i]);

            if v_fwd[i] >= v_bwd[i] {
                power_kw[i] = 0.0;
                v_fwd[i + 1] = v_bwd[i];
            } else {
                let mode_z = track.kappa[i].abs() > 0.001;
                let (cda, cla) = if mode_z {
                    (cda_z, cla_z)
                } else {
                    (cda_x, cla_x)
                };

                let q = 0.5 * rho * v * v;
                let a_vert = v * v * slope_change[i] / (ds * ds);
                let f_drag = q * cda;
                let f_roll = c_rr * (m * (g + a_vert) + q * cla);
                let f_slope = m * g * track.slope[i];

                let f_eng_max = 1000.0 * pwr / v.max(10.0);
                let normal_load = m * (g + a_vert) + q * cla;
                let f_drive = f_eng_max.min(mu * normal_load);
                power_kw[i] = if f_eng_max > 0.0 {
                    pwr * (f_drive / f_eng_max)
                } else {
                    0.0
                };

                let f_net = f_drive - f_drag - f_roll - f_slope;
                let a = f_net / m;
                v_fwd[i + 1] = (v * v + 2.0 * a * ds).max(0.0).sqrt();
            }

            let heat = 1000.0 * vehicle.engine.heat_alpha() * power_kw[i];
            let cool = (vehicle.engine.cooling_base_w() + vehicle.engine.cooling_speed_w_per_ms() * v)
                * (temp[i] - vehicle.engine.ambient_temp_c());
            temp[i + 1] = temp[i] + ((heat - cool) / vehicle.engine.thermal_capacity_j_per_c()) * dt;

            if i > 0 {
                let rpm_current_gear = vehicle.rpm_from_speed_gear(v, gear[i - 1]);
                let power_current_gear =
                    vehicle.derating_factor(temp[i]) * vehicle.power_kw_from_rpm(rpm_current_gear);
                if power_current_gear >= power_kw[i]
                    && rpm_current_gear >= vehicle.engine.idle_rpm()
                    && rpm_current_gear <= vehicle.engine.max_rpm()
                {
                    gear[i] = gear[i - 1];
                } else {
                    gear[i] = best_gear;
                }
            }
        }

        gear[n - 1] = gear[n - 2];
    }

    let v_final: Vec<f64> = v_fwd
        .iter()
        .zip(v_bwd.iter())
        .map(|(f, b)| f.min(*b))
        .collect();

    let mut dt = vec![0.0; n];
    for i in 1..n {
        let v_safe_i = v_final[i].max(1.0);
        let v_safe_prev = v_final[i - 1].max(1.0);
        dt[i] = ds / (0.5 * (v_safe_i + v_safe_prev));
    }
    let t = cumulative_sum(&dt);

    Ok(SpeedSolution {
        s: track.s.clone(),
        t,
        v: v_final,
        v_corner,
        power_kw,
        temp_c: temp,
        gear,
    })
}

pub fn resample_telemetry<A: Aero, C: Chassis, E: Engine>(
    track: &TrackProfile,
    solution: &SpeedSolution,
    vehicle: &Vehicle<A, C, E>,
    hz: f64,
) -> Result<TelemetryData, SimError> {
    validate_track(track)?;
    if hz <= 0.0 {
        return Err(SimError::InvalidConfig("hz must be > 0"));
    }

    let n = solution.t.len();
    if n < 2 {
        return Err(SimError::InvalidTrack("solution must contain at least 2 points"));
    }

    let t_end = *solution
        .t
        .last()
        .ok_or(SimError::InvalidTrack("empty solution time vector"))?;
    let t = build_time_samples(t_end, hz);

    let s_t: Vec<f64> = t
        .iter()
        .map(|&ti| interp_1d(ti, &solution.t, &solution.s))
        .collect();
    let v_t: Vec<f64> = t
        .iter()
        .map(|&ti| interp_1d(ti, &solution.t, &solution.v))
        .collect();
    let power_t: Vec<f64> = t
        .iter()
        .map(|&ti| interp_1d(ti, &solution.t, &solution.power_kw))
        .collect();
    let temp_t: Vec<f64> = t
        .iter()
        .map(|&ti| interp_1d(ti, &solution.t, &solution.temp_c))
        .collect();
    let gear_t: Vec<i32> = t
        .iter()
        .map(|&ti| interp_1d(ti, &solution.t, &u8_vec_to_f64(&solution.gear)).round() as i32)
        .collect();

    let x_t: Vec<f64> = s_t.iter().map(|&si| interp_1d(si, &track.s, &track.x)).collect();
    let y_t: Vec<f64> = s_t.iter().map(|&si| interp_1d(si, &track.s, &track.y)).collect();
    let heading_t: Vec<f64> = s_t
        .iter()
        .map(|&si| interp_1d(si, &track.s, &track.heading))
        .collect();
    let kappa_t: Vec<f64> = s_t
        .iter()
        .map(|&si| interp_1d(si, &track.s, &track.kappa))
        .collect();
    let slope_t: Vec<f64> = s_t
        .iter()
        .map(|&si| interp_1d(si, &track.s, &track.slope))
        .collect();
    let slope_change_t = gradient_uniform(&slope_t);

    let a_long = moving_average_same(&gradient(&v_t, &t), 5);

    let mut throttle = vec![0.0; t.len()];
    let mut brake = vec![0.0; t.len()];
    let mut rpm = vec![0.0; t.len()];
    let mut power_out_kw = vec![0.0; t.len()];

    for i in 0..t.len() {
        let v = v_t[i];
        let gear_i = gear_t[i].clamp(1, vehicle.engine.gear_count() as i32) as u8;
        let g_ratio = vehicle.engine.gear_ratio(gear_i);
        rpm[i] = v * 60.0 * g_ratio / (2.0 * std::f64::consts::PI * vehicle.chassis.wheel_radius());

        let p_theo = vehicle.power_kw_from_rpm(rpm[i]);
        let p_act = p_theo * vehicle.derating_factor(temp_t[i]);

        if power_t[i] > 0.0 {
            brake[i] = 0.0;
            throttle[i] = (p_act / power_t[i]).clamp(0.0, 1.2);
        } else {
            throttle[i] = 0.0;
            brake[i] = 1.0;
        }
        power_out_kw[i] = p_act * throttle[i];
    }

    let g_ref = vehicle.chassis.gravity();
    let g_lat: Vec<f64> = v_t
        .iter()
        .zip(kappa_t.iter())
        .map(|(v, k)| (v * v * k) / g_ref)
        .collect();
    let g_long: Vec<f64> = a_long.iter().map(|a| a / g_ref).collect();

    let grad_s = gradient_uniform(&s_t);
    let g_vert: Vec<f64> = v_t
        .iter()
        .zip(slope_change_t.iter())
        .zip(grad_s.iter())
        .map(|((v, dslope), ds_local)| (v * v * dslope) / g_ref / (ds_local * ds_local).max(1e-9))
        .collect();

    Ok(TelemetryData {
        time_s: t,
        s_m: s_t,
        x_m: x_t,
        y_m: y_t,
        heading_rad: heading_t,
        speed_kph: v_t.iter().map(|v| v * 3.6).collect(),
        rpm,
        gear: gear_t,
        throttle_pct: throttle.iter().map(|v| v * 100.0).collect(),
        brake_pct: brake.iter().map(|v| v * 100.0).collect(),
        g_lat,
        g_long,
        g_vert,
        engine_temp_c: temp_t,
        engine_power_w: power_out_kw.iter().map(|p| p * 1000.0).collect(),
    })
}

fn validate_track(track: &TrackProfile) -> Result<(), SimError> {
    let n = track.s.len();
    if n < 2 {
        return Err(SimError::InvalidTrack("need at least 2 track points"));
    }
    if track.x.len() != n
        || track.y.len() != n
        || track.z.len() != n
        || track.kappa.len() != n
        || track.slope.len() != n
        || track.heading.len() != n
    {
        return Err(SimError::InvalidTrack(
            "all track vectors must have identical lengths",
        ));
    }
    for i in 1..n {
        if track.s[i] <= track.s[i - 1] {
            return Err(SimError::InvalidTrack("s must be strictly increasing"));
        }
    }
    Ok(())
}

fn build_time_samples(t_end: f64, hz: f64) -> Vec<f64> {
    if t_end <= 0.0 {
        return vec![0.0];
    }
    let dt = 1.0 / hz;
    let mut t = Vec::new();
    let mut acc = 0.0;
    while acc < t_end {
        t.push(acc);
        acc += dt;
    }
    if t.is_empty() {
        t.push(0.0);
    }
    t
}

fn u8_vec_to_f64(data: &[u8]) -> Vec<f64> {
    data.iter().map(|v| *v as f64).collect()
}

fn interp_1d(x: f64, xp: &[f64], fp: &[f64]) -> f64 {
    if xp.is_empty() || fp.is_empty() {
        return 0.0;
    }
    if x <= xp[0] {
        return fp[0];
    }
    if x >= xp[xp.len() - 1] {
        return fp[fp.len() - 1];
    }

    let hi = xp.partition_point(|&v| v <= x);
    let i0 = hi.saturating_sub(1);
    let x0 = xp[i0];
    let x1 = xp[hi];
    if (x1 - x0).abs() < f64::EPSILON {
        return fp[i0];
    }
    let t = (x - x0) / (x1 - x0);
    fp[i0] + t * (fp[hi] - fp[i0])
}

fn gradient_uniform(y: &[f64]) -> Vec<f64> {
    let n = y.len();
    if n < 2 {
        return vec![0.0; n];
    }
    let mut out = vec![0.0; n];
    out[0] = y[1] - y[0];
    for i in 1..(n - 1) {
        out[i] = (y[i + 1] - y[i - 1]) * 0.5;
    }
    out[n - 1] = y[n - 1] - y[n - 2];
    out
}

fn gradient(y: &[f64], x: &[f64]) -> Vec<f64> {
    let n = y.len();
    if n < 2 {
        return vec![0.0; n];
    }
    let mut out = vec![0.0; n];
    let dx0 = (x[1] - x[0]).max(1e-12);
    out[0] = (y[1] - y[0]) / dx0;
    for i in 1..(n - 1) {
        let dxi = (x[i + 1] - x[i - 1]).max(1e-12);
        out[i] = (y[i + 1] - y[i - 1]) / dxi;
    }
    let dxn = (x[n - 1] - x[n - 2]).max(1e-12);
    out[n - 1] = (y[n - 1] - y[n - 2]) / dxn;
    out
}

fn moving_average_same(data: &[f64], window_len: usize) -> Vec<f64> {
    if window_len < 3 {
        return data.to_vec();
    }
    let n = data.len();
    let mut out = vec![0.0; n];
    let half = window_len / 2;
    for (i, out_i) in out.iter_mut().enumerate().take(n) {
        let start = i.saturating_sub(half);
        let end = (i + half + 1).min(n);
        let mut sum = 0.0;
        for val in &data[start..end] {
            sum += *val;
        }
        *out_i = sum / (end - start) as f64;
    }
    out
}

fn cumulative_sum(data: &[f64]) -> Vec<f64> {
    let mut out = Vec::with_capacity(data.len());
    let mut acc = 0.0;
    for &v in data {
        acc += v;
        out.push(acc);
    }
    out
}
