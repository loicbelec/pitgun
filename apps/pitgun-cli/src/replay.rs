use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use pitgun_contract::{
    canonical_json_bytes, canonical_json_digest, ArtifactIdentity, DerivedMetricProcessorV1,
    DerivedMetricStatisticV1, DerivedMetricV1, DerivedMetricsV1, DeterministicRunContractV1,
    Digest, RunBundleArtifactV1, RunBundleManifestV1, RunBundleReceiptV1,
    RunBundleTelemetryRecordV1, ScenarioIdentity, TelemetrySummaryV1,
};
use pitgun_core::{
    aggregate_telemetry_parameter, TelemetryAggregateConfig, TelemetryAggregateKind,
};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

const MANIFEST_FILE: &str = "manifest.json";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReplayPhase {
    Replay,
    Verification,
}

/// Failure to load/replay evidence or a deterministic verification mismatch.
#[derive(Debug)]
pub(crate) struct ReplayError {
    phase: ReplayPhase,
    message: String,
}

impl ReplayError {
    fn replay(message: impl Into<String>) -> Self {
        Self {
            phase: ReplayPhase::Replay,
            message: message.into(),
        }
    }

    fn verification(message: impl Into<String>) -> Self {
        Self {
            phase: ReplayPhase::Verification,
            message: message.into(),
        }
    }

    pub(crate) const fn exit_code(&self) -> u8 {
        match self.phase {
            ReplayPhase::Replay => 40,
            ReplayPhase::Verification => 50,
        }
    }
}

impl fmt::Display for ReplayError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.phase {
            ReplayPhase::Replay => write!(formatter, "Run replay failed: {}", self.message),
            ReplayPhase::Verification => {
                write!(formatter, "Run verification failed: {}", self.message)
            }
        }
    }
}

impl std::error::Error for ReplayError {}

/// Evidence recalculated exclusively from one committed bundle directory.
#[derive(Debug)]
pub(crate) struct ReplayReport {
    pub(crate) root: PathBuf,
    pub(crate) run_id: Digest,
    pub(crate) frame_count: u64,
    pub(crate) batch_count: u64,
    pub(crate) metrics: DerivedMetricsV1,
}

struct ArtifactBytes {
    scenario: Vec<u8>,
    contract: Vec<u8>,
    output: Vec<u8>,
    telemetry: Vec<u8>,
    telemetry_summary: Vec<u8>,
    metrics: Vec<u8>,
    receipt: Vec<u8>,
}

struct LoadedTelemetry {
    records: Vec<RunBundleTelemetryRecordV1>,
    batch_count: u64,
}

