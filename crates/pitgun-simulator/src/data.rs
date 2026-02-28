use std::collections::HashMap;

#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;

use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::errors::SimulatorError;
use crate::models::{
    AeroConfig, ChassisConfig, EngineConfig, EngineThermalConfig, TireConfig, TrackConfig,
    VehicleConfig,
};
use crate::profiles::{CompetitorProfile, DrivingStyle, EngineMode};
use crate::provider::InMemoryConfigProvider;

const SCHEMA_VERSION: u32 = 1;
const TRACK_SAMPLE_POINTS: usize = 420;

const EMBEDDED_FILES: &[(&str, &str)] = &[
    ("aero/none.json", include_str!("../data/aero/none.json")),
    ("aero/basic.json", include_str!("../data/aero/basic.json")),
    ("aero/active.json", include_str!("../data/aero/active.json")),
    ("chassis/default.json", include_str!("../data/chassis/default.json")),
    ("chassis/f1_2026.json", include_str!("../data/chassis/f1_2026.json")),
    ("circuits/default.json", include_str!("../data/circuits/default.json")),
    ("circuits/monaco.json", include_str!("../data/circuits/monaco.json")),
    ("circuits/monza.json", include_str!("../data/circuits/monza.json")),
    ("circuits/spa.json", include_str!("../data/circuits/spa.json")),
    ("circuits/suzuka.json", include_str!("../data/circuits/suzuka.json")),
    ("drivers/aggressive.json", include_str!("../data/drivers/aggressive.json")),
    ("drivers/balanced.json", include_str!("../data/drivers/balanced.json")),
    ("drivers/conservative.json", include_str!("../data/drivers/conservative.json")),
    ("engines/v6t.json", include_str!("../data/engines/v6t.json")),
    (
        "engines/v6t_hybrid.json",
        include_str!("../data/engines/v6t_hybrid.json"),
    ),
    ("engines/v8_1960.json", include_str!("../data/engines/v8_1960.json")),
    ("engines/v8_1970.json", include_str!("../data/engines/v8_1970.json")),
    ("tires/hard.json", include_str!("../data/tires/hard.json")),
    ("tires/medium.json", include_str!("../data/tires/medium.json")),
    ("tires/soft.json", include_str!("../data/tires/soft.json")),
    (
        "vehicles/classic_v8_1960.json",
        include_str!("../data/vehicles/classic_v8_1960.json"),
    ),
    (
        "vehicles/classic_v8_1970.json",
        include_str!("../data/vehicles/classic_v8_1970.json"),
    ),
    ("vehicles/f1_2026.json", include_str!("../data/vehicles/f1_2026.json")),
    (
        "vehicles/modern_v6t.json",
        include_str!("../data/vehicles/modern_v6t.json"),
    ),
];

#[derive(Debug, Clone, Default)]
pub struct DataRegistry {
    aeros: HashMap<String, AeroConfig>,
    chassis: HashMap<String, ChassisConfig>,
    engines: HashMap<String, EngineConfig>,
    tires: HashMap<String, TireConfig>,
    tracks: HashMap<String, TrackConfig>,
    vehicles: HashMap<String, VehicleConfig>,
    profiles: HashMap<String, CompetitorProfile>,
}

