//! Deterministic synthetic telemetry source for local testing and tooling.
//!
//! This crate provides physics simulation sources for the Pitgun framework.
//!
//! # Source Implementations
//!
//! - **AsyncPhysicsSource**: Async source implementing [`TelemetrySource`](pitgun_contract::TelemetrySource)
//!   with configurable frame rate and deterministic output
//! - **PhysicsSource**: Legacy synchronous source implementing the `Source` trait
//!
//! # Usage
//!
//! ## Async Source (Recommended)
//!
//! ```rust,ignore
//! use pitgun_source_physics::{AsyncPhysicsSource, AsyncPhysicsConfig};
//! use pitgun_contract::TelemetrySource;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = AsyncPhysicsConfig::new()
//!         .with_tick_hz(60)
//!         .with_duration_ticks(600);
//!
//!     let mut source = AsyncPhysicsSource::new(config);
//!     source.start().await?;
//!
//!     let mut rx = source.subscribe();
//!     while let Some(frame) = rx.recv().await {
//!         println!("Frame: {} samples", frame.sample_count());
//!     }
//!     Ok(())
//! }
//! ```
//!
//! ## Legacy Sync Source
//!
//! The source emits `pitgun_core::EventBatch` values with integer nanosecond
//! timestamps derived from `tick_hz`. For each tick, all channels share the
//! same `ts_ns` and a batch contains `batch_ticks` consecutive ticks.
//!
//! Determinism scope: best-effort with `f64` math; expect stable results within
//! a single build and platform. A fixed-point implementation can replace the
//! floating-point model later.

// Async source (implements TelemetrySource)
mod async_source;
pub use async_source::{param_ids, AsyncPhysicsConfig, AsyncPhysicsSource};

// Re-export contract types for convenience
pub use pitgun_contract::{
    SourceConfig, SourceError, SourceMetadata, SourceState, SourceStats, SourceType,
    TelemetrySource,
};

// Legacy source implementation
use pitgun_contract::SignedSimulationContractV1;
use pitgun_core::{Event, EventBatch, Source};
use pitgun_signing::SigningKey;
use serde_json::Value as JsonValue;
use std::f64::consts::TAU;
use std::time::{SystemTime, UNIX_EPOCH};

const GRAVITY_MPS2: f64 = 9.81;
const MAX_STEER_DEG: f64 = 6.0;
const BASE_POWER_KPH_PER_S: f64 = 55.0;
const BASE_BRAKE_KPH_PER_S: f64 = 85.0;

#[derive(Debug)]
pub enum ContractError {
    InvalidJson(String),
    MissingField(String),
    InvalidEnum { field: String, value: String },
    InvalidSignature,
    Expired,
    SigningSecret(String),
    OutOfRange { field: String, value: f64 },
}

impl std::fmt::Display for ContractError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContractError::InvalidJson(message) => write!(f, "invalid JSON: {message}"),
            ContractError::MissingField(field) => write!(f, "missing field: {field}"),
            ContractError::InvalidEnum { field, value } => {
                write!(f, "invalid enum value for {field}: {value}")
            }
            ContractError::InvalidSignature => write!(f, "invalid signature"),
            ContractError::Expired => write!(f, "contract is expired"),
            ContractError::SigningSecret(message) => {
                write!(f, "signing secret error: {message}")
            }
            ContractError::OutOfRange { field, value } => {
                write!(f, "out of range {field}: {value}")
            }
        }
    }
}

impl std::error::Error for ContractError {}

#[derive(Clone, Copy, Debug)]
pub enum FuelMixture {
    Lean,
    Standard,
    Rich,
    StratQualif,
}

impl FuelMixture {
    fn power_multiplier(self) -> f64 {
        match self {
            FuelMixture::Lean => 0.98,
            FuelMixture::Standard => 1.0,
            FuelMixture::Rich => 1.04,
            FuelMixture::StratQualif => 1.08,
        }
    }

    fn cooling_multiplier(self) -> f64 {
        match self {
            FuelMixture::Lean => 0.92,
            FuelMixture::Standard => 1.0,
            FuelMixture::Rich => 1.06,
            FuelMixture::StratQualif => 0.95,
        }
    }