/// Loads, deterministically replays, and verifies a committed Run Bundle V1.
pub(crate) fn replay_and_verify(root: &Path) -> Result<ReplayReport, ReplayError> {
    if !root.is_dir() {
        return Err(ReplayError::replay(format!(
            "bundle root {} is not a directory",
            root.display()
        )));
    }

    let manifest_bytes = read_regular_file(&root.join(MANIFEST_FILE))?;
    let manifest: RunBundleManifestV1 = parse_canonical_json(MANIFEST_FILE, &manifest_bytes)?;
    manifest
        .validate()
        .map_err(|error| ReplayError::replay(format!("invalid manifest.json: {error}")))?;

    let bytes = load_artifacts(root, &manifest)?;
    let scenario: Value = parse_canonical_json("scenario.json", &bytes.scenario)?;
    require_schema_version("scenario.json", &scenario)?;
    let contract: DeterministicRunContractV1 =
        parse_canonical_json("contract.json", &bytes.contract)?;
    let output: Value = parse_canonical_json("output.json", &bytes.output)?;
    require_schema_version("output.json", &output)?;
    let declared_summary: TelemetrySummaryV1 =
        parse_canonical_json("telemetry-summary.json", &bytes.telemetry_summary)?;
    let declared_metrics: DerivedMetricsV1 = parse_canonical_json("metrics.json", &bytes.metrics)?;
    declared_metrics
        .validate()
        .map_err(|error| ReplayError::replay(format!("invalid metrics.json: {error}")))?;
    let receipt: RunBundleReceiptV1 = parse_canonical_json("receipt.json", &bytes.receipt)?;
    let telemetry = load_telemetry(&bytes.telemetry)?;

    verify_artifact_digests(&manifest, &bytes)?;
    verify_contract_and_scenario(&manifest, &contract, &scenario)?;
    contract
        .verify_receipt(&receipt.receipt)
        .map_err(|error| ReplayError::verification(format!("receipt.json: {error}")))?;
    verify_equal(
        "receipt output digest",
        receipt.receipt.output_digest,
        manifest.canonical_artifacts.output.digest,
    )?;
    verify_equal(
        "receipt telemetry summary digest",
        receipt.receipt.telemetry_summary_digest,
        manifest.canonical_artifacts.telemetry_summary.digest,
    )?;

    let recalculated_summary = TelemetrySummaryV1::from_ordered_frames(
        telemetry.batch_count,
        telemetry.records.iter().map(|record| &record.frame),
        0,
    )
    .map_err(|error| ReplayError::replay(format!("cannot summarize telemetry.jsonl: {error}")))?;
    if recalculated_summary != declared_summary {
        return Err(ReplayError::verification(
            "telemetry-summary.json does not match replayed telemetry.jsonl",
        ));
    }

    let recalculated_metrics = recalculate_metrics(&declared_metrics, &telemetry)?;
    if recalculated_metrics != declared_metrics {
        return Err(ReplayError::verification(
            "metrics.json does not match replayed telemetry.jsonl",
        ));
    }

    Ok(ReplayReport {
        root: root.to_path_buf(),
        run_id: manifest.run_id,
        frame_count: recalculated_summary.frame_count(),
        batch_count: recalculated_summary.batch_count(),
        metrics: recalculated_metrics,
    })
}

fn load_artifacts(
    root: &Path,
    manifest: &RunBundleManifestV1,
) -> Result<ArtifactBytes, ReplayError> {
    Ok(ArtifactBytes {
        scenario: read_artifact(root, &manifest.canonical_artifacts.scenario)?,
        contract: read_artifact(root, &manifest.canonical_artifacts.contract)?,
        output: read_artifact(root, &manifest.canonical_artifacts.output)?,
        telemetry: read_artifact(root, &manifest.canonical_artifacts.telemetry)?,
        telemetry_summary: read_artifact(root, &manifest.canonical_artifacts.telemetry_summary)?,
        metrics: read_artifact(root, &manifest.canonical_artifacts.metrics)?,
        receipt: read_artifact(root, &manifest.execution_artifacts.receipt)?,
    })
}

fn read_artifact(root: &Path, artifact: &RunBundleArtifactV1) -> Result<Vec<u8>, ReplayError> {
    read_regular_file(&root.join(&artifact.path))
}

fn read_regular_file(path: &Path) -> Result<Vec<u8>, ReplayError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        ReplayError::replay(format!("cannot inspect {}: {error}", path.display()))
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(ReplayError::replay(format!(
            "{} must be a regular file inside the bundle",
            path.display()
        )));
    }
    fs::read(path)
        .map_err(|error| ReplayError::replay(format!("cannot read {}: {error}", path.display())))
}

fn parse_canonical_json<T: DeserializeOwned + Serialize>(
    name: &str,
    bytes: &[u8],
) -> Result<T, ReplayError> {
    let value: T = serde_json::from_slice(bytes)
        .map_err(|error| ReplayError::replay(format!("invalid {name}: {error}")))?;
    let canonical = canonical_json_bytes(&value)
        .map_err(|error| ReplayError::replay(format!("cannot canonicalize {name}: {error}")))?;
    if canonical != bytes {
        return Err(ReplayError::replay(format!(
            "{name} is not canonically encoded"
        )));
    }
    Ok(value)
}

fn require_schema_version(name: &str, value: &Value) -> Result<(), ReplayError> {
    if value
        .as_object()
        .and_then(|object| object.get("schema_version"))
        .and_then(Value::as_str)
        .is_none()
    {
        return Err(ReplayError::replay(format!(
            "{name} has no string schema_version"
        )));
    }
    Ok(())
}

