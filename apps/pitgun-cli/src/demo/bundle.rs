use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use pitgun_contract::{
    canonical_json_bytes, DeterministicRunContractV1, Digest, ExecutionId, Identifier,
    RunBundleArtifactV1, RunBundleCanonicalArtifactsV1, RunBundleExecutionArtifactsV1,
    RunBundleManifestV1, RunBundleManifestVersion, RunBundleMediaType, RunBundleReceiptV1,
    RunBundleReceiptVersion, RunBundleTelemetryRecordV1, RunBundleTelemetryRecordVersion,
    RuntimeIdentity, SemanticVersion, TelemetrySummaryV1,
};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

use super::racing::RacingDemoRun;

const MANIFEST_FILE: &str = "manifest.json";
const SCENARIO_FILE: &str = "scenario.json";
const CONTRACT_FILE: &str = "contract.json";
const OUTPUT_FILE: &str = "output.json";
const TELEMETRY_FILE: &str = "telemetry.jsonl";
const TELEMETRY_SUMMARY_FILE: &str = "telemetry-summary.json";
const RECEIPT_FILE: &str = "receipt.json";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BundleDisposition {
    Created,
    Reused,
}

impl fmt::Display for BundleDisposition {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Created => formatter.write_str("created"),
            Self::Reused => formatter.write_str("reused"),
        }
    }
}

#[derive(Debug)]
pub(crate) struct PersistedBundle {
    pub(crate) path: PathBuf,
    pub(crate) disposition: BundleDisposition,
}

#[derive(Debug)]
pub(crate) struct BundleError {
    message: String,
}

impl BundleError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub(crate) const fn exit_code(&self) -> u8 {
        30
    }
}

impl fmt::Display for BundleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Run bundle failed: {}", self.message)
    }
}

impl std::error::Error for BundleError {}

struct CanonicalArtifacts {
    scenario: Vec<u8>,
    contract: Vec<u8>,
    output: Vec<u8>,
    telemetry: Vec<u8>,
    telemetry_summary: Vec<u8>,
}

impl CanonicalArtifacts {
    fn from_run(run: &RacingDemoRun) -> Result<Self, BundleError> {
        Ok(Self {
            scenario: run.scenario_json.clone(),
            contract: canonical_json_bytes(&run.contract).map_err(bundle_error)?,
            output: run
                .evidence
                .canonical_output_bytes()
                .map_err(bundle_error)?,
            telemetry: encode_telemetry(run)?,
            telemetry_summary: run
                .evidence
                .canonical_telemetry_summary_bytes()
                .map_err(bundle_error)?,
        })
    }

    fn references(&self) -> RunBundleCanonicalArtifactsV1 {
        RunBundleCanonicalArtifactsV1 {
            scenario: reference(
                SCENARIO_FILE,
                RunBundleMediaType::ApplicationJson,
                &self.scenario,
            ),
            contract: reference(
                CONTRACT_FILE,
                RunBundleMediaType::ApplicationJson,
                &self.contract,
            ),
            output: reference(
                OUTPUT_FILE,
                RunBundleMediaType::ApplicationJson,
                &self.output,
            ),
            telemetry: reference(
                TELEMETRY_FILE,
                RunBundleMediaType::ApplicationNdjson,
                &self.telemetry,
            ),
            telemetry_summary: reference(
                TELEMETRY_SUMMARY_FILE,
                RunBundleMediaType::ApplicationJson,
                &self.telemetry_summary,
            ),
            metrics: None,
        }
    }
}

pub(crate) fn persist(
    run: &RacingDemoRun,
    requested_path: Option<&Path>,
) -> Result<PersistedBundle, BundleError> {
    let artifacts = CanonicalArtifacts::from_run(run)?;
    let expected_references = artifacts.references();
    let destination =
        requested_path.map_or_else(|| default_destination(&run.run_id), Path::to_path_buf);
    validate_destination_shape(&destination)?;

    if destination.exists() {
        if !destination.is_dir() {
            return Err(BundleError::new(format!(
                "destination {} is not a directory",
                destination.display()
            )));
        }
        let manifest = validate_bundle(&destination, Some(&run.run_id))?;
        if manifest.canonical_artifacts != expected_references {
            return Err(BundleError::new(format!(
                "existing bundle {} conflicts with newly calculated canonical evidence",
                destination.display()
            )));
        }
        return Ok(PersistedBundle {
            path: destination,
            disposition: BundleDisposition::Reused,
        });
    }

    let parent = destination.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|error| {
        BundleError::new(format!(
            "cannot create parent {}: {error}",
            parent.display()
        ))
    })?;
    let leaf = destination
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| BundleError::new("destination must have a UTF-8 directory name"))?;
    let staging = parent.join(format!(".{leaf}.tmp-{}", Uuid::now_v7()));
    fs::create_dir(&staging).map_err(|error| {
        BundleError::new(format!(
            "cannot create staging directory {}: {error}",
            staging.display()
        ))
    })?;

    let result = write_and_commit(run, &artifacts, &staging, &destination);
    if result.is_err() {
        let _ = fs::remove_dir_all(&staging);
    }
    result?;

    Ok(PersistedBundle {
        path: destination,
        disposition: BundleDisposition::Created,
    })
}

