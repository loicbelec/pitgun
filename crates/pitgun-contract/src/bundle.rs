//! Portable manifest and receipt schemas for deterministic run bundles.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{Digest, ExecutionReceiptV1, TelemetryFrame};

/// Wire version of a complete deterministic run-bundle manifest.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum RunBundleManifestVersion {
    /// First portable directory contract.
    #[serde(rename = "pitgun.run-bundle-manifest/v1")]
    V1,
}

/// Wire version of the execution receipt stored in a run bundle.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum RunBundleReceiptVersion {
    /// Wrapper around the domain-neutral V1 execution receipt.
    #[serde(rename = "pitgun.run-bundle-receipt/v1")]
    V1,
}

/// Wire version carried by every line of `telemetry.jsonl`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum RunBundleTelemetryRecordVersion {
    /// One ordered canonical telemetry frame.
    #[serde(rename = "pitgun.telemetry-record/v1")]
    V1,
}

/// One self-describing line in the canonical telemetry stream artifact.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunBundleTelemetryRecordV1 {
    pub schema_version: RunBundleTelemetryRecordVersion,
    pub ordinal: u64,
    pub frame: TelemetryFrame,
}

/// Media type of a file referenced by the bundle manifest.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum RunBundleMediaType {
    /// RFC 8259 JSON, encoded canonically for deterministic artifacts.
    #[serde(rename = "application/json")]
    ApplicationJson,
    /// One canonical JSON value per UTF-8 line.
    #[serde(rename = "application/x-ndjson")]
    ApplicationNdjson,
}

/// A content-addressed file whose path is relative to the bundle root.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunBundleArtifactV1 {
    /// Portable path relative to `manifest.json`.
    pub path: String,
    /// Exact media type used to encode the file.
    pub media_type: RunBundleMediaType,
    /// SHA-256 of the exact stored bytes.
    pub digest: Digest,
}

/// Deterministic evidence that must be identical for the same logical run.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunBundleCanonicalArtifactsV1 {
    pub scenario: RunBundleArtifactV1,
    pub contract: RunBundleArtifactV1,
    pub output: RunBundleArtifactV1,
    pub telemetry: RunBundleArtifactV1,
    pub telemetry_summary: RunBundleArtifactV1,
    pub metrics: RunBundleArtifactV1,
}

/// Evidence tied to one concrete execution rather than the logical run.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunBundleExecutionArtifactsV1 {
    pub receipt: RunBundleArtifactV1,
}

/// Root index of a portable deterministic run directory.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunBundleManifestV1 {
    pub schema_version: RunBundleManifestVersion,
    pub run_id: Digest,
    pub canonical_artifacts: RunBundleCanonicalArtifactsV1,
    pub execution_artifacts: RunBundleExecutionArtifactsV1,
}

/// Versioned wrapper for a concrete execution receipt.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunBundleReceiptV1 {
    pub schema_version: RunBundleReceiptVersion,
    pub receipt: ExecutionReceiptV1,
}

/// Structural errors in a V1 bundle manifest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RunBundleManifestError {
    UnexpectedPath {
        artifact: &'static str,
        expected: &'static str,
        actual: String,
    },
    UnexpectedMediaType {
        artifact: &'static str,
        expected: RunBundleMediaType,
        actual: RunBundleMediaType,
    },
}

impl fmt::Display for RunBundleManifestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedPath {
                artifact,
                expected,
                actual,
            } => write!(
                formatter,
                "invalid {artifact} path {actual:?}; expected {expected:?}"
            ),
            Self::UnexpectedMediaType {
                artifact,
                expected,
                actual,
            } => write!(
                formatter,
                "invalid {artifact} media type {actual:?}; expected {expected:?}"
            ),
        }
    }
}

impl std::error::Error for RunBundleManifestError {}

