use std::collections::HashMap;

#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;

use serde::Deserialize;
use serde_json::Value;

use crate::errors::SimulatorError;
use crate::models::{
    AeroConfig, ChassisConfig, DriverConfig, EngineConfig, EngineThermalConfig, TireConfig,
    TrackConfig, VehicleConfig,
};
use crate::profiles::{CompetitorProfile, DrivingStyle, EngineMode, builtin_profiles};
use crate::provider::InMemoryConfigProvider;

const DEFAULT_PIT_LOSS_MS: u64 = 22_000;
const SCHEMA_VERSION: u32 = 1;
include!(concat!(env!("OUT_DIR"), "/embedded_files.rs"));

#[derive(Debug, Clone, Default)]
pub struct DataRegistry {
    aeros: HashMap<String, AeroConfig>,
    chassis: HashMap<String, ChassisConfig>,
    engines: HashMap<String, EngineConfig>,
    tires: HashMap<String, TireConfig>,
    tracks: HashMap<String, TrackConfig>,
    vehicles: HashMap<String, VehicleConfig>,
    drivers: HashMap<String, DriverConfig>,
    profiles: HashMap<String, CompetitorProfile>,
}

impl DataRegistry {
    pub fn load_default() -> Result<Self, SimulatorError> {
        let mut registry = Self::default();
        for profile in builtin_profiles() {
            registry.profiles.insert(profile.id.clone(), profile);
        }
        for (path, contents) in embedded_files()? {
            registry.apply_file(&path, contents, false)?;
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
        for value in self.drivers.into_values() {
            provider.insert_driver(value);
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
        for driver in self.drivers.values() {
            driver.validate()?;
        }
        for track in self.tracks.values() {
            track.validate()?;
        }

        if !self.drivers.contains_key("default") {
            return Err(SimulatorError::InvalidConfig {
                kind: "driver",
                id: "default".to_string(),
                reason: "missing required default driver".to_string(),
            });
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
        let (category, file_name) = relative_path.split_once('/').ok_or_else(|| {
            SimulatorError::Parse(format!("invalid data pack path '{relative_path}'"))
        })?;

        if !file_name.ends_with(".json") {
            return Ok(());
        }

        let raw_str = std::str::from_utf8(raw).map_err(|err| {
            SimulatorError::Parse(format!("invalid UTF-8 in '{relative_path}': {err}"))
        })?;
        let value: Value = serde_json::from_str(raw_str).map_err(|err| {
            SimulatorError::Parse(format!("failed to parse '{relative_path}': {err}"))
        })?;

        let stem = file_name.trim_end_matches(".json");

        match category {
            "aero" => self.insert_aero(parse_aero_value(stem, &value)?, allow_override),
            "chassis" => self.insert_chassis(parse_chassis_value(stem, &value)?, allow_override),
            "engines" => self.insert_engine(parse_engine_value(stem, &value)?, allow_override),
            "tires" => self.insert_tire(parse_tire_value(stem, &value)?, allow_override),
            "circuits" => self.insert_track(parse_track_value(stem, &value)?, allow_override),
            "vehicles" => self.insert_vehicle(parse_vehicle_value(stem, &value)?, allow_override),
            "drivers" => self.insert_driver(parse_driver_value(stem, &value)?, allow_override),
            "profiles" => self.insert_profile(parse_profile_value(stem, &value)?, allow_override),
            _ => Ok(()),
        }
    }

    fn insert_aero(
        &mut self,
        value: AeroConfig,
        allow_override: bool,
    ) -> Result<(), SimulatorError> {
        insert_unique(
            &mut self.aeros,
            "aero",
            value.id.clone(),
            value,
            allow_override,
        )
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

    fn insert_tire(
        &mut self,
        value: TireConfig,
        allow_override: bool,
    ) -> Result<(), SimulatorError> {
        insert_unique(
            &mut self.tires,
            "tire",
            value.id.clone(),
            value,
            allow_override,
        )
    }

    fn insert_track(
        &mut self,
        value: TrackConfig,
        allow_override: bool,
    ) -> Result<(), SimulatorError> {
        insert_unique(
            &mut self.tracks,
            "track",
            value.id.clone(),
            value,
            allow_override,
        )
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

    fn insert_driver(
        &mut self,
        value: DriverConfig,
        allow_override: bool,
    ) -> Result<(), SimulatorError> {
        insert_unique(
            &mut self.drivers,
            "driver",
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
            "aero",
            "chassis",
            "circuits",
            "drivers",
            "engines",
            "profiles",
            "tires",
            "vehicles",
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

    pub fn tracks(&self) -> Vec<TrackConfig> {
        let mut items = self.tracks.values().cloned().collect::<Vec<_>>();
        items.sort_by(|left, right| left.id.cmp(&right.id));
        items
    }

    pub fn vehicles(&self) -> Vec<VehicleConfig> {
        let mut items = self.vehicles.values().cloned().collect::<Vec<_>>();
        items.sort_by(|left, right| left.id.cmp(&right.id));
        items
    }

    pub fn aeros(&self) -> Vec<AeroConfig> {
        let mut items = self.aeros.values().cloned().collect::<Vec<_>>();
        items.sort_by(|left, right| left.id.cmp(&right.id));
        items
    }

    pub fn chassis(&self) -> Vec<ChassisConfig> {
        let mut items = self.chassis.values().cloned().collect::<Vec<_>>();
        items.sort_by(|left, right| left.id.cmp(&right.id));
        items
    }

    pub fn engines(&self) -> Vec<EngineConfig> {
        let mut items = self.engines.values().cloned().collect::<Vec<_>>();
        items.sort_by(|left, right| left.id.cmp(&right.id));
        items
    }

    pub fn tires(&self) -> Vec<TireConfig> {
        let mut items = self.tires.values().cloned().collect::<Vec<_>>();
        items.sort_by(|left, right| left.id.cmp(&right.id));
        items
    }

    pub fn drivers(&self) -> Vec<DriverConfig> {
        let mut items = self.drivers.values().cloned().collect::<Vec<_>>();
        items.sort_by(|left, right| left.id.cmp(&right.id));
        items
    }

    pub fn profiles(&self) -> Vec<CompetitorProfile> {
        let mut items = self.profiles.values().cloned().collect::<Vec<_>>();
        items.sort_by(|left, right| left.id.cmp(&right.id));
        items
    }
}

fn embedded_files() -> Result<Vec<(String, &'static [u8])>, SimulatorError> {
    Ok(EMBEDDED_FILES
        .iter()
        .map(|(path, bytes)| ((*path).to_string(), *bytes))
        .collect())
}

fn parse_aero_value(file_stem: &str, value: &Value) -> Result<AeroConfig, SimulatorError> {
    let id = parsed_id(file_stem, value, false)?;
    let config = AeroConfig {
        id,
        cd_a_straight: read_required_f64(value, &["cd_a_straight", "cdA_x"])?,
        cd_a_corner: read_required_f64(value, &["cd_a_corner", "cdA_z"])?,
        cl_a_straight: read_required_f64(value, &["cl_a_straight", "clA_x"])?,
        cl_a_corner: read_required_f64(value, &["cl_a_corner", "clA_z"])?,
    };
    config.validate()?;
    Ok(config)
}

fn parse_chassis_value(file_stem: &str, value: &Value) -> Result<ChassisConfig, SimulatorError> {
    let id = parsed_id(file_stem, value, false)?;
    let config = ChassisConfig {
        id,
        mass_empty_kg: read_required_f64(value, &["mass_empty_kg", "mass_empty"])?,
        wheel_radius_m: read_required_f64(value, &["wheel_radius_m", "r_wheel"])?,
        mu0: read_required_f64(value, &["mu0"])?,
        rolling_resistance: read_required_f64(value, &["rolling_resistance", "c_rr"])?,
        air_density: read_required_f64(value, &["air_density", "rho"])?,
        gravity: read_optional_f64(value, &["gravity", "g"]).unwrap_or(9.81),
    };
    config.validate()?;
    Ok(config)
}

fn parse_tire_value(file_stem: &str, value: &Value) -> Result<TireConfig, SimulatorError> {
    let id = parsed_id(file_stem, value, false)?;
    let config = TireConfig {
        id,
        mu_scale: read_required_f64(value, &["mu_scale"])?,
        wear_per_s: read_required_f64(value, &["wear_per_s"])?,
        wear_load_k: read_required_f64(value, &["wear_load_k"])?,
        wear_grip_k: read_required_f64(value, &["wear_grip_k"])?,
        wear_min: read_required_f64(value, &["wear_min"])?,
        temp_opt_c: read_required_f64(value, &["temp_opt_c", "temp_opt"])?,
        temp_sigma_c: read_required_f64(value, &["temp_sigma_c", "temp_sigma"])?,
        temp_min_k: read_required_f64(value, &["temp_min_k"])?,
        heat_k: read_required_f64(value, &["heat_k"])?,
        cool_k: read_required_f64(value, &["cool_k"])?,
    };
    config.validate()?;
    Ok(config)
}

fn parse_vehicle_value(file_stem: &str, value: &Value) -> Result<VehicleConfig, SimulatorError> {
    let id = parsed_id(file_stem, value, false)?;
    let config = VehicleConfig {
        id,
        engine_id: read_required_string(value, &["engine_id", "engine"])?,
        aero_id: read_required_string(value, &["aero_id", "aero"])?,
        chassis_id: read_required_string(value, &["chassis_id", "chassis"])?,
        tire_id: read_optional_string(value, &["tire_id", "tire"])
            .unwrap_or_else(|| "medium".to_string()),
    };

    if config.engine_id.is_empty() || config.aero_id.is_empty() || config.chassis_id.is_empty() {
        return Err(SimulatorError::InvalidConfig {
            kind: "vehicle",
            id: config.id.clone(),
            reason: "engine/aero/chassis refs must be non-empty".to_string(),
        });
    }

    Ok(config)
}

fn parse_engine_value(file_stem: &str, value: &Value) -> Result<EngineConfig, SimulatorError> {
    if value.get("max_rpm").is_some() {
        parse_engine_compact(file_stem, value)
    } else {
        parse_engine_trackeagle(file_stem, value)
    }
}

fn parse_engine_compact(file_stem: &str, value: &Value) -> Result<EngineConfig, SimulatorError> {
    let id = parsed_id(file_stem, value, false)?;
    let max_rpm = read_required_f64(value, &["max_rpm"])?;
    let tq_peak = read_required_f64(value, &["tq_peak"])?;
    let g_last_total = read_required_f64(value, &["g_last_total"])?;
    let gear_count = read_required_usize(value, &["gear_count"])?.max(2);
    let idle_rpm = read_optional_f64(value, &["idle_rpm"]).unwrap_or(400.0);
    let fuel_burn_kg_per_s = read_optional_f64(value, &["fuel_burn_kg_per_s"]).unwrap_or(0.02);
    let thermal_value = value
        .get("thermal")
        .ok_or_else(|| SimulatorError::Parse(format!("missing thermal block for engine '{id}'")))?;

    build_compact_engine(
        id,
        max_rpm,
        tq_peak,
        g_last_total,
        gear_count,
        idle_rpm,
        fuel_burn_kg_per_s,
        EngineThermalConfig {
            ambient_temp_c: read_required_f64(thermal_value, &["ambient_temp_c"])?,
            initial_temp_c: read_required_f64(thermal_value, &["initial_temp_c"])?,
            capacity_j_per_c: read_required_f64(thermal_value, &["capacity_j_per_c"])?,
            heat_alpha: read_required_f64(thermal_value, &["heat_alpha"])?,
            cooling_base_w: read_required_f64(thermal_value, &["cooling_base_w"])?,
            cooling_speed_w_per_ms: read_required_f64(thermal_value, &["cooling_speed_w_per_ms"])?,
            soft_temp_c: read_required_f64(thermal_value, &["soft_temp_c"])?,
            derate_per_c: read_required_f64(thermal_value, &["derate_per_c"])?,
        },
    )
}

fn parse_engine_trackeagle(file_stem: &str, value: &Value) -> Result<EngineConfig, SimulatorError> {
    let id = parsed_id(file_stem, value, false)?;
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

    let engine = EngineConfig {
        id,
        rpm_samples,
        torque_samples,
        gear_ratios: build_gear_ratios(
            parsed.gearbox.g1_total,
            parsed.gearbox.g_last_total,
            parsed.gearbox.gear_count.max(2),
        ),
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

fn parse_track_value(file_stem: &str, value: &Value) -> Result<TrackConfig, SimulatorError> {
    if value.get("distance_m").is_some() {
        parse_compact_track(file_stem, value)
    } else {
        parse_trackeagle_track(file_stem, value)
    }
}

fn parse_compact_track(file_stem: &str, value: &Value) -> Result<TrackConfig, SimulatorError> {
    let id = parsed_id(file_stem, value, true)?;
    let distance_m = read_required_f64(value, &["distance_m"])?;
    let radius_x = read_required_f64(value, &["radius_x"])?;
    let radius_y = read_required_f64(value, &["radius_y"])?;
    let wobble_x = read_required_f64(value, &["wobble_x"])?;
    let wobble_y = read_required_f64(value, &["wobble_y"])?;
    let slope_amp_m = read_required_f64(value, &["slope_amp_m"])?;
    let pit_loss_ms = read_optional_u64(value, &["pit_loss_ms"]).unwrap_or(DEFAULT_PIT_LOSS_MS);

    let points = 420usize;
    let mut s = Vec::with_capacity(points);
    let mut x = Vec::with_capacity(points);
    let mut y = Vec::with_capacity(points);
    let mut z = Vec::with_capacity(points);

    for i in 0..points {
        let t = i as f64 / (points - 1) as f64;
        let theta = t * std::f64::consts::TAU;
        s.push(t * distance_m);
        x.push(
            radius_x * theta.cos()
                + wobble_x * (2.6 * theta).cos() * 0.55
                + wobble_x * (4.2 * theta).sin() * 0.15,
        );
        y.push(
            radius_y * theta.sin()
                + wobble_y * (1.8 * theta).sin() * 0.60
                + wobble_y * (3.3 * theta).cos() * 0.20,
        );
        z.push(slope_amp_m * (1.7 * theta).sin() * 0.5 + slope_amp_m * (0.4 * theta).cos() * 0.2);
    }

    build_track_from_arrays(id, None, s, x, y, z, None, None, None, pit_loss_ms)
}

fn parse_trackeagle_track(file_stem: &str, value: &Value) -> Result<TrackConfig, SimulatorError> {
    let id = parsed_id(file_stem, value, true)?;
    let data = value.get("data").unwrap_or(value);
    let country_code = derive_track_country_code(value);

    let s = read_vec(data, "s_m")?;
    let x = read_vec(data, "x_m")?;
    let y = read_vec(data, "y_m")?;
    let z = read_vec(data, "z_m")?;
    let curvature = if data.get("curvature_radpm").is_some() {
        Some(read_vec(data, "curvature_radpm")?)
    } else {
        None
    };
    let slope = if data.get("slope_pct").is_some() {
        Some(read_vec(data, "slope_pct")?)
    } else if data.get("slope").is_some() {
        Some(read_vec(data, "slope")?)
    } else {
        None
    };
    let heading = if data.get("heading_rad").is_some() {
        Some(read_vec(data, "heading_rad")?)
    } else {
        None
    };

    let pit_loss_ms = read_optional_u64(value, &["pit_loss_ms"]).unwrap_or(DEFAULT_PIT_LOSS_MS);
    build_track_from_arrays(
        id,
        country_code,
        s,
        x,
        y,
        z,
        curvature,
        slope,
        heading,
        pit_loss_ms,
    )
}

fn parse_profile_value(
    file_stem: &str,
    value: &Value,
) -> Result<CompetitorProfile, SimulatorError> {
    parse_explicit_profile(file_stem, value)
}

fn parse_explicit_profile(
    file_stem: &str,
    value: &Value,
) -> Result<CompetitorProfile, SimulatorError> {
    let id = parsed_id(file_stem, value, false)?;
    let display_name = read_optional_string(value, &["display_name"]).unwrap_or_else(|| id.clone());
    let style = parse_driving_style(
        read_optional_string(value, &["style"])
            .unwrap_or_else(|| "balanced".to_string())
            .as_str(),
    )?;
    let engine_mode = parse_engine_mode(
        read_optional_string(value, &["engine_mode"])
            .unwrap_or_else(|| "balanced".to_string())
            .as_str(),
    )?;

    Ok(CompetitorProfile {
        id,
        display_name,
        style,
        engine_mode,
        tire_id: read_optional_string(value, &["tire_id"]).unwrap_or_else(|| "medium".to_string()),
        downforce_bias: read_optional_f64(value, &["downforce_bias"]).unwrap_or(0.0),
        gear_ratio_bias: read_optional_f64(value, &["gear_ratio_bias"]).unwrap_or(0.0),
        pace_variance_ms: read_optional_f64(value, &["pace_variance_ms"]).unwrap_or(30.0),
    })
}

fn parse_driver_value(file_stem: &str, value: &Value) -> Result<DriverConfig, SimulatorError> {
    #[derive(Debug, Deserialize)]
    struct DriverJson {
        #[serde(default)]
        id: Option<String>,
        #[serde(default)]
        display_name: Option<String>,
        #[serde(default = "default_aggressiveness")]
        aggressiveness: f64,
    }

    let mut parsed: DriverJson = serde_json::from_value(value.clone()).map_err(|err| {
        SimulatorError::Parse(format!("driver '{}' parse failed: {err}", file_stem))
    })?;

    let id = schema_version(value)
        .map(|_| explicit_id(value, file_stem, false))
        .transpose()?
        .or_else(|| parsed.id.take())
        .unwrap_or_else(|| file_stem.to_string());
    let driver = DriverConfig {
        id: id.clone(),
        display_name: parsed.display_name.unwrap_or(id),
        aggressiveness: parsed.aggressiveness.clamp(0.0, 1.0),
    };
    driver.validate()?;
    Ok(driver)
}

fn build_compact_engine(
    id: String,
    max_rpm: f64,
    tq_peak: f64,
    g_last_total: f64,
    gear_count: usize,
    idle_rpm: f64,
    fuel_burn_kg_per_s: f64,
    thermal: EngineThermalConfig,
) -> Result<EngineConfig, SimulatorError> {
    let step = 250.0;
    let mut rpm_samples = Vec::new();
    let mut torque_samples = Vec::new();
    let mut rpm = 0.0;
    while rpm <= max_rpm + step * 0.5 {
        rpm_samples.push(rpm);
        let normalized = if max_rpm > 0.0 { rpm / max_rpm } else { 0.0 };
        let tq = if normalized < 0.7 {
            tq_peak * (0.70 + 0.40 * normalized)
        } else {
            tq_peak * (1.0 - 0.65 * (normalized - 0.7))
        }
        .max(0.12);
        torque_samples.push(tq);
        rpm += step;
    }

    let g1_total = 14.0;
    let mut gear_ratios = Vec::with_capacity(gear_count);
    for idx in 0..gear_count {
        let a = idx as f64 / (gear_count - 1) as f64;
        gear_ratios.push(g1_total * (g_last_total / g1_total).powf(a));
    }

    let engine = EngineConfig {
        id,
        rpm_samples,
        torque_samples,
        gear_ratios,
        idle_rpm,
        max_rpm,
        thermal,
        fuel_burn_kg_per_s,
    };
    engine.validate()?;
    Ok(engine)
}

fn build_track_from_arrays(
    id: String,
    country_code: Option<String>,
    s: Vec<f64>,
    x: Vec<f64>,
    y: Vec<f64>,
    z: Vec<f64>,
    curvature: Option<Vec<f64>>,
    slope: Option<Vec<f64>>,
    heading: Option<Vec<f64>>,
    pit_loss_ms: u64,
) -> Result<TrackConfig, SimulatorError> {
    let heading = heading.unwrap_or_else(|| derive_heading(&x, &y));
    let curvature = curvature.unwrap_or_else(|| vec![0.0; s.len()]);
    let slope = slope.unwrap_or_else(|| derive_gradient(&s, &z));

    let track = TrackConfig {
        id,
        country_code,
        s_m: s,
        x_m: x,
        y_m: y,
        z_m: z,
        curvature_radpm: curvature,
        slope,
        heading_rad: heading,
        pit_loss_ms,
    };

    track.validate()?;
    Ok(track)
}

fn derive_track_country_code(value: &Value) -> Option<String> {
    let meta = value.get("meta")?;
    if let Some(explicit) = read_optional_string(meta, &["country_code"]) {
        let normalized = explicit.trim().to_uppercase();
        if normalized.len() == 2 && normalized.chars().all(|ch| ch.is_ascii_alphabetic()) {
            return Some(normalized);
        }
    }

    let raw_id = read_optional_string(meta, &["id"])?;
    let prefix = raw_id.split('-').next()?.trim().to_uppercase();
    if prefix.len() == 2 && prefix.chars().all(|ch| ch.is_ascii_alphabetic()) {
        Some(prefix)
    } else {
        None
    }
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

fn parsed_id(
    file_stem: &str,
    value: &Value,
    normalize_track: bool,
) -> Result<String, SimulatorError> {
    match schema_version(value) {
        Some(version) => {
            ensure_schema(version, file_stem)?;
            explicit_id(value, file_stem, normalize_track)
        }
        None => Ok(derived_id(file_stem, normalize_track)),
    }
}

fn explicit_id(
    value: &Value,
    file_stem: &str,
    normalize_track: bool,
) -> Result<String, SimulatorError> {
    let id = read_required_string(value, &["id"])?;
    if id.trim().is_empty() {
        return Err(SimulatorError::InvalidConfig {
            kind: "data",
            id: file_stem.to_string(),
            reason: "id must be non-empty".to_string(),
        });
    }
    Ok(if normalize_track {
        normalize_track_id(&id)
    } else {
        id
    })
}

fn derived_id(file_stem: &str, normalize_track: bool) -> String {
    if normalize_track {
        derive_track_id(file_stem)
    } else {
        file_stem.to_string()
    }
}

fn derive_track_id(file_stem: &str) -> String {
    let slug = file_stem.split('-').next().unwrap_or(file_stem);
    normalize_track_id(slug)
}

fn schema_version(value: &Value) -> Option<u32> {
    value
        .get("schema_version")
        .and_then(Value::as_u64)
        .map(|value| value as u32)
}

fn ensure_schema(version: u32, file_stem: &str) -> Result<(), SimulatorError> {
    if version != SCHEMA_VERSION {
        return Err(SimulatorError::InvalidConfig {
            kind: "data",
            id: file_stem.to_string(),
            reason: format!("unsupported schema_version {version}, expected {SCHEMA_VERSION}"),
        });
    }
    Ok(())
}

fn read_required_f64(value: &Value, keys: &[&str]) -> Result<f64, SimulatorError> {
    read_optional_f64(value, keys).ok_or_else(|| {
        SimulatorError::Parse(format!("missing numeric field '{}'", keys.join("' or '")))
    })
}

fn read_optional_f64(value: &Value, keys: &[&str]) -> Option<f64> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_f64))
}

fn read_required_u64(value: &Value, keys: &[&str]) -> Result<u64, SimulatorError> {
    read_optional_u64(value, keys).ok_or_else(|| {
        SimulatorError::Parse(format!("missing integer field '{}'", keys.join("' or '")))
    })
}

fn read_optional_u64(value: &Value, keys: &[&str]) -> Option<u64> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_u64))
}

fn read_required_usize(value: &Value, keys: &[&str]) -> Result<usize, SimulatorError> {
    let raw = read_required_u64(value, keys)?;
    usize::try_from(raw).map_err(|_| {
        SimulatorError::Parse(format!(
            "integer field '{}' exceeds usize",
            keys.join("' or '")
        ))
    })
}

fn read_required_string(value: &Value, keys: &[&str]) -> Result<String, SimulatorError> {
    read_optional_string(value, keys).ok_or_else(|| {
        SimulatorError::Parse(format!("missing string field '{}'", keys.join("' or '")))
    })
}

fn read_optional_string(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str).map(str::to_string))
}

fn read_vec(value: &Value, key: &str) -> Result<Vec<f64>, SimulatorError> {
    let arr = value
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| SimulatorError::Parse(format!("missing or invalid array key '{key}'")))?;
    let mut out = Vec::with_capacity(arr.len());
    for item in arr {
        let Some(num) = item.as_f64() else {
            return Err(SimulatorError::Parse(format!(
                "non-numeric value in array '{key}'"
            )));
        };
        out.push(num);
    }
    Ok(out)
}

fn normalize_track_id(track_id: &str) -> String {
    track_id
        .chars()
        .filter(|ch| !matches!(ch, '-' | '_' | ' '))
        .flat_map(char::to_uppercase)
        .collect()
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

fn default_fuel_burn() -> f64 {
    0.02
}

fn default_aggressiveness() -> f64 {
    0.5
}

fn build_range_series(start: f64, end: f64, step: f64) -> Vec<f64> {
    if step <= 0.0 || end < start {
        return vec![start];
    }
    let mut out = Vec::new();
    let mut value = start;
    while value <= end + step * 0.5 {
        out.push(value);
        value += step;
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

fn parse_driving_style(raw: &str) -> Result<DrivingStyle, SimulatorError> {
    match raw {
        "conservative" => Ok(DrivingStyle::Conservative),
        "balanced" => Ok(DrivingStyle::Balanced),
        "aggressive" => Ok(DrivingStyle::Aggressive),
        _ => Err(SimulatorError::Parse(format!(
            "unknown driving style '{raw}'"
        ))),
    }
}

fn parse_engine_mode(raw: &str) -> Result<EngineMode, SimulatorError> {
    match raw {
        "economy" => Ok(EngineMode::Economy),
        "balanced" => Ok(EngineMode::Balanced),
        "push" => Ok(EngineMode::Push),
        _ => Err(SimulatorError::Parse(format!(
            "unknown engine mode '{raw}'"
        ))),
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};

    fn existing_reference_root() -> PathBuf {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let candidates = [
            manifest_dir.join("data"),
            manifest_dir.join("../../../tooling/pitgun_simulator/data"),
        ];

        candidates
            .into_iter()
            .find(|path| path.is_dir())
            .unwrap_or_else(|| panic!("no reference data directory found from {}", manifest_dir.display()))
    }

    fn collect_reference_pack(root: &Path) -> HashMap<String, Vec<u8>> {
        let mut files = HashMap::new();
        for category in [
            "aero", "chassis", "circuits", "drivers", "engines", "tires", "vehicles",
        ] {
            let dir = root.join(category);
            let entries = std::fs::read_dir(&dir).expect("read reference category");
            for entry in entries {
                let path = entry.expect("dir entry").path();
                if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                    continue;
                }
                let name = path
                    .file_name()
                    .and_then(|value| value.to_str())
                    .expect("utf-8 filename");
                files.insert(
                    format!("{category}/{name}"),
                    std::fs::read(&path).expect("read reference file"),
                );
            }
        }
        files
    }

    fn python_reference_pack() -> HashMap<String, Vec<u8>> {
        collect_reference_pack(&existing_reference_root())
    }

    fn embedded_reference_pack() -> HashMap<String, Vec<u8>> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data");
        collect_reference_pack(&root)
    }

    #[test]
    fn reference_pack_helper_resolves_existing_data_root() {
        assert!(existing_reference_root().is_dir());
    }

    #[test]
    fn reference_pack_contains_expected_categories_and_loads() {
        let reference = python_reference_pack();
        assert!(!reference.is_empty());
        for category in [
            "aero", "chassis", "circuits", "drivers", "engines", "tires", "vehicles",
        ] {
            assert!(
                reference.keys().any(|key| key.starts_with(&format!("{category}/"))),
                "missing reference category {category}"
            );
        }

        let registry =
            DataRegistry::load_from_bytes_map(reference).expect("reference pack should load");
        let provider = registry.into_provider();
        let vehicle = crate::provider::ConfigProvider::get_vehicle(&provider, "f1_2026")
            .expect("f1_2026 vehicle");
        let track =
            crate::provider::ConfigProvider::get_track(&provider, "SPA").expect("SPA track");

        assert_eq!(vehicle.engine_id, "v6t_hybrid");
        assert!(track.s_m.len() > 1000);
    }

    #[test]
    fn loads_embedded_pack() {
        let registry = DataRegistry::load_default().expect("embedded pack should load");
        let provider = registry.into_provider();
        let vehicle = crate::provider::ConfigProvider::get_vehicle(&provider, "f1_2026")
            .expect("f1_2026 vehicle");
        let track =
            crate::provider::ConfigProvider::get_track(&provider, "SPA").expect("SPA track");

        assert_eq!(vehicle.engine_id, "v6t_hybrid");
        assert_eq!(track.pit_loss_ms, DEFAULT_PIT_LOSS_MS);
    }

    #[test]
    fn loads_trackeagle_tracks_from_embedded_pack() {
        let registry = DataRegistry::load_default().expect("embedded pack should load");
        let provider = registry.into_provider();
        let track = crate::provider::ConfigProvider::get_track(&provider, "ZANDVOORT")
            .expect("zandvoort track");

        assert!(track.s_m.len() > 1000);
        assert_eq!(track.pit_loss_ms, DEFAULT_PIT_LOSS_MS);
        assert_eq!(track.country_code.as_deref(), Some("NL"));
    }

    #[test]
    fn loads_driver_catalog_from_embedded_pack() {
        let registry = DataRegistry::load_default().expect("embedded pack should load");
        let provider = registry.into_provider();
        let driver = crate::provider::ConfigProvider::get_driver(&provider, "smooth_operator")
            .expect("smooth operator driver");

        assert!(driver.aggressiveness > 0.0);
        assert!(!driver.display_name.trim().is_empty());
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
  "cdA_x": 0.7,
  "cdA_z": 0.8,
  "clA_x": 2.1,
  "clA_z": 3.0
}"#,
        )
        .expect("override file");

        let provider = DataRegistry::load_from_dir(temp.path())
            .expect("load from dir")
            .into_provider();
        let aero =
            crate::provider::ConfigProvider::get_aero(&provider, "basic").expect("basic aero");

        assert_eq!(aero.cd_a_straight, 0.7);
        assert_eq!(aero.cl_a_corner, 3.0);
    }

