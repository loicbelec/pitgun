use std::collections::HashMap;

use serde::Deserialize;
use serde_json::Value;

use crate::errors::SimulatorError;
use crate::models::{
    AeroConfig, ChassisConfig, EngineConfig, EngineThermalConfig, TireConfig, TrackConfig,
    VehicleConfig,
};
use crate::profiles::CompetitorProfile;

pub trait ConfigProvider: Send + Sync {
    fn get_vehicle(&self, id: &str) -> Result<VehicleConfig, SimulatorError>;
    fn get_aero(&self, id: &str) -> Result<AeroConfig, SimulatorError>;
    fn get_chassis(&self, id: &str) -> Result<ChassisConfig, SimulatorError>;
    fn get_engine(&self, id: &str) -> Result<EngineConfig, SimulatorError>;
    fn get_tire(&self, id: &str) -> Result<TireConfig, SimulatorError>;
    fn get_track(&self, id: &str) -> Result<TrackConfig, SimulatorError>;
    fn get_profile(&self, id: &str) -> Result<CompetitorProfile, SimulatorError>;
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryConfigProvider {
    vehicles: HashMap<String, VehicleConfig>,
    aeros: HashMap<String, AeroConfig>,
    chassis: HashMap<String, ChassisConfig>,
    engines: HashMap<String, EngineConfig>,
    tires: HashMap<String, TireConfig>,
    tracks: HashMap<String, TrackConfig>,
    profiles: HashMap<String, CompetitorProfile>,
}

impl InMemoryConfigProvider {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert_vehicle(&mut self, value: VehicleConfig) {
        self.vehicles.insert(value.id.clone(), value);
    }

    pub fn insert_aero(&mut self, value: AeroConfig) {
        self.aeros.insert(value.id.clone(), value);
    }

    pub fn insert_chassis(&mut self, value: ChassisConfig) {
        self.chassis.insert(value.id.clone(), value);
    }

    pub fn insert_engine(&mut self, value: EngineConfig) {
        self.engines.insert(value.id.clone(), value);
    }

    pub fn insert_tire(&mut self, value: TireConfig) {
        self.tires.insert(value.id.clone(), value);
    }

    pub fn insert_track(&mut self, value: TrackConfig) {
        self.tracks.insert(value.id.clone(), value);
    }

    pub fn insert_profile(&mut self, value: CompetitorProfile) {
        self.profiles.insert(value.id.clone(), value);
    }

    pub fn insert_engine_from_json(&mut self, id: &str, json: &str) -> Result<(), SimulatorError> {
        let value: Value = serde_json::from_str(json)
            .map_err(|err| SimulatorError::Parse(format!("engine JSON parse error: {err}")))?;
        let engine = parse_engine_value(id, &value)?;
        self.insert_engine(engine);
        Ok(())
    }

    pub fn insert_aero_from_json(&mut self, id: &str, json: &str) -> Result<(), SimulatorError> {
        let value: Value = serde_json::from_str(json)
            .map_err(|err| SimulatorError::Parse(format!("aero JSON parse error: {err}")))?;
        let aero = parse_aero_value(id, &value)?;
        self.insert_aero(aero);
        Ok(())
    }

    pub fn insert_chassis_from_json(&mut self, id: &str, json: &str) -> Result<(), SimulatorError> {
        let value: Value = serde_json::from_str(json)
            .map_err(|err| SimulatorError::Parse(format!("chassis JSON parse error: {err}")))?;
        let chassis = parse_chassis_value(id, &value)?;
        self.insert_chassis(chassis);
        Ok(())
    }

    pub fn insert_tire_from_json(&mut self, id: &str, json: &str) -> Result<(), SimulatorError> {
        let value: Value = serde_json::from_str(json)
            .map_err(|err| SimulatorError::Parse(format!("tire JSON parse error: {err}")))?;
        let tire = parse_tire_value(id, &value)?;
        self.insert_tire(tire);
        Ok(())
    }

    pub fn insert_vehicle_from_json(&mut self, id: &str, json: &str) -> Result<(), SimulatorError> {
        let value: Value = serde_json::from_str(json)
            .map_err(|err| SimulatorError::Parse(format!("vehicle JSON parse error: {err}")))?;
        let vehicle = parse_vehicle_value(id, &value)?;
        self.insert_vehicle(vehicle);
        Ok(())
    }

    pub fn insert_track_from_json(&mut self, id: &str, json: &str) -> Result<(), SimulatorError> {
        let value: Value = serde_json::from_str(json)
            .map_err(|err| SimulatorError::Parse(format!("track JSON parse error: {err}")))?;
        let track = parse_track_value(id, &value)?;
        self.insert_track(track);
        Ok(())
    }