impl RunBundleManifestV1 {
    /// Validates the fixed V1 file layout and media types.
    pub fn validate(&self) -> Result<(), RunBundleManifestError> {
        validate_artifact(
            "scenario",
            &self.canonical_artifacts.scenario,
            "scenario.json",
            RunBundleMediaType::ApplicationJson,
        )?;
        validate_artifact(
            "contract",
            &self.canonical_artifacts.contract,
            "contract.json",
            RunBundleMediaType::ApplicationJson,
        )?;
        validate_artifact(
            "output",
            &self.canonical_artifacts.output,
            "output.json",
            RunBundleMediaType::ApplicationJson,
        )?;
        validate_artifact(
            "telemetry",
            &self.canonical_artifacts.telemetry,
            "telemetry.jsonl",
            RunBundleMediaType::ApplicationNdjson,
        )?;
        validate_artifact(
            "telemetry summary",
            &self.canonical_artifacts.telemetry_summary,
            "telemetry-summary.json",
            RunBundleMediaType::ApplicationJson,
        )?;
        validate_artifact(
            "metrics",
            &self.canonical_artifacts.metrics,
            "metrics.json",
            RunBundleMediaType::ApplicationJson,
        )?;
        validate_artifact(
            "receipt",
            &self.execution_artifacts.receipt,
            "receipt.json",
            RunBundleMediaType::ApplicationJson,
        )
    }
}

fn validate_artifact(
    name: &'static str,
    artifact: &RunBundleArtifactV1,
    expected_path: &'static str,
    expected_media_type: RunBundleMediaType,
) -> Result<(), RunBundleManifestError> {
    if artifact.path != expected_path {
        return Err(RunBundleManifestError::UnexpectedPath {
            artifact: name,
            expected: expected_path,
            actual: artifact.path.clone(),
        });
    }
    if artifact.media_type != expected_media_type {
        return Err(RunBundleManifestError::UnexpectedMediaType {
            artifact: name,
            expected: expected_media_type,
            actual: artifact.media_type,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact(path: &str, media_type: RunBundleMediaType) -> RunBundleArtifactV1 {
        RunBundleArtifactV1 {
            path: path.to_owned(),
            media_type,
            digest: Digest::from_bytes(path.as_bytes()),
        }
    }

    fn manifest() -> RunBundleManifestV1 {
        RunBundleManifestV1 {
            schema_version: RunBundleManifestVersion::V1,
            run_id: Digest::from_bytes(b"run"),
            canonical_artifacts: RunBundleCanonicalArtifactsV1 {
                scenario: artifact("scenario.json", RunBundleMediaType::ApplicationJson),
                contract: artifact("contract.json", RunBundleMediaType::ApplicationJson),
                output: artifact("output.json", RunBundleMediaType::ApplicationJson),
                telemetry: artifact("telemetry.jsonl", RunBundleMediaType::ApplicationNdjson),
                telemetry_summary: artifact(
                    "telemetry-summary.json",
                    RunBundleMediaType::ApplicationJson,
                ),
                metrics: artifact("metrics.json", RunBundleMediaType::ApplicationJson),
            },
            execution_artifacts: RunBundleExecutionArtifactsV1 {
                receipt: artifact("receipt.json", RunBundleMediaType::ApplicationJson),
            },
        }
    }

    #[test]
    fn v1_layout_is_portable_and_strict() {
        manifest().validate().expect("valid V1 manifest");

        let mut invalid = manifest();
        invalid.canonical_artifacts.output.path = "../output.json".to_owned();
        assert!(matches!(
            invalid.validate(),
            Err(RunBundleManifestError::UnexpectedPath {
                artifact: "output",
                ..
            })
        ));
    }

    #[test]
    fn unknown_manifest_fields_are_rejected() {
        let mut value = serde_json::to_value(manifest()).expect("manifest value");
        value["unexpected"] = serde_json::json!(true);
        assert!(serde_json::from_value::<RunBundleManifestV1>(value).is_err());
    }
}