fn write_and_commit(
    run: &RacingDemoRun,
    artifacts: &CanonicalArtifacts,
    staging: &Path,
    destination: &Path,
) -> Result<(), BundleError> {
    write_file(staging, SCENARIO_FILE, &artifacts.scenario)?;
    write_file(staging, CONTRACT_FILE, &artifacts.contract)?;
    write_file(staging, OUTPUT_FILE, &artifacts.output)?;
    write_file(staging, TELEMETRY_FILE, &artifacts.telemetry)?;
    write_file(
        staging,
        TELEMETRY_SUMMARY_FILE,
        &artifacts.telemetry_summary,
    )?;

    let receipt = execution_receipt(run)?;
    let receipt_bytes = canonical_json_bytes(&receipt).map_err(bundle_error)?;
    write_file(staging, RECEIPT_FILE, &receipt_bytes)?;

    let manifest = RunBundleManifestV1 {
        schema_version: RunBundleManifestVersion::V1,
        run_id: run.run_id,
        canonical_artifacts: artifacts.references(),
        execution_artifacts: RunBundleExecutionArtifactsV1 {
            receipt: reference(
                RECEIPT_FILE,
                RunBundleMediaType::ApplicationJson,
                &receipt_bytes,
            ),
        },
    };
    manifest.validate().map_err(bundle_error)?;
    let manifest_bytes = canonical_json_bytes(&manifest).map_err(bundle_error)?;
    write_file(staging, MANIFEST_FILE, &manifest_bytes)?;
    validate_bundle(staging, Some(&run.run_id))?;

    fs::rename(staging, destination).map_err(|error| {
        BundleError::new(format!(
            "cannot commit {} to {}: {error}",
            staging.display(),
            destination.display()
        ))
    })
}

fn execution_receipt(run: &RacingDemoRun) -> Result<RunBundleReceiptV1, BundleError> {
    let executable = std::env::current_exe()
        .map_err(|error| BundleError::new(format!("cannot locate current executable: {error}")))?;
    let executable_bytes = fs::read(&executable).map_err(|error| {
        BundleError::new(format!(
            "cannot read runtime artifact {}: {error}",
            executable.display()
        ))
    })?;
    let execution_id: ExecutionId = Uuid::now_v7()
        .hyphenated()
        .to_string()
        .parse()
        .map_err(bundle_error)?;
    let runtime = RuntimeIdentity {
        engine: Identifier::new("pitgun-cli").map_err(bundle_error)?,
        engine_version: SemanticVersion::new(env!("CARGO_PKG_VERSION")).map_err(bundle_error)?,
        target: Identifier::new(format!(
            "{}-{}",
            std::env::consts::ARCH,
            std::env::consts::OS
        ))
        .map_err(bundle_error)?,
        artifact_digest: Digest::from_bytes(&executable_bytes),
    };
    let receipt = run
        .evidence
        .execution_receipt(&run.contract, execution_id, runtime)
        .map_err(bundle_error)?;
    Ok(RunBundleReceiptV1 {
        schema_version: RunBundleReceiptVersion::V1,
        receipt,
    })
}

fn encode_telemetry(run: &RacingDemoRun) -> Result<Vec<u8>, BundleError> {
    let mut bytes = Vec::new();
    for (ordinal, frame) in run
        .output
        .player_batches
        .iter()
        .flat_map(|batch| batch.frames.iter())
        .enumerate()
    {
        let ordinal = u64::try_from(ordinal)
            .map_err(|_| BundleError::new("telemetry ordinal overflowed u64"))?;
        let record = RunBundleTelemetryRecordV1 {
            schema_version: RunBundleTelemetryRecordVersion::V1,
            ordinal,
            frame: frame.clone(),
        };
        bytes.extend(canonical_json_bytes(&record).map_err(bundle_error)?);
        bytes.push(b'\n');
    }
    Ok(bytes)
}

