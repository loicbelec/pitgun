use std::collections::HashMap;

#[cfg(all(feature = "json-files", not(target_arch = "wasm32")))]
use crate::data::DataRegistry;
use crate::errors::SimulatorError;
use crate::models::{
    AeroConfig, ChassisConfig, DriverConfig, EngineConfig, TireConfig, TrackConfig, VehicleConfig,
};
use crate::profiles::CompetitorProfile;

pub trait ConfigProvider: Send + Sync {
    fn get_vehicle(&self, id: &str) -> Result<VehicleConfig, SimulatorError>;
    fn get_aero(&self, id: &str) -> Result<AeroConfig, SimulatorError>;
    fn get_chassis(&self, id: &str) -> Result<ChassisConfig, SimulatorError>;
    fn get_engine(&self, id: &str) -> Result<EngineConfig, SimulatorError>;
    fn get_tire(&self, id: &str) -> Result<TireConfig, SimulatorError>;
    fn get_track(&self, id: &str) -> Result<TrackConfig, SimulatorError>;
    fn get_driver(&self, id: &str) -> Result<DriverConfig, SimulatorError>;
    fn get_profile(&self, id: &str) -> Result<CompetitorProfile, SimulatorError>;
    fn list_vehicles(&self) -> Result<Vec<VehicleConfig>, SimulatorError>;
    fn list_aeros(&self) -> Result<Vec<AeroConfig>, SimulatorError>;
    fn list_chassis(&self) -> Result<Vec<ChassisConfig>, SimulatorError>;
    fn list_tracks(&self) -> Result<Vec<TrackConfig>, SimulatorError>;
    fn list_engines(&self) -> Result<Vec<EngineConfig>, SimulatorError>;
    fn list_tires(&self) -> Result<Vec<TireConfig>, SimulatorError>;
    fn list_drivers(&self) -> Result<Vec<DriverConfig>, SimulatorError>;
    fn list_profiles(&self) -> Result<Vec<CompetitorProfile>, SimulatorError>;
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryConfigProvider {
    vehicles: HashMap<String, VehicleConfig>,
    aeros: HashMap<String, AeroConfig>,
    chassis: HashMap<String, ChassisConfig>,
    engines: HashMap<String, EngineConfig>,
    tires: HashMap<String, TireConfig>,
    tracks: HashMap<String, TrackConfig>,
    drivers: HashMap<String, DriverConfig>,
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

    pub fn insert_driver(&mut self, value: DriverConfig) {
        self.drivers.insert(value.id.clone(), value);
    }

    pub fn insert_profile(&mut self, value: CompetitorProfile) {
        self.profiles.insert(value.id.clone(), value);
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

    fn get_driver(&self, id: &str) -> Result<DriverConfig, SimulatorError> {
        Self::get_from(&self.drivers, "driver", id)
    }

    fn list_vehicles(&self) -> Result<Vec<VehicleConfig>, SimulatorError> {
        let mut items = self.vehicles.values().cloned().collect::<Vec<_>>();
        items.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(items)
    }

    fn list_aeros(&self) -> Result<Vec<AeroConfig>, SimulatorError> {
        let mut items = self.aeros.values().cloned().collect::<Vec<_>>();
        items.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(items)
    }

    fn list_chassis(&self) -> Result<Vec<ChassisConfig>, SimulatorError> {
        let mut items = self.chassis.values().cloned().collect::<Vec<_>>();
        items.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(items)
    }

    fn list_tracks(&self) -> Result<Vec<TrackConfig>, SimulatorError> {
        let mut items = self.tracks.values().cloned().collect::<Vec<_>>();
        items.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(items)
    }

    fn list_engines(&self) -> Result<Vec<EngineConfig>, SimulatorError> {
        let mut items = self.engines.values().cloned().collect::<Vec<_>>();
        items.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(items)
    }

    fn list_tires(&self) -> Result<Vec<TireConfig>, SimulatorError> {
        let mut items = self.tires.values().cloned().collect::<Vec<_>>();
        items.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(items)
    }

    fn list_drivers(&self) -> Result<Vec<DriverConfig>, SimulatorError> {
        let mut items = self.drivers.values().cloned().collect::<Vec<_>>();
        items.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(items)
    }

    fn list_profiles(&self) -> Result<Vec<CompetitorProfile>, SimulatorError> {
        let mut items = self.profiles.values().cloned().collect::<Vec<_>>();
        items.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(items)
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

    fn load_provider(&self) -> Result<InMemoryConfigProvider, SimulatorError> {
        DataRegistry::load_from_dir(&self.root).map(DataRegistry::into_provider)
    }
}

#[cfg(all(feature = "json-files", not(target_arch = "wasm32")))]
impl ConfigProvider for JsonFileConfigProvider {
    fn get_vehicle(&self, id: &str) -> Result<VehicleConfig, SimulatorError> {
        self.load_provider()?.get_vehicle(id)
    }

    fn get_aero(&self, id: &str) -> Result<AeroConfig, SimulatorError> {
        self.load_provider()?.get_aero(id)
    }

    fn get_chassis(&self, id: &str) -> Result<ChassisConfig, SimulatorError> {
        self.load_provider()?.get_chassis(id)
    }

    fn get_engine(&self, id: &str) -> Result<EngineConfig, SimulatorError> {
        self.load_provider()?.get_engine(id)
    }

    fn get_tire(&self, id: &str) -> Result<TireConfig, SimulatorError> {
        self.load_provider()?.get_tire(id)
    }

    fn get_track(&self, id: &str) -> Result<TrackConfig, SimulatorError> {
        self.load_provider()?.get_track(id)
    }

    fn get_driver(&self, id: &str) -> Result<DriverConfig, SimulatorError> {
        self.load_provider()?.get_driver(id)
    }

    fn get_profile(&self, id: &str) -> Result<CompetitorProfile, SimulatorError> {
        self.load_provider()?.get_profile(id)
    }

    fn list_vehicles(&self) -> Result<Vec<VehicleConfig>, SimulatorError> {
        self.load_provider()?.list_vehicles()
    }

    fn list_aeros(&self) -> Result<Vec<AeroConfig>, SimulatorError> {
        self.load_provider()?.list_aeros()
    }

    fn list_chassis(&self) -> Result<Vec<ChassisConfig>, SimulatorError> {
        self.load_provider()?.list_chassis()
    }

    fn list_tracks(&self) -> Result<Vec<TrackConfig>, SimulatorError> {
        self.load_provider()?.list_tracks()
    }

    fn list_engines(&self) -> Result<Vec<EngineConfig>, SimulatorError> {
        self.load_provider()?.list_engines()
    }

    fn list_tires(&self) -> Result<Vec<TireConfig>, SimulatorError> {
        self.load_provider()?.list_tires()
    }

    fn list_drivers(&self) -> Result<Vec<DriverConfig>, SimulatorError> {
        self.load_provider()?.list_drivers()
    }

    fn list_profiles(&self) -> Result<Vec<CompetitorProfile>, SimulatorError> {
        self.load_provider()?.list_profiles()
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

    fn get_driver(&self, id: &str) -> Result<DriverConfig, SimulatorError> {
        Err(SimulatorError::InvalidInput(format!(
            "JsonFileConfigProvider unavailable on this target/feature set (driver {id})"
        )))
    }

    fn get_profile(&self, id: &str) -> Result<CompetitorProfile, SimulatorError> {
        Err(SimulatorError::InvalidInput(format!(
            "JsonFileConfigProvider unavailable on this target/feature set (profile {id})"
        )))
    }

    fn list_vehicles(&self) -> Result<Vec<VehicleConfig>, SimulatorError> {
        Err(SimulatorError::InvalidInput(
            "JsonFileConfigProvider unavailable on this target/feature set (list vehicles)"
                .to_string(),
        ))
    }

    fn list_aeros(&self) -> Result<Vec<AeroConfig>, SimulatorError> {
        Err(SimulatorError::InvalidInput(
            "JsonFileConfigProvider unavailable on this target/feature set (list aeros)"
                .to_string(),
        ))
    }

    fn list_chassis(&self) -> Result<Vec<ChassisConfig>, SimulatorError> {
        Err(SimulatorError::InvalidInput(
            "JsonFileConfigProvider unavailable on this target/feature set (list chassis)"
                .to_string(),
        ))
    }

    fn list_tracks(&self) -> Result<Vec<TrackConfig>, SimulatorError> {
        Err(SimulatorError::InvalidInput(
            "JsonFileConfigProvider unavailable on this target/feature set (list tracks)"
                .to_string(),
        ))
    }

    fn list_engines(&self) -> Result<Vec<EngineConfig>, SimulatorError> {
        Err(SimulatorError::InvalidInput(
            "JsonFileConfigProvider unavailable on this target/feature set (list engines)"
                .to_string(),
        ))
    }

    fn list_tires(&self) -> Result<Vec<TireConfig>, SimulatorError> {
        Err(SimulatorError::InvalidInput(
            "JsonFileConfigProvider unavailable on this target/feature set (list tires)"
                .to_string(),
        ))
    }

    fn list_drivers(&self) -> Result<Vec<DriverConfig>, SimulatorError> {
        Err(SimulatorError::InvalidInput(
            "JsonFileConfigProvider unavailable on this target/feature set (list drivers)"
                .to_string(),
        ))
    }

    fn list_profiles(&self) -> Result<Vec<CompetitorProfile>, SimulatorError> {
        Err(SimulatorError::InvalidInput(
            "JsonFileConfigProvider unavailable on this target/feature set (list profiles)"
                .to_string(),
        ))
    }
}
