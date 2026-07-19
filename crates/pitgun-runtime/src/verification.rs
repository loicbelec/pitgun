//! Pure verification of deterministic Run Bundle V1 evidence.

use std::error::Error;
use std::fmt;

use pitgun_contract::{
    ArtifactIdentity, CanonicalJsonError, DeterministicRunContractV1, Digest,
    RunBundleManifestError, RunBundleManifestV1, RunBundleReceiptV1, RunBundleTelemetryRecordV1,
    RunContractError, ScenarioIdentity, TelemetrySummaryError, TelemetrySummaryV1,
};

/// Exact bytes loaded for every artifact referenced by a Run Bundle V1
/// manifest.
#[derive(Clone, Copy, Debug)]
pub struct RunBundleArtifactBytes<'a> {
    pub scenario: &'a [u8],
    pub contract: &'a [u8],
    pub output: &'a [u8],
    pub telemetry: &'a [u8],
    pub telemetry_summary: &'a [u8],
    pub metrics: &'a [u8],
    pub receipt: &'a [u8],
}

/// Domain-neutral identities extracted from a parsed scenario by an
/// application adapter.
#[derive(Clone, Copy, Debug)]
pub struct ScenarioBinding<'a> {
    pub scenario: &'a ScenarioIdentity,
    pub model: &'a ArtifactIdentity,
    pub data_pack: &'a ArtifactIdentity,
    pub input_digest: Digest,
}

/// Typed values and raw artifact bytes required for pure Run Bundle
/// verification.
#[derive(Clone, Copy, Debug)]
pub struct LoadedRunBundle<'a> {
    pub manifest: &'a RunBundleManifestV1,
    pub contract: &'a DeterministicRunContractV1,
    pub receipt: &'a RunBundleReceiptV1,
    pub scenario: ScenarioBinding<'a>,
    pub artifacts: RunBundleArtifactBytes<'a>,
    pub declared_telemetry_summary: &'a TelemetrySummaryV1,
    pub telemetry_records: &'a [RunBundleTelemetryRecordV1],
}

/// Domain-neutral evidence recalculated from a fully loaded Run Bundle V1.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedRunBundle {
    pub run_id: Digest,
    pub telemetry_summary: TelemetrySummaryV1,
}