    fn from_str(field: &str, value: &str) -> Result<Self, ContractError> {
        match value {
            "lean" => Ok(FuelMixture::Lean),
            "standard" => Ok(FuelMixture::Standard),
            "rich" => Ok(FuelMixture::Rich),
            "strat_qualif" => Ok(FuelMixture::StratQualif),
            other => Err(ContractError::InvalidEnum {
                field: field.to_string(),
                value: other.to_string(),
            }),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum ErsDeploymentMap {
    Linear,
    TopSpeedBias,
    AccelerationBias,
    Balanced,
}

impl ErsDeploymentMap {
    fn accel_multiplier(self, speed_kph: f64) -> f64 {
        match self {
            ErsDeploymentMap::Linear => 1.0,
            ErsDeploymentMap::Balanced => 1.02,
            ErsDeploymentMap::TopSpeedBias => {
                if speed_kph >= 250.0 {
                    1.08
                } else {
                    0.96
                }
            }
            ErsDeploymentMap::AccelerationBias => {
                if speed_kph <= 180.0 {
                    1.08
                } else {
                    0.98
                }
            }
        }
    }

    fn aggression(self) -> f64 {
        match self {
            ErsDeploymentMap::Linear => 0.4,
            ErsDeploymentMap::Balanced => 0.5,
            ErsDeploymentMap::TopSpeedBias => 0.6,
            ErsDeploymentMap::AccelerationBias => 0.7,
        }
    }

    fn from_str(field: &str, value: &str) -> Result<Self, ContractError> {
        match value {
            "linear" => Ok(ErsDeploymentMap::Linear),
            "top_speed_bias" => Ok(ErsDeploymentMap::TopSpeedBias),
            "acceleration_bias" => Ok(ErsDeploymentMap::AccelerationBias),
            "balanced" => Ok(ErsDeploymentMap::Balanced),
            other => Err(ContractError::InvalidEnum {
                field: field.to_string(),
                value: other.to_string(),
            }),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum ActiveSuspensionMode {
    Static,
    RakeControl,
    AntiDive,
    FullActive,
}

#[derive(Clone, Copy, Debug)]
struct SuspensionFactors {
    drag_multiplier: f64,
    downforce_multiplier: f64,
    stability_bias: f64,
}

impl ActiveSuspensionMode {
    fn factors(self) -> SuspensionFactors {
        match self {
            ActiveSuspensionMode::Static => SuspensionFactors {
                drag_multiplier: 1.0,
                downforce_multiplier: 1.0,
                stability_bias: 0.7,
            },
            ActiveSuspensionMode::RakeControl => SuspensionFactors {
                drag_multiplier: 1.03,
                downforce_multiplier: 1.06,
                stability_bias: 0.9,
            },
            ActiveSuspensionMode::AntiDive => SuspensionFactors {
                drag_multiplier: 1.01,
                downforce_multiplier: 1.03,
                stability_bias: 1.0,
            },
            ActiveSuspensionMode::FullActive => SuspensionFactors {
                drag_multiplier: 1.04,
                downforce_multiplier: 1.1,
                stability_bias: 1.2,
            },
        }
    }

    fn from_str(field: &str, value: &str) -> Result<Self, ContractError> {
        match value {
            "static" => Ok(ActiveSuspensionMode::Static),
            "rake_control" => Ok(ActiveSuspensionMode::RakeControl),
            "anti_dive" => Ok(ActiveSuspensionMode::AntiDive),
            "full_active" => Ok(ActiveSuspensionMode::FullActive),
            other => Err(ContractError::InvalidEnum {
                field: field.to_string(),
                value: other.to_string(),
            }),
        }
    }
}

#[derive(Clone, Debug)]
pub struct PhysicsSourceConfig {
    /// Simulation tick rate in Hz.
    pub tick_hz: u32,
    /// Timestamp (ns) for tick 0.
    pub start_ts_ns: u64,
    /// Number of ticks per emitted batch.
    pub batch_ticks: u32,
    /// Total number of ticks to emit before end-of-stream.
    pub duration_ticks: u64,
    /// Front wing angle (deg) used for aero coefficients.
    pub aero_front_wing_angle_deg: f64,
    /// Rear wing angle (deg) used for aero coefficients.
    pub aero_rear_wing_angle_deg: f64,
    /// Optional turbo boost pressure (bar).
    pub turbo_boost_pressure_bar: Option<f64>,
    /// Final drive ratio for RPM derivation.
    pub gear_ratio_final: f64,
    /// Fuel mixture choice influencing power and cooling.
    pub fuel_mixture: FuelMixture,
    /// ERS deployment map used to bias acceleration.
    pub ers_deployment_map: ErsDeploymentMap,
    /// Traction control slip threshold (ratio).
    pub traction_control_slip: f64,
    /// Active suspension mode affecting drag and stability.
    pub active_suspension_mode: ActiveSuspensionMode,
}

impl Default for PhysicsSourceConfig {
    fn default() -> Self {
        Self {
            tick_hz: 60,
            start_ts_ns: 0,
            batch_ticks: 10,
            duration_ticks: 600,
            aero_front_wing_angle_deg: 18.0,
            aero_rear_wing_angle_deg: 22.0,
            turbo_boost_pressure_bar: None,
            gear_ratio_final: 4.0,
            fuel_mixture: FuelMixture::Standard,
            ers_deployment_map: ErsDeploymentMap::Balanced,
            traction_control_slip: 0.15,
            active_suspension_mode: ActiveSuspensionMode::Static,
        }
    }
}

impl PhysicsSourceConfig {
    pub fn from_signed_simulation_contract(
        signed: &SignedSimulationContractV1,
    ) -> Result<Self, ContractError> {
        let bytes = signed
            .contract
            .signing_bytes()
            .map_err(|err| ContractError::InvalidJson(err.to_string()))?;
        let key = SigningKey::from_env()
            .map_err(|err| ContractError::SigningSecret(err.to_string()))?;
        if !key.verify(&bytes, &signed.signature) {
            return Err(ContractError::InvalidSignature);
        }

        let now_ms = now_ms();
        if now_ms > signed.contract.expires_at_ms {
            return Err(ContractError::Expired);
        }

        let params = &signed.contract.parameters;
        let front_wing = get_required_f64(params, &["aero", "front_wing_angle"])?;
        let rear_wing = get_required_f64(params, &["aero", "rear_wing_angle"])?;
        let gear_ratio = get_required_f64(params, &["powertrain", "gear_ratio_final"])?;
        let fuel_mixture = get_required_str(params, &["powertrain", "fuel_mixture"])?;
        let ers_map = get_required_str(params, &["powertrain", "ers_deployment_map"])?;
        let turbo_boost = get_optional_f64(params, &["powertrain", "turbo_boost_pressure"])?;
        let traction_control_slip =
            get_optional_f64(params, &["electronics", "traction_control_slip"])?
                .unwrap_or(0.15);
        let suspension_mode =
            get_optional_str(params, &["chassis", "active_suspension_mode"])?
                .unwrap_or("static");

        let fuel_mixture = FuelMixture::from_str("powertrain.fuel_mixture", fuel_mixture)?;
        let ers_deployment_map = ErsDeploymentMap::from_str("powertrain.ers_deployment_map", ers_map)?;
        let active_suspension_mode =
            ActiveSuspensionMode::from_str("chassis.active_suspension_mode", suspension_mode)?;

        let mut config = PhysicsSourceConfig::default();
        config.aero_front_wing_angle_deg = front_wing;
        config.aero_rear_wing_angle_deg = rear_wing;
        config.gear_ratio_final = gear_ratio;
        config.turbo_boost_pressure_bar = turbo_boost;
        config.fuel_mixture = fuel_mixture;
        config.ers_deployment_map = ers_deployment_map;
        config.traction_control_slip = traction_control_slip;
        config.active_suspension_mode = active_suspension_mode;

        if config.tick_hz == 0 {
            return Err(ContractError::OutOfRange {
                field: "tick_hz".to_string(),
                value: config.tick_hz as f64,
            });
        }
        if config.batch_ticks == 0 {
            return Err(ContractError::OutOfRange {
                field: "batch_ticks".to_string(),
                value: config.batch_ticks as f64,
            });
        }
        if config.duration_ticks == 0 {
            return Err(ContractError::OutOfRange {
                field: "duration_ticks".to_string(),
                value: config.duration_ticks as f64,
            });
        }

        Ok(config)
    }
}

fn get_required_value<'a>(
    value: &'a JsonValue,
    path: &[&str],
) -> Result<&'a JsonValue, ContractError> {
    let mut current = value;
    let mut full_path = String::new();
    for (idx, key) in path.iter().enumerate() {
        if idx > 0 {
            full_path.push('.');
        }
        full_path.push_str(key);
        let Some(next) = current.as_object().and_then(|map| map.get(*key)) else {
            return Err(ContractError::MissingField(full_path));
        };
        current = next;
    }
    Ok(current)
}

fn get_optional_value<'a>(value: &'a JsonValue, path: &[&str]) -> Option<&'a JsonValue> {
    let mut current = value;
    for key in path {
        let Some(next) = current.as_object().and_then(|map| map.get(*key)) else {
            return None;
        };
        current = next;
    }
    Some(current)
}

fn get_required_f64(value: &JsonValue, path: &[&str]) -> Result<f64, ContractError> {
    let value = get_required_value(value, path)?;
    let number = value.as_f64().ok_or_else(|| {
        ContractError::InvalidJson(format!("{} must be a number", path.join(".")))
    })?;
    if !number.is_finite() {
        return Err(ContractError::InvalidJson(format!(
            "{} must be finite",
            path.join(".")
        )));
    }
    Ok(number)
}

fn get_optional_f64(value: &JsonValue, path: &[&str]) -> Result<Option<f64>, ContractError> {
    let Some(value) = get_optional_value(value, path) else {
        return Ok(None);
    };
    let number = value.as_f64().ok_or_else(|| {
        ContractError::InvalidJson(format!("{} must be a number", path.join(".")))
    })?;
    if !number.is_finite() {
        return Err(ContractError::InvalidJson(format!(
            "{} must be finite",
            path.join(".")
        )));
    }
    Ok(Some(number))
}

fn get_required_str<'a>(value: &'a JsonValue, path: &[&str]) -> Result<&'a str, ContractError> {
    let value = get_required_value(value, path)?;
    value.as_str().ok_or_else(|| {
        ContractError::InvalidJson(format!("{} must be a string", path.join(".")))
    })
}

