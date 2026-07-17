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