/// Failure produced while verifying loaded deterministic evidence.
#[derive(Debug)]
pub enum RunBundleVerificationError {
    InvalidManifest(RunBundleManifestError),
    ArtifactDigestMismatch {
        artifact: &'static str,
        expected: Digest,
        actual: Digest,
    },
    CanonicalIdentity(CanonicalJsonError),
    ManifestRunIdMismatch {
        expected: Digest,
        actual: Digest,
    },
    ScenarioIdentityMismatch,
    ModelIdentityMismatch,
    DataPackIdentityMismatch,
    InputDigestMismatch {
        expected: Digest,
        actual: Digest,
    },
    Receipt(RunContractError),
    ReceiptOutputDigestMismatch {
        expected: Digest,
        actual: Digest,
    },
    ReceiptTelemetryDigestMismatch {
        expected: Digest,
        actual: Digest,
    },
    TelemetryOrdinalMismatch {
        record: usize,
        expected: u64,
        actual: u64,
    },
    TelemetryBatchOrdinalMismatch {
        record: usize,
        actual: u64,
    },
    TelemetryCountOverflow(&'static str),
    TelemetrySummary(TelemetrySummaryError),
    TelemetrySummaryMismatch,
}

impl fmt::Display for RunBundleVerificationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidManifest(error) => write!(formatter, "invalid manifest.json: {error}"),
            Self::ArtifactDigestMismatch {
                artifact,
                expected,
                actual,
            } => write!(
                formatter,
                "{artifact} digest mismatch: expected {expected}, got {actual}"
            ),
            Self::CanonicalIdentity(error) => {
                write!(
                    formatter,
                    "cannot calculate canonical run identity: {error}"
                )
            }
            Self::ManifestRunIdMismatch { expected, actual } => write!(
                formatter,
                "manifest run_id mismatch: expected {expected}, got {actual}"
            ),
            Self::ScenarioIdentityMismatch => {
                formatter.write_str("scenario.json identity does not match contract.json")
            }
            Self::ModelIdentityMismatch => {
                formatter.write_str("scenario.json model does not match contract.json")
            }
            Self::DataPackIdentityMismatch => {
                formatter.write_str("scenario.json data pack does not match contract.json")
            }
            Self::InputDigestMismatch { expected, actual } => write!(
                formatter,
                "scenario input digest mismatch: expected {expected}, got {actual}"
            ),
            Self::Receipt(error) => write!(formatter, "receipt.json: {error}"),
            Self::ReceiptOutputDigestMismatch { expected, actual } => write!(
                formatter,
                "receipt output digest mismatch: expected {expected}, got {actual}"
            ),
            Self::ReceiptTelemetryDigestMismatch { expected, actual } => write!(
                formatter,
                "receipt telemetry summary digest mismatch: expected {expected}, got {actual}"
            ),
            Self::TelemetryOrdinalMismatch {
                record,
                expected,
                actual,
            } => write!(
                formatter,
                "telemetry record {} has ordinal {actual}, expected {expected}",
                record + 1
            ),
            Self::TelemetryBatchOrdinalMismatch { record, actual } => write!(
                formatter,
                "telemetry record {} has non-contiguous batch ordinal {actual}",
                record + 1
            ),
            Self::TelemetryCountOverflow(field) => {
                write!(formatter, "telemetry {field} overflowed u64")
            }
            Self::TelemetrySummary(error) => {
                write!(formatter, "cannot summarize telemetry records: {error}")
            }
            Self::TelemetrySummaryMismatch => formatter
                .write_str("telemetry-summary.json does not match replayed telemetry.jsonl"),
        }
    }
}

impl Error for RunBundleVerificationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidManifest(error) => Some(error),
            Self::CanonicalIdentity(error) => Some(error),
            Self::Receipt(error) => Some(error),
            Self::TelemetrySummary(error) => Some(error),
            _ => None,
        }
    }
}

/// Verifies one fully loaded Run Bundle without filesystem or network access.
pub fn verify_loaded_run_bundle(
    loaded: LoadedRunBundle<'_>,
) -> Result<VerifiedRunBundle, RunBundleVerificationError> {
    loaded
        .manifest
        .validate()
        .map_err(RunBundleVerificationError::InvalidManifest)?;
    let telemetry_summary = summarize_telemetry(loaded.telemetry_records)?;
    verify_run_bundle_artifacts(loaded.manifest, loaded.artifacts)?;

    let run_id = loaded
        .contract
        .run_id()
        .map_err(RunBundleVerificationError::CanonicalIdentity)?;
    if loaded.manifest.run_id != run_id {
        return Err(RunBundleVerificationError::ManifestRunIdMismatch {
            expected: run_id,
            actual: loaded.manifest.run_id,
        });
    }
    if loaded.scenario.scenario != &loaded.contract.scenario {
        return Err(RunBundleVerificationError::ScenarioIdentityMismatch);
    }
    if loaded.scenario.model != &loaded.contract.model {
        return Err(RunBundleVerificationError::ModelIdentityMismatch);
    }
    if loaded.scenario.data_pack != &loaded.contract.data_pack {
        return Err(RunBundleVerificationError::DataPackIdentityMismatch);
    }
    if loaded.scenario.input_digest != loaded.contract.input.digest {
        return Err(RunBundleVerificationError::InputDigestMismatch {
            expected: loaded.contract.input.digest,
            actual: loaded.scenario.input_digest,
        });
    }

    loaded
        .contract
        .verify_receipt(&loaded.receipt.receipt)
        .map_err(RunBundleVerificationError::Receipt)?;
    verify_receipt_digests(loaded.manifest, loaded.receipt)?;

    if &telemetry_summary != loaded.declared_telemetry_summary {
        return Err(RunBundleVerificationError::TelemetrySummaryMismatch);
    }

    Ok(VerifiedRunBundle {
        run_id,
        telemetry_summary,
    })
}