fn get_optional_str<'a>(
    value: &'a JsonValue,
    path: &[&str],
) -> Result<Option<&'a str>, ContractError> {
    let Some(value) = get_optional_value(value, path) else {
        return Ok(None);
    };
    let text = value.as_str().ok_or_else(|| {
        ContractError::InvalidJson(format!("{} must be a string", path.join(".")))
    })?;
    Ok(Some(text))
}

fn now_ms() -> i64 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    now.as_millis() as i64
}

pub struct PhysicsSource {
    config: PhysicsSourceConfig,
    channels: PhysicsChannels,
    tick: u64,
    speed_kph: f64,
    engine_temp_c: f64,
    instability: f64,
    dt_s: f64,
    dt_ns: u64,
    channels_per_tick: usize,
    end_emitted: bool,
}

impl PhysicsSource {
    pub fn new(mut config: PhysicsSourceConfig) -> Self {
        if config.tick_hz == 0 {
            config.tick_hz = 1;
        }
        if config.batch_ticks == 0 {
            config.batch_ticks = 1;
        }
        let dt_s = 1.0 / config.tick_hz as f64;
        // Note: integer division can drift vs. dt_s for non-divisible tick_hz.
        let dt_ns = 1_000_000_000u64 / config.tick_hz as u64;
        let channels = PhysicsChannels::new(config.turbo_boost_pressure_bar.is_some());
        let channels_per_tick = channels.len();
        Self {
            config,
            channels,
            tick: 0,
            speed_kph: 0.0,
            engine_temp_c: 90.0,
            instability: 5.0,
            dt_s,
            dt_ns,
            channels_per_tick,
            end_emitted: false,
        }
    }

