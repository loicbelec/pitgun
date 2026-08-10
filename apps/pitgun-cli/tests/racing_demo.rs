use std::fs;
use std::path::Path;
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use pitgun_contract::{canonical_json_bytes, RunBundleTelemetryRecordV1, SampleValue};
use serde_json::{json, Value};

fn temporary_bundle(label: &str) -> std::path::PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("pitgun-cli-{label}-{}-{nonce}", std::process::id()))
}

fn run_demo(bundle: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pitgun"))
        .args(["demo", "racing", "--seed", "42", "--output"])
        .arg(bundle)
        .output()
        .expect("pitgun demo process must start")
}

fn run_replay(bundle: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pitgun"))
        .arg("replay")
        .arg(bundle)
        .output()
        .expect("pitgun replay process must start")
}

fn racing_scenario() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("scenarios/racing-demo-v1.json")
}

fn racing_batch_scenarios() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("scenarios/racing-batch-v1")
}

fn run_batch(scenario: &Path, seed: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pitgun"))
        .args(["run", "racing", "--scenario"])
        .arg(scenario)
        .args(["--seed", seed])
        .output()
        .expect("pitgun batch process must start")
}

fn assert_replay_failure(bundle: &Path, exit_code: i32, diagnostic: &str) {
    let rejected = run_replay(bundle);
    assert_eq!(
        rejected.status.code(),
        Some(exit_code),
        "unexpected status; stderr:\n{}",
        String::from_utf8_lossy(&rejected.stderr)
    );
    assert!(rejected.stdout.is_empty());
    let stderr = String::from_utf8(rejected.stderr).expect("failure stderr must be UTF-8");
    assert!(
        stderr.contains(diagnostic),
        "missing diagnostic {diagnostic:?} in:\n{stderr}"
    );
    assert!(!stderr.contains("VERIFIED"));
}

fn mutate_json(path: &Path, mutate: impl FnOnce(&mut Value)) {
    let mut value: Value =
        serde_json::from_slice(&fs::read(path).expect("JSON artifact")).expect("valid JSON");
    mutate(&mut value);
    fs::write(
        path,
        canonical_json_bytes(&value).expect("canonical mutated JSON"),
    )
    .expect("write mutated JSON");
}

fn mutate_first_telemetry(path: &Path, speed: f64, batch_ordinal: Option<u64>) {
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
    let speed_sample = first
        .frame
        .samples
        .iter_mut()
        .find(|sample| sample.parameter_id == 5005)
        .expect("speed sample");
    speed_sample.value = SampleValue::F64(speed);

    let mut mutated = Vec::new();
    for record in records {
        mutated.extend(canonical_json_bytes(&record).expect("canonical telemetry record"));
        mutated.push(b'\n');
    }
    fs::write(path, mutated).expect("write mutated telemetry");
}