    pub fn insert_profile_from_json(&mut self, id: &str, json: &str) -> Result<(), SimulatorError> {
        let value: Value = serde_json::from_str(json)
            .map_err(|err| SimulatorError::Parse(format!("profile JSON parse error: {err}")))?;
        let profile = parse_profile_value(id, &value)?;
        self.insert_profile(profile);
        Ok(())
    }

    fn get_from<T: Clone>(
        map: &HashMap<String, T>,
        kind: &'static str,
        id: &str,
    ) -> Result<T, SimulatorError> {
        map.get(id)
            .cloned()
            .ok_or_else(|| SimulatorError::MissingConfig {
                kind,
                id: id.to_string(),
            })
    }
}

impl ConfigProvider for InMemoryConfigProvider {
    fn get_vehicle(&self, id: &str) -> Result<VehicleConfig, SimulatorError> {
        Self::get_from(&self.vehicles, "vehicle", id)
    }

    fn get_aero(&self, id: &str) -> Result<AeroConfig, SimulatorError> {
        Self::get_from(&self.aeros, "aero", id)
    }

    fn get_chassis(&self, id: &str) -> Result<ChassisConfig, SimulatorError> {
        Self::get_from(&self.chassis, "chassis", id)
    }

    fn get_engine(&self, id: &str) -> Result<EngineConfig, SimulatorError> {
        Self::get_from(&self.engines, "engine", id)
    }

    fn get_tire(&self, id: &str) -> Result<TireConfig, SimulatorError> {
        Self::get_from(&self.tires, "tire", id)
    }

    fn get_track(&self, id: &str) -> Result<TrackConfig, SimulatorError> {
        Self::get_from(&self.tracks, "track", id)
    }

    fn get_profile(&self, id: &str) -> Result<CompetitorProfile, SimulatorError> {
        Self::get_from(&self.profiles, "profile", id)
    }
}

#[cfg(all(feature = "json-files", not(target_arch = "wasm32")))]
#[derive(Debug, Clone)]
pub struct JsonFileConfigProvider {
    root: std::path::PathBuf,
}

#[cfg(all(feature = "json-files", not(target_arch = "wasm32")))]
impl JsonFileConfigProvider {
    pub fn new(root: impl Into<std::path::PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn read_json(&self, category: &str, id: &str) -> Result<Value, SimulatorError> {
        let path = self.root.join(category).join(format!("{id}.json"));
        let raw =
            std::fs::read_to_string(&path).map_err(|err| SimulatorError::Io(err.to_string()))?;
        serde_json::from_str(&raw).map_err(|err| SimulatorError::Parse(err.to_string()))
    }
}

#[cfg(all(feature = "json-files", not(target_arch = "wasm32")))]
impl ConfigProvider for JsonFileConfigProvider {
    fn get_vehicle(&self, id: &str) -> Result<VehicleConfig, SimulatorError> {
        parse_vehicle_value(id, &self.read_json("vehicles", id)?)
    }

    fn get_aero(&self, id: &str) -> Result<AeroConfig, SimulatorError> {
        parse_aero_value(id, &self.read_json("aero", id)?)
    }

    fn get_chassis(&self, id: &str) -> Result<ChassisConfig, SimulatorError> {
        parse_chassis_value(id, &self.read_json("chassis", id)?)
    }

    fn get_engine(&self, id: &str) -> Result<EngineConfig, SimulatorError> {
        parse_engine_value(id, &self.read_json("engines", id)?)
    }

    fn get_tire(&self, id: &str) -> Result<TireConfig, SimulatorError> {
        parse_tire_value(id, &self.read_json("tires", id)?)
    }

    fn get_track(&self, id: &str) -> Result<TrackConfig, SimulatorError> {
        parse_track_value(id, &self.read_json("circuits", id)?)
    }

    fn get_profile(&self, id: &str) -> Result<CompetitorProfile, SimulatorError> {
        let value = self.read_json("drivers", id)?;
        parse_profile_value(id, &value)
    }
}

#[cfg(not(all(feature = "json-files", not(target_arch = "wasm32"))))]
#[derive(Debug, Clone)]
pub struct JsonFileConfigProvider;

#[cfg(not(all(feature = "json-files", not(target_arch = "wasm32"))))]
impl JsonFileConfigProvider {
    pub fn new(_: impl Into<std::path::PathBuf>) -> Self {
        Self
    }
}

#[cfg(not(all(feature = "json-files", not(target_arch = "wasm32"))))]
impl ConfigProvider for JsonFileConfigProvider {
    fn get_vehicle(&self, id: &str) -> Result<VehicleConfig, SimulatorError> {
        Err(SimulatorError::InvalidInput(format!(
            "JsonFileConfigProvider unavailable on this target/feature set (vehicle {id})"
        )))
    }