    /// Deterministic driver profile: throttle ramps over 2s, then periodic
    /// lift/brake pulses, with a smooth sinusoidal steering input.
    fn driver_inputs(&self, time_s: f64) -> (f64, f64, f64) {
        let ramp = if time_s < 2.0 { time_s / 2.0 } else { 1.0 };
        let mut throttle = 100.0 * ramp;
        let mut brake: f64 = 0.0;

        if time_s >= 2.0 {
            let cycle = (time_s - 2.0) % 5.0;
            if cycle < 0.35 {
                throttle *= 0.85;
            } else if cycle < 0.65 {
                brake = 10.0;
                throttle *= 0.75;
            } else if (3.1..3.4).contains(&cycle) {
                brake = 6.0;
                throttle *= 0.88;
            }
        }

        let steering_angle_deg = MAX_STEER_DEG * (TAU * time_s / 7.0).sin();
        (
            throttle.clamp(0.0, 100.0),
            brake.clamp(0.0, 100.0),
            steering_angle_deg,
        )
    }

    fn aero_forces(&self, speed_kph: f64) -> (f64, f64, f64, SuspensionFactors) {
        let speed_mps = speed_kph / 3.6;
        let wing_drag = self.config.aero_front_wing_angle_deg * 0.012
            + self.config.aero_rear_wing_angle_deg * 0.015;
        let wing_downforce = self.config.aero_front_wing_angle_deg * 0.02
            + self.config.aero_rear_wing_angle_deg * 0.03;
        let suspension = self.config.active_suspension_mode.factors();
        let drag_multiplier = (1.0 + wing_drag) * suspension.drag_multiplier;
        let downforce_multiplier = (1.0 + wing_downforce) * suspension.downforce_multiplier;
        let drag_n = 12.0 * drag_multiplier * speed_mps * speed_mps;
        let downforce_n = 24.0 * downforce_multiplier * speed_mps * speed_mps;
        let drag_kph_per_s = drag_n * 0.00018;
        (drag_n, downforce_n, drag_kph_per_s, suspension)
    }

