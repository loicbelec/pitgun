//! Versioned data contracts owned by the Racing domain.
//!
//! The crate contains wire-facing Racing schemas only. Physical equations,
//! simulation orchestration, policy evaluation and generic runtime evidence
//! belong to their respective crates.

mod authority;
mod catalog;
mod race;

pub use authority::{SignedSimulationContractV1, SimulationContractV1};
pub use catalog::{
    RacingCatalogError, RacingCircuitPresentationV1, RacingDriverPresentationV1,
    RacingPresentationIndexV1, RacingPresentationIndexVersion, RacingSimulationIndexV1,
    RacingSimulationIndexVersion,
};
pub use race::{
    CircuitCatalogEntry, CompetitorSpec, CompetitorStatus, CompetitorStintStrategy,
    EngineCatalogEntry, RaceInput, RaceOutput, RaceStint, RunPackage, StandingEntry, TuningSpec,
    VehicleClass, resolve_vehicle_class,
};
