//! Native opt-in fuel-domain outcome. Legacy APIs still reject shortfalls.
//! A failed lap/service is not a committed driving prefix or a physical stop location.
use crate::{
    ResolvedSimulationRequestV3, ResolvedSimulationSessionV3, ResolvedSimulationStepV3,
    SimulationResult, VehicleState,
};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FuelShortfallPhaseV1 {
    DrivingLap,
    Service,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FuelShortfallV1 {
    pub attempted_lap: u16,
    pub phase: FuelShortfallPhaseV1,
    pub required_kg: f64,
    pub available_kg: f64,
}
impl fmt::Display for FuelShortfallV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.phase {
            FuelShortfallPhaseV1::DrivingLap => write!(
                f,
                "V3 fuel depleted on lap {}: required {:.6} kg, available {:.6} kg",
                self.attempted_lap, self.required_kg, self.available_kg
            ),
            FuelShortfallPhaseV1::Service => write!(
                f,
                "energy V1: fuel depleted during service after lap {}",
                self.attempted_lap
            ),
        }
    }
}

#[derive(Debug)]
pub(crate) enum NativeFailureV3 {
    Technical(String),
    Fuel(FuelShortfallV1),
}
impl From<String> for NativeFailureV3 {
    fn from(s: String) -> Self {
        Self::Technical(s)
    }
}
impl From<&str> for NativeFailureV3 {
    fn from(s: &str) -> Self {
        Self::Technical(s.into())
    }
}
impl fmt::Display for NativeFailureV3 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Technical(s) => s.fmt(f),
            Self::Fuel(s) => s.fmt(f),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FuelProgressV1 {
    pub completed_laps: u16,
    pub elapsed_s: f64,
    pub distance_m: f64,
    pub state: VehicleState,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FuelAwareStepV1 {
    Lap {
        progress: FuelProgressV1,
    },
    Finished {
        result: Box<SimulationResult>,
    },
    FuelNotCompleted {
        progress: FuelProgressV1,
        shortfall: FuelShortfallV1,
    },
}

/// Conservative lap-budget outcome, not an exact within-lap depletion trajectory.
/// Only previously yielded full laps/services count. Never invent zero fuel,
/// uncomputed telemetry, a completed GP, or a new service/refuelling action.
pub struct FuelAwareSessionV1 {
    inner: ResolvedSimulationSessionV3,
    progress: FuelProgressV1,
}
impl FuelAwareSessionV1 {
    pub fn new(input: ResolvedSimulationRequestV3) -> Result<Self, String> {
        if input.thermal_physics_v2.is_none() || input.fuel_mass.is_none() {
            return Err("fuel outcome V1 requires the explicit energy/thermal candidate".into());
        }
        let progress = FuelProgressV1 {
            completed_laps: 0,
            elapsed_s: 0.0,
            distance_m: 0.0,
            state: input.state.clone(),
        };
        Ok(Self {
            inner: ResolvedSimulationSessionV3::new(input)?,
            progress,
        })
    }
    pub fn advance(&mut self) -> Result<FuelAwareStepV1, String> {
        match self.inner.advance_typed() {
            Ok(ResolvedSimulationStepV3::Lap(lap)) => {
                self.progress = FuelProgressV1 {
                    completed_laps: lap.lap_index,
                    elapsed_s: lap.cumulative_time_s,
                    distance_m: lap.cumulative_distance_m,
                    state: lap.state_after_lap,
                };
                Ok(FuelAwareStepV1::Lap {
                    progress: self.progress.clone(),
                })
            }
            Ok(ResolvedSimulationStepV3::Complete(result)) => {
                Ok(FuelAwareStepV1::Finished { result })
            }
            Err(NativeFailureV3::Fuel(shortfall)) => Ok(FuelAwareStepV1::FuelNotCompleted {
                progress: self.progress.clone(),
                shortfall,
            }),
            Err(NativeFailureV3::Technical(s)) => Err(s),
        }
    }
}