fn load_telemetry(bytes: &[u8]) -> Result<LoadedTelemetry, ReplayError> {
    if !bytes.is_empty() && !bytes.ends_with(b"\n") {
        return Err(ReplayError::replay(
            "telemetry.jsonl must end with a newline",
        ));
    }
    let text = std::str::from_utf8(bytes)
        .map_err(|error| ReplayError::replay(format!("telemetry.jsonl is not UTF-8: {error}")))?;
    let mut records = Vec::new();
    let mut batch_count = 0_u64;
    for (index, line) in text.lines().enumerate() {
        let record: RunBundleTelemetryRecordV1 =
            parse_canonical_json("telemetry.jsonl record", line.as_bytes())?;
        let expected_ordinal = u64::try_from(index)
            .map_err(|_| ReplayError::replay("telemetry record ordinal overflowed u64"))?;
        if record.ordinal != expected_ordinal {
            return Err(ReplayError::replay(format!(
                "telemetry.jsonl line {} has ordinal {}, expected {}",
                index + 1,
                record.ordinal,
                expected_ordinal
            )));
        }
        if record.batch_ordinal == batch_count {
            batch_count = batch_count
                .checked_add(1)
                .ok_or_else(|| ReplayError::replay("telemetry batch count overflowed u64"))?;
        } else if batch_count == 0 || record.batch_ordinal != batch_count - 1 {
            return Err(ReplayError::replay(format!(
                "telemetry.jsonl line {} has non-contiguous batch ordinal {}",
                index + 1,
                record.batch_ordinal
            )));
        }
        records.push(record);
    }
    Ok(LoadedTelemetry {
        records,
        batch_count,
    })
}

fn verify_artifact_digests(
    manifest: &RunBundleManifestV1,
    bytes: &ArtifactBytes,
) -> Result<(), ReplayError> {
    for (name, expected, actual_bytes) in [
        (
            "scenario.json",
            manifest.canonical_artifacts.scenario.digest,
            bytes.scenario.as_slice(),
        ),
        (
            "contract.json",
            manifest.canonical_artifacts.contract.digest,
            bytes.contract.as_slice(),
        ),
        (
            "output.json",
            manifest.canonical_artifacts.output.digest,
            bytes.output.as_slice(),
        ),
        (
            "telemetry.jsonl",
            manifest.canonical_artifacts.telemetry.digest,
            bytes.telemetry.as_slice(),
        ),
        (
            "telemetry-summary.json",
            manifest.canonical_artifacts.telemetry_summary.digest,
            bytes.telemetry_summary.as_slice(),
        ),
        (
            "metrics.json",
            manifest.canonical_artifacts.metrics.digest,
            bytes.metrics.as_slice(),
        ),
        (
            "receipt.json",
            manifest.execution_artifacts.receipt.digest,
            bytes.receipt.as_slice(),
        ),
    ] {
        let actual = Digest::from_bytes(actual_bytes);
        if actual != expected {
            return Err(ReplayError::verification(format!(
                "{name} digest mismatch: expected {expected}, got {actual}"
            )));
        }
    }
    Ok(())
}

fn verify_contract_and_scenario(
    manifest: &RunBundleManifestV1,
    contract: &DeterministicRunContractV1,
    scenario: &Value,
) -> Result<(), ReplayError> {
    let calculated_run_id = contract.run_id().map_err(|error| {
        ReplayError::replay(format!("cannot calculate contract run_id: {error}"))
    })?;
    verify_equal("manifest run_id", manifest.run_id, calculated_run_id)?;

    let scenario_identity: ScenarioIdentity = scenario_field(scenario, "scenario")?;
    let model: ArtifactIdentity = scenario_field(scenario, "model")?;
    let data_pack: ArtifactIdentity = scenario_field(scenario, "data_pack")?;
    if scenario_identity != contract.scenario {
        return Err(ReplayError::verification(
            "scenario.json identity does not match contract.json",
        ));
    }
    if model != contract.model {
        return Err(ReplayError::verification(
            "scenario.json model does not match contract.json",
        ));
    }
    if data_pack != contract.data_pack {
        return Err(ReplayError::verification(
            "scenario.json data pack does not match contract.json",
        ));
    }
    let request = scenario
        .get("request")
        .ok_or_else(|| ReplayError::replay("scenario.json has no request field"))?;
    let input_digest = canonical_json_digest(request).map_err(|error| {
        ReplayError::replay(format!("cannot calculate scenario request digest: {error}"))
    })?;
    verify_equal("scenario input digest", contract.input.digest, input_digest)
}