fn validate_bundle(
    root: &Path,
    expected_run_id: Option<&Digest>,
) -> Result<RunBundleManifestV1, BundleError> {
    let manifest_bytes = read_file(root, MANIFEST_FILE)?;
    let manifest: RunBundleManifestV1 = parse_canonical_json(MANIFEST_FILE, &manifest_bytes)?;
    manifest.validate().map_err(bundle_error)?;
    if expected_run_id.is_some_and(|expected| expected != &manifest.run_id) {
        return Err(BundleError::new(format!(
            "bundle {} belongs to run {}, not {}",
            root.display(),
            manifest.run_id,
            expected_run_id.expect("checked above")
        )));
    }

    validate_reference(root, &manifest.canonical_artifacts.scenario)?;
    validate_reference(root, &manifest.canonical_artifacts.contract)?;
    validate_reference(root, &manifest.canonical_artifacts.output)?;
    validate_reference(root, &manifest.canonical_artifacts.telemetry)?;
    validate_reference(root, &manifest.canonical_artifacts.telemetry_summary)?;
    if let Some(metrics) = &manifest.canonical_artifacts.metrics {
        validate_reference(root, metrics)?;
    }
    validate_reference(root, &manifest.execution_artifacts.receipt)?;

    let scenario: Value = parse_artifact_json(root, &manifest.canonical_artifacts.scenario)?;
    require_schema_version(SCENARIO_FILE, &scenario)?;
    let contract: DeterministicRunContractV1 =
        parse_artifact_json(root, &manifest.canonical_artifacts.contract)?;
    let contract_run_id = contract.run_id().map_err(bundle_error)?;
    if contract_run_id != manifest.run_id {
        return Err(BundleError::new(format!(
            "contract run_id {contract_run_id} does not match manifest {}",
            manifest.run_id
        )));
    }
    let output: Value = parse_artifact_json(root, &manifest.canonical_artifacts.output)?;
    require_schema_version(OUTPUT_FILE, &output)?;
    let summary: TelemetrySummaryV1 =
        parse_artifact_json(root, &manifest.canonical_artifacts.telemetry_summary)?;
    validate_telemetry(
        root,
        &manifest.canonical_artifacts.telemetry,
        summary.frame_count(),
    )?;
    let receipt: RunBundleReceiptV1 =
        parse_artifact_json(root, &manifest.execution_artifacts.receipt)?;
    contract
        .verify_receipt(&receipt.receipt)
        .map_err(bundle_error)?;
    if receipt.receipt.output_digest != manifest.canonical_artifacts.output.digest {
        return Err(BundleError::new(
            "receipt output digest does not match manifest",
        ));
    }
    if receipt.receipt.telemetry_summary_digest
        != manifest.canonical_artifacts.telemetry_summary.digest
    {
        return Err(BundleError::new(
            "receipt telemetry summary digest does not match manifest",
        ));
    }

    Ok(manifest)
}

fn validate_telemetry(
    root: &Path,
    artifact: &RunBundleArtifactV1,
    expected_frames: u64,
) -> Result<(), BundleError> {
    let bytes = read_file(root, &artifact.path)?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|error| BundleError::new(format!("{} is not UTF-8: {error}", artifact.path)))?;
    let mut count = 0_u64;
    for (index, line) in text.lines().enumerate() {
        let record: RunBundleTelemetryRecordV1 =
            parse_canonical_json(&artifact.path, line.as_bytes())?;
        if record.ordinal != count {
            return Err(BundleError::new(format!(
                "{} line {} has ordinal {}, expected {}",
                artifact.path,
                index + 1,
                record.ordinal,
                count
            )));
        }
        count = count
            .checked_add(1)
            .ok_or_else(|| BundleError::new("telemetry frame count overflowed u64"))?;
    }
    if !bytes.is_empty() && !bytes.ends_with(b"\n") {
        return Err(BundleError::new(format!(
            "{} must end with a newline",
            artifact.path
        )));
    }
    if count != expected_frames {
        return Err(BundleError::new(format!(
            "{} contains {count} frames, summary declares {expected_frames}",
            artifact.path
        )));
    }
    Ok(())
}

fn parse_artifact_json<T: DeserializeOwned + Serialize>(
    root: &Path,
    artifact: &RunBundleArtifactV1,
) -> Result<T, BundleError> {
    let bytes = read_file(root, &artifact.path)?;
    parse_canonical_json(&artifact.path, &bytes)
}

