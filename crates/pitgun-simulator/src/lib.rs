mod catalog;
mod data;
mod drivers;
mod errors;
mod models;
mod profiles;
mod provider;
mod simulator;
mod state;
mod telemetry;
mod tuning;

pub use errors::SimulatorError;
pub use data::DataRegistry;
pub use drivers::{
    DriverEffects, apply_driver_to_tire, default_driver_id, deterministic_lap_delta_ms,
    driver_effects,
};
pub use models::{
    AeroConfig, ChassisConfig, DriverConfig, EngineConfig, EngineThermalConfig, TireConfig,
    TrackConfig, VehicleConfig,
};
pub use profiles::{CompetitorProfile, DrivingStyle, EngineMode, SessionKind};
pub use provider::{ConfigProvider, InMemoryConfigProvider, JsonFileConfigProvider};
pub use simulator::{LapInput, LapOutput, Simulator};
pub use state::SimulatorState;
pub use telemetry::TelemetryFrame;
pub use tuning::Tuning;

pub fn default_in_memory_provider() -> InMemoryConfigProvider {
    catalog::default_in_memory_provider()
}