impl DataRegistry {
    pub fn load_default() -> Result<Self, SimulatorError> {
        let mut registry = Self::default();
        for (path, raw) in EMBEDDED_FILES {
            registry.apply_file(path, raw.as_bytes(), false)?;
        }
        registry.validate()?;
        Ok(registry)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn load_from_dir(path: impl AsRef<Path>) -> Result<Self, SimulatorError> {
        let mut registry = Self::load_default()?;
        registry.merge_dir(path.as_ref())?;
        registry.validate()?;
        Ok(registry)
    }

    #[cfg(target_arch = "wasm32")]
    pub fn load_from_dir(_: impl AsRef<std::path::Path>) -> Result<Self, SimulatorError> {
        Err(SimulatorError::InvalidInput(
            "filesystem data packs are unavailable on wasm32".to_string(),
        ))
    }

    pub fn load_from_bytes_map(files: HashMap<String, Vec<u8>>) -> Result<Self, SimulatorError> {
        let mut registry = Self::load_default()?;
        let mut keys = files.keys().cloned().collect::<Vec<_>>();
        keys.sort();
        for key in keys {
            let Some(bytes) = files.get(&key) else {
                continue;
            };
            registry.apply_file(&key, bytes, true)?;
        }
        registry.validate()?;
        Ok(registry)
    }

    pub fn into_provider(self) -> InMemoryConfigProvider {
        let mut provider = InMemoryConfigProvider::new();
        for value in self.aeros.into_values() {
            provider.insert_aero(value);
        }
        for value in self.chassis.into_values() {
            provider.insert_chassis(value);
        }
        for value in self.engines.into_values() {
            provider.insert_engine(value);
        }
        for value in self.tires.into_values() {
            provider.insert_tire(value);
        }
        for value in self.tracks.into_values() {
            provider.insert_track(value);
        }
        for value in self.vehicles.into_values() {
            provider.insert_vehicle(value);
        }
        for value in self.profiles.into_values() {
            provider.insert_profile(value);
        }
        provider
    }

    fn validate(&self) -> Result<(), SimulatorError> {
        for aero in self.aeros.values() {
            aero.validate()?;
        }
        for chassis in self.chassis.values() {
            chassis.validate()?;
        }
        for engine in self.engines.values() {
            engine.validate()?;
        }
        for tire in self.tires.values() {
            tire.validate()?;
        }
        for track in self.tracks.values() {
            track.validate()?;
        }

        if !self.profiles.contains_key("balanced") {
            return Err(SimulatorError::InvalidConfig {
                kind: "profile",
                id: "balanced".to_string(),
                reason: "missing required default profile".to_string(),
            });
        }

        for profile in self.profiles.values() {
            if !self.tires.contains_key(&profile.tire_id) {
                return Err(SimulatorError::InvalidConfig {
                    kind: "profile",
                    id: profile.id.clone(),
                    reason: format!("unknown tire reference '{}'", profile.tire_id),
                });
            }
        }

        for vehicle in self.vehicles.values() {
            if !self.engines.contains_key(&vehicle.engine_id) {
                return Err(SimulatorError::InvalidConfig {
                    kind: "vehicle",
                    id: vehicle.id.clone(),
                    reason: format!("unknown engine reference '{}'", vehicle.engine_id),
                });
            }
            if !self.aeros.contains_key(&vehicle.aero_id) {
                return Err(SimulatorError::InvalidConfig {
                    kind: "vehicle",
                    id: vehicle.id.clone(),
                    reason: format!("unknown aero reference '{}'", vehicle.aero_id),
                });
            }
            if !self.chassis.contains_key(&vehicle.chassis_id) {
                return Err(SimulatorError::InvalidConfig {
                    kind: "vehicle",
                    id: vehicle.id.clone(),
                    reason: format!("unknown chassis reference '{}'", vehicle.chassis_id),
                });
            }
            if !self.tires.contains_key(&vehicle.tire_id) {
                return Err(SimulatorError::InvalidConfig {
                    kind: "vehicle",
                    id: vehicle.id.clone(),
                    reason: format!("unknown tire reference '{}'", vehicle.tire_id),
                });
            }
        }

        Ok(())
    }

    fn apply_file(
        &mut self,
        relative_path: &str,
        raw: &[u8],
        allow_override: bool,
    ) -> Result<(), SimulatorError> {
        let mut parts = relative_path.split('/');
        let Some(category) = parts.next() else {
            return Err(SimulatorError::Parse(format!(
                "invalid data pack path '{relative_path}'"
            )));
        };

        let raw_str = std::str::from_utf8(raw).map_err(|err| {
            SimulatorError::Parse(format!("invalid UTF-8 in '{relative_path}': {err}"))
        })?;

        match category {
            "aero" => {
                let data = parse_json::<AeroData>("aero", relative_path, raw_str)?;
                ensure_schema("aero", &data.id, data.schema_version)?;
                self.insert_aero(
                    AeroConfig {
                        id: data.id,
                        cd_a_straight: data.cd_a_straight,
                        cd_a_corner: data.cd_a_corner,
                        cl_a_straight: data.cl_a_straight,
                        cl_a_corner: data.cl_a_corner,
                    },
                    allow_override,
                )
            }
            "chassis" => {
                let data = parse_json::<ChassisData>("chassis", relative_path, raw_str)?;
                ensure_schema("chassis", &data.id, data.schema_version)?;
                self.insert_chassis(
                    ChassisConfig {
                        id: data.id,
                        mass_empty_kg: data.mass_empty_kg,
                        wheel_radius_m: data.wheel_radius_m,
                        mu0: data.mu0,
                        rolling_resistance: data.rolling_resistance,
                        air_density: data.air_density,
                        gravity: data.gravity,
                    },
                    allow_override,
                )
            }
            "engines" => {
                let data = parse_json::<EngineData>("engine", relative_path, raw_str)?;
                ensure_schema("engine", &data.id, data.schema_version)?;
                self.insert_engine(build_engine(data), allow_override)
            }
            "tires" => {
                let data = parse_json::<TireData>("tire", relative_path, raw_str)?;
                ensure_schema("tire", &data.id, data.schema_version)?;
                self.insert_tire(
                    TireConfig {
                        id: data.id,
                        mu_scale: data.mu_scale,
                        wear_per_s: data.wear_per_s,
                        wear_load_k: data.wear_load_k,
                        wear_grip_k: data.wear_grip_k,
                        wear_min: data.wear_min,
                        temp_opt_c: data.temp_opt_c,
                        temp_sigma_c: data.temp_sigma_c,
                        temp_min_k: data.temp_min_k,
                        heat_k: data.heat_k,
                        cool_k: data.cool_k,
                    },
                    allow_override,
                )
            }
            "circuits" => {
                let data = parse_json::<CircuitData>("track", relative_path, raw_str)?;
                ensure_schema("track", &data.id, data.schema_version)?;
                self.insert_track(build_track(data), allow_override)
            }
            "vehicles" => {
                let data = parse_json::<VehicleData>("vehicle", relative_path, raw_str)?;
                ensure_schema("vehicle", &data.id, data.schema_version)?;
                self.insert_vehicle(
                    VehicleConfig {
                        id: data.id,
                        aero_id: data.aero_id,
                        chassis_id: data.chassis_id,
                        engine_id: data.engine_id,
                        tire_id: data.tire_id,
                    },
                    allow_override,
                )
            }
            "drivers" => {
                let data = parse_json::<DriverData>("profile", relative_path, raw_str)?;
                ensure_schema("profile", &data.id, data.schema_version)?;
                self.insert_profile(
                    CompetitorProfile {
                        id: data.id,
                        display_name: data.display_name,
                        style: data.style,
                        engine_mode: data.engine_mode,
                        tire_id: data.tire_id,
                        downforce_bias: data.downforce_bias,
                        gear_ratio_bias: data.gear_ratio_bias,
                        pace_variance_ms: data.pace_variance_ms,
                    },
                    allow_override,
                )
            }
            _ => Err(SimulatorError::Parse(format!(
                "unknown data pack category '{category}'"
            ))),
        }
    }

    fn insert_aero(&mut self, value: AeroConfig, allow_override: bool) -> Result<(), SimulatorError> {
        insert_unique(&mut self.aeros, "aero", value.id.clone(), value, allow_override)
    }

    fn insert_chassis(
        &mut self,
        value: ChassisConfig,
        allow_override: bool,
    ) -> Result<(), SimulatorError> {
        insert_unique(
            &mut self.chassis,
            "chassis",
            value.id.clone(),
            value,
            allow_override,
        )
    }

    fn insert_engine(
        &mut self,
        value: EngineConfig,
        allow_override: bool,
    ) -> Result<(), SimulatorError> {
        insert_unique(
            &mut self.engines,
            "engine",
            value.id.clone(),
            value,
            allow_override,
        )
    }

    fn insert_tire(&mut self, value: TireConfig, allow_override: bool) -> Result<(), SimulatorError> {
        insert_unique(&mut self.tires, "tire", value.id.clone(), value, allow_override)
    }

    fn insert_track(
        &mut self,
        value: TrackConfig,
        allow_override: bool,
    ) -> Result<(), SimulatorError> {
        insert_unique(&mut self.tracks, "track", value.id.clone(), value, allow_override)
    }

    fn insert_vehicle(
        &mut self,
        value: VehicleConfig,
        allow_override: bool,
    ) -> Result<(), SimulatorError> {
        insert_unique(
            &mut self.vehicles,
            "vehicle",
            value.id.clone(),
            value,
            allow_override,
        )
    }

    fn insert_profile(
        &mut self,
        value: CompetitorProfile,
        allow_override: bool,
    ) -> Result<(), SimulatorError> {
        insert_unique(
            &mut self.profiles,
            "profile",
            value.id.clone(),
            value,
            allow_override,
        )
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn merge_dir(&mut self, root: &Path) -> Result<(), SimulatorError> {
        for category in [
            "aero", "chassis", "circuits", "drivers", "engines", "tires", "vehicles",
        ] {
            let dir = root.join(category);
            if !dir.exists() {
                continue;
            }
            let entries = std::fs::read_dir(&dir)
                .map_err(|err| SimulatorError::Io(format!("{}: {err}", dir.display())))?;

            let mut paths = entries
                .map(|entry| {
                    entry
                        .map(|value| value.path())
                        .map_err(|err| SimulatorError::Io(err.to_string()))
                })
                .collect::<Result<Vec<_>, _>>()?;
            paths.sort();

            for path in paths {
                if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                    continue;
                }
                let bytes = std::fs::read(&path)
                    .map_err(|err| SimulatorError::Io(format!("{}: {err}", path.display())))?;
                let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
                    return Err(SimulatorError::Parse(format!(
                        "invalid filename '{}'",
                        path.display()
                    )));
                };
                let relative = format!("{category}/{file_name}");
                self.apply_file(&relative, &bytes, true)?;
            }
        }
        Ok(())
    }
}