    #[test]
    fn bytes_map_rejects_unknown_references() {
        let mut files = HashMap::new();
        files.insert(
            "vehicles/bad.json".to_string(),
            br#"{
  "engine": "missing",
  "aero": "basic",
  "chassis": "f1_2026",
  "tire": "medium"
}"#
            .to_vec(),
        );

        let err = DataRegistry::load_from_bytes_map(files).expect_err("invalid refs must fail");
        assert!(err.to_string().contains("unknown engine reference"));
    }

    #[test]
    fn loads_python_reference_pack_without_exceptions() {
        let registry =
            DataRegistry::load_from_bytes_map(python_reference_pack()).expect("reference pack");
        let provider = registry.into_provider();

        let aero = crate::provider::ConfigProvider::get_aero(&provider, "modern")
            .expect("modern aero");
        let engine = crate::provider::ConfigProvider::get_engine(&provider, "v10_1990")
            .expect("v10_1990 engine");
        let vehicle = crate::provider::ConfigProvider::get_vehicle(&provider, "classic_v10_1990")
            .expect("classic_v10_1990 vehicle");
        let track =
            crate::provider::ConfigProvider::get_track(&provider, "SPA").expect("SPA track");

        assert!(aero.cl_a_corner > 0.0);
        assert!(engine.max_rpm > 0.0);
        assert_eq!(vehicle.engine_id, "v10_1990");
        assert!(track.s_m.len() > 1000);
    }

    #[test]
    fn builtin_profiles_are_available_without_profile_files() {
        let registry = DataRegistry::load_default().expect("embedded pack should load");
        let profiles = registry.profiles();
        let ids = profiles.into_iter().map(|profile| profile.id).collect::<Vec<_>>();

        assert!(ids.iter().any(|id| id == "balanced"));
        assert!(ids.iter().any(|id| id == "aggressive"));
        assert!(ids.iter().any(|id| id == "conservative"));
    }
}
