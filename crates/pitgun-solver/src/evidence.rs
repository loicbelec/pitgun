//! Canonical evidence emitted by the Racing reference workload.

use std::fmt;

use pitgun_contract::{
    CanonicalJsonError, DeterministicRunContractV1, Digest, ExecutionId, ExecutionReceiptV1,
    RuntimeIdentity, TelemetrySummaryError, TelemetrySummaryV1, canonical_json_bytes,
    canonical_json_digest,
};
use serde::{Deserialize, Serialize};

use crate::{RaceOutput, StandingStatus};

/// Wire version of the canonical Racing domain output.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum RacingOutputVersion {
    /// Racing result fields covered by the first portable-exact contract.
    #[serde(rename = "pitgun.racing-output/v1")]
    V1,
}

/// Canonical status of one Racing competitor.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RacingStandingStatusV1 {
    /// The competitor completed the run.
    Finished,
    /// The competitor did not finish for the supplied stable reason.
    Dnf { reason: String },
    /// The competitor was disqualified for the supplied stable reason.
    Dsq { reason: String },
}

/// Canonical standing included in the versioned Racing domain output.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RacingStandingV1 {
    /// Stable competitor identifier supplied by the run input.
    pub competitor_id: String,
    /// One-based finishing position.
    pub position: u32,
    /// Total elapsed race time in milliseconds.
    pub total_time_ms: u64,
    /// Best completed lap time in milliseconds.
    pub best_lap_ms: u64,
    /// Number of completed laps.
    pub laps_completed: u16,
    /// Gap to the leader in milliseconds.
    pub gap_to_leader_ms: u64,
    /// Final status of the competitor.
    pub status: RacingStandingStatusV1,
}

/// Complete canonical Racing result, excluding the separately summarized stream.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RacingOutputV1 {
    /// Version controlling the canonical wire representation.
    pub schema_version: RacingOutputVersion,
    /// Final standings in finishing order.
    pub standings: Vec<RacingStandingV1>,
    /// Total elapsed time of the player race in milliseconds.
    pub total_time_ms: u64,
    /// Laps on which the player entered the pits.
    pub player_pit_laps: Vec<u16>,
    /// Player lap times in chronological order, in milliseconds.
    pub player_lap_times_ms: Vec<u64>,
}

/// Canonical domain output and telemetry evidence produced by one Racing run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RacingRunEvidenceV1 {
    /// Canonical domain result used to calculate `output_digest`.
    pub output: RacingOutputV1,
    /// Domain-neutral stream summary used to calculate `telemetry_summary_digest`.
    pub telemetry_summary: TelemetrySummaryV1,
}

/// Failures produced while constructing or hashing Racing evidence.
#[derive(Debug)]
pub enum RacingEvidenceError {
    /// A batch count cannot be represented by the summary schema.
    BatchCountOverflow,
    /// Telemetry aggregation violated a V1 summary invariant.
    TelemetrySummary(TelemetrySummaryError),
    /// Canonical serialization of one evidence artifact failed.
    CanonicalJson(CanonicalJsonError),
}

impl fmt::Display for RacingEvidenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BatchCountOverflow => formatter.write_str("Racing batch count overflowed u64"),
            Self::TelemetrySummary(error) => error.fmt(formatter),
            Self::CanonicalJson(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for RacingEvidenceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::BatchCountOverflow => None,
            Self::TelemetrySummary(error) => Some(error),
            Self::CanonicalJson(error) => Some(error),
        }
    }
}

impl From<TelemetrySummaryError> for RacingEvidenceError {
    fn from(error: TelemetrySummaryError) -> Self {
        Self::TelemetrySummary(error)
    }
}

impl From<CanonicalJsonError> for RacingEvidenceError {
    fn from(error: CanonicalJsonError) -> Self {
        Self::CanonicalJson(error)
    }
}

impl RacingRunEvidenceV1 {
    /// Projects the mutable runtime output into the stable V1 evidence schemas.
    pub fn from_race_output(output: &RaceOutput) -> Result<Self, RacingEvidenceError> {
        let batch_count = u64::try_from(output.player_batches.len())
            .map_err(|_| RacingEvidenceError::BatchCountOverflow)?;
        let frames = output
            .player_batches
            .iter()
            .flat_map(|batch| batch.frames.iter());
        let telemetry_summary = TelemetrySummaryV1::from_ordered_frames(batch_count, frames, 0)?;

        let standings = output
            .standings
            .iter()
            .map(|standing| RacingStandingV1 {
                competitor_id: standing.competitor_id.clone(),
                position: standing.position,
                total_time_ms: standing.total_time_ms,
                best_lap_ms: standing.best_lap_ms,
                laps_completed: standing.laps_completed,
                gap_to_leader_ms: standing.gap_to_leader_ms,
                status: match &standing.status {
                    StandingStatus::Finished => RacingStandingStatusV1::Finished,
                    StandingStatus::Dnf { reason } => RacingStandingStatusV1::Dnf {
                        reason: reason.clone(),
                    },
                    StandingStatus::Dsq { reason } => RacingStandingStatusV1::Dsq {
                        reason: reason.clone(),
                    },
                },
            })
            .collect();

        Ok(Self {
            output: RacingOutputV1 {
                schema_version: RacingOutputVersion::V1,
                standings,
                total_time_ms: output.total_time_ms,
                player_pit_laps: output.player_pit_laps.clone(),
                player_lap_times_ms: output.player_lap_times_ms.clone(),
            },
            telemetry_summary,
        })
    }

    /// Returns RFC 8785 bytes for the complete V1 Racing domain result.
    pub fn canonical_output_bytes(&self) -> Result<Vec<u8>, CanonicalJsonError> {
        canonical_json_bytes(&self.output)
    }

    /// Returns RFC 8785 bytes for the domain-neutral telemetry summary.
    pub fn canonical_telemetry_summary_bytes(&self) -> Result<Vec<u8>, CanonicalJsonError> {
        canonical_json_bytes(&self.telemetry_summary)
    }

    /// Calculates the complete canonical Racing output digest.
    pub fn output_digest(&self) -> Result<Digest, CanonicalJsonError> {
        canonical_json_digest(&self.output)
    }

    /// Calculates the canonical telemetry summary digest.
    pub fn telemetry_summary_digest(&self) -> Result<Digest, CanonicalJsonError> {
        canonical_json_digest(&self.telemetry_summary)
    }

    /// Binds these canonical artifacts to one concrete execution receipt.
    pub fn execution_receipt(
        &self,
        contract: &DeterministicRunContractV1,
        execution_id: ExecutionId,
        runtime: RuntimeIdentity,
    ) -> Result<ExecutionReceiptV1, CanonicalJsonError> {
        ExecutionReceiptV1::for_contract(
            contract,
            execution_id,
            runtime,
            self.output_digest()?,
            self.telemetry_summary_digest()?,
        )
    }
}