    fn get_aero(&self, id: &str) -> Result<AeroConfig, SimulatorError> {
        Err(SimulatorError::InvalidInput(format!(
            "JsonFileConfigProvider unavailable on this target/feature set (aero {id})"
        )))
    }

    fn get_chassis(&self, id: &str) -> Result<ChassisConfig, SimulatorError> {
        Err(SimulatorError::InvalidInput(format!(
            "JsonFileConfigProvider unavailable on this target/feature set (chassis {id})"
        )))
    }

    fn get_engine(&self, id: &str) -> Result<EngineConfig, SimulatorError> {
        Err(SimulatorError::InvalidInput(format!(
            "JsonFileConfigProvider unavailable on this target/feature set (engine {id})"
        )))
    }

    fn get_tire(&self, id: &str) -> Result<TireConfig, SimulatorError> {
        Err(SimulatorError::InvalidInput(format!(
            "JsonFileConfigProvider unavailable on this target/feature set (tire {id})"
        )))
    }

    fn get_track(&self, id: &str) -> Result<TrackConfig, SimulatorError> {
        Err(SimulatorError::InvalidInput(format!(
            "JsonFileConfigProvider unavailable on this target/feature set (track {id})"
        )))
    }

    fn get_profile(&self, id: &str) -> Result<CompetitorProfile, SimulatorError> {
        Err(SimulatorError::InvalidInput(format!(
            "JsonFileConfigProvider unavailable on this target/feature set (profile {id})"
        )))
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum SeriesSpec {
    Values(Vec<f64>),
    Range { start: f64, end: f64, step: f64 },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum TorqueSegment {
    Linspace { start: f64, end: f64, num: usize },
    List { values: Vec<f64> },
}

#[derive(Debug, Deserialize)]
struct GearboxSpec {
    g1_total: f64,
    g_last_total: f64,
    gear_count: usize,
}

#[derive(Debug, Deserialize)]
struct ThermalJson {
    t_amb: f64,
    t_init: f64,
    c_th: f64,
    alpha_heat: f64,
    p_cool0: f64,
    k_cool: f64,
    t_soft: f64,
    beta_derate: f64,
}

#[derive(Debug, Deserialize)]
struct EngineJson {
    n_rpm: SeriesSpec,
    trq_segments: Vec<TorqueSegment>,
    gearbox: GearboxSpec,
    n_idle: f64,
    n_max: f64,
    thermal: ThermalJson,
    #[serde(default = "default_fuel_burn")]
    fuel_burn_kg_per_s: f64,
}

#[derive(Debug, Deserialize)]
struct DriverJson {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default = "default_aggressiveness")]
    aggressiveness: f64,
}

fn default_fuel_burn() -> f64 {
    0.02
}

fn default_aggressiveness() -> f64 {
    0.5
}

fn parse_aero_value(id: &str, value: &Value) -> Result<AeroConfig, SimulatorError> {
    let read = |k: &str| value.get(k).and_then(Value::as_f64).unwrap_or(0.0);
    let config = AeroConfig {
        id: id.to_string(),
        cd_a_straight: read("cdA_x"),
        cd_a_corner: read("cdA_z"),
        cl_a_straight: read("clA_x"),
        cl_a_corner: read("clA_z"),
    };
    config.validate()?;
    Ok(config)
}

fn parse_chassis_value(id: &str, value: &Value) -> Result<ChassisConfig, SimulatorError> {
    let read = |k: &str| value.get(k).and_then(Value::as_f64).unwrap_or(0.0);
    let config = ChassisConfig {
        id: id.to_string(),
        mass_empty_kg: read("mass_empty"),
        wheel_radius_m: read("r_wheel"),
        mu0: read("mu0"),
        rolling_resistance: read("c_rr"),
        air_density: read("rho"),
        gravity: value.get("g").and_then(Value::as_f64).unwrap_or(9.81),
    };
    config.validate()?;
    Ok(config)
}

fn parse_tire_value(id: &str, value: &Value) -> Result<TireConfig, SimulatorError> {
    let read = |k: &str| value.get(k).and_then(Value::as_f64).unwrap_or(0.0);
    let config = TireConfig {
        id: id.to_string(),
        mu_scale: read("mu_scale"),
        wear_per_s: read("wear_per_s"),
        wear_load_k: read("wear_load_k"),
        wear_grip_k: read("wear_grip_k"),
        wear_min: read("wear_min"),
        temp_opt_c: read("temp_opt"),
        temp_sigma_c: read("temp_sigma"),
        temp_min_k: read("temp_min_k"),
        heat_k: read("heat_k"),
        cool_k: read("cool_k"),
    };
    config.validate()?;
    Ok(config)
}

fn parse_vehicle_value(id: &str, value: &Value) -> Result<VehicleConfig, SimulatorError> {
    let config = VehicleConfig {
        id: id.to_string(),
        engine_id: value
            .get("engine")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        aero_id: value
            .get("aero")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        chassis_id: value
            .get("chassis")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        tire_id: {
            let tire = value
                .get("tire")
                .and_then(Value::as_str)
                .unwrap_or("medium");
            tire.to_string()
        },
    };

    if config.engine_id.is_empty() || config.aero_id.is_empty() || config.chassis_id.is_empty() {
        return Err(SimulatorError::InvalidConfig {
            kind: "vehicle",
            id: id.to_string(),
            reason: "engine/aero/chassis refs must be non-empty".to_string(),
        });
    }

    Ok(config)
}

fn parse_engine_value(id: &str, value: &Value) -> Result<EngineConfig, SimulatorError> {
    let parsed: EngineJson = serde_json::from_value(value.clone())
        .map_err(|err| SimulatorError::Parse(format!("engine '{id}' parse failed: {err}")))?;

    let rpm_samples = match parsed.n_rpm {
        SeriesSpec::Values(values) => values,
        SeriesSpec::Range { start, end, step } => build_range_series(start, end, step),
    };

    let mut torque_samples = Vec::new();
    for segment in parsed.trq_segments {
        match segment {
            TorqueSegment::Linspace { start, end, num } => {
                if num == 0 {
                    continue;
                }
                if num == 1 {
                    torque_samples.push(start);
                } else {
                    for i in 0..num {
                        let a = i as f64 / (num - 1) as f64;
                        torque_samples.push(start + (end - start) * a);
                    }
                }
            }
            TorqueSegment::List { values } => torque_samples.extend(values),
        }
    }

    if torque_samples.len() > rpm_samples.len() {
        torque_samples.truncate(rpm_samples.len());
    }
    while torque_samples.len() < rpm_samples.len() {
        let tail = torque_samples.last().copied().unwrap_or(0.0);
        torque_samples.push(tail);
    }

    let ratios = build_gear_ratios(
        parsed.gearbox.g1_total,
        parsed.gearbox.g_last_total,
        parsed.gearbox.gear_count.max(2),
    );

    let engine = EngineConfig {
        id: id.to_string(),
        rpm_samples,
        torque_samples,
        gear_ratios: ratios,
        idle_rpm: parsed.n_idle,
        max_rpm: parsed.n_max,
        thermal: EngineThermalConfig {
            ambient_temp_c: parsed.thermal.t_amb,
            initial_temp_c: parsed.thermal.t_init,
            capacity_j_per_c: parsed.thermal.c_th,
            heat_alpha: parsed.thermal.alpha_heat,
            cooling_base_w: parsed.thermal.p_cool0,
            cooling_speed_w_per_ms: parsed.thermal.k_cool,
            soft_temp_c: parsed.thermal.t_soft,
            derate_per_c: parsed.thermal.beta_derate,
        },
        fuel_burn_kg_per_s: parsed.fuel_burn_kg_per_s,
    };
    engine.validate()?;
    Ok(engine)
}

fn parse_track_value(id: &str, value: &Value) -> Result<TrackConfig, SimulatorError> {
    let data = value.get("data").unwrap_or(value);

    let s = read_vec(data, "s_m")?;
    let x = read_vec(data, "x_m")?;
    let y = read_vec(data, "y_m")?;
    let z = read_vec(data, "z_m")?;
    let curvature = if data.get("curvature_radpm").is_some() {
        read_vec(data, "curvature_radpm")?
    } else {
        vec![0.0; s.len()]
    };
    let slope = if data.get("slope_pct").is_some() {
        read_vec(data, "slope_pct")?
    } else if data.get("slope").is_some() {
        read_vec(data, "slope")?
    } else {
        derive_gradient(&s, &z)
    };
    let heading = if data.get("heading_rad").is_some() {
        read_vec(data, "heading_rad")?
    } else {
        derive_heading(&x, &y)
    };

    let track = TrackConfig {
        id: id.to_string(),
        s_m: s,
        x_m: x,
        y_m: y,
        z_m: z,
        curvature_radpm: curvature,
        slope,
        heading_rad: heading,
    };

    track.validate()?;
    Ok(track)
}

fn parse_profile_value(id: &str, value: &Value) -> Result<CompetitorProfile, SimulatorError> {
    let parsed: DriverJson = serde_json::from_value(value.clone())
        .map_err(|err| SimulatorError::Parse(format!("driver '{id}' parse failed: {err}")))?;

    let a = parsed.aggressiveness.clamp(0.0, 1.0);
    let (style, engine_mode) = if a < 0.33 {
        (
            crate::profiles::DrivingStyle::Conservative,
            crate::profiles::EngineMode::Economy,
        )
    } else if a < 0.66 {
        (
            crate::profiles::DrivingStyle::Balanced,
            crate::profiles::EngineMode::Balanced,
        )
    } else {
        (
            crate::profiles::DrivingStyle::Aggressive,
            crate::profiles::EngineMode::Push,
        )
    };

    Ok(CompetitorProfile {
        id: parsed.id.unwrap_or_else(|| id.to_string()),
        display_name: parsed.display_name.unwrap_or_else(|| id.to_string()),
        style,
        engine_mode,
        tire_id: "medium".to_string(),
        downforce_bias: 0.0,
        gear_ratio_bias: 0.0,
        pace_variance_ms: 20.0 + 60.0 * a,
    })
}

fn read_vec(value: &Value, key: &str) -> Result<Vec<f64>, SimulatorError> {
    let arr = value
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| SimulatorError::Parse(format!("missing or invalid array key '{key}'")))?;
    let mut out = Vec::with_capacity(arr.len());
    for item in arr {
        let Some(v) = item.as_f64() else {
            return Err(SimulatorError::Parse(format!(
                "non-numeric value in array '{key}'"
            )));
        };
        out.push(v);
    }
    Ok(out)
}

fn derive_heading(x: &[f64], y: &[f64]) -> Vec<f64> {
    let n = x.len().min(y.len());
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
    heading
}

fn derive_gradient(s: &[f64], values: &[f64]) -> Vec<f64> {
    let n = s.len().min(values.len());
    let mut out = vec![0.0; n];
    for i in 0..n {
        let i0 = i.saturating_sub(1);
        let i1 = (i + 1).min(n - 1);
        let ds = (s[i1] - s[i0]).abs().max(1e-6);
        out[i] = (values[i1] - values[i0]) / ds;
    }
    out
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

fn build_range_series(start: f64, end: f64, step: f64) -> Vec<f64> {
    if step <= 0.0 || end < start {
        return vec![start];
    }
    let mut out = Vec::new();
    let mut v = start;
    while v <= end + step * 0.5 {
        out.push(v);
        v += step;
    }
    out
}

fn build_gear_ratios(g1_total: f64, g_last_total: f64, gear_count: usize) -> Vec<f64> {
    let mut out = Vec::with_capacity(gear_count);
    if gear_count == 1 {
        return vec![g1_total];
    }
    for gear in 0..gear_count {
        let a = gear as f64 / (gear_count - 1) as f64;
        out.push(g1_total * (g_last_total / g1_total).powf(a));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_engine_shape_from_trackeagle_layout() {
        let raw = serde_json::json!({
            "n_rpm": {"start": 0.0, "end": 1000.0, "step": 250.0},
            "trq_segments": [
                {"type": "linspace", "start": 0.1, "end": 0.2, "num": 3},
                {"type": "list", "values": [0.2, 0.15]}
            ],
            "gearbox": {"g1_total": 10.0, "g_last_total": 5.0, "gear_count": 4},
            "n_idle": 500.0,
            "n_max": 15000.0,
            "thermal": {
                "t_amb": 35.0,
                "t_init": 90.0,
                "c_th": 100000.0,
                "alpha_heat": 0.4,
                "p_cool0": 0.0,
                "k_cool": 40.0,
                "t_soft": 110.0,
                "beta_derate": 0.01
            }
        });

        let engine = parse_engine_value("e1", &raw).expect("engine parse");
        assert_eq!(engine.id, "e1");
        assert_eq!(engine.rpm_samples.len(), 5);
        assert_eq!(engine.torque_samples.len(), 5);
        assert_eq!(engine.gear_ratios.len(), 4);
    }
}
