use md5::{Digest, Md5};
use rand::rngs::StdRng;
use rand::{RngCore, SeedableRng};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AeroParams {
    pub cd_a_x: f64,
    pub cd_a_z: f64,
    pub cl_a_x: f64,
    pub cl_a_z: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChassisParams {
    pub mass_empty: f64,
    pub r_wheel: f64,
    pub mu0: f64,
    pub c_rr: f64,
    pub rho: f64,
    pub g: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EngineParams {
    pub n_rpm: Vec<f64>,
    pub trq: Vec<f64>,
    pub gear_ratios: Vec<f64>,
    pub n_upshift: f64,
    pub n_downshift: f64,
    pub n_idle: f64,
    pub n_max: f64,
    pub t_amb: f64,
    pub t_init: f64,
    pub c_th: f64,
    pub alpha_heat: f64,
    pub p_cool0: f64,
    pub k_cool: f64,
    pub t_soft: f64,
    pub beta_derate: f64,
    pub fuel_burn_kg_per_s: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TireParams {
    pub mu_scale: f64,
    pub wear_per_s: f64,
    pub wear_load_k: f64,
    pub wear_grip_k: f64,
    pub wear_min: f64,
    pub temp_opt: f64,
    pub temp_sigma: f64,
    pub temp_min_k: f64,
    pub heat_k: f64,
    pub cool_k: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HybridParams {
    pub battery_capacity_kwh: f64,
    pub battery_min_soc: f64,
    pub battery_max_soc: f64,
    pub max_deploy_kw: f64,
    pub max_regen_kw: f64,
    pub deploy_efficiency: f64,
    pub regen_efficiency: f64,
    pub mass_kg: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VehicleParams {
    pub chassis: ChassisParams,
    pub aero: AeroParams,
    pub engine: EngineParams,
    pub tire: TireParams,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hybrid: Option<HybridParams>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VehicleState {
    pub fuel_mass: f64,
    pub tire_wear: f64,
    pub tire_temp: f64,
    pub engine_temp: f64,
    #[serde(default = "default_battery_soc")]
    pub battery_soc: f64,
    #[serde(default)]
    pub exit_speed_mps: f64,
    #[serde(default = "default_exit_gear")]
    pub exit_gear: u8,
}

impl Default for VehicleState {
    fn default() -> Self {
        Self {
            fuel_mass: 100.0,
            tire_wear: 0.0,
            tire_temp: 90.0,
            engine_temp: 90.0,
            battery_soc: default_battery_soc(),
            exit_speed_mps: 0.0,
            exit_gear: default_exit_gear(),
        }
    }
}

impl VehicleState {
    pub fn total_mass_delta(&self) -> f64 {
        self.fuel_mass
    }
}

const fn default_exit_gear() -> u8 {
    1
}

const fn default_battery_soc() -> f64 {
    0.0
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Track {
    pub s: Vec<f64>,
    pub x: Vec<f64>,
    pub y: Vec<f64>,
    pub z: Vec<f64>,
    pub kappa: Vec<f64>,
    pub slope: Vec<f64>,
    pub heading: Vec<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SimConfig {
    pub ds: f64,
    pub max_speed: f64,
    pub pit_time_penalty_s: f64,
    pub pit_tire_temp: Option<f64>,
    pub tire_temp_amb: f64,
    pub sim_seed: u64,
}

impl Default for SimConfig {
    fn default() -> Self {
        Self {
            ds: 0.0,
            max_speed: 400.0,
            pit_time_penalty_s: 20.0,
            pit_tire_temp: None,
            tire_temp_amb: 35.0,
            sim_seed: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Tuning {
    pub aero_points: i32,
    pub chassis_points: i32,
    pub cooling_points: i32,
    pub engine_points: i32,
    pub downforce_slider: f64,
    pub gear_ratio_slider: f64,
}

impl Default for Tuning {
    fn default() -> Self {
        Self {
            aero_points: 0,
            chassis_points: 0,
            cooling_points: 0,
            engine_points: 0,
            downforce_slider: 0.0,
            gear_ratio_slider: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Driver {
    pub id: String,
    pub display_name: String,
    pub aggressiveness: f64,
}

impl Default for Driver {
    fn default() -> Self {
        Self {
            id: "default".to_string(),
            display_name: "Default Driver".to_string(),
            aggressiveness: 0.5,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DriverEffects {
    pub tire_wear_multiplier: f64,
    pub lap_time_noise_std_ms: i32,
    pub peak_pace_bonus_ms: i32,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EnergyMode {
    #[default]
    Balanced,
    Attack,
    Harvest,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PitStop {
    pub lap: u16,
    pub tire: TireParams,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct PitPlan {
    #[serde(default)]
    pub stops: Vec<PitStop>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SimulationRequest {
    pub track: Track,
    pub vehicle: VehicleParams,
    pub state: VehicleState,
    #[serde(default)]
    pub config: SimConfig,
    #[serde(default)]
    pub energy_mode: EnergyMode,
    #[serde(default = "default_lap_count")]
    pub lap_count: u16,
    #[serde(default)]
    pub pit_plan: PitPlan,
    #[serde(default)]
    pub driver: Driver,
    #[serde(default)]
    pub tuning: Option<Tuning>,
}

fn default_lap_count() -> u16 {
    1
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SimulationSolution {
    pub s: Vec<f64>,
    pub t: Vec<f64>,
    pub v: Vec<f64>,
    pub power: Vec<f64>,
    pub temp: Vec<f64>,
    pub gear: Vec<u8>,
    pub lap_index: Vec<u16>,
    pub tire_temp: Vec<f64>,
    pub tire_wear: Vec<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SimulationResult {
    pub solution: SimulationSolution,
    pub final_state: VehicleState,
    pub lap_times_s: Vec<f64>,
    pub total_time_s: f64,
    pub applied_vehicle: VehicleParams,
    pub applied_driver: Driver,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ResampledTelemetry {
    pub time_s: Vec<f64>,
    pub s_m: Vec<f64>,
    pub x_m: Vec<f64>,
    pub y_m: Vec<f64>,
    pub heading_rad: Vec<f64>,
    pub speed_kph: Vec<f64>,
    pub rpm: Vec<f64>,
    pub gear: Vec<u8>,
    pub throttle_pct: Vec<f64>,
    pub brake_pct: Vec<f64>,
    pub g_lat: Vec<f64>,
    pub g_long: Vec<f64>,
    pub g_vert: Vec<f64>,
    pub engine_temp_c: Vec<f64>,
    pub engine_power_w: Vec<f64>,
    pub tire_temp_c: Option<Vec<f64>>,
    pub tire_wear_pct: Option<Vec<f64>>,
    pub tire_mu: Option<Vec<f64>>,
    pub n_lap: Option<Vec<u16>>,
}

pub fn run_simulation(input: &SimulationRequest) -> Result<SimulationResult, String> {
    validate_track(&input.track)?;

    let lap_count = input.lap_count.max(1);
    let driver = input.driver.clone();
    let effects = driver_effects(&driver);
    let tuned_vehicle = match &input.tuning {
        Some(tuning) => apply_tuning(&input.vehicle, tuning),
        None => input.vehicle.clone(),
    };
    let mut vehicle = tuned_vehicle.clone();
    vehicle.tire = apply_driver_to_tire(&vehicle.tire, &effects);

    let s = &input.track.s;
    let n = s.len();
    let ds = if input.config.ds > 0.0 {
        input.config.ds
    } else {
        (s[1] - s[0]).abs().max(1e-9)
    };
    let slope_change = gradient_equal_spacing(&input.track.slope);

    let mut out_s = Vec::new();
    let mut out_t = Vec::new();
    let mut out_v = Vec::new();
    let mut out_power = Vec::new();
    let mut out_temp = Vec::new();
    let mut out_gear = Vec::new();
    let mut out_lap = Vec::new();
    let mut out_tire_temp = Vec::new();
    let mut out_tire_wear = Vec::new();

    let mut state_curr = input.state.clone();
    let initial_tire_temp = input.state.tire_temp;
    let mut t_offset = 0.0;
    let mut s_offset = 0.0;
    let mut prev_end_speed: Option<f64> = if input.state.exit_speed_mps > 0.0 {
        Some(input.state.exit_speed_mps)
    } else {
        None
    };
    let mut prev_end_gear: Option<u8> = if input.state.exit_gear > 0 {
        Some(input.state.exit_gear)
    } else {
        None
    };
    let mut lap_times_s = Vec::with_capacity(lap_count as usize);

    let mut pit_stops = input.pit_plan.stops.clone();
    pit_stops.sort_by_key(|stop| stop.lap);

    for lap_idx in 1..=lap_count {
        let mass = vehicle.chassis.mass_empty + state_curr.total_mass_delta();
        let tire_curr = tire_for_lap(&vehicle.tire, &pit_stops, lap_idx);
        let v_corner = corner_speed_limit(
            &input.track,
            &vehicle,
            &state_curr,
            &input.config,
            &tire_curr,
        );

        let mut v_bwd = v_corner.clone();
        for i in (0..(n - 1)).rev() {
            let v_target = v_bwd[i + 1];
            let (drag, downforce) = aero_forces(v_target, &vehicle.aero, &vehicle.chassis, true);

            let f_drag = drag;
            let f_roll = vehicle.chassis.c_rr * (mass * vehicle.chassis.g + downforce);
            let f_slope = mass * vehicle.chassis.g * input.track.slope[i];

            let a_vert = v_target * v_target * slope_change[i] / ds / ds;
            let normal_load = mass * (vehicle.chassis.g + a_vert) + downforce;
            let mu_eff = effective_mu(
                vehicle.chassis.mu0,
                state_curr.tire_wear,
                state_curr.tire_temp,
                &tire_curr,
            );
            let grip_avail = mu_eff * normal_load;

            let f_lat_req = mass * v_target * v_target * input.track.kappa[i].abs();
            let f_brake_max = if f_lat_req >= grip_avail {
                0.0
            } else {
                (grip_avail * grip_avail - f_lat_req * f_lat_req).sqrt()
            };

            let mut a_decel = (f_brake_max + f_drag + f_roll + f_slope) / mass.max(1e-9);
            a_decel = a_decel.min(6.0 * vehicle.chassis.g);

            let v_max_braking = (v_target * v_target + 2.0 * a_decel * ds).max(0.0).sqrt();
            if v_bwd[i] > v_max_braking {
                v_bwd[i] = v_max_braking;
            }
        }

        let mut v_fwd = vec![0.0; n];
        let mut temp = vec![0.0; n];
        let mut tire_temp = vec![0.0; n];
        let mut tire_wear = vec![0.0; n];
        let mut gear = vec![1u8; n];
        let mut power = vec![0.0; n];

        v_fwd[n - 1] = match prev_end_speed {
            Some(speed) => speed.min(v_bwd[n - 1]),
            None => 0.0,
        };
        temp[n - 1] = state_curr.engine_temp;
        tire_temp[n - 1] = state_curr.tire_temp;
        tire_wear[n - 1] = state_curr.tire_wear;
        gear[n - 1] = prev_end_gear.unwrap_or(1);

        v_fwd[0] = v_fwd[n - 1];
        temp[0] = temp[n - 1];
        tire_temp[0] = tire_temp[n - 1];
        tire_wear[0] = tire_wear[n - 1];
        gear[0] = gear[n - 1];

        for i in 0..(n - 1) {
            let v = v_fwd[i].min(v_bwd[i]);
            let v_safe = v.max(1.0);
            let dt = ds / v_safe;

            let (mut pwr, _, best_gear) =
                best_power_at_speed(v_safe, &vehicle.engine, &vehicle.chassis);
            pwr *= derating_factor(temp[i], &vehicle.engine);

            if v_fwd[i] >= v_bwd[i] {
                power[i] = 0.0;
                v_fwd[i + 1] = v_bwd[i];
            } else {
                let mode_corner = input.track.kappa[i].abs() > 0.001;
                let (drag, downforce) =
                    aero_forces(v_safe, &vehicle.aero, &vehicle.chassis, mode_corner);

                let a_vert = v_safe * v_safe * slope_change[i] / ds / ds;
                let f_drag = drag;
                let f_roll =
                    vehicle.chassis.c_rr * (mass * (vehicle.chassis.g + a_vert) + downforce);
                let f_slope = mass * vehicle.chassis.g * input.track.slope[i];

                let f_eng_max = 1000.0 * pwr / v_safe.max(10.0);
                let normal_load = mass * (vehicle.chassis.g + a_vert) + downforce;
                let mu_eff =
                    effective_mu(vehicle.chassis.mu0, tire_wear[i], tire_temp[i], &tire_curr);
                let f_drive = f_eng_max.min(mu_eff * normal_load);

                power[i] = if f_eng_max > 0.0 {
                    pwr * (f_drive / f_eng_max)
                } else {
                    0.0
                };

                let f_net = f_drive - f_drag - f_roll - f_slope;
                let a = f_net / mass.max(1e-9);
                v_fwd[i + 1] = (v_safe * v_safe + 2.0 * a * ds).max(0.0).sqrt();
            }

            let heat = 1000.0 * vehicle.engine.alpha_heat * power[i];
            let cool = (vehicle.engine.p_cool0 + vehicle.engine.k_cool * v_safe)
                * (temp[i] - vehicle.engine.t_amb);
            temp[i + 1] = temp[i] + (heat - cool) / vehicle.engine.c_th.max(1e-9) * dt;

            let a_long = (v_fwd[i + 1] * v_fwd[i + 1] - v_safe * v_safe) / (2.0 * ds).max(1e-3);
            let a_lat = v_safe * v_safe * input.track.kappa[i];
            let load_metric = a_lat * a_lat + a_long * a_long;

            let tire_heat = tire_curr.heat_k * load_metric;
            let tire_cool = tire_curr.cool_k * v_safe * (tire_temp[i] - input.config.tire_temp_amb);
            tire_temp[i + 1] = (tire_temp[i] + (tire_heat - tire_cool) * dt).max(0.0);

            let wear_rate = tire_curr.wear_per_s + tire_curr.wear_load_k * load_metric;
            tire_wear[i + 1] = (tire_wear[i] + wear_rate * dt).min(1.0);

            if i > 0 {
                let prev_idx = gear[i - 1].saturating_sub(1) as usize;
                let ratio = vehicle
                    .engine
                    .gear_ratios
                    .get(prev_idx)
                    .copied()
                    .unwrap_or(0.0);
                let rpm_current = rpm_from_speed_gear(v_safe, ratio, &vehicle.chassis);
                let pwr_current = derating_factor(temp[i], &vehicle.engine)
                    * power_kw_from_rpm(rpm_current, &vehicle.engine);
                gear[i] = if vehicle.engine.n_idle <= rpm_current
                    && rpm_current <= vehicle.engine.n_max
                    && pwr_current >= power[i]
                {
                    gear[i - 1]
                } else {
                    best_gear
                };
            }
        }

        gear[n - 1] = if n > 1 { gear[n - 2] } else { gear[n - 1] };

        let v_final: Vec<f64> = v_fwd
            .iter()
            .zip(v_bwd.iter())
            .map(|(left, right)| left.min(*right))
            .collect();

        let mut dt = vec![0.0; n];
        let v_safe: Vec<f64> = v_final.iter().map(|value| value.max(1.0)).collect();
        for i in 1..n {
            dt[i] = ds / (0.5 * (v_safe[i] + v_safe[i - 1]));
        }
        let t = cumulative_sum(&dt);

        let lap_time = *t
            .last()
            .ok_or_else(|| "simulation produced an empty time grid".to_string())?;
        let lap_time_delta_ms = effects.peak_pace_bonus_ms as f64
            + lap_noise_ms(input.config.sim_seed, &driver.id, lap_idx, &effects);
        let lap_time_adj = (lap_time + lap_time_delta_ms / 1000.0).max(0.1);
        let time_scale = lap_time_adj / lap_time.max(1e-6);
        let t_scaled: Vec<f64> = t.iter().map(|value| value * time_scale).collect();
        lap_times_s.push(lap_time_adj);

        let start_idx = if lap_idx == 1 { 0 } else { 1 };
        out_s.extend(
            input.track.s[start_idx..]
                .iter()
                .map(|value| value + s_offset),
        );
        out_t.extend(t_scaled[start_idx..].iter().map(|value| value + t_offset));
        out_v.extend_from_slice(&v_final[start_idx..]);
        out_power.extend_from_slice(&power[start_idx..]);
        out_temp.extend_from_slice(&temp[start_idx..]);
        out_gear.extend_from_slice(&gear[start_idx..]);
        out_tire_temp.extend_from_slice(&tire_temp[start_idx..]);
        out_tire_wear.extend_from_slice(&tire_wear[start_idx..]);
        out_lap.extend((start_idx..n).map(|_| lap_idx));

        t_offset += *t_scaled.last().unwrap_or(&0.0);
        s_offset += *input.track.s.last().unwrap_or(&0.0);
        prev_end_speed = v_final.last().copied();
        prev_end_gear = gear.last().copied();

        let mut fuel_left =
            (state_curr.fuel_mass - vehicle.engine.fuel_burn_kg_per_s * lap_time_adj).max(0.0);
        if !fuel_left.is_finite() {
            fuel_left = 0.0;
        }
        let mut wear_next = *tire_wear.last().unwrap_or(&state_curr.tire_wear);
        let mut tire_temp_next = *tire_temp.last().unwrap_or(&state_curr.tire_temp);

        if let Some(pit_stop) = pit_stops.iter().find(|stop| stop.lap == lap_idx) {
            t_offset += input.config.pit_time_penalty_s.max(0.0);
            wear_next = 0.0;
            tire_temp_next = input.config.pit_tire_temp.unwrap_or(initial_tire_temp);
            vehicle.tire = apply_driver_to_tire(&pit_stop.tire, &effects);
            prev_end_speed = None;
            prev_end_gear = None;
        }

        state_curr = VehicleState {
            fuel_mass: fuel_left,
            tire_wear: wear_next,
            tire_temp: tire_temp_next,
            engine_temp: *temp.last().unwrap_or(&state_curr.engine_temp),
            battery_soc: state_curr.battery_soc,
            exit_speed_mps: prev_end_speed.unwrap_or(0.0),
            exit_gear: prev_end_gear.unwrap_or(default_exit_gear()),
        };
    }

    let solution = SimulationSolution {
        s: out_s,
        t: out_t,
        v: out_v,
        power: out_power,
        temp: out_temp,
        gear: out_gear,
        lap_index: out_lap,
        tire_temp: out_tire_temp,
        tire_wear: out_tire_wear,
    };
    let total_time_s = solution.t.last().copied().unwrap_or(0.0);

    Ok(SimulationResult {
        solution,
        final_state: state_curr,
        lap_times_s,
        total_time_s,
        applied_vehicle: vehicle,
        applied_driver: driver,
    })
}

pub fn resample_telemetry(
    track: &Track,
    solution: &SimulationSolution,
    vehicle: &VehicleParams,
    hz: f64,
) -> Result<ResampledTelemetry, String> {
    validate_track(track)?;
    if solution.t.is_empty() {
        return Ok(ResampledTelemetry::default());
    }

    let t_end = *solution.t.last().unwrap_or(&0.0);
    if t_end <= 0.0 {
        return Ok(ResampledTelemetry::default());
    }

    let dt = 1.0 / hz.max(1e-6);
    let mut t = Vec::new();
    let mut ts = 0.0;
    while ts < t_end {
        t.push(ts);
        ts += dt;
    }
    if t.is_empty() {
        return Ok(ResampledTelemetry::default());
    }

    let s_t: Vec<f64> = t
        .iter()
        .map(|value| interp_linear(*value, &solution.t, &solution.s))
        .collect();
    let v_t: Vec<f64> = t
        .iter()
        .map(|value| interp_linear(*value, &solution.t, &solution.v))
        .collect();
    let power_t: Vec<f64> = t
        .iter()
        .map(|value| interp_linear(*value, &solution.t, &solution.power))
        .collect();
    let temp_t: Vec<f64> = t
        .iter()
        .map(|value| interp_linear(*value, &solution.t, &solution.temp))
        .collect();
    let gear_grid = u8s_to_f64(&solution.gear);
    let lap_grid = u16s_to_f64(&solution.lap_index);

    let gear_t: Vec<u8> = t
        .iter()
        .map(|value| {
            interp_linear(*value, &solution.t, &gear_grid)
                .round()
                .max(1.0) as u8
        })
        .collect();
    let lap_t: Vec<u16> = t
        .iter()
        .map(|value| {
            interp_linear(*value, &solution.t, &lap_grid)
                .round()
                .max(0.0) as u16
        })
        .collect();
    let tire_temp_t: Vec<f64> = t
        .iter()
        .map(|value| interp_linear(*value, &solution.t, &solution.tire_temp))
        .collect();
    let tire_wear_t: Vec<f64> = t
        .iter()
        .map(|value| interp_linear(*value, &solution.t, &solution.tire_wear))
        .collect();

    let track_len = *track.s.last().unwrap_or(&0.0);
    let s_mod: Vec<f64> = if track_len > 0.0 {
        s_t.iter()
            .map(|value| value.rem_euclid(track_len))
            .collect()
    } else {
        s_t.clone()
    };

    let x_t: Vec<f64> = s_mod
        .iter()
        .map(|value| interp_linear(*value, &track.s, &track.x))
        .collect();
    let y_t: Vec<f64> = s_mod
        .iter()
        .map(|value| interp_linear(*value, &track.s, &track.y))
        .collect();
    let heading_t: Vec<f64> = s_mod
        .iter()
        .map(|value| interp_linear(*value, &track.s, &track.heading))
        .collect();
    let kappa_t: Vec<f64> = s_mod
        .iter()
        .map(|value| interp_linear(*value, &track.s, &track.kappa))
        .collect();
    let slope_t: Vec<f64> = s_mod
        .iter()
        .map(|value| interp_linear(*value, &track.s, &track.slope))
        .collect();
    let slope_change_t = gradient_equal_spacing(&slope_t);

    let mut a_long = gradient_with_coords(&v_t, &t);
    a_long = human_smoothing(&a_long, 5);

    let mut throttle = vec![0.0; t.len()];
    let mut brake = vec![0.0; t.len()];
    let mut rpm = vec![0.0; t.len()];
    let mut power_out = vec![0.0; t.len()];

    for i in 0..t.len() {
        let v = v_t[i];
        let gear_idx = gear_t[i].max(1) as usize - 1;
        let ratio = vehicle
            .engine
            .gear_ratios
            .get(gear_idx)
            .copied()
            .unwrap_or(0.0);
        rpm[i] = rpm_from_speed_gear(v, ratio, &vehicle.chassis);

        let p_theo = power_kw_from_rpm(rpm[i], &vehicle.engine);
        let p_act = p_theo * derating_factor(temp_t[i], &vehicle.engine);

        if power_t[i] > 0.0 {
            brake[i] = 0.0;
            throttle[i] = clamp(p_act / power_t[i], 0.0, 1.2);
        } else {
            throttle[i] = 0.0;
            brake[i] = 1.0;
        }

        power_out[i] = p_act * throttle[i];
    }

    let g_lat: Vec<f64> = v_t
        .iter()
        .zip(kappa_t.iter())
        .map(|(v, k)| v * v * k / 9.81)
        .collect();
    let g_long: Vec<f64> = a_long.iter().map(|value| value / 9.81).collect();
    let s_grad = gradient_equal_spacing(&s_t);
    let g_vert: Vec<f64> = v_t
        .iter()
        .zip(slope_change_t.iter())
        .zip(s_grad.iter())
        .map(|((v, slope_change), ds_sample)| {
            let denom = ds_sample * ds_sample;
            if denom.abs() < 1e-12 {
                0.0
            } else {
                v * v * slope_change / 9.81 / denom
            }
        })
        .collect();
    let tire_mu: Vec<f64> = tire_temp_t
        .iter()
        .zip(tire_wear_t.iter())
        .map(|(temp, wear)| effective_mu(vehicle.chassis.mu0, *wear, *temp, &vehicle.tire))
        .collect();

    Ok(ResampledTelemetry {
        time_s: t,
        s_m: s_t,
        x_m: x_t,
        y_m: y_t,
        heading_rad: heading_t,
        speed_kph: v_t.iter().map(|value| value * 3.6).collect(),
        rpm,
        gear: gear_t,
        throttle_pct: throttle.iter().map(|value| value * 100.0).collect(),
        brake_pct: brake.iter().map(|value| value * 100.0).collect(),
        g_lat,
        g_long,
        g_vert,
        engine_temp_c: temp_t,
        engine_power_w: power_out.iter().map(|value| value * 1000.0).collect(),
        tire_temp_c: Some(tire_temp_t),
        tire_wear_pct: Some(tire_wear_t.iter().map(|value| value * 100.0).collect()),
        tire_mu: Some(tire_mu),
        n_lap: Some(lap_t),
    })
}

pub fn driver_effects(driver: &Driver) -> DriverEffects {
    let a = clamp(driver.aggressiveness, 0.0, 1.0);
    DriverEffects {
        tire_wear_multiplier: lerp(0.92, 1.18, a),
        lap_time_noise_std_ms: python_round_to_i32(lerp(20.0, 80.0, a)),
        peak_pace_bonus_ms: python_round_to_i32(lerp(-20.0, -90.0, a)),
    }
}

pub fn apply_driver_to_tire(tire: &TireParams, effects: &DriverEffects) -> TireParams {
    let mut adjusted = tire.clone();
    adjusted.wear_per_s *= effects.tire_wear_multiplier;
    adjusted
}

fn diminishing_tuning_effect(points: i32) -> f64 {
    let clamped = clamp(points as f64, 0.0, 100.0);
    if clamped <= 0.0 {
        return 0.0;
    }
    1.0 - (-clamped / 20.0).exp()
}

pub fn apply_tuning(vehicle: &VehicleParams, tuning: &Tuning) -> VehicleParams {
    let aero_eff = diminishing_tuning_effect(tuning.aero_points);
    let chassis_eff = diminishing_tuning_effect(tuning.chassis_points);
    let cooling_eff = diminishing_tuning_effect(tuning.cooling_points);
    let engine_eff = diminishing_tuning_effect(tuning.engine_points);
    let df = clamp(tuning.downforce_slider, 0.0, 1.0);
    let gr = clamp(tuning.gear_ratio_slider, 0.0, 1.0);
    let cooling_ratio = cooling_eff;

    let aero_k = 1.0 + 0.10 * aero_eff;
    // Keep these affine ramps easy to port back to Python.
    // At the midpoint (df = 0.5) they remain close to neutral, but
    // full-downforce now pays a materially larger drag bill.
    let drag_blend = 0.68 + 0.64 * df;
    let df_blend = 0.84 + 0.32 * df;

    let aero = AeroParams {
        cd_a_x: vehicle.aero.cd_a_x * aero_k * drag_blend * 0.95,
        cd_a_z: vehicle.aero.cd_a_z * aero_k * drag_blend * 1.05,
        cl_a_x: vehicle.aero.cl_a_x * aero_k * df_blend * 0.95,
        cl_a_z: vehicle.aero.cl_a_z * aero_k * df_blend * 1.05,
    };

    let grip_blend = 1.0 + 0.08 * chassis_eff;
    let chassis = ChassisParams {
        mass_empty: vehicle.chassis.mass_empty,
        r_wheel: vehicle.chassis.r_wheel,
        mu0: vehicle.chassis.mu0 * grip_blend,
        c_rr: vehicle.chassis.c_rr,
        rho: vehicle.chassis.rho,
        g: vehicle.chassis.g,
    };

    // Cooling used to be too forgiving. These affine ramps make low cooling
    // clearly under-provisioned and high cooling meaningfully safer, while
    // staying simple enough to mirror in Python.
    let cooling_scale = 0.35 + 1.30 * cooling_ratio;
    let t_soft_offset = -5.0 + 10.0 * cooling_ratio;
    let beta_derate_scale = 1.15 - 0.30 * cooling_ratio;
    let trq: Vec<f64> = vehicle
        .engine
        .trq
        .iter()
        .map(|value| value * (1.0 + 0.01 * engine_eff))
        .collect();
    let scale = 1.10 - 0.20 * gr;
    let gear_ratios: Vec<f64> = vehicle
        .engine
        .gear_ratios
        .iter()
        .map(|value| value * scale)
        .collect();

    let engine = EngineParams {
        n_rpm: vehicle.engine.n_rpm.clone(),
        trq,
        gear_ratios,
        n_upshift: vehicle.engine.n_upshift,
        n_downshift: vehicle.engine.n_downshift,
        n_idle: vehicle.engine.n_idle,
        n_max: vehicle.engine.n_max,
        t_amb: vehicle.engine.t_amb,
        t_init: vehicle.engine.t_init,
        c_th: vehicle.engine.c_th,
        alpha_heat: vehicle.engine.alpha_heat,
        p_cool0: vehicle.engine.p_cool0 * cooling_scale,
        k_cool: vehicle.engine.k_cool * cooling_scale,
        t_soft: vehicle.engine.t_soft + t_soft_offset,
        beta_derate: vehicle.engine.beta_derate * beta_derate_scale,
        fuel_burn_kg_per_s: vehicle.engine.fuel_burn_kg_per_s,
    };

    VehicleParams {
        chassis,
        aero,
        engine,
        tire: vehicle.tire.clone(),
        hybrid: vehicle.hybrid.clone(),
    }
}

pub fn effective_mu(mu0: f64, tire_wear: f64, tire_temp: f64, tire: &TireParams) -> f64 {
    let wear_k = (1.0 - tire.wear_grip_k * tire_wear).max(tire.wear_min);
    let temp_z = (tire_temp - tire.temp_opt) / tire.temp_sigma.max(1e-3);
    let temp_k = (-temp_z * temp_z).exp().max(tire.temp_min_k);
    mu0 * tire.mu_scale * wear_k * temp_k
}

pub fn derating_factor(temp: f64, engine: &EngineParams) -> f64 {
    if temp <= engine.t_soft {
        1.0
    } else {
        (1.0 - (temp - engine.t_soft) * engine.beta_derate).max(0.2)
    }
}

pub fn rpm_from_speed_gear(speed: f64, gear_ratio: f64, chassis: &ChassisParams) -> f64 {
    if gear_ratio <= 0.0 || chassis.r_wheel <= 0.0 {
        0.0
    } else {
        speed * 60.0 * gear_ratio / (std::f64::consts::TAU * chassis.r_wheel)
    }
}

pub fn power_kw_from_rpm(rpm: f64, engine: &EngineParams) -> f64 {
    interp_linear_with_edges(rpm, &engine.n_rpm, &engine.trq, Some(0.0), Some(0.0))
        * rpm
        * std::f64::consts::PI
        / 30.0
}

pub fn best_power_at_speed(
    speed: f64,
    engine: &EngineParams,
    chassis: &ChassisParams,
) -> (f64, f64, u8) {
    let mut pwr_max = 0.0;
    let mut rpm_pmax = 0.0;
    let mut gear_choice = 1u8;

    for (idx, ratio) in engine.gear_ratios.iter().enumerate() {
        let rpm = rpm_from_speed_gear(speed, *ratio, chassis);
        let pwr = power_kw_from_rpm(rpm, engine);
        if pwr > pwr_max {
            pwr_max = pwr;
            rpm_pmax = rpm;
            gear_choice = (idx + 1) as u8;
        }
    }

    (pwr_max, rpm_pmax, gear_choice)
}

fn validate_track(track: &Track) -> Result<(), String> {
    let n = track.s.len();
    if n < 3 {
        return Err("track must contain at least 3 samples".to_string());
    }
    for len in [
        track.x.len(),
        track.y.len(),
        track.z.len(),
        track.kappa.len(),
        track.slope.len(),
        track.heading.len(),
    ] {
        if len != n {
            return Err("track vectors must share the same length".to_string());
        }
    }
    if !track.s.windows(2).all(|window| window[1] > window[0]) {
        return Err("track.s must be strictly increasing".to_string());
    }
    Ok(())
}

fn tire_for_lap(default_tire: &TireParams, pit_stops: &[PitStop], lap: u16) -> TireParams {
    pit_stops
        .iter()
        .filter(|stop| stop.lap < lap)
        .last()
        .map(|stop| stop.tire.clone())
        .unwrap_or_else(|| default_tire.clone())
}

fn corner_speed_limit(
    track: &Track,
    vehicle: &VehicleParams,
    state: &VehicleState,
    cfg: &SimConfig,
    tire: &TireParams,
) -> Vec<f64> {
    let n = track.s.len();
    let mut out = vec![cfg.max_speed; n];

    for (idx, value) in out.iter_mut().enumerate() {
        let k_val = track.kappa[idx].abs();
        if k_val < 1e-5 {
            *value = cfg.max_speed;
            continue;
        }

        let mut v = 70.0;
        for _ in 0..5 {
            let (_, downforce) = aero_forces(v, &vehicle.aero, &vehicle.chassis, true);
            let mu_eff = effective_mu(vehicle.chassis.mu0, state.tire_wear, state.tire_temp, tire);
            let a_lat_max = mu_eff
                * (vehicle.chassis.g
                    + downforce
                        / (vehicle.chassis.mass_empty + state.total_mass_delta()).max(1e-9));
            v = (a_lat_max / k_val).max(1e-1).sqrt();
        }
        *value = v.min(cfg.max_speed);
    }

    out
}

fn aero_forces(
    speed: f64,
    aero: &AeroParams,
    chassis: &ChassisParams,
    corner_mode: bool,
) -> (f64, f64) {
    let (cd_a, cl_a) = if corner_mode {
        (aero.cd_a_z, aero.cl_a_z)
    } else {
        (aero.cd_a_x, aero.cl_a_x)
    };
    let q = 0.5 * chassis.rho * speed * speed;
    (q * cd_a, q * cl_a)
}

fn lap_noise_ms(sim_seed: u64, driver_id: &str, lap_idx: u16, effects: &DriverEffects) -> f64 {
    if effects.lap_time_noise_std_ms <= 0 {
        return 0.0;
    }

    let seed = deterministic_noise_seed(sim_seed, driver_id, lap_idx);
    let mut rng = StdRng::seed_from_u64(seed);
    let u1 =
        ((rng.next_u64() as f64) / (u64::MAX as f64)).clamp(f64::MIN_POSITIVE, 1.0 - f64::EPSILON);
    let u2 = (rng.next_u64() as f64) / (u64::MAX as f64);
    let unit = (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos();
    unit * effects.lap_time_noise_std_ms as f64
}

fn deterministic_noise_seed(sim_seed: u64, driver_id: &str, lap_idx: u16) -> u64 {
    let seed_str = format!("{sim_seed}:{driver_id}:{lap_idx}");
    let digest = Md5::digest(seed_str.as_bytes());
    u32::from_be_bytes([digest[0], digest[1], digest[2], digest[3]]) as u64
}

fn interp_linear(x: f64, xs: &[f64], ys: &[f64]) -> f64 {
    interp_linear_with_edges(x, xs, ys, None, None)
}

fn interp_linear_with_edges(
    x: f64,
    xs: &[f64],
    ys: &[f64],
    left: Option<f64>,
    right: Option<f64>,
) -> f64 {
    let n = xs.len().min(ys.len());
    if n == 0 {
        return 0.0;
    }
    if n == 1 {
        return ys[0];
    }
    if x <= xs[0] {
        return left.unwrap_or(ys[0]);
    }
    if x >= xs[n - 1] {
        return right.unwrap_or(ys[n - 1]);
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
        ys[lo]
    } else {
        let a = (x - x0) / (x1 - x0);
        ys[lo] + (ys[hi] - ys[lo]) * a
    }
}

fn human_smoothing(values: &[f64], window_len: usize) -> Vec<f64> {
    if window_len < 3 || values.is_empty() {
        return values.to_vec();
    }

    let mut out = vec![0.0; values.len()];
    let half = window_len / 2;
    for (idx, slot) in out.iter_mut().enumerate() {
        let mut sum = 0.0;
        for offset in 0..window_len {
            let source_idx = idx as isize + offset as isize - half as isize;
            if (0..values.len() as isize).contains(&source_idx) {
                sum += values[source_idx as usize];
            }
        }
        *slot = sum / window_len as f64;
    }
    out
}

fn cumulative_sum(values: &[f64]) -> Vec<f64> {
    let mut out = Vec::with_capacity(values.len());
    let mut acc = 0.0;
    for value in values {
        acc += *value;
        out.push(acc);
    }
    out
}

fn gradient_equal_spacing(values: &[f64]) -> Vec<f64> {
    let n = values.len();
    if n == 0 {
        return Vec::new();
    }
    if n == 1 {
        return vec![0.0];
    }

    let mut out = vec![0.0; n];
    out[0] = values[1] - values[0];
    for i in 1..(n - 1) {
        out[i] = (values[i + 1] - values[i - 1]) * 0.5;
    }
    out[n - 1] = values[n - 1] - values[n - 2];
    out
}

fn gradient_with_coords(values: &[f64], coords: &[f64]) -> Vec<f64> {
    let n = values.len().min(coords.len());
    if n == 0 {
        return Vec::new();
    }
    if n == 1 {
        return vec![0.0];
    }

    let mut out = vec![0.0; n];
    let dx0 = (coords[1] - coords[0]).abs().max(1e-12);
    out[0] = (values[1] - values[0]) / dx0;
    for i in 1..(n - 1) {
        let dx = (coords[i + 1] - coords[i - 1]).abs().max(1e-12);
        out[i] = (values[i + 1] - values[i - 1]) / dx;
    }
    let dxn = (coords[n - 1] - coords[n - 2]).abs().max(1e-12);
    out[n - 1] = (values[n - 1] - values[n - 2]) / dxn;
    out
}

fn clamp(value: f64, lo: f64, hi: f64) -> f64 {
    value.max(lo).min(hi)
}

fn lerp(x0: f64, x1: f64, a: f64) -> f64 {
    x0 + (x1 - x0) * a
}

fn python_round_to_i32(value: f64) -> i32 {
    value.round_ties_even() as i32
}

fn u8s_to_f64(values: &[u8]) -> Vec<f64> {
    values.iter().map(|value| *value as f64).collect()
}

fn u16s_to_f64(values: &[u16]) -> Vec<f64> {
    values.iter().map(|value| *value as f64).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_vehicle() -> VehicleParams {
        VehicleParams {
            aero: AeroParams {
                cd_a_x: 1.05,
                cd_a_z: 1.18,
                cl_a_x: 0.55,
                cl_a_z: 1.25,
            },
            chassis: ChassisParams {
                mass_empty: 800.0,
                r_wheel: 0.34,
                mu0: 1.9,
                c_rr: 0.012,
                rho: 1.225,
                g: 9.81,
            },
            engine: EngineParams {
                n_rpm: vec![0.0, 4000.0, 8000.0, 12000.0, 15000.0],
                trq: vec![180.0, 420.0, 620.0, 560.0, 320.0],
                gear_ratios: vec![14.0, 10.5, 8.0, 6.4, 5.2, 4.3],
                n_upshift: 0.0,
                n_downshift: 0.0,
                n_idle: 400.0,
                n_max: 15000.0,
                t_amb: 35.0,
                t_init: 90.0,
                c_th: 100000.0,
                alpha_heat: 0.45,
                p_cool0: 0.0,
                k_cool: 45.0,
                t_soft: 110.0,
                beta_derate: 0.02,
                fuel_burn_kg_per_s: 0.02,
            },
            tire: TireParams {
                mu_scale: 1.0,
                wear_per_s: 0.00012,
                wear_load_k: 0.00002,
                wear_grip_k: 0.3,
                wear_min: 0.72,
                temp_opt: 92.0,
                temp_sigma: 22.0,
                temp_min_k: 0.82,
                heat_k: 0.0035,
                cool_k: 0.0012,
            },
            hybrid: None,
        }
    }

    fn straight_track(length_m: f64, samples: usize) -> Track {
        let step = length_m / (samples.saturating_sub(1) as f64);
        let s: Vec<f64> = (0..samples).map(|idx| idx as f64 * step).collect();
        let x = s.clone();
        let y = vec![0.0; samples];
        let z = vec![0.0; samples];
        let kappa = vec![0.0; samples];
        let slope = vec![0.0; samples];
        let heading = vec![0.0; samples];
        Track {
            s,
            x,
            y,
            z,
            kappa,
            slope,
            heading,
        }
    }

    fn tight_track(length_m: f64, samples: usize, curvature: f64) -> Track {
        let step = length_m / (samples.saturating_sub(1) as f64);
        let s: Vec<f64> = (0..samples).map(|idx| idx as f64 * step).collect();
        let radius = 1.0 / curvature.max(1e-6);
        let theta_end = length_m / radius;
        let x: Vec<f64> = s
            .iter()
            .map(|value| {
                let theta = theta_end * (*value / length_m.max(1e-6));
                radius * theta.sin()
            })
            .collect();
        let y: Vec<f64> = s
            .iter()
            .map(|value| {
                let theta = theta_end * (*value / length_m.max(1e-6));
                radius * (1.0 - theta.cos())
            })
            .collect();
        let z = vec![0.0; samples];
        let kappa = vec![curvature; samples];
        let slope = vec![0.0; samples];
        let heading: Vec<f64> = s
            .iter()
            .map(|value| theta_end * (*value / length_m.max(1e-6)))
            .collect();
        Track {
            s,
            x,
            y,
            z,
            kappa,
            slope,
            heading,
        }
    }

    fn run_case(track: Track, tuning: Tuning, laps: u16) -> SimulationResult {
        run_simulation(&SimulationRequest {
            track,
            vehicle: test_vehicle(),
            state: VehicleState {
                fuel_mass: 80.0,
                tire_wear: 0.0,
                tire_temp: 90.0,
                engine_temp: 90.0,
                battery_soc: 0.0,
                exit_speed_mps: 0.0,
                exit_gear: 1,
            },
            config: SimConfig {
                ds: 0.0,
                max_speed: 400.0,
                pit_time_penalty_s: 20.0,
                pit_tire_temp: None,
                tire_temp_amb: 35.0,
                sim_seed: 7,
            },
            lap_count: laps,
            pit_plan: PitPlan::default(),
            driver: Driver::default(),
            energy_mode: EnergyMode::Balanced,
            tuning: Some(tuning),
        })
        .expect("simulation should succeed")
    }

    #[test]
    fn full_downforce_now_pays_more_drag_than_downforce_gain() {
        let vehicle = test_vehicle();
        let tuned = apply_tuning(
            &vehicle,
            &Tuning {
                aero_points: 0,
                chassis_points: 0,
                cooling_points: 10,
                engine_points: 0,
                downforce_slider: 1.0,
                gear_ratio_slider: 0.5,
            },
        );

        let drag_factor = tuned.aero.cd_a_x / vehicle.aero.cd_a_x;
        let downforce_factor = tuned.aero.cl_a_z / vehicle.aero.cl_a_z;

        assert!(
            drag_factor > downforce_factor,
            "expected drag factor ({drag_factor}) to exceed downforce factor ({downforce_factor})",
        );
    }

    #[test]
    fn low_downforce_is_faster_on_a_fast_track_but_slower_on_a_tight_track() {
        let fast_track = straight_track(5000.0, 220);
        let technical_track = tight_track(2200.0, 220, 0.0105);

        let low_df = Tuning {
            aero_points: 10,
            chassis_points: 10,
            cooling_points: 10,
            engine_points: 10,
            downforce_slider: 0.0,
            gear_ratio_slider: 0.7,
        };
        let high_df = Tuning {
            downforce_slider: 1.0,
            gear_ratio_slider: 0.3,
            ..low_df.clone()
        };

        let fast_low = run_case(fast_track, low_df.clone(), 1);
        let fast_high = run_case(straight_track(5000.0, 220), high_df.clone(), 1);
        assert!(
            fast_low.total_time_s < fast_high.total_time_s,
            "expected low downforce to win on fast track: {} vs {}",
            fast_low.total_time_s,
            fast_high.total_time_s,
        );

        let technical_low = run_case(technical_track, low_df, 1);
        let technical_high = run_case(tight_track(2200.0, 220, 0.0105), high_df, 1);
        assert!(
            technical_high.total_time_s < technical_low.total_time_s,
            "expected high downforce to win on tight track: {} vs {}",
            technical_high.total_time_s,
            technical_low.total_time_s,
        );
    }

    #[test]
    fn neglected_cooling_triggers_visible_derating() {
        let base = Tuning {
            aero_points: 10,
            chassis_points: 10,
            cooling_points: 0,
            engine_points: 10,
            downforce_slider: 0.4,
            gear_ratio_slider: 0.6,
        };
        let low_cooling = run_case(straight_track(5200.0, 260), base.clone(), 4);
        let high_cooling = run_case(
            straight_track(5200.0, 260),
            Tuning {
                cooling_points: 20,
                ..base
            },
            4,
        );

        let low_derate = derating_factor(
            low_cooling.final_state.engine_temp,
            &low_cooling.applied_vehicle.engine,
        );
        let high_derate = derating_factor(
            high_cooling.final_state.engine_temp,
            &high_cooling.applied_vehicle.engine,
        );

        assert!(
            low_cooling.final_state.engine_temp > high_cooling.final_state.engine_temp + 10.0,
            "expected low cooling to run much hotter: {} vs {}",
            low_cooling.final_state.engine_temp,
            high_cooling.final_state.engine_temp,
        );
        assert!(
            low_derate < 0.98,
            "expected low cooling to trigger derating, got {low_derate}",
        );
        assert!(
            low_derate < high_derate,
            "expected low cooling derate ({low_derate}) to be worse than high cooling ({high_derate})",
        );
    }

    #[test]
    fn tuning_points_have_diminishing_returns() {
        let gain_10_to_20 = diminishing_tuning_effect(20) - diminishing_tuning_effect(10);
        let gain_50_to_60 = diminishing_tuning_effect(60) - diminishing_tuning_effect(50);

        assert!(gain_10_to_20 > gain_50_to_60);
        assert!(diminishing_tuning_effect(100) < 1.0);
        assert!(diminishing_tuning_effect(100) > diminishing_tuning_effect(20));
    }
}
