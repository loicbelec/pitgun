use super::*;
use crate::control_prototype::{Action, PlayerObservation, ProposalSource};
use pitgun_contract::Digest;
use serde_json::{Value, json};

fn fixture() -> (String, String, String, String) {
    (
        include_str!("../../fixtures/control/monza-five-laps.json").into(),
        serde_json::to_string(
            &RacingCatalogSnapshot::embedded_model_v3_fuel_contract()
                .unwrap()
                .to_bundle()
                .unwrap(),
        )
        .unwrap(),
        json!({"schema_version":"pitgun.racing-driver-instruction-timeline/v1", "events":[]})
            .to_string(),
        json!({"execution_id":"transport-test", "runtime_digest":Digest::from_bytes(b"test"),
        "decision_laps":[1,3,4], "allowed_tire_ids":["hard","soft"]})
        .to_string(),
    )
}

fn start() -> u32 {
    let (a, b, c, d) = fixture();
    let response: Value = serde_json::from_str(&start_experimental_controlled_racing_session_json(
        a, b, c, d,
    ))
    .unwrap();
    assert!(response.get("error").is_none(), "{response}");
    response["handle"].as_u64().unwrap() as u32
}

#[test]
fn transport_accepts_full_resolution_request_budget_and_refuses_overflow() {
    let (request, catalog, timeline, config) = fixture();
    let padded = format!(
        "{request}{}",
        " ".repeat(MAX_CONTROL_REQUEST_BYTES - request.len())
    );
    let accepted: Value = serde_json::from_str(&start_experimental_controlled_racing_session_json(
        padded.clone(),
        catalog.clone(),
        timeline.clone(),
        config.clone(),
    ))
    .unwrap();
    let handle = accepted["handle"]
        .as_u64()
        .expect("1 MiB request should be admitted") as u32;
    assert!(!release_experimental_controlled_racing_session_json(handle).contains("error"));
    let rejected: Value = serde_json::from_str(&start_experimental_controlled_racing_session_json(
        padded + " ",
        catalog,
        timeline,
        config,
    ))
    .unwrap();
    assert!(
        rejected["error"]
            .as_str()
            .unwrap()
            .contains("1048576 bytes")
    );
}

fn pull(h: u32) -> Value {
    serde_json::from_str(&pull_experimental_controlled_racing_session_json(h)).unwrap()
}
fn submit(h: u32, p: &Proposal) -> Value {
    serde_json::from_str(&propose_experimental_controlled_racing_session_json(
        h,
        serde_json::to_string(p).unwrap(),
    ))
    .unwrap()
}
fn observe(h: u32) -> Value {
    loop {
        let r = pull(h);
        assert!(r.get("error").is_none(), "{r}");
        if r["kind"] == "decision" {
            return r;
        }
    }
}
fn proposal(r: &Value, action: Action, key: &str) -> Proposal {
    // Preserve the digest emitted by the runtime; consumers never reconstruct
    // floating-point observation hashes through JSON in another language.
    let o: PlayerObservation = serde_json::from_value(r["observation"].clone()).unwrap();
    Proposal {
        version: o.version,
        run_digest: o.run_digest,
        decision_sequence: o.decision_sequence,
        completed_laps: o.completed_laps,
        observation_digest: serde_json::from_value(r["observation_digest"].clone()).unwrap(),
        idempotency_key: key.into(),
        source: ProposalSource::ReferenceController,
        action,
        decision_at_us: None,
    }
}

#[test]
fn transport_freezes_rejects_retries_and_consumes_terminal_handle() {
    let h = start();
    let r = observe(h);
    for _ in 0..4 {
        assert_eq!(pull(h), r);
    }
    let p = proposal(
        &r,
        Action::PitNextLap {
            tire_id: "hard".into(),
        },
        "pit",
    );
    let mut bad = p.clone();
    bad.completed_laps = 0;
    assert!(submit(h, &bad)["error"].is_string());
    assert_eq!(pull(h), r);
    let receipt = submit(h, &p);
    assert_eq!(receipt["receipt"]["pit_after_lap"], 2);
    assert_eq!(submit(h, &p), receipt);
    let mut conflict = p.clone();
    conflict.action = Action::KeepPlan;
    assert!(submit(h, &conflict)["error"].is_string());
    loop {
        let r = pull(h);
        assert!(r.get("error").is_none(), "{r}");
        if r["kind"] == "decision" {
            let seq = r["observation"]["decision_sequence"].as_u64().unwrap();
            assert!(
                submit(h, &proposal(&r, Action::KeepPlan, &format!("keep-{seq}")))
                    .get("error")
                    .is_none()
            );
        } else if r["complete"] == true {
            break;
        }
    }
    assert!(pull(h)["error"].is_string());
    assert_eq!(submit(h, &p), receipt);
    let done: Value =
        serde_json::from_str(&complete_experimental_controlled_racing_session_json(h)).unwrap();
    assert_eq!(done["result"]["player_pit_laps"], json!([2]));
    let compact: crate::evidence::RacingOutputV1 =
        serde_json::from_value(done["native_evidence"]["output"].clone()).unwrap();
    assert_eq!(compact.player_pit_laps, vec![2]);
    let summary: pitgun_contract::TelemetrySummaryV1 =
        serde_json::from_value(done["native_evidence"]["telemetry_summary"].clone()).unwrap();
    assert!(summary.frame_count() > 0);
    assert!(serde_json::to_vec(&done["native_evidence"]).unwrap().len() < 65536);
    assert!(
        done["native_evidence"]["output"]
            .get("player_batches")
            .is_none()
    );
    assert_eq!(
        done["journal"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| e["kind"] == "applied")
            .count(),
        3
    );
    assert!(
        done["canonical_result_digest"]
            .as_str()
            .unwrap()
            .starts_with("sha256:")
    );
    assert!(pull(h)["error"].is_string());
}

