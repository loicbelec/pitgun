//! Deterministic physical and mathematical Solver for the Racing domain.
//!
//! This crate owns resolved vehicle, track and state inputs plus the numerical
//! algorithms that produce a physical solution. Race/session orchestration,
//! catalog lookup, telemetry envelopes and browser bindings belong to the
//! Racing Simulator.

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
pub struct VehicleParams {
    pub chassis: ChassisParams,
    pub aero: AeroParams,
    pub engine: EngineParams,
    pub tire: TireParams,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VehicleState {
    pub fuel_mass: f64,
    pub tire_wear: f64,
    pub tire_temp: f64,
    pub engine_temp: f64,
}

impl Default for VehicleState {
    fn default() -> Self {
        Self {
            fuel_mass: 100.0,
            tire_wear: 0.0,
            tire_temp: 90.0,
            engine_temp: 90.0,
        }
    }
}

impl VehicleState {
    pub fn total_mass_delta(&self) -> f64 {
        self.fuel_mass
    }
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum TuningResponseVersion {
    #[serde(rename = "pitgun.racing-tuning-response/v1")]
    V1,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TuningResponseV1 {
    pub schema_version: TuningResponseVersion,
    pub development_points_cap: f64,
    pub aero_development_gain: f64,
    pub drag_base: f64,
    pub drag_slider_gain: f64,
    pub downforce_base: f64,
    pub downforce_slider_gain: f64,
    pub straight_aero_scale: f64,
    pub corner_aero_scale: f64,
    pub chassis_grip_development_gain: f64,
    pub cooling_base: f64,
    pub cooling_development_gain: f64,
    pub engine_torque_development_gain: f64,
    pub gear_ratio_base: f64,
    pub gear_ratio_slider_reduction: f64,
}

impl Default for TuningResponseV1 {
    fn default() -> Self {
        Self {
            schema_version: TuningResponseVersion::V1,
            development_points_cap: 20.0,
            aero_development_gain: 0.10,
            drag_base: 0.85,
            drag_slider_gain: 0.30,
            downforce_base: 0.75,
            downforce_slider_gain: 0.55,
            straight_aero_scale: 0.95,
            corner_aero_scale: 1.05,
            chassis_grip_development_gain: 0.08,
            cooling_base: 0.75,
            cooling_development_gain: 0.50,
            engine_torque_development_gain: 0.01,
            gear_ratio_base: 1.10,
            gear_ratio_slider_reduction: 0.20,
        }
    }
}

impl TuningResponseV1 {
    pub fn validate(&self) -> Result<(), String> {
        let finite = [
            self.development_points_cap,
            self.aero_development_gain,
            self.drag_base,
            self.drag_slider_gain,
            self.downforce_base,
            self.downforce_slider_gain,
            self.straight_aero_scale,
            self.corner_aero_scale,
            self.chassis_grip_development_gain,
            self.cooling_base,
            self.cooling_development_gain,
            self.engine_torque_development_gain,
            self.gear_ratio_base,
            self.gear_ratio_slider_reduction,
        ];
        if finite.iter().any(|value| !value.is_finite()) {
            return Err("tuning response coefficients must be finite".to_string());
        }
        if self.development_points_cap <= 0.0 {
            return Err("development_points_cap must be positive".to_string());
        }
        for (name, value) in [
            ("drag_base", self.drag_base),
            ("downforce_base", self.downforce_base),
            ("straight_aero_scale", self.straight_aero_scale),
            ("corner_aero_scale", self.corner_aero_scale),
            ("cooling_base", self.cooling_base),
            ("gear_ratio_base", self.gear_ratio_base),
        ] {
            if value <= 0.0 {
                return Err(format!("{name} must be positive"));
            }
        }
        for (name, value) in [
            ("aero_development_gain", self.aero_development_gain),
            ("drag_slider_gain", self.drag_slider_gain),
            ("downforce_slider_gain", self.downforce_slider_gain),
            (
                "chassis_grip_development_gain",
                self.chassis_grip_development_gain,
            ),
            ("cooling_development_gain", self.cooling_development_gain),
            (
                "engine_torque_development_gain",
                self.engine_torque_development_gain,
            ),
            (
                "gear_ratio_slider_reduction",
                self.gear_ratio_slider_reduction,
            ),
        ] {
            if value < 0.0 {
                return Err(format!("{name} must not be negative"));
            }
        }
        if self.gear_ratio_base - self.gear_ratio_slider_reduction <= 0.0 {
            return Err(
                "gear_ratio_slider_reduction must remain below gear_ratio_base".to_string(),
            );
        }
        Ok(())
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
    #[serde(default = "default_lap_count")]
    pub lap_count: u16,
    #[serde(default)]
    pub pit_plan: PitPlan,
    #[serde(default)]
    pub driver: Driver,
    #[serde(default)]
    pub tuning: Option<Tuning>,
}

/// Fully physical input accepted by Racing Game Model V3.
///
/// Unlike [`SimulationRequest`], this boundary cannot carry game development
/// points or setup sliders. The Racing Simulator must resolve those choices to
/// physical vehicle parameters before invoking the Solver.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResolvedSimulationRequestV3 {
    pub track: Track,
    pub vehicle: VehicleParams,
    pub state: VehicleState,
    #[serde(default)]
    pub config: SimConfig,
    #[serde(default = "default_lap_count")]
    pub lap_count: u16,
    #[serde(default)]
    pub pit_plan: PitPlan,
    #[serde(default)]
    pub driver: Driver,
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

pub const CORNER_CURVATURE_THRESHOLD_RAD_PER_M: f64 = 0.001;
pub const AERO_FULL_STRAIGHT_CURVATURE_RAD_PER_M: f64 = 0.0;
pub const AERO_FULL_CORNER_CURVATURE_RAD_PER_M: f64 = 0.001;
pub const LONGITUDINAL_ACCELERATION_THRESHOLD_MPS2: f64 = 0.05;
pub const NEAR_MAX_RPM_RATIO: f64 = 0.98;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
pub enum CurvatureAeroResponse {
    #[default]
    LegacyBinary,
    ContinuousV1,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum SpatialIntegration {
    UniformGridCompatibility,
    PerSegmentV1,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
pub struct CircuitDescriptorsV1 {
    pub length_m: f64,
    pub straight_distance_m: f64,
    pub corner_distance_m: f64,
    pub corner_distance_share: f64,
    pub absolute_curvature_integral_rad: f64,
    pub maximum_absolute_curvature_rad_per_m: f64,
    pub elevation_gain_m: f64,
    pub elevation_loss_m: f64,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
pub enum SetupResponseDiagnosticsVersion {
    #[default]
    #[serde(rename = "pitgun.racing-setup-response/v1")]
    V1,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
pub struct SetupResponseDiagnosticsV1 {
    pub schema_version: SetupResponseDiagnosticsVersion,
    pub corner_curvature_threshold_rad_per_m: f64,
    pub longitudinal_acceleration_threshold_mps2: f64,
    pub near_max_rpm_ratio: f64,
    pub circuit: CircuitDescriptorsV1,
    pub observed_time_s: f64,
    pub straight_time_s: f64,
    pub corner_time_s: f64,
    pub mean_straight_speed_kph: f64,
    pub mean_corner_speed_kph: f64,
    pub acceleration_time_s: f64,
    pub braking_time_s: f64,
    pub steady_speed_time_s: f64,
    pub gear_shift_count: u64,
    pub near_max_rpm_time_s: f64,
    pub maximum_observed_rpm: f64,
    pub maximum_rpm_utilization: f64,
    pub maximum_gear_used: u8,
    pub aerodynamic_drag_work_kj: f64,
    pub mean_downforce_n: f64,
    pub maximum_downforce_n: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SimulationResult {
    pub solution: SimulationSolution,
    pub final_state: VehicleState,
    pub lap_times_s: Vec<f64>,
    pub total_time_s: f64,
    pub applied_vehicle: VehicleParams,
    pub applied_driver: Driver,
    #[serde(default)]
    pub diagnostics: SetupResponseDiagnosticsV1,
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
    run_simulation_with_tuning_response(input, &TuningResponseV1::default())
}

pub fn run_simulation_with_tuning_response(
    input: &SimulationRequest,
    tuning_response: &TuningResponseV1,
) -> Result<SimulationResult, String> {
    run_simulation_with_model_response(input, tuning_response, CurvatureAeroResponse::LegacyBinary)
}

/// Runs an offline model experiment with an explicit curvature response.
///
/// The published model V1 remains on [`CurvatureAeroResponse::LegacyBinary`].
/// Model V2 selects [`CurvatureAeroResponse::ContinuousV1`] explicitly through
/// its versioned workload; callers cannot change that selection through input.
pub fn run_simulation_with_model_response(
    input: &SimulationRequest,
    tuning_response: &TuningResponseV1,
    curvature_response: CurvatureAeroResponse,
) -> Result<SimulationResult, String> {
    tuning_response.validate()?;
    validate_track(&input.track)?;
    let tuned_vehicle = match &input.tuning {
        Some(tuning) => apply_tuning_with_response(&input.vehicle, tuning, tuning_response)?,
        None => input.vehicle.clone(),
    };

    run_resolved_simulation_kernel(
        input,
        tuned_vehicle,
        curvature_response,
        SpatialIntegration::UniformGridCompatibility,
    )
}

/// Executes the first Racing Game Model V3 mechanical slice.
///
/// The request already contains a physically resolved vehicle. Track
/// integration uses each explicit segment length and derives vertical-path
/// curvature from the distance coordinate. This Rust-only candidate boundary
/// is not selected by any production catalog yet.
pub fn run_resolved_simulation_v3(
    input: &ResolvedSimulationRequestV3,
) -> Result<SimulationResult, String> {
    validate_track(&input.track)?;
    validate_resolved_track_v3(&input.track)?;
    validate_resolved_vehicle(&input.vehicle)?;
    validate_resolved_state_v3(&input.state)?;
    validate_resolved_config_v3(&input.config)?;
    for stop in &input.pit_plan.stops {
        validate_tire_params(&stop.tire)?;
    }

    let compatibility_request = SimulationRequest {
        track: input.track.clone(),
        vehicle: input.vehicle.clone(),
        state: input.state.clone(),
        config: input.config.clone(),
        lap_count: input.lap_count,
        pit_plan: input.pit_plan.clone(),
        driver: input.driver.clone(),
        tuning: None,
    };

    run_resolved_simulation_kernel(
        &compatibility_request,
        input.vehicle.clone(),
        CurvatureAeroResponse::ContinuousV1,
        SpatialIntegration::PerSegmentV1,
    )
}

fn run_resolved_simulation_kernel(
    input: &SimulationRequest,
    tuned_vehicle: VehicleParams,
    curvature_response: CurvatureAeroResponse,
    spatial_integration: SpatialIntegration,
) -> Result<SimulationResult, String> {
    let lap_count = input.lap_count.max(1);
    let driver = input.driver.clone();
    let effects = driver_effects(&driver);
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
    let slope_gradient = gradient_with_coords(&input.track.slope, &input.track.s);

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
    let mut prev_end_speed: Option<f64> = None;
    let mut prev_end_gear: Option<u8> = None;
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
            curvature_response,
        );

        let mut v_bwd = v_corner.clone();
        for i in (0..(n - 1)).rev() {
            let segment_ds = segment_distance(spatial_integration, s, ds, i);
            let v_target = v_bwd[i + 1];
            let (drag, downforce) = aero_forces(
                v_target,
                input.track.kappa[i],
                &vehicle.aero,
                &vehicle.chassis,
                curvature_response,
                true,
            );

            let f_drag = drag;
            let f_roll = vehicle.chassis.c_rr * (mass * vehicle.chassis.g + downforce);
            let f_slope = mass * vehicle.chassis.g * input.track.slope[i];

            let a_vert = vertical_acceleration(
                spatial_integration,
                v_target,
                slope_change[i],
                slope_gradient[i],
                ds,
            );
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

            let v_max_braking = (v_target * v_target + 2.0 * a_decel * segment_ds)
                .max(0.0)
                .sqrt();
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
            let segment_ds = segment_distance(spatial_integration, s, ds, i);
            let v = v_fwd[i].min(v_bwd[i]);
            let v_safe = v.max(1.0);
            let dt = segment_ds / v_safe;

            let (mut pwr, _, best_gear) =
                best_power_at_speed(v_safe, &vehicle.engine, &vehicle.chassis);
            pwr *= derating_factor(temp[i], &vehicle.engine);

            if v_fwd[i] >= v_bwd[i] {
                power[i] = 0.0;
                v_fwd[i + 1] = v_bwd[i];
            } else {
                let (drag, downforce) = aero_forces(
                    v_safe,
                    input.track.kappa[i],
                    &vehicle.aero,
                    &vehicle.chassis,
                    curvature_response,
                    input.track.kappa[i].abs() > CORNER_CURVATURE_THRESHOLD_RAD_PER_M,
                );

                let a_vert = vertical_acceleration(
                    spatial_integration,
                    v_safe,
                    slope_change[i],
                    slope_gradient[i],
                    ds,
                );
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
                v_fwd[i + 1] = (v_safe * v_safe + 2.0 * a * segment_ds).max(0.0).sqrt();
            }

            let heat = 1000.0 * vehicle.engine.alpha_heat * power[i];
            let cool = (vehicle.engine.p_cool0 + vehicle.engine.k_cool * v_safe)
                * (temp[i] - vehicle.engine.t_amb);
            temp[i + 1] = temp[i] + (heat - cool) / vehicle.engine.c_th.max(1e-9) * dt;

            let a_long =
                (v_fwd[i + 1] * v_fwd[i + 1] - v_safe * v_safe) / (2.0 * segment_ds).max(1e-3);
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
            let segment_ds = segment_distance(spatial_integration, s, ds, i - 1);
            dt[i] = segment_ds / (0.5 * (v_safe[i] + v_safe[i - 1]));
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
    let diagnostics = diagnose_setup_response_with_model_response(
        &input.track,
        &solution,
        &vehicle,
        curvature_response,
    )?;

    Ok(SimulationResult {
        solution,
        final_state: state_curr,
        lap_times_s,
        total_time_s,
        applied_vehicle: vehicle,
        applied_driver: driver,
        diagnostics,
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

pub fn apply_tuning(vehicle: &VehicleParams, tuning: &Tuning) -> VehicleParams {
    apply_tuning_with_response(vehicle, tuning, &TuningResponseV1::default())
        .expect("the built-in Racing tuning response is valid")
}

pub fn apply_tuning_with_response(
    vehicle: &VehicleParams,
    tuning: &Tuning,
    response: &TuningResponseV1,
) -> Result<VehicleParams, String> {
    response.validate()?;
    let points_cap = response.development_points_cap;
    let aero_pts = clamp(tuning.aero_points as f64, 0.0, points_cap);
    let chassis_pts = clamp(tuning.chassis_points as f64, 0.0, points_cap);
    let cooling_pts = clamp(tuning.cooling_points as f64, 0.0, points_cap);
    let engine_pts = clamp(tuning.engine_points as f64, 0.0, points_cap);
    let df = clamp(tuning.downforce_slider, 0.0, 1.0);
    let gr = clamp(tuning.gear_ratio_slider, 0.0, 1.0);

    let aero_k = 1.0 + response.aero_development_gain * (aero_pts / points_cap);
    let drag_blend = response.drag_base + response.drag_slider_gain * df;
    let df_blend = response.downforce_base + response.downforce_slider_gain * df;

    let aero = AeroParams {
        cd_a_x: vehicle.aero.cd_a_x * aero_k * drag_blend * response.straight_aero_scale,
        cd_a_z: vehicle.aero.cd_a_z * aero_k * drag_blend * response.corner_aero_scale,
        cl_a_x: vehicle.aero.cl_a_x * aero_k * df_blend * response.straight_aero_scale,
        cl_a_z: vehicle.aero.cl_a_z * aero_k * df_blend * response.corner_aero_scale,
    };

    let grip_blend = 1.0 + response.chassis_grip_development_gain * (chassis_pts / points_cap);
    let chassis = ChassisParams {
        mass_empty: vehicle.chassis.mass_empty,
        r_wheel: vehicle.chassis.r_wheel,
        mu0: vehicle.chassis.mu0 * grip_blend,
        c_rr: vehicle.chassis.c_rr,
        rho: vehicle.chassis.rho,
        g: vehicle.chassis.g,
    };

    let cool_k =
        response.cooling_base + response.cooling_development_gain * (cooling_pts / points_cap);
    let trq: Vec<f64> = vehicle
        .engine
        .trq
        .iter()
        .map(|value| {
            value * (1.0 + response.engine_torque_development_gain * (engine_pts / points_cap))
        })
        .collect();
    let scale = response.gear_ratio_base - response.gear_ratio_slider_reduction * gr;
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
        p_cool0: vehicle.engine.p_cool0 * cool_k,
        k_cool: vehicle.engine.k_cool * cool_k,
        t_soft: vehicle.engine.t_soft,
        beta_derate: vehicle.engine.beta_derate,
        fuel_burn_kg_per_s: vehicle.engine.fuel_burn_kg_per_s,
    };

    Ok(VehicleParams {
        chassis,
        aero,
        engine,
        tire: vehicle.tire.clone(),
    })
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

fn validate_resolved_track_v3(track: &Track) -> Result<(), String> {
    for (name, values) in [
        ("track.s", track.s.as_slice()),
        ("track.x", track.x.as_slice()),
        ("track.y", track.y.as_slice()),
        ("track.z", track.z.as_slice()),
        ("track.kappa", track.kappa.as_slice()),
        ("track.slope", track.slope.as_slice()),
        ("track.heading", track.heading.as_slice()),
    ] {
        if values.iter().any(|value| !value.is_finite()) {
            return Err(format!("{name} must contain only finite values"));
        }
    }
    Ok(())
}

/// Validates the physical vehicle supplied to Racing Game Model V3.
///
/// The current public field names are retained for historical serialization,
/// while this boundary makes their physical domains explicit: SI units are
/// used throughout (kg, m, s, N, W, rpm and degrees Celsius) except for named
/// dimensionless coefficients and normalized wear.
pub fn validate_resolved_vehicle(vehicle: &VehicleParams) -> Result<(), String> {
    let chassis = &vehicle.chassis;
    for (name, value) in [
        ("vehicle.chassis.mass_empty_kg", chassis.mass_empty),
        ("vehicle.chassis.wheel_radius_m", chassis.r_wheel),
        ("vehicle.chassis.base_grip_coefficient", chassis.mu0),
        ("vehicle.chassis.air_density_kg_per_m3", chassis.rho),
        ("vehicle.chassis.gravity_m_per_s2", chassis.g),
    ] {
        require_positive(name, value)?;
    }
    require_non_negative(
        "vehicle.chassis.rolling_resistance_coefficient",
        chassis.c_rr,
    )?;

    for (name, value) in [
        ("vehicle.aero.straight_drag_area_m2", vehicle.aero.cd_a_x),
        ("vehicle.aero.corner_drag_area_m2", vehicle.aero.cd_a_z),
        (
            "vehicle.aero.straight_downforce_area_m2",
            vehicle.aero.cl_a_x,
        ),
        ("vehicle.aero.corner_downforce_area_m2", vehicle.aero.cl_a_z),
    ] {
        require_non_negative(name, value)?;
    }

    let engine = &vehicle.engine;
    if engine.n_rpm.len() < 2 || engine.n_rpm.len() != engine.trq.len() {
        return Err(
            "vehicle.engine torque curve must contain at least two matching rpm/torque samples"
                .to_string(),
        );
    }
    if !engine
        .n_rpm
        .iter()
        .all(|value| value.is_finite() && *value >= 0.0)
        || !engine.n_rpm.iter().any(|value| *value > 0.0)
        || !engine.n_rpm.windows(2).all(|window| window[1] > window[0])
    {
        return Err(
            "vehicle.engine rpm samples must be finite, non-negative, increasing and contain a positive value"
                .to_string(),
        );
    }
    if engine
        .trq
        .iter()
        .any(|value| !value.is_finite() || *value < 0.0)
    {
        return Err("vehicle.engine torque samples must be finite and non-negative".to_string());
    }
    if engine.gear_ratios.is_empty()
        || engine
            .gear_ratios
            .iter()
            .any(|value| !value.is_finite() || *value <= 0.0)
    {
        return Err("vehicle.engine gear ratios must be finite and positive".to_string());
    }
    for (name, value) in [
        ("vehicle.engine.idle_rpm", engine.n_idle),
        ("vehicle.engine.maximum_rpm", engine.n_max),
        ("vehicle.engine.thermal_capacity_j_per_c", engine.c_th),
    ] {
        require_positive(name, value)?;
    }
    require_non_negative("vehicle.engine.upshift_rpm", engine.n_upshift)?;
    require_non_negative("vehicle.engine.downshift_rpm", engine.n_downshift)?;
    if engine.n_idle >= engine.n_max
        || (engine.n_upshift > 0.0
            && (engine.n_downshift > engine.n_upshift || engine.n_upshift > engine.n_max))
    {
        return Err(
            "vehicle.engine rpm limits must satisfy idle < maximum and, when configured, downshift <= upshift <= maximum"
                .to_string(),
        );
    }
    for (name, value) in [
        ("vehicle.engine.ambient_temperature_c", engine.t_amb),
        ("vehicle.engine.initial_temperature_c", engine.t_init),
        ("vehicle.engine.soft_limit_temperature_c", engine.t_soft),
    ] {
        require_finite(name, value)?;
    }
    for (name, value) in [
        ("vehicle.engine.heat_fraction", engine.alpha_heat),
        ("vehicle.engine.base_cooling_w_per_c", engine.p_cool0),
        ("vehicle.engine.speed_cooling_w_s_per_m_c", engine.k_cool),
        ("vehicle.engine.derate_per_c", engine.beta_derate),
        (
            "vehicle.engine.fuel_burn_kg_per_s",
            engine.fuel_burn_kg_per_s,
        ),
    ] {
        require_non_negative(name, value)?;
    }

    validate_tire_params(&vehicle.tire)
}

fn validate_tire_params(tire: &TireParams) -> Result<(), String> {
    require_positive("vehicle.tire.grip_multiplier", tire.mu_scale)?;
    require_positive("vehicle.tire.optimal_temperature_c", tire.temp_opt)?;
    require_positive("vehicle.tire.temperature_sigma_c", tire.temp_sigma)?;
    for (name, value) in [
        ("vehicle.tire.base_wear_per_s", tire.wear_per_s),
        ("vehicle.tire.load_wear_gain", tire.wear_load_k),
        ("vehicle.tire.wear_grip_loss", tire.wear_grip_k),
        ("vehicle.tire.minimum_temperature_factor", tire.temp_min_k),
        ("vehicle.tire.heat_gain", tire.heat_k),
        ("vehicle.tire.cooling_gain", tire.cool_k),
    ] {
        require_non_negative(name, value)?;
    }
    if !tire.wear_min.is_finite() || !(0.0..=1.0).contains(&tire.wear_min) {
        return Err("vehicle.tire.minimum_wear_factor must be finite and in [0, 1]".to_string());
    }
    Ok(())
}

fn validate_resolved_state_v3(state: &VehicleState) -> Result<(), String> {
    require_non_negative("state.fuel_mass_kg", state.fuel_mass)?;
    if !state.tire_wear.is_finite() || !(0.0..=1.0).contains(&state.tire_wear) {
        return Err("state.tire_wear must be finite and in [0, 1]".to_string());
    }
    require_non_negative("state.tire_temperature_c", state.tire_temp)?;
    require_non_negative("state.engine_temperature_c", state.engine_temp)
}

fn validate_resolved_config_v3(config: &SimConfig) -> Result<(), String> {
    require_positive("config.maximum_speed_m_per_s", config.max_speed)?;
    require_non_negative("config.pit_time_penalty_s", config.pit_time_penalty_s)?;
    require_non_negative("config.tire_ambient_temperature_c", config.tire_temp_amb)?;
    if let Some(temperature) = config.pit_tire_temp {
        require_non_negative("config.pit_tire_temperature_c", temperature)?;
    }
    Ok(())
}

fn require_finite(name: &str, value: f64) -> Result<(), String> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(format!("{name} must be finite"))
    }
}

fn require_positive(name: &str, value: f64) -> Result<(), String> {
    if value.is_finite() && value > 0.0 {
        Ok(())
    } else {
        Err(format!("{name} must be finite and positive"))
    }
}

fn require_non_negative(name: &str, value: f64) -> Result<(), String> {
    if value.is_finite() && value >= 0.0 {
        Ok(())
    } else {
        Err(format!("{name} must be finite and non-negative"))
    }
}

fn segment_distance(
    integration: SpatialIntegration,
    s: &[f64],
    compatibility_ds: f64,
    segment_index: usize,
) -> f64 {
    match integration {
        SpatialIntegration::UniformGridCompatibility => compatibility_ds,
        SpatialIntegration::PerSegmentV1 => s[segment_index + 1] - s[segment_index],
    }
}

fn vertical_acceleration(
    integration: SpatialIntegration,
    speed: f64,
    compatibility_slope_change: f64,
    slope_gradient_per_m: f64,
    compatibility_ds: f64,
) -> f64 {
    match integration {
        SpatialIntegration::UniformGridCompatibility => {
            speed * speed * compatibility_slope_change / compatibility_ds / compatibility_ds
        }
        SpatialIntegration::PerSegmentV1 => speed * speed * slope_gradient_per_m,
    }
}

fn tire_for_lap(default_tire: &TireParams, pit_stops: &[PitStop], lap: u16) -> TireParams {
    pit_stops
        .iter()
        .rfind(|stop| stop.lap < lap)
        .map(|stop| stop.tire.clone())
        .unwrap_or_else(|| default_tire.clone())
}

fn corner_speed_limit(
    track: &Track,
    vehicle: &VehicleParams,
    state: &VehicleState,
    cfg: &SimConfig,
    tire: &TireParams,
    curvature_response: CurvatureAeroResponse,
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
            let (_, downforce) = aero_forces(
                v,
                k_val,
                &vehicle.aero,
                &vehicle.chassis,
                curvature_response,
                true,
            );
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
    curvature_rad_per_m: f64,
    aero: &AeroParams,
    chassis: &ChassisParams,
    response: CurvatureAeroResponse,
    legacy_corner_mode: bool,
) -> (f64, f64) {
    let blend = match response {
        CurvatureAeroResponse::LegacyBinary => f64::from(legacy_corner_mode),
        CurvatureAeroResponse::ContinuousV1 => curvature_aero_blend(curvature_rad_per_m),
    };
    let cd_a = lerp(aero.cd_a_x, aero.cd_a_z, blend);
    let cl_a = lerp(aero.cl_a_x, aero.cl_a_z, blend);
    let q = 0.5 * chassis.rho * speed * speed;
    (q * cd_a, q * cl_a)
}

/// Maps absolute track curvature to one continuous aerodynamic state.
///
/// True straights retain the straight coefficients, sustained high-curvature
/// samples retain the corner coefficients, and the transition uses a cubic
/// smoothstep with zero derivative at both boundaries. Every Solver pass and
/// diagnostic force calculation shares this function.
pub fn curvature_aero_blend(curvature_rad_per_m: f64) -> f64 {
    let normalized = ((curvature_rad_per_m.abs() - AERO_FULL_STRAIGHT_CURVATURE_RAD_PER_M)
        / (AERO_FULL_CORNER_CURVATURE_RAD_PER_M - AERO_FULL_STRAIGHT_CURVATURE_RAD_PER_M))
        .clamp(0.0, 1.0);
    normalized * normalized * (3.0 - 2.0 * normalized)
}

pub fn describe_circuit(track: &Track) -> Result<CircuitDescriptorsV1, String> {
    validate_track(track)?;

    let mut straight_distance_m = 0.0;
    let mut corner_distance_m = 0.0;
    let mut absolute_curvature_integral_rad = 0.0;
    let mut maximum_absolute_curvature_rad_per_m = 0.0_f64;
    let mut elevation_gain_m = 0.0;
    let mut elevation_loss_m = 0.0;

    for index in 1..track.s.len() {
        let distance_m = track.s[index] - track.s[index - 1];
        let curvature_rad_per_m = 0.5 * (track.kappa[index].abs() + track.kappa[index - 1].abs());
        maximum_absolute_curvature_rad_per_m =
            maximum_absolute_curvature_rad_per_m.max(curvature_rad_per_m);
        absolute_curvature_integral_rad += curvature_rad_per_m * distance_m;
        if curvature_rad_per_m >= CORNER_CURVATURE_THRESHOLD_RAD_PER_M {
            corner_distance_m += distance_m;
        } else {
            straight_distance_m += distance_m;
        }

        let elevation_delta_m = track.z[index] - track.z[index - 1];
        if elevation_delta_m >= 0.0 {
            elevation_gain_m += elevation_delta_m;
        } else {
            elevation_loss_m += -elevation_delta_m;
        }
    }

    let length_m = straight_distance_m + corner_distance_m;
    Ok(CircuitDescriptorsV1 {
        length_m,
        straight_distance_m,
        corner_distance_m,
        corner_distance_share: if length_m > 0.0 {
            corner_distance_m / length_m
        } else {
            0.0
        },
        absolute_curvature_integral_rad,
        maximum_absolute_curvature_rad_per_m,
        elevation_gain_m,
        elevation_loss_m,
    })
}

pub fn diagnose_setup_response(
    track: &Track,
    solution: &SimulationSolution,
    vehicle: &VehicleParams,
) -> Result<SetupResponseDiagnosticsV1, String> {
    diagnose_setup_response_with_model_response(
        track,
        solution,
        vehicle,
        CurvatureAeroResponse::LegacyBinary,
    )
}

pub fn diagnose_setup_response_with_model_response(
    track: &Track,
    solution: &SimulationSolution,
    vehicle: &VehicleParams,
    curvature_response: CurvatureAeroResponse,
) -> Result<SetupResponseDiagnosticsV1, String> {
    let circuit = describe_circuit(track)?;
    let sample_count = solution
        .s
        .len()
        .min(solution.t.len())
        .min(solution.v.len())
        .min(solution.gear.len())
        .min(solution.lap_index.len());
    if sample_count < 2 {
        return Err("solution must contain at least 2 aligned samples".to_string());
    }

    let mut observed_time_s = 0.0;
    let mut straight_time_s = 0.0;
    let mut corner_time_s = 0.0;
    let mut straight_speed_time_m = 0.0;
    let mut corner_speed_time_m = 0.0;
    let mut acceleration_time_s = 0.0;
    let mut braking_time_s = 0.0;
    let mut steady_speed_time_s = 0.0;
    let mut gear_shift_count = 0_u64;
    let mut near_max_rpm_time_s = 0.0;
    let mut maximum_observed_rpm = 0.0_f64;
    let mut maximum_gear_used = 0_u8;
    let mut aerodynamic_drag_work_j = 0.0;
    let mut downforce_time_ns = 0.0;
    let mut maximum_downforce_n = 0.0_f64;

    for index in 1..sample_count {
        if solution.lap_index[index] != solution.lap_index[index - 1] {
            continue;
        }
        let elapsed_s = solution.t[index] - solution.t[index - 1];
        let distance_m = solution.s[index] - solution.s[index - 1];
        if !elapsed_s.is_finite()
            || !distance_m.is_finite()
            || elapsed_s <= 0.0
            || distance_m <= 0.0
        {
            continue;
        }

        let speed_mps = 0.5 * (solution.v[index] + solution.v[index - 1]);
        if !speed_mps.is_finite() || speed_mps < 0.0 {
            return Err("solution contains an invalid speed".to_string());
        }
        let track_position_m =
            (0.5 * (solution.s[index] + solution.s[index - 1])).rem_euclid(circuit.length_m);
        let curvature_rad_per_m = interp_linear(track_position_m, &track.s, &track.kappa).abs();
        let corner_mode = curvature_rad_per_m >= CORNER_CURVATURE_THRESHOLD_RAD_PER_M;
        if corner_mode {
            corner_time_s += elapsed_s;
            corner_speed_time_m += speed_mps * elapsed_s;
        } else {
            straight_time_s += elapsed_s;
            straight_speed_time_m += speed_mps * elapsed_s;
        }

        let acceleration_mps2 = (solution.v[index] - solution.v[index - 1]) / elapsed_s;
        if acceleration_mps2 > LONGITUDINAL_ACCELERATION_THRESHOLD_MPS2 {
            acceleration_time_s += elapsed_s;
        } else if acceleration_mps2 < -LONGITUDINAL_ACCELERATION_THRESHOLD_MPS2 {
            braking_time_s += elapsed_s;
        } else {
            steady_speed_time_s += elapsed_s;
        }

        if solution.gear[index] != solution.gear[index - 1] {
            gear_shift_count += 1;
        }
        let gear_index = solution.gear[index].max(1) as usize - 1;
        maximum_gear_used = maximum_gear_used.max(solution.gear[index]);
        if let Some(ratio) = vehicle.engine.gear_ratios.get(gear_index) {
            let rpm = rpm_from_speed_gear(speed_mps, *ratio, &vehicle.chassis);
            maximum_observed_rpm = maximum_observed_rpm.max(rpm);
            if rpm >= NEAR_MAX_RPM_RATIO * vehicle.engine.n_max {
                near_max_rpm_time_s += elapsed_s;
            }
        }

        let (drag_n, downforce_n) = aero_forces(
            speed_mps,
            curvature_rad_per_m,
            &vehicle.aero,
            &vehicle.chassis,
            curvature_response,
            corner_mode,
        );
        aerodynamic_drag_work_j += drag_n * distance_m;
        downforce_time_ns += downforce_n * elapsed_s;
        maximum_downforce_n = maximum_downforce_n.max(downforce_n);
        observed_time_s += elapsed_s;
    }

    if observed_time_s <= 0.0 {
        return Err("solution contains no measurable segments".to_string());
    }

    Ok(SetupResponseDiagnosticsV1 {
        schema_version: SetupResponseDiagnosticsVersion::V1,
        corner_curvature_threshold_rad_per_m: CORNER_CURVATURE_THRESHOLD_RAD_PER_M,
        longitudinal_acceleration_threshold_mps2: LONGITUDINAL_ACCELERATION_THRESHOLD_MPS2,
        near_max_rpm_ratio: NEAR_MAX_RPM_RATIO,
        circuit,
        observed_time_s,
        straight_time_s,
        corner_time_s,
        mean_straight_speed_kph: if straight_time_s > 0.0 {
            3.6 * straight_speed_time_m / straight_time_s
        } else {
            0.0
        },
        mean_corner_speed_kph: if corner_time_s > 0.0 {
            3.6 * corner_speed_time_m / corner_time_s
        } else {
            0.0
        },
        acceleration_time_s,
        braking_time_s,
        steady_speed_time_s,
        gear_shift_count,
        near_max_rpm_time_s,
        maximum_observed_rpm,
        maximum_rpm_utilization: if vehicle.engine.n_max > 0.0 {
            maximum_observed_rpm / vehicle.engine.n_max
        } else {
            0.0
        },
        maximum_gear_used,
        aerodynamic_drag_work_kj: aerodynamic_drag_work_j / 1000.0,
        mean_downforce_n: downforce_time_ns / observed_time_s,
        maximum_downforce_n,
    })
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