fn parse_canonical_json<T: DeserializeOwned + Serialize>(
    name: &str,
    bytes: &[u8],
) -> Result<T, BundleError> {
    let value: T = serde_json::from_slice(bytes)
        .map_err(|error| BundleError::new(format!("invalid {name}: {error}")))?;
    let canonical = canonical_json_bytes(&value).map_err(bundle_error)?;
    if canonical != bytes {
        return Err(BundleError::new(format!(
            "{name} is not canonically encoded"
        )));
    }
    Ok(value)
}

fn require_schema_version(name: &str, value: &Value) -> Result<(), BundleError> {
    if value
        .as_object()
        .and_then(|object| object.get("schema_version"))
        .and_then(Value::as_str)
        .is_none()
    {
        return Err(BundleError::new(format!(
            "{name} has no string schema_version"
        )));
    }
    Ok(())
}

fn validate_reference(root: &Path, artifact: &RunBundleArtifactV1) -> Result<(), BundleError> {
    let bytes = read_file(root, &artifact.path)?;
    let actual = Digest::from_bytes(&bytes);
    if actual != artifact.digest {
        return Err(BundleError::new(format!(
            "{} digest mismatch: expected {}, got {}",
            artifact.path, artifact.digest, actual
        )));
    }
    Ok(())
}

fn reference(path: &str, media_type: RunBundleMediaType, bytes: &[u8]) -> RunBundleArtifactV1 {
    RunBundleArtifactV1 {
        path: path.to_owned(),
        media_type,
        digest: Digest::from_bytes(bytes),
    }
}

fn write_file(root: &Path, name: &str, bytes: &[u8]) -> Result<(), BundleError> {
    let path = root.join(name);
    fs::write(&path, bytes)
        .map_err(|error| BundleError::new(format!("cannot write {}: {error}", path.display())))
}

fn read_file(root: &Path, name: &str) -> Result<Vec<u8>, BundleError> {
    let path = root.join(name);
    let metadata = fs::symlink_metadata(&path)
        .map_err(|error| BundleError::new(format!("cannot inspect {}: {error}", path.display())))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(BundleError::new(format!(
            "{} must be a regular file inside the bundle",
            path.display()
        )));
    }
    fs::read(&path)
        .map_err(|error| BundleError::new(format!("cannot read {}: {error}", path.display())))
}

fn default_destination(run_id: &Digest) -> PathBuf {
    PathBuf::from("pitgun-runs").join(run_id.to_string().replacen(':', "-", 1))
}

fn validate_destination_shape(destination: &Path) -> Result<(), BundleError> {
    if destination.as_os_str().is_empty() || destination.file_name().is_none() {
        return Err(BundleError::new(format!(
            "destination {} must name a bundle directory",
            destination.display()
        )));
    }
    Ok(())
}

fn bundle_error(error: impl fmt::Display) -> BundleError {
    BundleError::new(error.to_string())
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use crate::demo::racing::{run, RacingArgs};

    fn temporary_path(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("pitgun-{label}-{}-{nonce}", std::process::id()))
    }

    #[test]
    fn creates_and_reuses_an_immutable_bundle() {
        let root = temporary_path("bundle-reuse");
        let first_run = run(&RacingArgs {
            seed: 42,
            output: Some(root.clone()),
        })
        .expect("Racing run");

        let created = persist(&first_run, Some(&root)).expect("created bundle");
        assert_eq!(created.disposition, BundleDisposition::Created);
        let manifest_before = fs::read(root.join(MANIFEST_FILE)).expect("manifest before reuse");

        let repeated_run = run(&RacingArgs {
            seed: 42,
            output: Some(root.clone()),
        })
        .expect("repeated Racing run");
        let reused = persist(&repeated_run, Some(&root)).expect("reused bundle");
        assert_eq!(reused.disposition, BundleDisposition::Reused);
        let manifest_after = fs::read(root.join(MANIFEST_FILE)).expect("manifest after reuse");
        assert_eq!(manifest_before, manifest_after);

        fs::remove_dir_all(root).expect("remove test bundle");
    }

    #[test]
    fn rejects_a_tampered_existing_bundle_without_modifying_it() {
        let root = temporary_path("bundle-tamper");
        let run = run(&RacingArgs {
            seed: 42,
            output: Some(root.clone()),
        })
        .expect("Racing run");
        persist(&run, Some(&root)).expect("created bundle");
        fs::write(root.join(OUTPUT_FILE), b"{}").expect("tamper with output for validation test");

        let error = persist(&run, Some(&root)).expect_err("tampered bundle must fail");
        assert!(error.to_string().contains("digest mismatch"));
        assert_eq!(
            fs::read(root.join(OUTPUT_FILE)).expect("tampered file"),
            b"{}"
        );

        fs::remove_dir_all(root).expect("remove test bundle");
    }
}
