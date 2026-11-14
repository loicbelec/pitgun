use super::*;
use pitgun_contract::{VerificationReasonCode, VerificationStatus};
use pitgun_racing_simulator::{
    control_prototype::{ControlStep, FallbackReason, ProposalSource, reference_proposal},
    service_control::{
        RacingServiceAuthorizationResponseV1, RacingServiceDecisionV1,
        RacingServiceVerificationSubmissionV1, service_control_evidence, start_service_control,
    },
};
use pitgun_signing::VerificationKeyring;

fn fixture() -> (AppState, RacingRunAttemptAuthorizationRequestV1) {
    let mut state = test_state_for("0.14.0", "v1.8.0");
    state.service_catalog = Some(
        RacingCatalogSnapshot::from_release_dir(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../catalogs/racing/v1.7.0"),
        )
        .unwrap(),
    );
    state.service_runtime = Some(Digest::from_bytes(b"unit-test runtime bytes"));
    let mut r: pitgun_racing_simulator::RunRaceRequest = serde_json::from_str(include_str!(
        "../../../../crates/pitgun-racing-simulator/fixtures/control/monza-five-laps.json"
    ))
    .unwrap();
    r.input.era = 3;
    r.input.hz = 5.0;
    r.input.race.laps = 7;
    r.input.race.competitors[0].stint_strategy=Some(serde_json::from_value(json!({"stints":[{"tire_id":"soft","laps":3},{"tire_id":"medium","laps":2},{"tire_id":"hard","laps":2}],"pit_laps":[3,5]})).unwrap());
    let request = RacingRunAttemptAuthorizationRequestV1 {
        subject: "test.player".parse().unwrap(),
        execution_id: "018f3b78-7e9a-7d20-a5e1-4ed92f02a591".parse().unwrap(),
        seed: Seed::new(7),
        input: r.input,
        catalog_release: state
            .service_catalog
            .as_ref()
            .unwrap()
            .release_identity()
            .clone(),
    };
    (state, request)
}
fn verifier(state: &AppState) -> pitgun_verifier::RacingVerifier {
    let mut ring = VerificationKeyring::new();
    ring.insert(
        state.config.signing_key_id.clone(),
        state.signing_key.clone().unwrap(),
    );
    pitgun_verifier::RacingVerifier::new(
        ring,
        state.config.audience.clone(),
        pitgun_racing_simulator::racing_model_v3_component_candidate_identity(),
        state.policy_identity.clone(),
        state.service_catalog.clone(),
        ArtifactIdentity {
            id: "pitgun.verifier".parse().unwrap(),
            version: "1.0.0".parse().unwrap(),
            digest: Digest::from_bytes(b"test verifier"),
        },
    )
    .with_service_runtime(state.service_runtime.unwrap())
}
fn execute(
    state: &AppState,
    response: &RacingServiceAuthorizationResponseV1,
    mode: &str,
) -> (
    Vec<RacingServiceDecisionV1>,
    pitgun_racing_simulator::RaceOutput,
) {
    let catalog = state.service_catalog.as_ref().unwrap();
    let mut session = start_service_control(response, catalog).unwrap();
    let mut history = vec![];
    loop {
        match session.advance().unwrap() {
            ControlStep::AwaitingDecision(o) => {
                let mut p =
                    reference_proposal(&o, if mode == "pit" { 1 } else { 0 }, "hard").unwrap();
                if mode == "timeout" {
                    p.source = ProposalSource::Fallback {
                        reason: FallbackReason::Timeout,
                    };
                    p.decision_at_us = Some(o.window.as_ref().unwrap().deadline_at_us);
                }
                let applied = session.propose(p).unwrap();
                history.push(RacingServiceDecisionV1 {
                    observation_digest: o.digest().unwrap(),
                    applied,
                });
            }
            ControlStep::Batch(batch) => {
                for r in batch.records() {
                    if let pitgun_contract::IncrementalExecutionStreamEventV1::Complete(output) =
                        r.event()
                    {
                        return (history, output.clone());
                    }
                }
            }
        }
    }
}
#[test]
fn signed_services_keep_changed_pit_and_expiry_verify_with_distinct_final_identities() {
    let (state, request) = fixture();
    let response = build_signed_racing_service_authorization(1000, &state, request).unwrap();
    let v = verifier(&state);
    let mut results = vec![];
    for mode in ["keep", "pit", "timeout"] {
        let (history, output) = execute(&state, &response, mode);
        let s = service_control_evidence(
            response.clone(),
            state.service_catalog.as_ref().unwrap(),
            history,
            &output,
        )
        .unwrap();
        let verdict = v.verify_service_native_reference(&s, 1100).unwrap();
        assert_eq!(verdict.status, VerificationStatus::Verified);
        assert_ne!(verdict.run_id, response.signed.authorization.initial_run_id);
        assert_eq!(
            verdict.execution_id,
            response.signed.authorization.execution_id
        );
        assert_eq!(
            output.player_pit_laps,
            if mode == "pit" { vec![2] } else { vec![3, 5] }
        );
        results.push(verdict.run_id);
    }
    assert_ne!(results[0], results[1]);
    assert_ne!(results[0], results[2]);
}
#[test]
fn rechained_service_history_and_forged_effects_do_not_pass_independent_replay() {
    let (state, request) = fixture();
    let response = build_signed_racing_service_authorization(1000, &state, request).unwrap();
    let (history, output) = execute(&state, &response, "keep");
    assert_eq!(history.len(), 2);
    let v = verifier(&state);
    for kind in [
        "missing",
        "extra",
        "reordered",
        "observation",
        "late",
        "wrong-tire",
        "wrong-effect",
    ] {
        let mut h = history.clone();
        match kind {
            "missing" => {
                h.remove(0);
            }
            "extra" => h.push(h[0].clone()),
            "reordered" => h.swap(0, 1),
            "observation" => {
                h[0].observation_digest = Digest::from_bytes(b"forged observation");
                h[0].applied.proposal.observation_digest = h[0].observation_digest;
            }
            "late" => {
                h[0].applied.proposal.decision_at_us =
                    Some(h[0].applied.proposal.decision_at_us.unwrap() + 1_000_000_000)
            }
            "wrong-tire" => {
                h[0].applied.proposal.action =
                    pitgun_racing_simulator::control_prototype::Action::PitNextLap {
                        tire_id: "undelegated".into(),
                    }
            }
            "wrong-effect" => h[0].applied.pit_after_lap = Some(2),
            _ => unreachable!(),
        }
        // Recompute the history digest, final identity AND receipt like a hostile
        // client could. Hash-chain validation alone would not catch this.
        let s = service_control_evidence(
            response.clone(),
            state.service_catalog.as_ref().unwrap(),
            h,
            &output,
        )
        .unwrap();
        let verdict = v.verify_service_native_reference(&s, 1100).unwrap();
        assert_eq!(verdict.status, VerificationStatus::Rejected, "{kind}");
        assert_eq!(
            verdict.reason_code,
            Some(VerificationReasonCode::ReplayMismatch),
            "{kind}"
        );
    }
}
#[test]
fn service_signature_runtime_input_and_output_are_bound_and_routes_fail_closed() {
    let (mut state, request) = fixture();
    let response =
        build_signed_racing_service_authorization(1000, &state, request.clone()).unwrap();
    let (history, output) = execute(&state, &response, "keep");
    let good = service_control_evidence(
        response.clone(),
        state.service_catalog.as_ref().unwrap(),
        history,
        &output,
    )
    .unwrap();
    let v = verifier(&state);
    let mut cases: Vec<RacingServiceVerificationSubmissionV1> = vec![];
    let mut x = good.clone();
    x.signed_authorization.signature.replace_range(..1, "z");
    cases.push(x);
    let mut x = good.clone();
    x.decision_envelope.max_decisions = 1;
    cases.push(x);
    let mut x = good.clone();
    x.decision_envelope.runtime_digest = Digest::from_bytes(b"other runtime");
    cases.push(x);
    let mut x = good.clone();
    x.input.race.laps += 1;
    cases.push(x);
    let mut x = good.clone();
    x.output.total_time_ms += 1;
    cases.push(x);
    let mut x = good.clone();
    x.receipt.receipt.runtime.artifact_digest = Digest::from_bytes(b"forged runtime");
    cases.push(x);
    for s in cases {
        assert_eq!(
            v.verify_service_native_reference(&s, 1100).unwrap().status,
            VerificationStatus::Rejected
        );
    }
    assert_eq!(
        v.verify_service_native_reference(&good, 2_000_000)
            .unwrap()
            .reason_code,
        Some(VerificationReasonCode::AuthorizationExpired)
    );
    // Driver-mode signatures cannot authorize a service-control envelope.
    let old =
        build_signed_racing_run_attempt_authorization(1000, &state, racing_attempt_request(&state))
            .unwrap();
    let mut x = good;
    x.signed_authorization = old.signed;
    assert_eq!(
        v.verify_service_native_reference(&x, 1100).unwrap().status,
        VerificationStatus::Rejected
    );
    state.service_runtime = None;
    assert!(build_signed_racing_service_authorization(1000, &state, request.clone()).is_err());
    state.service_runtime = Some(Digest::from_bytes(b"test"));
    state.service_catalog = state.racing_catalog.clone();
    let mut incompatible = request;
    incompatible.catalog_release = state
        .service_catalog
        .as_ref()
        .unwrap()
        .release_identity()
        .clone();
    assert!(build_signed_racing_service_authorization(1000, &state, incompatible).is_err());
}

#[test]
fn service_worker_unavailability_is_pending_and_bad_authorization_never_starts_replay() {
    use pitgun_verifier::wasm_service_replay::ServiceReplayError;
    let (state, request) = fixture();
    let response = build_signed_racing_service_authorization(1000, &state, request).unwrap();
    let (history, output) = execute(&state, &response, "keep");
    let mut proof = service_control_evidence(
        response,
        state.service_catalog.as_ref().unwrap(),
        history,
        &output,
    )
    .unwrap();
    let v = verifier(&state);
    let pending = v
        .verify_service_with_replay(&proof, 1100, |_, _, _| Err(ServiceReplayError::Unavailable))
        .unwrap();
    assert_eq!(pending.status, VerificationStatus::Pending);
    assert_eq!(
        pending.reason_code,
        Some(VerificationReasonCode::DependencyUnavailable)
    );
    proof.signed_authorization.signature.replace_range(..1, "z");
    let rejected = v
        .verify_service_with_replay(&proof, 1100, |_, _, _| {
            panic!("unauthorized replay started")
        })
        .unwrap();
    assert_eq!(rejected.status, VerificationStatus::Rejected);
}