#[test]
fn transport_limits_bad_inputs_sessions_and_submission_journal() {
    let (a, b, c, mut d) = fixture();
    let mut cfg: Value = serde_json::from_str(&d).unwrap();
    cfg["decision_laps"] = json!((1..=65).collect::<Vec<_>>());
    d = cfg.to_string();
    let bad: Value = serde_json::from_str(&start_experimental_controlled_racing_session_json(
        a, b, c, d,
    ))
    .unwrap();
    assert!(bad["error"].as_str().unwrap().contains("limits"));
    let handles: Vec<_> = (0..MAX_SESSIONS).map(|_| start()).collect();
    let (a, b, c, d) = fixture();
    assert!(
        start_experimental_controlled_racing_session_json(a, b, c, d).contains("session limit")
    );
    let h = handles[0];
    let r = observe(h);
    for _ in 0..MAX_SUBMISSIONS {
        assert!(
            propose_experimental_controlled_racing_session_json(h, "{".into()).contains("error")
        );
    }
    assert!(
        submit(h, &proposal(&r, Action::KeepPlan, "budget"))["error"]
            .as_str()
            .unwrap()
            .contains("budget")
    );
    assert_eq!(pull(h), r);
    for h in handles {
        assert!(!release_experimental_controlled_racing_session_json(h).contains("error"));
    }
}

#[test]
fn incomplete_completion_and_release_do_not_produce_a_result() {
    let h = start();
    assert!(complete_experimental_controlled_racing_session_json(h).contains("not complete"));
    assert!(
        propose_experimental_controlled_racing_session_json(h, "x".repeat(4097))
            .contains("exceeds")
    );
    assert!(pull(h).get("error").is_none());
    assert!(!release_experimental_controlled_racing_session_json(h).contains("error"));
    assert!(release_experimental_controlled_racing_session_json(h).contains("unknown"));
}

#[test]
fn live_window_transport_marks_one_unaffected_prefix_and_accepts_only_the_bound_clock() {
    let (a, b, c, d) = fixture();
    let mut config: Value = serde_json::from_str(&d).unwrap();
    config["service_windows"] =
        json!({"schema_version":"pitgun.racing-service-window/v1","closing_margin_us":5_000_000});
    config["decision_laps"] = json!([1]);
    let start: Value = serde_json::from_str(&start_experimental_controlled_racing_session_json(
        a,
        b,
        c,
        config.to_string(),
    ))
    .unwrap();
    let h = start["handle"].as_u64().unwrap() as u32;
    let mut prefix_count = 0;
    let boundary = loop {
        let r = pull(h);
        assert!(r.get("error").is_none(), "{r}");
        if r["control_prefix"] == true {
            prefix_count += 1;
            assert_eq!(r["kind"], "batch");
        }
        if r["kind"] == "decision" {
            break r;
        }
    };
    assert_eq!(prefix_count, 1);
    assert_eq!(pull(h), boundary);
    let window = &boundary["observation"]["window"];
    let mut late = proposal(&boundary, Action::KeepPlan, "late-clock");
    late.decision_at_us = window["deadline_at_us"].as_u64();
    assert!(submit(h, &late).get("error").is_some());
    let mut keep = proposal(&boundary, Action::KeepPlan, "expiry");
    keep.source = ProposalSource::Fallback {
        reason: crate::control_prototype::FallbackReason::Timeout,
    };
    keep.decision_at_us = window["deadline_at_us"].as_u64();
    assert!(submit(h, &keep).get("error").is_none());
    loop {
        let r = pull(h);
        assert!(r.get("error").is_none(), "{r}");
        if r["complete"] == true {
            break;
        }
    }
    let result: Value =
        serde_json::from_str(&complete_experimental_controlled_racing_session_json(h)).unwrap();
    assert_eq!(result["result"]["player_pit_laps"], json!([3]));
}
