//! Read-only counters and an explicit native diagnostic boundary. No admitted
//! workload selects the end-state-held ablation. Attempt work is not a race event.
use crate::ResolvedSimulationRequestV3;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BoundaryDiagnosticVersionV1 {
    #[serde(rename = "pitgun.lap-boundary-diagnostic/v1")]
    V1,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoundaryForecastV1 {
    MinimumGrip,
    /// Diagnostic ablation only; not a bound on evolving future grip.
    EndStateHeldDiagnostic,
    /// Native candidate: reachable wear, temperature floor, no deformation brake assist.
    ReachableWearV1,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoundaryDiagnosticRequestV1 {
    pub schema_version: BoundaryDiagnosticVersionV1,
    pub forecast: BoundaryForecastV1,
    /// Reproduce the former redundant work for paired equal-output cost checks.
    pub recompute_invariant: bool,
    pub input: ResolvedSimulationRequestV3,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoundaryLapWorkV1 {
    pub lap: u16,
    pub forecast_evaluations: u32,
    pub cache_hits: u32,
    pub backward_sweeps: u32,
    pub last_forecast_cap_mps: f64,
    pub last_assumed_temperature_c: f64,
    pub last_assumed_wear: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reachable: Option<ReachableBoundaryWorkV1>,
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ReachableBoundaryWorkV1 {
    pub anchor_cap_mps: f64,
    pub certificate_cap_mps: f64,
    pub committed_end_speed_mps: f64,
    pub certificate_checks: u32,
    pub uncommitted_cap_updates: u32,
    pub conservative_recompute: bool,
    pub service_reset: bool,
    pub future_start_wear: f64,
    pub future_end_wear_upper: f64,
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BoundaryWorkV1 {
    /// Attempt counters, including a rejected numerical pass. Not committed facts.
    pub laps: Vec<BoundaryLapWorkV1>,
}
