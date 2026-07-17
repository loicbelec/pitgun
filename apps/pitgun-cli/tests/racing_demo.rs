use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn temporary_bundle(label: &str) -> std::path::PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("pitgun-cli-{label}-{}-{nonce}", std::process::id()))
}

#[test]
fn racing_demo_command_completes_offline_and_reports_simulation() {
    let bundle = temporary_bundle("integration");
    let output = Command::new(env!("CARGO_BIN_EXE_pitgun"))
        .args(["demo", "racing", "--seed", "42", "--output"])
        .arg(&bundle)
        .output()
        .expect("pitgun binary must start");

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
    assert!(stdout.contains("status      SIMULATED"));
    assert!(!stdout.contains("VERIFIED"));

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
    std::fs::remove_dir_all(bundle).expect("remove integration bundle");
}

#[test]
fn incomplete_existing_destination_fails_as_bundle_error() {
    let bundle = temporary_bundle("incomplete");
    std::fs::create_dir(&bundle).expect("create incomplete destination");

    let output = Command::new(env!("CARGO_BIN_EXE_pitgun"))
        .args(["demo", "racing", "--seed", "42", "--output"])
        .arg(&bundle)
        .output()
        .expect("pitgun binary must start");

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

    std::fs::remove_dir_all(bundle).expect("remove incomplete destination");
}