fn parse_json<T: DeserializeOwned>(kind: &str, path: &str, raw: &str) -> Result<T, SimulatorError> {
    serde_json::from_str(raw).map_err(|err| {
        SimulatorError::Parse(format!("failed to parse {kind} file '{path}': {err}"))
    })
}

fn ensure_schema(kind: &'static str, id: &str, schema_version: u32) -> Result<(), SimulatorError> {
    if schema_version != SCHEMA_VERSION {
        return Err(SimulatorError::InvalidConfig {
            kind,
            id: id.to_string(),
            reason: format!(
                "unsupported schema_version {schema_version}, expected {SCHEMA_VERSION}"
            ),
        });
    }
    Ok(())
}

fn insert_unique<T>(
    map: &mut HashMap<String, T>,
    kind: &'static str,
    id: String,
    value: T,
    allow_override: bool,
) -> Result<(), SimulatorError> {
    if map.contains_key(&id) && !allow_override {
        return Err(SimulatorError::InvalidConfig {
            kind,
            id,
            reason: "duplicate id in data pack".to_string(),
        });
    }
    map.insert(id, value);
    Ok(())
}

fn build_engine(data: EngineData) -> EngineConfig {
    let gear_count = data.gear_count.max(2);
    let step = 250.0;
    let mut rpm_samples = Vec::new();
    let mut torque_samples = Vec::new();
    let mut rpm = 0.0;
    while rpm <= data.max_rpm + step * 0.5 {
        rpm_samples.push(rpm);
        let normalized = if data.max_rpm > 0.0 {
            rpm / data.max_rpm
        } else {
            0.0
        };
        let tq = if normalized < 0.7 {
            data.tq_peak * (0.70 + 0.40 * normalized)
        } else {
            data.tq_peak * (1.0 - 0.65 * (normalized - 0.7))
        }
        .max(0.12);
        torque_samples.push(tq);
        rpm += step;
    }

    let g1_total = 14.0;
    let mut gear_ratios = Vec::with_capacity(gear_count);
    for idx in 0..gear_count {
        let a = idx as f64 / (gear_count - 1) as f64;
        gear_ratios.push(g1_total * (data.g_last_total / g1_total).powf(a));
    }

    EngineConfig {
        id: data.id,
        rpm_samples,
        torque_samples,
        gear_ratios,
        idle_rpm: data.idle_rpm,
        max_rpm: data.max_rpm,
        thermal: EngineThermalConfig {
            ambient_temp_c: data.thermal.ambient_temp_c,
            initial_temp_c: data.thermal.initial_temp_c,
            capacity_j_per_c: data.thermal.capacity_j_per_c,
            heat_alpha: data.thermal.heat_alpha,
            cooling_base_w: data.thermal.cooling_base_w,
            cooling_speed_w_per_ms: data.thermal.cooling_speed_w_per_ms,
            soft_temp_c: data.thermal.soft_temp_c,
            derate_per_c: data.thermal.derate_per_c,
        },
        fuel_burn_kg_per_s: data.fuel_burn_kg_per_s,
    }
}