fn scenario_field<T: DeserializeOwned>(scenario: &Value, name: &str) -> Result<T, ReplayError> {
    let value = scenario
        .get(name)
        .cloned()
        .ok_or_else(|| ReplayError::replay(format!("scenario.json has no {name} field")))?;
    serde_json::from_value(value)
        .map_err(|error| ReplayError::replay(format!("invalid scenario.json {name}: {error}")))
}

fn recalculate_metrics(
    declared: &DerivedMetricsV1,
    telemetry: &LoadedTelemetry,
) -> Result<DerivedMetricsV1, ReplayError> {
    let frames: Vec<_> = telemetry
        .records
        .iter()
        .map(|record| &record.frame)
        .collect();
    let mut recalculated = Vec::with_capacity(declared.metrics.len());
    for metric in &declared.metrics {
        let kind = match (metric.processor, metric.statistic) {
            (DerivedMetricProcessorV1::TelemetryAggregateV1, DerivedMetricStatisticV1::Maximum) => {
                TelemetryAggregateKind::Maximum
            }
        };
        let result = aggregate_telemetry_parameter(
            frames.iter().copied(),
            TelemetryAggregateConfig {
                parameter_id: metric.parameter_id,
                kind,
            },
        )
        .map_err(|error| {
            ReplayError::replay(format!("cannot recalculate metric {}: {error}", metric.id))
        })?;
        recalculated.push(DerivedMetricV1 {
            id: metric.id.clone(),
            processor: metric.processor,
            parameter_id: metric.parameter_id,
            statistic: metric.statistic,
            unit: metric.unit.clone(),
            sample_count: result.sample_count,
            value: result.value,
        });
    }
    DerivedMetricsV1::new(recalculated)
        .map_err(|error| ReplayError::replay(format!("invalid recalculated metrics: {error}")))
}