/// Verifies the exact bytes of every artifact before an application adapter
/// parses their semantic content.
pub fn verify_run_bundle_artifacts(
    manifest: &RunBundleManifestV1,
    bytes: RunBundleArtifactBytes<'_>,
) -> Result<(), RunBundleVerificationError> {
    for (artifact, expected, actual_bytes) in [
        (
            "scenario.json",
            manifest.canonical_artifacts.scenario.digest,
            bytes.scenario,
        ),
        (
            "contract.json",
            manifest.canonical_artifacts.contract.digest,
            bytes.contract,
        ),
        (
            "output.json",
            manifest.canonical_artifacts.output.digest,
            bytes.output,
        ),
        (
            "telemetry.jsonl",
            manifest.canonical_artifacts.telemetry.digest,
            bytes.telemetry,
        ),
        (
            "telemetry-summary.json",
            manifest.canonical_artifacts.telemetry_summary.digest,
            bytes.telemetry_summary,
        ),
        (
            "metrics.json",
            manifest.canonical_artifacts.metrics.digest,
            bytes.metrics,
        ),
        (
            "receipt.json",
            manifest.execution_artifacts.receipt.digest,
            bytes.receipt,
        ),
    ] {
        let actual = Digest::from_bytes(actual_bytes);
        if actual != expected {
            return Err(RunBundleVerificationError::ArtifactDigestMismatch {
                artifact,
                expected,
                actual,
            });
        }
    }
    Ok(())
}

fn verify_receipt_digests(
    manifest: &RunBundleManifestV1,
    receipt: &RunBundleReceiptV1,
) -> Result<(), RunBundleVerificationError> {
    let expected_output = manifest.canonical_artifacts.output.digest;
    if receipt.receipt.output_digest != expected_output {
        return Err(RunBundleVerificationError::ReceiptOutputDigestMismatch {
            expected: expected_output,
            actual: receipt.receipt.output_digest,
        });
    }
    let expected_telemetry = manifest.canonical_artifacts.telemetry_summary.digest;
    if receipt.receipt.telemetry_summary_digest != expected_telemetry {
        return Err(RunBundleVerificationError::ReceiptTelemetryDigestMismatch {
            expected: expected_telemetry,
            actual: receipt.receipt.telemetry_summary_digest,
        });
    }
    Ok(())
}

fn summarize_telemetry(
    records: &[RunBundleTelemetryRecordV1],
) -> Result<TelemetrySummaryV1, RunBundleVerificationError> {
    let mut batch_count = 0_u64;
    for (index, record) in records.iter().enumerate() {
        let expected_ordinal = u64::try_from(index)
            .map_err(|_| RunBundleVerificationError::TelemetryCountOverflow("record ordinal"))?;
        if record.ordinal != expected_ordinal {
            return Err(RunBundleVerificationError::TelemetryOrdinalMismatch {
                record: index,
                expected: expected_ordinal,
                actual: record.ordinal,
            });
        }
        if record.batch_ordinal == batch_count {
            batch_count = batch_count.checked_add(1).ok_or(
                RunBundleVerificationError::TelemetryCountOverflow("batch count"),
            )?;
        } else if batch_count == 0 || record.batch_ordinal != batch_count - 1 {
            return Err(RunBundleVerificationError::TelemetryBatchOrdinalMismatch {
                record: index,
                actual: record.batch_ordinal,
            });
        }
    }

    TelemetrySummaryV1::from_ordered_frames(
        batch_count,
        records.iter().map(|record| &record.frame),
        0,
    )
    .map_err(RunBundleVerificationError::TelemetrySummary)
}
