//! Versioned data contracts owned by the Racing domain.
//!
//! The crate contains wire-facing Racing schemas only. Physical equations,
//! simulation orchestration, policy evaluation and generic runtime evidence
//! belong to their respective crates.

mod authority;
mod catalog;
mod completed_run;
mod components;
mod driver;
mod instruction_authorization;
mod model_parameters;
mod race;

pub use authority::{SignedSimulationContractV1, SimulationContractV1};
pub use catalog::{
    RacingCatalogError, RacingCircuitPresentationV1, RacingDriverPresentationV1,
    RacingPresentationIndexV1, RacingPresentationIndexVersion, RacingSimulationIndexV1,
    RacingSimulationIndexVersion,
};
pub use completed_run::{
    RacingCompletedDriverInstructionHistoryV1, RacingCompletedRunError, RacingCompletedRunInputV1,
    RacingCompletedRunInputVersion,
};
pub use components::{
    ComponentCapabilityDefinitionV1, ComponentCapabilityError, ComponentCapabilityProfileV1,
    ComponentCapabilityProfileVersion, InstalledVehicleComponentV1, ResolvedVehicleCapabilitiesV1,
    ResolvedVehicleCapabilitiesVersion, UnavailableVehicleCapabilityV1, VehicleCapability,
    VehicleComponentKind,
};
pub use driver::{
    RacingDriverContractError, RacingDriverControlProfileV1, RacingDriverControlProfileVersion,
    RacingDriverInstructionBoundaryGranularityV1, RacingDriverInstructionBoundaryV1,
    RacingDriverInstructionEventV1, RacingDriverInstructionProfileV1,
    RacingDriverInstructionProfileVersion, RacingDriverInstructionTimelineV1,
    RacingDriverInstructionTimelineVersion, RacingDriverResourceV2, RacingDriverResourceVersion,
    RacingDriverTraitsV1, RacingDriverUtilizationResponseV1, RacingDrivingMode,
    RacingDrivingModeCommitmentsV1,
};
pub use instruction_authorization::{
    RACING_DRIVER_INSTRUCTION_AUTHORIZATION_ID, RACING_DRIVER_INSTRUCTION_AUTHORIZATION_VERSION,
    RacingDriverInstructionAuthorizationV1, RacingDriverInstructionAuthorizationVersion,
    RacingInstructionAuthorizationError,
};
pub use model_parameters::{
    RacingAerodynamicStateResponseV1, RacingDevelopmentResolutionV1, RacingModelCompatibilityV1,
    RacingModelParametersError, RacingModelParametersIdentityV1, RacingModelParametersPurpose,
    RacingModelParametersV1, RacingModelParametersVersion, RacingSetupResponseV1,
};
pub use race::{
    CircuitCatalogEntry, CompetitorSpec, CompetitorStatus, CompetitorStintStrategy,
    EngineCatalogEntry, RaceInput, RaceOutput, RaceStint, RunPackage, StandingEntry, TuningSpec,
    VehicleClass, VehicleComponentSelectionV1, VehicleComponentSelectionVersion,
    resolve_vehicle_class,
};