fn build_track(data: CircuitData) -> TrackConfig {
    let mut s = Vec::with_capacity(TRACK_SAMPLE_POINTS);
    let mut x = Vec::with_capacity(TRACK_SAMPLE_POINTS);
    let mut y = Vec::with_capacity(TRACK_SAMPLE_POINTS);
    let mut z = Vec::with_capacity(TRACK_SAMPLE_POINTS);

    for i in 0..TRACK_SAMPLE_POINTS {
        let t = i as f64 / (TRACK_SAMPLE_POINTS - 1) as f64;
        let theta = t * std::f64::consts::TAU;
        s.push(t * data.distance_m);
        x.push(
            data.radius_x * theta.cos()
                + data.wobble_x * (2.6 * theta).cos() * 0.55
                + data.wobble_x * (4.2 * theta).sin() * 0.15,
        );
        y.push(
            data.radius_y * theta.sin()
                + data.wobble_y * (1.8 * theta).sin() * 0.60
                + data.wobble_y * (3.3 * theta).cos() * 0.20,
        );
        z.push(
            data.slope_amp_m * (1.7 * theta).sin() * 0.5
                + data.slope_amp_m * (0.4 * theta).cos() * 0.2,
        );
    }

    let mut heading = vec![0.0; TRACK_SAMPLE_POINTS];
    for i in 0..TRACK_SAMPLE_POINTS {
        let i0 = i.saturating_sub(1);
        let i1 = (i + 1).min(TRACK_SAMPLE_POINTS - 1);
        let dx = x[i1] - x[i0];
        let dy = y[i1] - y[i0];
        heading[i] = dy.atan2(dx);
    }
    for i in 1..TRACK_SAMPLE_POINTS {
        heading[i] = unwrap_angle(heading[i], heading[i - 1]);
    }

    let mut curvature = vec![0.0; TRACK_SAMPLE_POINTS];
    let mut slope = vec![0.0; TRACK_SAMPLE_POINTS];
    for i in 0..TRACK_SAMPLE_POINTS {
        let i0 = i.saturating_sub(1);
        let i1 = (i + 1).min(TRACK_SAMPLE_POINTS - 1);
        let ds = (s[i1] - s[i0]).max(1e-6);
        curvature[i] = (heading[i1] - heading[i0]) / ds;
        slope[i] = (z[i1] - z[i0]) / ds;
    }

    TrackConfig {
        id: data.id,
        s_m: s,
        x_m: x,
        y_m: y,
        z_m: z,
        curvature_radpm: curvature,
        slope,
        heading_rad: heading,
        pit_loss_ms: data.pit_loss_ms,
    }
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

#[derive(Debug, Deserialize)]
struct AeroData {
    schema_version: u32,
    id: String,
    cd_a_straight: f64,
    cd_a_corner: f64,
    cl_a_straight: f64,
    cl_a_corner: f64,
}

#[derive(Debug, Deserialize)]
struct ChassisData {
    schema_version: u32,
    id: String,
    mass_empty_kg: f64,
    wheel_radius_m: f64,
    mu0: f64,
    rolling_resistance: f64,
    air_density: f64,
    gravity: f64,
}

#[derive(Debug, Deserialize)]
struct EngineData {
    schema_version: u32,
    id: String,
    max_rpm: f64,
    g_last_total: f64,
    tq_peak: f64,
    gear_count: usize,
    idle_rpm: f64,
    fuel_burn_kg_per_s: f64,
    thermal: ThermalData,
}

#[derive(Debug, Deserialize)]
struct ThermalData {
    ambient_temp_c: f64,
    initial_temp_c: f64,
    capacity_j_per_c: f64,
    heat_alpha: f64,
    cooling_base_w: f64,
    cooling_speed_w_per_ms: f64,
    soft_temp_c: f64,
    derate_per_c: f64,
}

#[derive(Debug, Deserialize)]
struct TireData {
    schema_version: u32,
    id: String,
    mu_scale: f64,
    wear_per_s: f64,
    wear_load_k: f64,
    wear_grip_k: f64,
    wear_min: f64,
    temp_opt_c: f64,
    temp_sigma_c: f64,
    temp_min_k: f64,
    heat_k: f64,
    cool_k: f64,
}

#[derive(Debug, Deserialize)]
struct CircuitData {
    schema_version: u32,
    id: String,
    distance_m: f64,
    radius_x: f64,
    radius_y: f64,
    wobble_x: f64,
    wobble_y: f64,
    slope_amp_m: f64,
    pit_loss_ms: u64,
}

#[derive(Debug, Deserialize)]
struct VehicleData {
    schema_version: u32,
    id: String,
    aero_id: String,
    chassis_id: String,
    engine_id: String,
    tire_id: String,
}

#[derive(Debug, Deserialize)]
struct DriverData {
    schema_version: u32,
    id: String,
    display_name: String,
    style: DrivingStyle,
    engine_mode: EngineMode,
    tire_id: String,
    downforce_bias: f64,
    gear_ratio_bias: f64,
    pace_variance_ms: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_embedded_pack() {
        let registry = DataRegistry::load_default().expect("embedded pack should load");
        let provider = registry.into_provider();
        let vehicle = crate::provider::ConfigProvider::get_vehicle(&provider, "f1_2026")
            .expect("f1_2026 vehicle");
        let track =
            crate::provider::ConfigProvider::get_track(&provider, "SPA").expect("SPA track");

        assert_eq!(vehicle.engine_id, "v6t_hybrid");
        assert_eq!(track.pit_loss_ms, 22_000);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn external_pack_overrides_defaults_by_id() {
        let temp = tempfile::tempdir().expect("temp dir");
        let aero_dir = temp.path().join("aero");
        std::fs::create_dir_all(&aero_dir).expect("aero dir");
        std::fs::write(
            aero_dir.join("basic.json"),
            r#"{
  "schema_version": 1,
  "id": "basic",
  "cd_a_straight": 0.7,
  "cd_a_corner": 0.8,
  "cl_a_straight": 2.1,
  "cl_a_corner": 3.0
}"#,
        )
        .expect("override file");

        let provider = DataRegistry::load_from_dir(temp.path())
            .expect("load from dir")
            .into_provider();
        let aero = crate::provider::ConfigProvider::get_aero(&provider, "basic")
            .expect("basic aero");

        assert_eq!(aero.cd_a_straight, 0.7);
        assert_eq!(aero.cl_a_corner, 3.0);
    }

    #[test]
    fn bytes_map_rejects_unknown_references() {
        let mut files = HashMap::new();
        files.insert(
            "vehicles/bad.json".to_string(),
            br#"{
  "schema_version": 1,
  "id": "broken",
  "aero_id": "basic",
  "chassis_id": "f1_2026",
  "engine_id": "missing",
  "tire_id": "medium"
}"#
            .to_vec(),
        );

        let err = DataRegistry::load_from_bytes_map(files).expect_err("invalid refs must fail");
        assert!(err.to_string().contains("unknown engine reference"));
    }
}