#[test]
fn distributed_binary_reports_its_version() {
    let output = Command::new(env!("CARGO_BIN_EXE_pitgun"))
        .arg("--version")
        .output()
        .expect("pitgun version process must start");

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        String::from_utf8(output.stdout).expect("version stdout must be UTF-8"),
        format!("pitgun {}\n", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn racing_demo_completes_the_verified_loop_and_replays_in_a_fresh_process() {
    let bundle = temporary_bundle("integration");
    let output = run_demo(&bundle);

    assert!(
        output.status.success(),
        "pitgun failed with stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "successful demo must keep stderr quiet"
    );

    let stdout = String::from_utf8(output.stdout).expect("stdout must be UTF-8");
    assert!(stdout.contains("scenario    racing.single-lap@1.0.0"));
    assert!(stdout.contains("seed        42"));
    assert!(stdout.contains(
        "run_id      sha256:89dc458a7460056dd519f5cda74c55c2b2b47f7091f1309ae10d11a2eb46a64a"
    ));
    assert!(stdout.contains("frames      427 in 7 batches"));
    assert!(stdout.contains("metric      racing.observed-maximum-speed = "));
    assert!(stdout.contains(" km/h"));
    assert!(stdout.contains(&format!("bundle      {} (created)", bundle.display())));
    assert!(stdout.contains("replay      OK"));
    assert!(stdout.contains("verification VERIFIED"));
    assert!(stdout.ends_with(
        "VERIFIED sha256:89dc458a7460056dd519f5cda74c55c2b2b47f7091f1309ae10d11a2eb46a64a\n"
    ));

    for name in [
        "manifest.json",
        "scenario.json",
        "contract.json",
        "output.json",
        "telemetry.jsonl",
        "telemetry-summary.json",
        "metrics.json",
        "receipt.json",
    ] {
        assert!(bundle.join(name).is_file(), "missing bundle file {name}");
    }

    let replay = run_replay(&bundle);
    assert!(
        replay.status.success(),
        "fresh replay failed with stderr:\n{}",
        String::from_utf8_lossy(&replay.stderr)
    );
    assert!(replay.stderr.is_empty());
    let replay_stdout = String::from_utf8(replay.stdout).expect("replay stdout must be UTF-8");
    assert!(replay_stdout.contains("telemetry   427 frames in 7 batches"));
    assert!(replay_stdout.contains("metric      racing.observed-maximum-speed = 355.60 km/h"));
    assert!(replay_stdout.ends_with(
        "VERIFIED sha256:89dc458a7460056dd519f5cda74c55c2b2b47f7091f1309ae10d11a2eb46a64a\n"
    ));

    fs::remove_dir_all(bundle).expect("remove integration bundle");
}

#[test]
fn racing_batch_emits_byte_identical_compact_results() {
    let first = run_batch(&racing_scenario(), "42");
    let second = run_batch(&racing_scenario(), "42");

    assert!(
        first.status.success(),
        "first batch failed with stderr:\n{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert!(
        second.status.success(),
        "second batch failed with stderr:\n{}",
        String::from_utf8_lossy(&second.stderr)
    );
    assert!(first.stderr.is_empty());
    assert!(second.stderr.is_empty());
    assert_eq!(first.stdout, second.stdout);

    let result: Value = serde_json::from_slice(&first.stdout).expect("compact result JSON");
    assert_eq!(result["schema_version"], "pitgun.batch-run-result/v1");
    assert_eq!(result["runtime"]["name"], "pitgun-cli");
    assert_eq!(result["scenario"]["id"], "racing.single-lap");
    assert_eq!(
        result["configuration_id"],
        "sha256:12a4207b2c26c814763a2a488054f7421e7cc3836a35e26fc16d96477c8744d7"
    );
    assert_eq!(
        result["run_id"],
        "sha256:89dc458a7460056dd519f5cda74c55c2b2b47f7091f1309ae10d11a2eb46a64a"
    );
    assert_eq!(result["summary"]["telemetry_frame_count"], 427);
    assert_eq!(result["summary"]["telemetry_batch_count"], 7);
    assert_eq!(
        result["summary"]["metrics"]["metrics"][0]["id"],
        "racing.observed-maximum-speed"
    );
    assert!(result["summary"].get("player_batches").is_none());
}

#[test]
fn racing_batch_fixture_executes_distinct_materialized_configurations() {
    let expected_configuration_ids = std::collections::BTreeMap::from([
        (
            "balanced",
            "sha256:12a4207b2c26c814763a2a488054f7421e7cc3836a35e26fc16d96477c8744d7",
        ),
        (
            "high-downforce",
            "sha256:f61df371a9cb5470410842e52a02e97b6763d71cbbe28b1f27b6cb7a83534611",
        ),
        (
            "low-downforce",
            "sha256:0dae0f776d53d34a4806b0b7a013c52b5bdf0e7e751560ffd0e4ea7b563651bf",
        ),
    ]);
    let mut scenarios: Vec<_> = fs::read_dir(racing_batch_scenarios())
        .expect("batch fixture directory")
        .map(|entry| entry.expect("batch fixture entry").path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect();
    scenarios.sort();
    assert_eq!(scenarios.len(), 3);

    let mut configuration_ids = std::collections::BTreeSet::new();
    let mut run_ids = std::collections::BTreeSet::new();
    for scenario in scenarios {
        let first = run_batch(&scenario, "42");
        let second = run_batch(&scenario, "42");
        assert!(
            first.status.success(),
            "{} failed with stderr:\n{}",
            scenario.display(),
            String::from_utf8_lossy(&first.stderr)
        );
        assert_eq!(first.stdout, second.stdout);

        let result: Value = serde_json::from_slice(&first.stdout).expect("batch result JSON");
        let family = scenario
            .file_stem()
            .and_then(|value| value.to_str())
            .expect("scenario family");
        assert_eq!(
            result["configuration_id"].as_str(),
            expected_configuration_ids.get(family).copied()
        );
        configuration_ids.insert(
            result["configuration_id"]
                .as_str()
                .expect("configuration identity")
                .to_owned(),
        );
        run_ids.insert(result["run_id"].as_str().expect("run identity").to_owned());
    }

    assert_eq!(configuration_ids.len(), 3);
    assert_eq!(run_ids.len(), 3);
}

#[test]
fn racing_batch_optionally_persists_result_and_full_bundle() {
    let root = temporary_bundle("batch-artifacts");
    let result_path = root.join("result.json");
    let bundle_path = root.join("bundle");
    let output = Command::new(env!("CARGO_BIN_EXE_pitgun"))
        .args(["run", "racing", "--scenario"])
        .arg(racing_scenario())
        .args(["--seed", "42", "--result"])
        .arg(&result_path)
        .arg("--bundle")
        .arg(&bundle_path)
        .output()
        .expect("pitgun batch process must start");

    assert!(
        output.status.success(),
        "batch failed with stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
    let result: Value =
        serde_json::from_slice(&fs::read(&result_path).expect("result file")).expect("result JSON");
    assert_eq!(result["schema_version"], "pitgun.batch-run-result/v1");
    assert!(bundle_path.join("manifest.json").is_file());
    assert!(bundle_path.join("telemetry.jsonl").is_file());

    fs::remove_dir_all(root).expect("remove batch artifacts");
}

#[test]
fn racing_batch_emits_structured_contract_failures() {
    let root = temporary_bundle("batch-invalid");
    fs::create_dir(&root).expect("create invalid fixture root");
    let scenario = root.join("invalid.json");
    fs::write(&scenario, br#"{"schema_version":"unknown"}"#).expect("write invalid scenario");

    let output = run_batch(&scenario, "42");
    let repeated = run_batch(&scenario, "42");

    assert_eq!(output.status.code(), Some(10));
    assert!(output.stdout.is_empty());
    assert_eq!(output.status.code(), repeated.status.code());
    assert_eq!(output.stdout, repeated.stdout);
    assert_eq!(output.stderr, repeated.stderr);
    let error: Value = serde_json::from_slice(&output.stderr).expect("structured batch error");
    assert_eq!(error["schema_version"], "pitgun.batch-run-error/v1");
    assert_eq!(error["phase"], "contract");
    assert_eq!(error["code"], "invalid_scenario");

    fs::remove_dir_all(root).expect("remove invalid fixture root");
}

#[test]
fn racing_replay_rejects_contract_output_and_telemetry_mutations() {
    let bundle = temporary_bundle("mutations");
    let output = run_demo(&bundle);
    assert!(
        output.status.success(),
        "pitgun failed with stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let contract_path = bundle.join("contract.json");
    let original_contract = fs::read(&contract_path).expect("contract artifact");
    mutate_json(&contract_path, |value| {
        value["random"]["seed"] = json!("43");
    });
    assert_replay_failure(&bundle, 50, "contract.json digest mismatch");
    fs::write(&contract_path, original_contract).expect("restore contract");

    let output_path = bundle.join("output.json");
    let original_output = fs::read(&output_path).expect("output artifact");
    mutate_json(&output_path, |value| {
        value["total_time_ms"] = json!(1);
    });
    assert_replay_failure(&bundle, 50, "output.json digest mismatch");
    fs::write(&output_path, original_output).expect("restore output");

    let telemetry_path = bundle.join("telemetry.jsonl");
    let original_telemetry = fs::read(&telemetry_path).expect("telemetry artifact");
    mutate_first_telemetry(&telemetry_path, 999.0, None);
    assert_replay_failure(&bundle, 50, "telemetry.jsonl digest mismatch");
    fs::write(&telemetry_path, &original_telemetry).expect("restore telemetry");

    mutate_first_telemetry(&telemetry_path, 999.0, Some(2));
    assert_replay_failure(&bundle, 40, "non-contiguous batch ordinal");

    fs::remove_dir_all(bundle).expect("remove mutation bundle");
}

#[test]
fn incomplete_existing_destination_fails_as_bundle_error() {
    let bundle = temporary_bundle("incomplete");
    std::fs::create_dir(&bundle).expect("create incomplete destination");

    let output = run_demo(&bundle);

    assert_eq!(output.status.code(), Some(30));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).expect("stderr must be UTF-8");
    assert!(stderr.contains("Run bundle failed"));
    assert!(stderr.contains("manifest.json"));
    assert!(!stderr.contains("VERIFIED"));
    assert!(
        bundle.is_dir(),
        "existing destination must remain untouched"
    );

    fs::remove_dir_all(bundle).expect("remove incomplete destination");
}