fn verify_equal<T>(name: &str, expected: T, actual: T) -> Result<(), ReplayError>
where
    T: fmt::Display + PartialEq,
{
    if expected == actual {
        Ok(())
    } else {
        Err(ReplayError::verification(format!(
            "{name} mismatch: expected {expected}, got {actual}"
        )))
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use pitgun_contract::{canonical_json_bytes, Digest, SampleValue};
    use serde_json::{json, Value};

    use super::*;
    use crate::demo::bundle::persist;
    use crate::demo::racing::{run, RacingArgs};

    const FILES: [&str; 8] = [
        "manifest.json",
        "scenario.json",
        "contract.json",
        "output.json",
        "telemetry.jsonl",
        "telemetry-summary.json",
        "metrics.json",
        "receipt.json",
    ];

    fn temporary_path(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "pitgun-replay-{label}-{}-{nonce}",
            std::process::id()
        ))
    }

    fn create_bundle() -> PathBuf {
        let root = temporary_path("base");
        let run = run(&RacingArgs {
            seed: 42,
            output: Some(root.clone()),
        })
        .expect("Racing run");
        persist(&run, Some(&root)).expect("persisted bundle");
        root
    }

    fn copy_bundle(source: &Path, label: &str) -> PathBuf {
        let destination = temporary_path(label);
        fs::create_dir(&destination).expect("copied bundle directory");
        for name in FILES {
            fs::copy(source.join(name), destination.join(name)).expect("copied bundle artifact");
        }
        destination
    }

    fn mutate_json(path: &Path, mutate: impl FnOnce(&mut Value)) {
        let bytes = fs::read(path).expect("JSON artifact");
        let mut value: Value = serde_json::from_slice(&bytes).expect("JSON value");
        mutate(&mut value);
        fs::write(
            path,
            canonical_json_bytes(&value).expect("canonical mutated JSON"),
        )
        .expect("mutated JSON artifact");
    }

    fn assert_json_mutation_rejected(
        source: &Path,
        label: &str,
        file: &str,
        expected_diagnostic: &str,
        mutate: impl FnOnce(&mut Value),
    ) {
        let root = copy_bundle(source, label);
        mutate_json(&root.join(file), mutate);
        let error = replay_and_verify(&root).expect_err("mutation must fail verification");
        assert_eq!(error.exit_code(), 50, "unexpected error: {error}");
        assert!(
            error.to_string().contains(expected_diagnostic),
            "unexpected diagnostic: {error}"
        );
        fs::remove_dir_all(root).expect("remove mutated bundle");
    }

    fn mutate_first_speed(path: &Path, value: f64, batch_ordinal: Option<u64>) {
        let bytes = fs::read(path).expect("telemetry artifact");
        let text = std::str::from_utf8(&bytes).expect("telemetry UTF-8");
        let mut records: Vec<RunBundleTelemetryRecordV1> = text
            .lines()
            .map(|line| serde_json::from_str(line).expect("telemetry record"))
            .collect();
        let first = records.first_mut().expect("first telemetry record");
        if let Some(batch_ordinal) = batch_ordinal {
            first.batch_ordinal = batch_ordinal;
        }
        let speed = first
            .frame
            .samples
            .iter_mut()
            .find(|sample| sample.parameter_id == 5005)
            .expect("speed sample");
        speed.value = SampleValue::F64(value);
        let mut mutated = Vec::new();
        for record in records {
            mutated.extend(canonical_json_bytes(&record).expect("canonical telemetry record"));
            mutated.push(b'\n');
        }
        fs::write(path, mutated).expect("mutated telemetry artifact");
    }

    #[test]
    fn verifies_an_unchanged_bundle_and_rejects_each_evidence_mutation() {
        let base = create_bundle();
        let report = replay_and_verify(&base).expect("verified base bundle");
        assert_eq!(report.frame_count, 427);
        assert_eq!(report.batch_count, 7);

        assert_json_mutation_rejected(
            &base,
            "contract",
            "contract.json",
            "contract.json digest mismatch",
            |value| value["random"]["seed"] = json!("43"),
        );
        assert_json_mutation_rejected(
            &base,
            "output",
            "output.json",
            "output.json digest mismatch",
            |value| value["total_time_ms"] = json!(1),
        );
        assert_json_mutation_rejected(
            &base,
            "summary",
            "telemetry-summary.json",
            "telemetry-summary.json digest mismatch",
            |value| value["frame_count"] = json!(428),
        );
        assert_json_mutation_rejected(
            &base,
            "metrics",
            "metrics.json",
            "metrics.json digest mismatch",
            |value| value["metrics"][0]["value"] = json!(999.0),
        );
        assert_json_mutation_rejected(
            &base,
            "receipt",
            "receipt.json",
            "receipt.json digest mismatch",
            |value| {
                value["receipt"]["output_digest"] =
                    json!(Digest::from_bytes(b"mutated output").to_string());
            },
        );
        assert_json_mutation_rejected(
            &base,
            "manifest",
            "manifest.json",
            "manifest run_id mismatch",
            |value| {
                value["run_id"] = json!(Digest::from_bytes(b"mutated run").to_string());
            },
        );

        let telemetry_root = copy_bundle(&base, "telemetry");
        mutate_first_speed(&telemetry_root.join("telemetry.jsonl"), 999.0, None);
        let telemetry_error =
            replay_and_verify(&telemetry_root).expect_err("telemetry mutation must fail");
        assert_eq!(telemetry_error.exit_code(), 50);
        assert!(telemetry_error
            .to_string()
            .contains("telemetry.jsonl digest mismatch"));
        fs::remove_dir_all(telemetry_root).expect("remove telemetry mutation");

        let order_root = copy_bundle(&base, "telemetry-order");
        mutate_first_speed(&order_root.join("telemetry.jsonl"), 999.0, Some(2));
        let order_error = replay_and_verify(&order_root).expect_err("invalid replay order");
        assert_eq!(order_error.exit_code(), 40);
        assert!(order_error.to_string().contains("batch ordinal"));
        fs::remove_dir_all(order_root).expect("remove order mutation");

        fs::remove_dir_all(base).expect("remove base bundle");
    }
}
