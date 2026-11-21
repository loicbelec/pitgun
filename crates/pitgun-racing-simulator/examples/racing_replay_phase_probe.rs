//! Offline native diagnostic, not an alternative verifier for pinned WASM.
//! Rebuilds native observations/receipts while retaining the recorded ACTIONS.
//! Native floats may differ: no native result is submitted or labelled VERIFIED.
use pitgun_contract::{Digest, IncrementalExecutionStreamEventV1, canonical_json_digest};
use pitgun_racing_contract::RacingDriverInstructionTimelineV1;
use pitgun_racing_simulator::{
    RacingCatalogSnapshot, RunRaceRequest,
    control_prototype::{
        ControlStep, LocalControlConfig, LocalControlledRacingSession, Proposal, ProposalSource,
    },
    evidence::RacingRunEvidenceV1,
    service_control::RacingServiceDecisionV1,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{fs, time::Instant};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Payload {
    request_json: String,
    catalog_bundle_json: String,
    timeline_json: String,
    config_json: String,
    history: Vec<RacingServiceDecisionV1>,
}
fn elapsed(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let file = std::env::args().nth(1).ok_or("Pass replay payload JSON")?;
    let expected_file = std::env::args()
        .nth(2)
        .ok_or("Pass expected compact evidence")?;
    let encode = !std::env::args().any(|s| s == "--no-wire");
    let start = Instant::now();
    let raw = fs::read(file)?;
    let payload: Payload = serde_json::from_slice(&raw)?;
    let expected: Value = serde_json::from_slice(&fs::read(expected_file)?)?;
    let request: RunRaceRequest = serde_json::from_str(&payload.request_json)?;
    let catalog = RacingCatalogSnapshot::from_bundle_json(&payload.catalog_bundle_json)?;
    let timeline: RacingDriverInstructionTimelineV1 = serde_json::from_str(&payload.timeline_json)?;
    let mut config: LocalControlConfig = serde_json::from_str(&payload.config_json)?;
    let native_digest = Digest::from_bytes(&fs::read(std::env::current_exe()?)?);
    config.runtime_digest = native_digest;
    let prepare_ms = elapsed(start);
    let start = Instant::now();
    let mut session = LocalControlledRacingSession::start(request, &catalog, timeline, config)?;
    let start_ms = elapsed(start);
    let work = Instant::now();
    let mut advance_ms = 0.;
    let mut encode_ms = 0.;
    let mut propose_ms = 0.;
    let mut capture_ms = 0.;
    let mut bytes = 0;
    let mut index = 0;
    let mut calls = vec![];
    let output = loop {
        let start = Instant::now();
        let step = session.advance()?;
        let duration = elapsed(start);
        advance_ms += duration;
        match step {
            ControlStep::AwaitingDecision(o) => {
                if encode {
                    let start = Instant::now();
                    let digest = o.digest()?;
                    bytes += json!({"schema_version":"pitgun.racing-control-pull/v1","handle":1,
                        "kind":"decision","observation":o,"observation_digest":digest})
                    .to_string()
                    .len();
                    encode_ms += elapsed(start);
                }
                let recorded = payload.history.get(index).ok_or("extra native window")?;
                let mut proposal = Proposal::for_observation(
                    &o,
                    format!("native-diagnostic-{index}"),
                    ProposalSource::ReferenceController,
                    recorded.applied.proposal.action.clone(),
                )?;
                if let Some(w) = &o.window {
                    proposal.decision_at_us =
                        Some((w.opened_at_us + 100_000).min(w.deadline_at_us));
                }
                let start = Instant::now();
                session.propose(proposal)?;
                propose_ms += elapsed(start);
                index += 1;
                calls.push(json!({"kind":"decision","advanceMs":duration}));
            }
            ControlStep::Batch(batch) => {
                let start = Instant::now();
                let out = batch.records().iter().find_map(|r| match r.event() {
                    IncrementalExecutionStreamEventV1::Complete(o) => Some(o.clone()),
                    _ => None,
                });
                capture_ms += elapsed(start);
                let mut encoded_bytes = 0;
                let start = Instant::now();
                if encode {
                    let mut response = json!({"schema_version":"pitgun.racing-control-pull/v1",
                        "handle":1,"kind":"batch","complete":out.is_some(),"batch":batch});
                    if session.has_window_prefix() {
                        response["control_prefix"] = json!(true);
                    }
                    encoded_bytes = response.to_string().len();
                    bytes += encoded_bytes;
                }
                let encoded_ms = elapsed(start);
                encode_ms += encoded_ms;
                calls.push(
                    json!({"kind":"batch","advanceMs":duration,"encodeMs":encoded_ms,
                    "wireBytes":encoded_bytes,"complete":out.is_some()}),
                );
                if let Some(out) = out {
                    break out;
                }
            }
        }
    };
    if index != payload.history.len() {
        return Err("missing native window".into());
    }
    let work_ms = elapsed(work);
    let start = Instant::now();
    let result_digest = canonical_json_digest(&output)?;
    let digest_ms = elapsed(start);
    let start = Instant::now();
    let evidence = RacingRunEvidenceV1::from_race_output(&output)?;
    let evidence_ms = elapsed(start);
    let compact = json!({"output":evidence.output,"telemetry_summary":evidence.telemetry_summary});
    let start = Instant::now();
    let complete_bytes = if encode {
        json!({"schema_version":"pitgun.racing-control-completion/v1","handle":1,
            "result":output,"canonical_result_digest":result_digest,"journal":session.journal(),
            "native_evidence":compact})
        .to_string()
        .len()
    } else {
        0
    };
    let complete_encode_ms = elapsed(start);
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "schemaVersion":"pitgun.native-replay-phase-profile/v1",
            "scope":"Native diagnostic with reconstructed observations/receipts and recorded actions; not pinned-WASM replay or a verifier verdict",
            "encodeWire":encode,"nativeArtifactDigest":native_digest,"payloadDigest":Digest::from_bytes(&raw),
            "prepareMs":prepare_ms,"startMs":start_ms,"workMs":work_ms,"advanceMs":advance_ms,
            "batchEncodeMs":encode_ms,"proposeMs":propose_ms,"captureMs":capture_ms,
            "canonicalDigestMs":digest_ms,"evidenceMs":evidence_ms,"completeEncodeMs":complete_encode_ms,
            "batchWireBytes":bytes,"completeWireBytes":complete_bytes,"compactBytes":compact.to_string().len(),
            "resultDigest":result_digest,"nativeCompactEqualsWasm":compact==expected,"decisions":index,"calls":calls,
            "limitations":["advance includes orchestration, hashing and solver work, not pure physics",
                "native uses its own artifact identity and fresh receipts; it cannot verify an existing WASM signature",
                "wire encoding mirrors production batch/completion construction but omits start/proposal response encoding",
                "no-wire is an offline diagnostic, not a production export or optimization"]
        }))?
    );
    Ok(())
}
