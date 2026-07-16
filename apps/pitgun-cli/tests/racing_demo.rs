use std::process::Command;

#[test]
fn racing_demo_command_completes_offline_and_reports_simulation() {
    let output = Command::new(env!("CARGO_BIN_EXE_pitgun"))
        .args(["demo", "racing", "--seed", "42"])
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
    assert!(stdout.contains("status      SIMULATED"));
    assert!(!stdout.contains("VERIFIED"));
}