    fn gear_for_speed(speed_kph: f64) -> u8 {
        if speed_kph < 25.0 {
            1
        } else if speed_kph < 50.0 {
            2
        } else if speed_kph < 80.0 {
            3
        } else if speed_kph < 120.0 {
            4
        } else if speed_kph < 170.0 {
            5
        } else if speed_kph < 220.0 {
            6
        } else if speed_kph < 270.0 {
            7
        } else {
            8
        }
    }

    fn rpm_for_speed(&self, speed_kph: f64) -> f64 {
        let rpm = speed_kph * self.config.gear_ratio_final * 18.0 + 2800.0;
        rpm.clamp(3000.0, 13_000.0)
    }

    fn update_engine_temp(&mut self, throttle_pct: f64, speed_kph: f64, boost: f64) {
        let heat = (throttle_pct / 100.0) * (8.0 + boost * 4.0);
        let cooling = (4.0 + speed_kph / 80.0) * self.config.fuel_mixture.cooling_multiplier();
        self.engine_temp_c += (heat - cooling) * self.dt_s;
        self.engine_temp_c = self.engine_temp_c.clamp(70.0, 140.0);
    }

    fn update_instability(
        &mut self,
        throttle_pct: f64,
        brake_pct: f64,
        speed_kph: f64,
        boost: f64,
        downforce_n: f64,
        suspension: SuspensionFactors,
    ) {
        let speed_term = (speed_kph / 350.0).clamp(0.0, 1.4);
        let tc_slip = self.config.traction_control_slip.clamp(0.05, 0.25);
        let tc_penalty = tc_slip * 30.0;
        let tc_help = (0.25 - tc_slip).max(0.0) * 12.0;
        let downforce_help = (downforce_n / 160_000.0).clamp(0.0, 1.0) * 2.0;
        let rise = 2.2 * speed_term
            + boost * 1.8
            + self.config.ers_deployment_map.aggression()
            + (throttle_pct / 100.0) * 0.8
            + (brake_pct / 100.0) * 0.6
            + tc_penalty;
        let drop = suspension.stability_bias + downforce_help + tc_help;
        self.instability += (rise - drop) * self.dt_s * 6.0;
        self.instability = self.instability.clamp(0.0, 100.0);
    }

