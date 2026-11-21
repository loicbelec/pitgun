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

fn fixture() -> (AppState, RacingServiceAuthorizationRequestV1) {
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
    let request = RacingServiceAuthorizationRequestV1 {
        subject: "test.player".parse().unwrap(),
        execution_id: "018f3b78-7e9a-7d20-a5e1-4ed92f02a591".parse().unwrap(),
        seed: Seed::new(7),
        runtime_digest: None,
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

#[test]
fn signed_wasm_transport_projects_evidence_and_refuses_sidecar_substitution() {
    use pitgun_racing_simulator::{
        complete_experimental_controlled_racing_session_json as complete,
        propose_experimental_controlled_racing_session_json as propose,
        pull_experimental_controlled_racing_session_json as pull,
        start_experimental_controlled_racing_session_json as start,
    };
    let (mut state, mut request) = fixture();
    state.service_catalog = Some(
        RacingCatalogSnapshot::from_release_dir(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../catalogs/racing/v1.8.1"),
        )
        .unwrap(),
    );
    request.catalog_release = state
        .service_catalog
        .as_ref()
        .unwrap()
        .release_identity()
        .clone();
    let response = build_signed_racing_service_authorization(1000, &state, request).unwrap();
    let input = response.canonical_input.clone();
    let request = pitgun_racing_simulator::RunRaceRequest {
        era: Some(input.era),
        hz: Some(input.hz),
        input,
        seed: 7,
    };
    let catalog =
        serde_json::to_string(&state.service_catalog.as_ref().unwrap().to_bundle().unwrap())
            .unwrap();
    let timeline =
        serde_json::to_string(&pitgun_racing_simulator::service_control::empty_timeline()).unwrap();
    let config = pitgun_racing_simulator::service_control::service_control_config(
        &response.decision_envelope,
        response.signed.authorization.execution_id,
    )
    .unwrap();
    let mut config = serde_json::to_value(config).unwrap();
    config["signed_service"] = json!({"signed":response.signed,"decision_envelope":response.decision_envelope,"catalog_release":response.catalog_release});
    let mut bad = config.clone();
    bad["service_windows"]["closing_margin_us"] = json!(1);
    assert!(
        start(
            serde_json::to_string(&request).unwrap(),
            catalog.clone(),
            timeline.clone(),
            bad.to_string()
        )
        .contains("mismatch")
    );
    let mut bad_request = request.clone();
    bad_request.seed = 8;
    assert!(
        start(
            serde_json::to_string(&bad_request).unwrap(),
            catalog.clone(),
            timeline.clone(),
            config.to_string()
        )
        .contains("mismatch")
    );
    let created: serde_json::Value = serde_json::from_str(&start(
        serde_json::to_string(&request).unwrap(),
        catalog,
        timeline,
        config.to_string(),
    ))
    .unwrap();
    let handle = created["handle"].as_u64().expect("signed transport start") as u32;
    loop {
        let next: serde_json::Value = serde_json::from_str(&pull(handle)).unwrap();
        assert!(next.get("error").is_none(), "{next}");
        if next["kind"] == "decision" {
            let observation: pitgun_racing_simulator::control_prototype::PlayerObservation =
                serde_json::from_value(next["observation"].clone()).unwrap();
            let p = reference_proposal(&observation, 1, "hard").unwrap();
            assert!(!propose(handle, serde_json::to_string(&p).unwrap()).contains("error"));
        } else if next["complete"] == true {
            break;
        }
    }
    let completed: serde_json::Value = serde_json::from_str(&complete(handle)).unwrap();
    let proof: RacingServiceVerificationSubmissionV1 =
        serde_json::from_value(completed["service_evidence"].clone()).unwrap();
    assert_eq!(proof.applied_history.len(), 1);
    assert_eq!(proof.output.player_pit_laps, vec![2]);
    assert_eq!(
        verifier(&state)
            .verify_service_native_reference(&proof, 1001)
            .unwrap()
            .status,
        VerificationStatus::Verified
    );
}

#[test]
fn service_admission_keeps_legacy_default_and_binds_only_operator_approved_runtime() {
    let (mut state, mut request) = fixture();
    let original = state.service_runtime.unwrap();
    let candidate = Digest::from_bytes(b"new reviewed WASM bytes");
    state.service_admitted_runtimes = vec![candidate];
    let old = build_signed_racing_service_authorization(1000, &state, request.clone()).unwrap();
    assert_eq!(old.decision_envelope.runtime_digest, original);
    request.runtime_digest = Some(candidate);
    let new = build_signed_racing_service_authorization(1000, &state, request.clone()).unwrap();
    assert_eq!(new.decision_envelope.runtime_digest, candidate);
    assert_ne!(
        old.signed.authorization.decision_envelope,
        new.signed.authorization.decision_envelope
    );
    request.runtime_digest = Some(original);
    assert_eq!(
        build_signed_racing_service_authorization(1000, &state, request.clone())
            .unwrap()
            .decision_envelope
            .runtime_digest,
        original
    );
    request.runtime_digest = Some(Digest::from_bytes(b"participant bytes"));
    assert!(matches!(
        build_signed_racing_service_authorization(1000, &state, request),
        Err(ContractError::BadRequest(_))
    ));
}

#[test]
fn service_admission_runtime_configuration_is_bounded_and_does_not_accept_paths() {
    let primary = Digest::from_bytes(b"primary");
    let candidate = Digest::from_bytes(b"candidate");
    assert!(
        service_admitted_runtimes(None, Some(primary))
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        service_admitted_runtimes(Some(&candidate.to_string()), Some(primary)).unwrap(),
        vec![candidate]
    );
    for value in [
        "".to_string(),
        "https://untrusted.invalid/runtime.wasm".into(),
        candidate.to_string() + "," + &candidate.to_string(),
        primary.to_string(),
    ] {
        assert!(service_admitted_runtimes(Some(&value), Some(primary)).is_err());
    }
    let many = (0..9)
        .map(|n| Digest::from_bytes(&[n]).to_string())
        .collect::<Vec<_>>()
        .join(",");
    assert!(service_admitted_runtimes(Some(&many), Some(primary)).is_err());
    assert!(service_admitted_runtimes(Some(&candidate.to_string()), None).is_err());
    let (_, request) = fixture();
    let mut body = json!({"subject":request.subject,"execution_id":request.execution_id,"seed":request.seed,
        "input":request.input,"catalog_release":request.catalog_release,"runtime_digest":candidate});
    assert!(serde_json::from_value::<RacingServiceAuthorizationRequestV1>(body.clone()).is_ok());
    body["runtime_url"] = json!("https://untrusted.invalid/runtime.wasm");
    assert!(serde_json::from_value::<RacingServiceAuthorizationRequestV1>(body).is_err());
}
