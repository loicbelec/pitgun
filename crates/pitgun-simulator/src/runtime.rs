use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use pitgun_contract::{Sample, SampleValue, SignalQuality, TelemetryFrame};
use pitgun_solver::{
    AeroParams as SolverAeroParams, ChassisParams as SolverChassisParams, Driver as SolverDriver,
    PitPlan as SolverPitPlan, PitStop as SolverPitStop, ResampledTelemetry,
    SimConfig as SolverSimConfig, SimulationRequest as SolverSimulationRequest, SimulationResult,
    TireParams as SolverTireParams, Track as SolverTrack, Tuning as SolverTuning,
    VehicleParams as SolverVehicleParams, VehicleState as SolverVehicleState, apply_tuning,
    resample_solution, solve,
};

use crate::drivers::apply_driver_to_tire;
use crate::errors::SimulatorError;
use crate::profiles::CompetitorProfile;
use crate::provider::ConfigProvider;

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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RuntimePitStop {
    pub lap: u16,
    pub tire_id: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SimulationRunRequest {
    pub vehicle_id: String,
    pub track_id: String,
    #[serde(default)]
    pub tuning: SolverTuning,
    #[serde(default)]
    pub initial_state: Option<SolverVehicleState>,
    #[serde(default = "default_lap_count")]
    pub lap_count: u16,
    #[serde(default)]
    pub pit_plan: Vec<RuntimePitStop>,
    #[serde(default)]
    pub driver_id: Option<String>,
    #[serde(default)]
    pub tire_id: Option<String>,
    #[serde(default)]
    pub profile_id: Option<String>,
    #[serde(default)]
    pub profile: Option<CompetitorProfile>,
    #[serde(default)]
    pub seed: Option<u64>,
    #[serde(default)]
    pub telemetry_hz: Option<f64>,
}

fn default_lap_count() -> u16 {
    1
}

#[derive(Debug, Clone)]
pub struct SimulationRunOutput {
    pub simulation: SimulationResult,
    pub telemetry: Option<ResampledTelemetry>,
    pub gateway_frames_5hz: Vec<TelemetryFrame>,
}

pub fn run_simulation(
    provider: &dyn ConfigProvider,
    request: &SimulationRunRequest,
) -> Result<SimulationRunOutput, SimulatorError> {
    let vehicle_config = provider.get_vehicle(&request.vehicle_id)?;
    let aero = provider.get_aero(&vehicle_config.aero_id)?;
    let chassis = provider.get_chassis(&vehicle_config.chassis_id)?;
    let engine = provider.get_engine(&vehicle_config.engine_id)?;
    let track = provider.get_track(&request.track_id)?;
    let profile = resolve_profile(
        provider,
        request.profile.clone(),
        request.profile_id.as_deref(),
    )?;
    let driver = resolve_driver(provider, request.driver_id.as_deref())?;
    let tire_id = request
        .tire_id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .unwrap_or(profile.tire_id.as_str());
    let base_tire = provider
        .get_tire(tire_id)
        .or_else(|_| provider.get_tire(&vehicle_config.tire_id))?;
    let tire = apply_driver_to_tire(
        &base_tire,
        &crate::drivers::driver_effects_from_aggressiveness(driver.aggressiveness),
    );

    let mut pit_stops = Vec::with_capacity(request.pit_plan.len());
    for stop in &request.pit_plan {
        let tire = provider.get_tire(&stop.tire_id)?;
        pit_stops.push(SolverPitStop {
            lap: stop.lap,
            tire: map_tire(&tire),
        });
    }

    let initial_state = request.initial_state.clone().unwrap_or(SolverVehicleState {
        fuel_mass: 100.0,
        tire_wear: 0.0,
        tire_temp: 90.0,
        engine_temp: engine.thermal.initial_temp_c,
        battery_soc: 0.0,
        exit_speed_mps: 0.0,
        exit_gear: 1,
    });

    let ds = track
        .s_m
        .windows(2)
        .next()
        .map(|window| window[1] - window[0])
        .unwrap_or(1.0);

    let solver_vehicle = apply_profile_to_vehicle(
        SolverVehicleParams {
            chassis: map_chassis(&chassis),
            aero: map_aero(&aero),
            engine: map_engine(&engine),
            tire: map_tire(&tire),
            hybrid: None,
        },
        &profile,
    );
    let solver_request = SolverSimulationRequest {
        track: map_track(&track),
        vehicle: apply_tuning(
            &solver_vehicle,
            &apply_profile_to_tuning(request.tuning.clone(), &profile),
        ),
        state: initial_state,
        config: SolverSimConfig {
            ds,
            max_speed: 400.0,
            pit_time_penalty_s: track.pit_loss_ms as f64 / 1000.0,
            pit_tire_temp: None,
            tire_temp_amb: 35.0,
            sim_seed: request.seed.unwrap_or(0),
        },
        energy_mode: pitgun_solver::EnergyMode::Balanced,
        lap_count: request.lap_count.max(1),
        pit_plan: SolverPitPlan { stops: pit_stops },
        driver,
        tuning: None,
    };

    let simulation =
        solve(&solver_request).map_err(|message| SimulatorError::InvalidInput(message))?;
    let telemetry = request
        .telemetry_hz
        .filter(|hz| *hz > 0.0)
        .map(|hz| {
            resample_solution(
                &solver_request.track,
                &simulation.solution,
                &simulation.applied_vehicle,
                hz,
            )
            .map_err(SimulatorError::InvalidInput)
        })
        .transpose()?;
    let player_telemetry = resample_solution(
        &solver_request.track,
        &simulation.solution,
        &simulation.applied_vehicle,
        5.0,
    )
    .map_err(SimulatorError::InvalidInput)?;
    let gateway_frames_5hz = gateway_frames(
        &player_telemetry,
        telemetry_session_id(request.seed.unwrap_or(0), &request.track_id, "player"),
        "pitwall-sim:player",
        &gateway_metadata(
            &request.track_id,
            &request.vehicle_id,
            &solver_request.driver.id,
            5.0,
        ),
    );

    Ok(SimulationRunOutput {
        simulation,
        telemetry,
        gateway_frames_5hz,
    })
}

fn resolve_profile(
    provider: &dyn ConfigProvider,
    explicit: Option<CompetitorProfile>,
    requested: Option<&str>,
) -> Result<CompetitorProfile, SimulatorError> {
    match (explicit, requested) {
        (Some(profile), _) => Ok(profile),
        (None, Some(id)) => provider
            .get_profile(id)
            .or_else(|_| provider.get_profile("balanced")),
        (None, None) => provider.get_profile("balanced"),
    }
}

fn resolve_driver(
    provider: &dyn ConfigProvider,
    requested: Option<&str>,
) -> Result<SolverDriver, SimulatorError> {
    let requested = requested
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("default");

    let driver_config = provider
        .get_driver(requested)
        .or_else(|_| provider.get_driver("default"))?;
    Ok(SolverDriver {
        id: driver_config.id,
        display_name: driver_config.display_name,
        aggressiveness: driver_config.aggressiveness,
    })
}

fn apply_profile_to_tuning(mut tuning: SolverTuning, profile: &CompetitorProfile) -> SolverTuning {
    tuning.downforce_slider = (tuning.downforce_slider + profile.downforce_bias).clamp(0.0, 1.0);
    tuning.gear_ratio_slider = (tuning.gear_ratio_slider + profile.gear_ratio_bias).clamp(0.0, 1.0);
    tuning
}

fn apply_profile_to_vehicle(
    mut vehicle: SolverVehicleParams,
    profile: &CompetitorProfile,
) -> SolverVehicleParams {
    let power = profile.power_multiplier();
    let heat = profile.heat_multiplier();
    let fuel = profile.fuel_multiplier();
    let tire_wear = profile.tire_wear_multiplier();

    for torque in &mut vehicle.engine.trq {
        *torque *= power;
    }
    vehicle.engine.alpha_heat *= heat;
    vehicle.engine.fuel_burn_kg_per_s *= fuel;
    vehicle.tire.wear_per_s *= tire_wear;

    vehicle
}

fn map_aero(value: &crate::models::AeroConfig) -> SolverAeroParams {
    SolverAeroParams {
        cd_a_x: value.cd_a_straight,
        cd_a_z: value.cd_a_corner,
        cl_a_x: value.cl_a_straight,
        cl_a_z: value.cl_a_corner,
    }
}

fn map_chassis(value: &crate::models::ChassisConfig) -> SolverChassisParams {
    SolverChassisParams {
        mass_empty: value.mass_empty_kg,
        r_wheel: value.wheel_radius_m,
        mu0: value.mu0,
        c_rr: value.rolling_resistance,
        rho: value.air_density,
        g: value.gravity,
    }
}

fn map_engine(value: &crate::models::EngineConfig) -> pitgun_solver::EngineParams {
    pitgun_solver::EngineParams {
        n_rpm: value.rpm_samples.clone(),
        trq: value.torque_samples.clone(),
        gear_ratios: value.gear_ratios.clone(),
        n_upshift: 0.0,
        n_downshift: 0.0,
        n_idle: value.idle_rpm,
        n_max: value.max_rpm,
        t_amb: value.thermal.ambient_temp_c,
        t_init: value.thermal.initial_temp_c,
        c_th: value.thermal.capacity_j_per_c,
        alpha_heat: value.thermal.heat_alpha,
        p_cool0: value.thermal.cooling_base_w,
        k_cool: value.thermal.cooling_speed_w_per_ms,
        t_soft: value.thermal.soft_temp_c,
        beta_derate: value.thermal.derate_per_c,
        fuel_burn_kg_per_s: value.fuel_burn_kg_per_s,
    }
}

fn map_tire(value: &crate::models::TireConfig) -> SolverTireParams {
    SolverTireParams {
        mu_scale: value.mu_scale,
        wear_per_s: value.wear_per_s,
        wear_load_k: value.wear_load_k,
        wear_grip_k: value.wear_grip_k,
        wear_min: value.wear_min,
        temp_opt: value.temp_opt_c,
        temp_sigma: value.temp_sigma_c,
        temp_min_k: value.temp_min_k,
        heat_k: value.heat_k,
        cool_k: value.cool_k,
    }
}

fn map_track(value: &crate::models::TrackConfig) -> SolverTrack {
    SolverTrack {
        s: value.s_m.clone(),
        x: value.x_m.clone(),
        y: value.y_m.clone(),
        z: value.z_m.clone(),
        kappa: value.curvature_radpm.clone(),
        slope: value.slope.clone(),
        heading: value.heading_rad.clone(),
    }
}

fn telemetry_session_id(seed: u64, track_id: &str, competitor_id: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    seed.hash(&mut hasher);
    track_id.hash(&mut hasher);
    competitor_id.hash(&mut hasher);
    (hasher.finish() & ((1u64 << 53) - 1)).max(1)
}

fn gateway_metadata(
    track_id: &str,
    vehicle_id: &str,
    driver_id: &str,
    sampling_hz: f64,
) -> HashMap<String, String> {
    HashMap::from([
        ("track_id".to_string(), track_id.to_string()),
        ("vehicle_id".to_string(), vehicle_id.to_string()),
        ("competitor_id".to_string(), "player".to_string()),
        ("driver_id".to_string(), driver_id.to_string()),
        ("role".to_string(), "player".to_string()),
        ("sampling_hz".to_string(), sampling_hz.to_string()),
    ])
}

fn sample(parameter_id: u16, value: f64) -> Sample {
    Sample::new(parameter_id, SampleValue::F64(value), SignalQuality::Good)
}

fn gateway_frames(
    telemetry: &ResampledTelemetry,
    session_id: u64,
    source_id: &str,
    metadata: &HashMap<String, String>,
) -> Vec<TelemetryFrame> {
    let lap_numbers = telemetry
        .n_lap
        .clone()
        .unwrap_or_else(|| vec![0; telemetry.time_s.len()]);
    let tire_temp = telemetry
        .tire_temp_c
        .clone()
        .unwrap_or_else(|| vec![0.0; telemetry.time_s.len()]);
    let tire_wear = telemetry
        .tire_wear_pct
        .clone()
        .unwrap_or_else(|| vec![0.0; telemetry.time_s.len()]);

    let mut frames = Vec::with_capacity(telemetry.time_s.len());
    for idx in 0..telemetry.time_s.len() {
        let timestamp_us = (telemetry.time_s[idx] * 1_000_000.0).round() as i64;
        frames.push(TelemetryFrame {
            session_id,
            sequence: idx as u64,
            timestamp_us,
            received_at_us: timestamp_us,
            source_id: source_id.to_string(),
            samples: vec![
                sample(PARAM_TIME_S, telemetry.time_s[idx]),
                sample(PARAM_DISTANCE_M, telemetry.s_m[idx]),
                sample(PARAM_X_M, telemetry.x_m[idx]),
                sample(PARAM_Y_M, telemetry.y_m[idx]),
                sample(PARAM_HEADING_RAD, telemetry.heading_rad[idx]),
                sample(PARAM_SPEED_KPH, telemetry.speed_kph[idx]),
                sample(PARAM_RPM, telemetry.rpm[idx]),
                sample(PARAM_GEAR, telemetry.gear[idx] as f64),
                sample(PARAM_THROTTLE_PCT, telemetry.throttle_pct[idx]),
                sample(PARAM_BRAKE_PCT, telemetry.brake_pct[idx]),
                sample(PARAM_G_LAT, telemetry.g_lat[idx]),
                sample(PARAM_G_LONG, telemetry.g_long[idx]),
                sample(PARAM_G_VERT, telemetry.g_vert[idx]),
                sample(PARAM_ENGINE_TEMP_C, telemetry.engine_temp_c[idx]),
                sample(PARAM_ENGINE_POWER_W, telemetry.engine_power_w[idx]),
                sample(PARAM_TIRE_TEMP_C, tire_temp[idx]),
                sample(PARAM_TIRE_WEAR_PCT, tire_wear[idx]),
            ],
            events: Vec::new(),
            lap_number: Some(lap_numbers[idx]),
            sector: None,
            lap_distance_m: Some(telemetry.s_m[idx] as f32),
            metadata: metadata.clone(),
        });
    }
    frames
}