    fn tick_values(&mut self) -> TickValues {
        let time_s = self.tick as f64 * self.dt_s;
        let (throttle_pct, brake_pct, steering_angle_deg) = self.driver_inputs(time_s);
        let boost = self.config.turbo_boost_pressure_bar.unwrap_or(0.0);

        let speed_prev = self.speed_kph;
        let (_drag_n_prev, _downforce_prev, drag_kph_per_s, _susp_prev) =
            self.aero_forces(speed_prev);
        let turbo_multiplier = 1.0 + boost * 0.18;
        let power_multiplier = self.config.fuel_mixture.power_multiplier()
            * self
                .config
                .ers_deployment_map
                .accel_multiplier(speed_prev)
            * turbo_multiplier;
        let throttle_force = (throttle_pct / 100.0) * BASE_POWER_KPH_PER_S * power_multiplier;
        let brake_force = (brake_pct / 100.0) * BASE_BRAKE_KPH_PER_S;
        let accel_kph_per_s = throttle_force - brake_force - drag_kph_per_s;

        self.speed_kph = (speed_prev + accel_kph_per_s * self.dt_s).max(0.0);

        let speed_mps_prev = speed_prev / 3.6;
        let speed_mps_now = self.speed_kph / 3.6;
        let g_long = (speed_mps_now - speed_mps_prev) / self.dt_s / GRAVITY_MPS2;
        let g_lat = ((steering_angle_deg / MAX_STEER_DEG) * (speed_mps_now / 25.0))
            .clamp(-2.5, 2.5);

        let (drag_n, downforce_n, _drag_kph_per_s, suspension) =
            self.aero_forces(self.speed_kph);
        self.update_engine_temp(throttle_pct, self.speed_kph, boost);
        self.update_instability(
            throttle_pct,
            brake_pct,
            self.speed_kph,
            boost,
            downforce_n,
            suspension,
        );

        let rpm = self.rpm_for_speed(self.speed_kph);
        let gear_index = Self::gear_for_speed(self.speed_kph) as f64;

        TickValues {
            speed_kph: self.speed_kph,
            rpm,
            gear_index,
            throttle_pct,
            brake_pct,
            steering_angle_deg,
            g_lat,
            g_long,
            engine_temp_c: self.engine_temp_c,
            current_drag_n: drag_n,
            current_downforce_n: downforce_n,
            instability_index: self.instability,
            boost_pressure_bar: self.config.turbo_boost_pressure_bar,
        }
    }
}

impl Source for PhysicsSource {
    fn next_batch(&mut self) -> Option<EventBatch> {
        if self.end_emitted {
            return None;
        }
        if self.tick >= self.config.duration_ticks {
            self.end_emitted = true;
            return Some(EventBatch {
                events: Vec::new(),
                aggregates: Vec::new(),
                end_of_stream: true,
            });
        }

        let remaining = self.config.duration_ticks - self.tick;
        let ticks_this_batch = (self.config.batch_ticks as u64).min(remaining);
        let mut events = Vec::with_capacity(ticks_this_batch as usize * self.channels_per_tick);

        for _ in 0..ticks_this_batch {
            let ts_ns = self
                .config
                .start_ts_ns
                .saturating_add(self.tick.saturating_mul(self.dt_ns));
            let values = self.tick_values();
            push_event(&mut events, &self.channels.speed_kph, ts_ns, values.speed_kph);
            push_event(&mut events, &self.channels.rpm, ts_ns, values.rpm);
            push_event(&mut events, &self.channels.gear_index, ts_ns, values.gear_index);
            push_event(
                &mut events,
                &self.channels.throttle_pct,
                ts_ns,
                values.throttle_pct,
            );
            push_event(
                &mut events,
                &self.channels.brake_pct,
                ts_ns,
                values.brake_pct,
            );
            push_event(
                &mut events,
                &self.channels.steering_angle_deg,
                ts_ns,
                values.steering_angle_deg,
            );
            push_event(&mut events, &self.channels.g_lat, ts_ns, values.g_lat);
            push_event(&mut events, &self.channels.g_long, ts_ns, values.g_long);
            push_event(
                &mut events,
                &self.channels.engine_temp_c,
                ts_ns,
                values.engine_temp_c,
            );
            push_event(
                &mut events,
                &self.channels.current_drag_n,
                ts_ns,
                values.current_drag_n,
            );
            push_event(
                &mut events,
                &self.channels.current_downforce_n,
                ts_ns,
                values.current_downforce_n,
            );
            push_event(
                &mut events,
                &self.channels.instability_index,
                ts_ns,
                values.instability_index,
            );
            if let Some(boost) = values.boost_pressure_bar {
                if let Some(channel) = &self.channels.boost_pressure_bar {
                    push_event(&mut events, channel, ts_ns, boost);
                }
            }
            self.tick += 1;
        }

        Some(EventBatch {
            events,
            aggregates: Vec::new(),
            end_of_stream: false,
        })
    }
}

