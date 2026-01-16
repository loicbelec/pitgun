use pitgun_contract::game::v1::GameSimulationRequestV1;
use pitgun_source_physics::game::simulate_request;
use pitgun_source_physics::game::summary::{compute_summary_metrics, TelemetrySummaryMetrics};
use serde::Deserialize;
use std::fs;

#[derive(Debug, Deserialize)]
struct ExpectedSummary {
    lap_time_s: f32,
    vmax_kph: f32,
    rpm_max: f32,
    temp_max_c: f32,
}

fn assert_close(name: &str, actual: f32, expected: f32) {
    let abs_tol: f32 = 1.0e-3;
    let rel_tol: f32 = 1.0e-4;
    let abs_diff = (actual - expected).abs();
    let rel_diff = if expected == 0.0 {
        abs_diff
    } else {
        abs_diff / expected.abs()
    };

    assert!(
        abs_diff <= abs_tol || rel_diff <= rel_tol,
        "{name} mismatch: expected {expected}, got {actual} (abs={abs_diff}, rel={rel_diff})"
    );
}

#[test]
fn fixtures_summary_matches_expected() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let fixtures = ["demo-oval_default", "demo-oval_extreme"];

    for fixture in fixtures {
        let request_path = format!("{}/fixtures/requests/{}.json", manifest_dir, fixture);
        let expected_path = format!("{}/fixtures/expected/{}.json", manifest_dir, fixture);

        let request_raw = fs::read_to_string(&request_path)
            .unwrap_or_else(|err| panic!("read {}: {err}", request_path));
        let request: GameSimulationRequestV1 =
            serde_json::from_str(&request_raw).expect("parse request json");

        let result = simulate_request(&request).expect("simulate");
        let telemetry = result.telemetry.as_deref().unwrap_or(&[]);
        let summary = compute_summary_metrics(telemetry);

        let expected_raw = match fs::read_to_string(&expected_path) {
            Ok(raw) => raw,
            Err(_) => {
                let pretty =
                    serde_json::to_string_pretty(&summary).expect("serialize summary for fixtures");
                eprintln!("missing expected fixture {}\n{}", expected_path, pretty);
                panic!("missing expected fixture {}", expected_path);
            }
        };
        let expected: ExpectedSummary =
            serde_json::from_str(&expected_raw).expect("parse expected summary");

        assert_close("lap_time_s", summary.lap_time_s, expected.lap_time_s);
        assert_close("vmax_kph", summary.vmax_kph, expected.vmax_kph);
        assert_close("rpm_max", summary.rpm_max, expected.rpm_max);
        assert_close("temp_max_c", summary.temp_max_c, expected.temp_max_c);
    }
}

// Keep fixture summary layout in sync with the test expectations.
fn _assert_summary_layout(summary: &TelemetrySummaryMetrics) {
    let _ = (
        summary.lap_time_s,
        summary.vmax_kph,
        summary.rpm_max,
        summary.temp_max_c,
    );
}