struct TickValues {
    speed_kph: f64,
    rpm: f64,
    gear_index: f64,
    throttle_pct: f64,
    brake_pct: f64,
    steering_angle_deg: f64,
    g_lat: f64,
    g_long: f64,
    engine_temp_c: f64,
    current_drag_n: f64,
    current_downforce_n: f64,
    instability_index: f64,
    boost_pressure_bar: Option<f64>,
}

struct PhysicsChannels {
    speed_kph: String,
    rpm: String,
    gear_index: String,
    throttle_pct: String,
    brake_pct: String,
    steering_angle_deg: String,
    g_lat: String,
    g_long: String,
    engine_temp_c: String,
    current_drag_n: String,
    current_downforce_n: String,
    instability_index: String,
    boost_pressure_bar: Option<String>,
}

impl PhysicsChannels {
    fn new(include_boost: bool) -> Self {
        Self {
            speed_kph: "speed_kph".to_string(),
            rpm: "rpm".to_string(),
            gear_index: "gear_index".to_string(),
            throttle_pct: "throttle_pct".to_string(),
            brake_pct: "brake_pct".to_string(),
            steering_angle_deg: "steering_angle_deg".to_string(),
            g_lat: "g_lat".to_string(),
            g_long: "g_long".to_string(),
            engine_temp_c: "engine_temp_c".to_string(),
            current_drag_n: "current_drag_n".to_string(),
            current_downforce_n: "current_downforce_n".to_string(),
            instability_index: "instability_index".to_string(),
            boost_pressure_bar: include_boost.then(|| "boost_pressure_bar".to_string()),
        }
    }

    fn len(&self) -> usize {
        let mut count = 12;
        if self.boost_pressure_bar.is_some() {
            count += 1;
        }
        count
    }
}

fn push_event(events: &mut Vec<Event>, channel: &String, ts_ns: u64, value: f64) {
    events.push(Event {
        channel: channel.clone(),
        ts_ns,
        value,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use pitgun_contract::SimulationContractV1;
    use pitgun_signing::SIGNING_SECRET_ENV;
    use serde_json::json;
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    #[test]
    fn determinism_first_batch() {
        let config = PhysicsSourceConfig::default();
        let mut left = PhysicsSource::new(config.clone());
        let mut right = PhysicsSource::new(config);

        let left_batch = left.next_batch().expect("expected first batch");
        let right_batch = right.next_batch().expect("expected first batch");

        assert_eq!(left_batch.events.len(), right_batch.events.len());
        for (l, r) in left_batch.events.iter().zip(right_batch.events.iter()) {
            assert_eq!(l.channel, r.channel);
            assert_eq!(l.ts_ns, r.ts_ns);
            assert_f64_close(l.value, r.value, 1e-9);
        }
        assert_eq!(left_batch.end_of_stream, right_batch.end_of_stream);
    }

    #[test]
    fn end_of_stream_after_duration() {
        let config = PhysicsSourceConfig {
            duration_ticks: 3,
            batch_ticks: 2,
            ..PhysicsSourceConfig::default()
        };
        let mut source = PhysicsSource::new(config);

        let first = source.next_batch().expect("expected batch 1");
        assert!(!first.end_of_stream);
        assert_eq!(distinct_ts(&first.events), 2);
        let second = source.next_batch().expect("expected batch 2");
        assert!(!second.end_of_stream);
        assert_eq!(distinct_ts(&second.events), 1);
        let eos = source.next_batch().expect("expected eos batch");
        assert!(eos.end_of_stream);
        assert!(eos.events.is_empty());
        assert!(source.next_batch().is_none());
    }

    #[test]
    fn timestamps_monotonic_and_spaced() {
        let config = PhysicsSourceConfig {
            tick_hz: 50,
            batch_ticks: 4,
            duration_ticks: 4,
            ..PhysicsSourceConfig::default()
        };
        let mut source = PhysicsSource::new(config);
        let batch = source.next_batch().expect("expected batch");
        let dt_ns = 1_000_000_000u64 / 50;

        let mut unique_ts = Vec::new();
        let mut last_ts = 0u64;
        for event in &batch.events {
            assert!(event.ts_ns >= last_ts);
            if unique_ts.last().copied() != Some(event.ts_ns) {
                unique_ts.push(event.ts_ns);
            }
            last_ts = event.ts_ns;
        }

        assert_eq!(unique_ts.len(), 4);
        assert_eq!(unique_ts[0], 0);
        for window in unique_ts.windows(2) {
            assert_eq!(window[1] - window[0], dt_ns);
        }
    }

    fn assert_f64_close(left: f64, right: f64, eps: f64) {
        let diff = (left - right).abs();
        assert!(
            diff <= eps,
            "expected {left} ~= {right} (diff={diff}, eps={eps})"
        );
    }

    fn distinct_ts(events: &[Event]) -> usize {
        let mut unique = Vec::new();
        for event in events {
            if unique.last().copied() != Some(event.ts_ns) {
                unique.push(event.ts_ns);
            }
        }
        unique.len()
    }

    fn with_signing_env<T>(secret: &str, action: impl FnOnce() -> T) -> T {
        static ENV_LOCK: Mutex<()> = Mutex::new(());
        let _guard = ENV_LOCK.lock().expect("env lock");
        let previous = std::env::var(SIGNING_SECRET_ENV).ok();
        unsafe {
            std::env::set_var(SIGNING_SECRET_ENV, secret);
        }
        let result = action();
        unsafe {
            match previous {
                Some(value) => std::env::set_var(SIGNING_SECRET_ENV, value),
                None => std::env::remove_var(SIGNING_SECRET_ENV),
            }
        }
        result
    }

    fn signed_contract(secret: &[u8], expires_at_ms: i64) -> SignedSimulationContractV1 {
        let contract = SimulationContractV1 {
            version: "SimulationContractV1".to_string(),
            issued_at_ms: 1_710_000_000_000,
            expires_at_ms,
            era: 3,
            category_levels: BTreeMap::from([
                ("mech_lvl".to_string(), 5),
                ("testing_lvl".to_string(), 10),
            ]),
            owned_upgrades: vec!["e2_turbocharger".to_string(), "e2_hybrid_sys".to_string()],
            parameters: json!({
                "aero": {
                    "front_wing_angle": 18.0,
                    "rear_wing_angle": 22.0
                },
                "powertrain": {
                    "gear_ratio_final": 4.1,
                    "turbo_boost_pressure": 1.6,
                    "fuel_mixture": "rich",
                    "ers_deployment_map": "balanced"
                }
            }),
            derived_constraints: None,
            policy_hash: "deadbeef".to_string(),
        };
        let bytes = contract.signing_bytes().expect("payload should serialize");
        let key = SigningKey::from_secret(secret).expect("secret should be valid");
        SignedSimulationContractV1 {
            contract,
            signature: key.sign(&bytes),
        }
    }

    #[test]
    fn config_from_signed_contract_maps_fields() {
        let expires_at_ms = now_ms() + 60_000;
        let signed = signed_contract(b"unit-test-secret", expires_at_ms);
        let config = with_signing_env("unit-test-secret", || {
            PhysicsSourceConfig::from_signed_simulation_contract(&signed)
                .expect("contract should map")
        });

        assert_eq!(config.aero_front_wing_angle_deg, 18.0);
        assert_eq!(config.aero_rear_wing_angle_deg, 22.0);
        assert_eq!(config.gear_ratio_final, 4.1);
        assert_eq!(config.turbo_boost_pressure_bar, Some(1.6));
        assert!(matches!(config.fuel_mixture, FuelMixture::Rich));
        assert!(matches!(
            config.ers_deployment_map,
            ErsDeploymentMap::Balanced
        ));
        assert_eq!(config.traction_control_slip, 0.15);
        assert!(matches!(
            config.active_suspension_mode,
            ActiveSuspensionMode::Static
        ));
    }

    #[test]
    fn invalid_signature_is_rejected() {
        let expires_at_ms = now_ms() + 60_000;
        let signed = signed_contract(b"unit-test-secret", expires_at_ms);
        let result = with_signing_env("wrong-secret", || {
            PhysicsSourceConfig::from_signed_simulation_contract(&signed)
        });

        match result {
            Err(ContractError::InvalidSignature) => {}
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn expired_contract_is_rejected() {
        let signed = signed_contract(b"unit-test-secret", 0);
        let result = with_signing_env("unit-test-secret", || {
            PhysicsSourceConfig::from_signed_simulation_contract(&signed)
        });

        match result {
            Err(ContractError::Expired) => {}
            other => panic!("unexpected result: {other:?}"),
        }
    }
}
