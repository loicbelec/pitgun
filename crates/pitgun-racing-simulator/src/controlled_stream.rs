//! Experimental local-only handle transport. Separate from signed driver-mode
//! execution. An explicit signed sidecar produces service evidence; omission
//! preserves legacy local handles and their underlying simulation identity.

use super::*;
use crate::control_prototype::{
    ControlStep, LocalControlConfig, LocalControlledRacingSession, Proposal,
};

const MAX_SESSIONS: usize = 4;
const MAX_SUBMISSIONS: usize = 256;
// The request includes the full-resolution circuit profile; 256 KiB excluded
// legitimate tracks such as Silverstone. Keep a bounded, explicit transport cap.
pub(crate) const MAX_CONTROL_REQUEST_BYTES: usize = 1024 * 1024;

struct Session {
    engine: LocalControlledRacingSession,
    output: Option<RaceOutput>,
    replay_evidence: Option<evidence::RacingRunEvidenceV1>,
    transport: Option<OutputTransport>,
    submissions: usize,
    signed_service: Option<(
        service_control::RacingServiceAuthorizationResponseV1,
        RacingCatalogSnapshot,
    )>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum OutputTransport {
    Presentation,
    Replay,
}

thread_local! {
    static SESSIONS: RefCell<BTreeMap<u32, Session>> = const { RefCell::new(BTreeMap::new()) };
    static NEXT: Cell<u32> = const { Cell::new(1) };
}

fn bounded<T: serde::de::DeserializeOwned>(raw: &str, limit: usize) -> Result<T, String> {
    if raw.len() > limit {
        return Err(format!("control JSON exceeds {limit} bytes"));
    }
    serde_json::from_str(raw).map_err(|e| e.to_string())
}

/// Pins an experimental fuel-contract session. No lap is calculated here.
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn start_experimental_controlled_racing_session_json(
    request_json: String,
    catalog_bundle_json: String,
    timeline_json: String,
    config_json: String,
) -> String {
    let create = || -> Result<Session, String> {
        let request: RunRaceRequest = bounded(&request_json, MAX_CONTROL_REQUEST_BYTES)?;
        let mut config_value: serde_json::Value = bounded(&config_json, 16 * 1024)?;
        let service_metadata = config_value
            .as_object_mut()
            .ok_or("control config must be an object")?
            .remove("signed_service");
        let config: LocalControlConfig =
            serde_json::from_value(config_value).map_err(|e| e.to_string())?;
        let timeline: RacingDriverInstructionTimelineV1 = bounded(&timeline_json, 64 * 1024)?;
        if catalog_bundle_json.len() > 16 * 1024 * 1024 {
            return Err("control catalog exceeds 16 MiB".into());
        }
        if request.input.race.laps > 128
            || request.input.race.competitors.len() > 16
            || config.decision_laps.len() > 64
            || config.allowed_tire_ids.len() > 16
            || !(1.0..=20.0).contains(&request.hz.unwrap_or(request.input.hz))
        {
            return Err("experimental control limits: 128 laps, 16 competitors, 64 windows, 16 tires, 1..20 Hz".into());
        }
        let snapshot = RacingCatalogSnapshot::from_bundle_json(&catalog_bundle_json)
            .map_err(|e| e.to_string())?;
        let signed_service = if let Some(mut metadata) = service_metadata {
            if metadata.as_object().map(|v| v.len()) != Some(3)
                || metadata.get("canonical_input").is_some()
            {
                return Err("invalid signed service sidecar".into());
            }
            metadata
                .as_object_mut()
                .ok_or("invalid signed service metadata")?
                .insert(
                    "canonical_input".into(),
                    serde_json::to_value(&request.input).map_err(|e| e.to_string())?,
                );
            let response: service_control::RacingServiceAuthorizationResponseV1 =
                serde_json::from_value(metadata).map_err(|e| e.to_string())?;
            let expected = service_control::validate_service_start(&response, &snapshot)?;
            if canonical_json_digest(&config).map_err(|e| e.to_string())?
                != canonical_json_digest(&expected).map_err(|e| e.to_string())?
                || request.seed
                    != response
                        .signed
                        .authorization
                        .initial_contract
                        .random
                        .seed
                        .get()
                || request.era != Some(request.input.era)
                || request.hz != Some(request.input.hz)
                || !timeline.events.is_empty()
            {
                return Err("signed service request/config/timeline mismatch".into());
            }
            Some((response, snapshot.clone()))
        } else {
            None
        };
        Ok(Session {
            engine: LocalControlledRacingSession::start(request, &snapshot, timeline, config)?,
            output: None,
            replay_evidence: None,
            transport: None,
            submissions: 0,
            signed_service,
        })
    };
    SESSIONS.with(|sessions| {
        let mut sessions = sessions.borrow_mut();
        if sessions.len() >= MAX_SESSIONS {
            return json_error("experimental control session limit reached");
        }
        let engine = match create() {
            Ok(engine) => engine,
            Err(error) => return json_error(&error),
        };
        NEXT.with(|next| {
            let mut handle = next.get().max(1);
            while sessions.contains_key(&handle) {
                handle = handle.wrapping_add(1).max(1);
            }
            sessions.insert(handle, engine);
            next.set(handle.wrapping_add(1).max(1));
            serde_json::json!({"schema_version":"pitgun.racing-control-start/v1", "handle":handle})
                .to_string()
        })
    })
}

/// A waiting result is stable: pulling it again does not advance the solver.
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn pull_experimental_controlled_racing_session_json(handle: u32) -> String {
    pull_controlled_session(handle, OutputTransport::Presentation)
}

/// Same engine/observations/proposal validation, without presentation batches
/// crossing the ABI. Experimental; does not replace a distributed artifact.
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn pull_experimental_controlled_racing_replay_json(handle: u32) -> String {
    pull_controlled_session(handle, OutputTransport::Replay)
}

fn pull_controlled_session(handle: u32, transport: OutputTransport) -> String {
    SESSIONS.with(|sessions| {
        let mut sessions = sessions.borrow_mut();
        let Some(session) = sessions.get_mut(&handle) else { return json_error("unknown control handle"); };
        if session.transport.is_some_and(|chosen| chosen != transport) {
            return json_error("cannot mix presentation and replay transports");
        }
        if session.output.is_some() || session.replay_evidence.is_some() { return json_error("control session already complete"); }
        session.transport = Some(transport);
        let step = match session.engine.advance() { Ok(step) => step, Err(error) => return json_error(&error) };
        match step {
            ControlStep::AwaitingDecision(observation) => {
                let digest = match observation.digest() { Ok(d) => d, Err(e) => return json_error(&e) };
                serde_json::json!({"schema_version":"pitgun.racing-control-pull/v1", "handle":handle,
                    "kind":"decision", "observation":observation, "observation_digest":digest}).to_string()
            }
            ControlStep::Batch(batch) => {
                if transport == OutputTransport::Replay {
                    // Borrow the terminal result once to project governed evidence.
                    // Avoid its full clone and the presentation JSON Value/tree.
                    for record in batch.records() {
                        if let IncrementalExecutionStreamEventV1::Complete(output) = record.event() {
                            let projected = match evidence::RacingRunEvidenceV1::from_race_output(output) {
                                Ok(e) => e, Err(e) => return json_error(&e.to_string()),
                            };
                            session.replay_evidence = Some(projected);
                            break;
                        }
                    }
                    let mut response = serde_json::json!({"schema_version":"pitgun.racing-control-replay-pull/v1",
                        "handle":handle,"kind":"batch","complete":session.replay_evidence.is_some()});
                    if session.engine.has_window_prefix() { response["control_prefix"] = serde_json::json!(true); }
                    return response.to_string();
                }
                session.output = batch.records().iter().find_map(|record| match record.event() {
                    IncrementalExecutionStreamEventV1::Complete(output) => Some(output.clone()), _ => None,
                });
                let mut response = serde_json::json!({"schema_version":"pitgun.racing-control-pull/v1", "handle":handle,
                    "kind":"batch", "complete":session.output.is_some(), "batch":batch});
                if session.engine.has_window_prefix() { response["control_prefix"] = serde_json::json!(true); }
                response.to_string()
            }
        }
    })
}

/// Accept/reject is a domain verdict, not permission for the host to pull again.
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn propose_experimental_controlled_racing_session_json(
    handle: u32,
    proposal_json: String,
) -> String {
    SESSIONS.with(|sessions| {
        let mut sessions = sessions.borrow_mut();
        let Some(session) = sessions.get_mut(&handle) else { return json_error("unknown control handle"); };
        if session.submissions >= MAX_SUBMISSIONS { return json_error("control submission budget exhausted"); }
        session.submissions += 1;
        let proposal: Proposal = match bounded(&proposal_json, 4096) { Ok(p) => p, Err(e) => return json_error(&e) };
        // Identical retries may return their original receipt even after finish.
        match session.engine.propose(proposal) {
            Ok(receipt) => serde_json::json!({"schema_version":"pitgun.racing-control-receipt/v1", "handle":handle,"receipt":receipt}).to_string(),
            Err(e) => json_error(&e),
        }
    })
}

/// Consumes the terminal handle. Journal and canonical digest are generated by
/// the same runtime that applied the instructions, not by JavaScript physics.
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn complete_experimental_controlled_racing_session_json(handle: u32) -> String {
    SESSIONS.with(|sessions| {
        let mut sessions = sessions.borrow_mut();
        let Some(session) = sessions.get(&handle) else { return json_error("unknown control handle"); };
        if session.transport == Some(OutputTransport::Replay) {
            return json_error("cannot mix presentation and replay transports");
        }
        let Some(output) = &session.output else { return json_error("control session is not complete"); };
        let digest = match canonical_json_digest(output) { Ok(d) => d, Err(e) => return json_error(&e.to_string()) };
        let evidence=match evidence::RacingRunEvidenceV1::from_race_output(output){Ok(v)=>v,Err(e)=>return json_error(&e.to_string())};
        let mut response = serde_json::json!({"schema_version":"pitgun.racing-control-completion/v1", "handle":handle,
            "result":output, "canonical_result_digest":digest, "journal":session.engine.journal(),
            "native_evidence":{"output":evidence.output,"telemetry_summary":evidence.telemetry_summary}});
        if let Some((authorization,catalog)) = &session.signed_service {
            let history = session.engine.journal().iter().filter_map(|entry| {
                if let control_prototype::JournalEntry::Applied(applied) = entry {
                    Some(service_control::RacingServiceDecisionV1 {observation_digest:applied.proposal.observation_digest,applied:applied.clone()})
                } else { None }
            }).collect();
            match service_control::service_control_evidence(authorization.clone(),catalog,history,output) {
                Ok(e) => response["service_evidence"] = serde_json::json!(e),
                Err(e) => return json_error(&e),
            }
        }
        let response=response.to_string();
        sessions.remove(&handle);
        response
    })
}

/// Consumes a terminal replay handle. Only the canonical domain output and
/// telemetry summary are returned; the presentation digest/journal are omitted.
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn complete_experimental_controlled_racing_replay_json(handle: u32) -> String {
    SESSIONS.with(|sessions| {
        let mut sessions = sessions.borrow_mut();
        let Some(session) = sessions.get(&handle) else { return json_error("unknown control handle"); };
        if session.transport == Some(OutputTransport::Presentation) {
            return json_error("cannot mix presentation and replay transports");
        }
        let Some(evidence) = &session.replay_evidence else { return json_error("control session is not complete"); };
        let response = serde_json::json!({"schema_version":"pitgun.racing-control-replay-completion/v1", "handle":handle,
            "native_evidence":{"output":evidence.output,"telemetry_summary":evidence.telemetry_summary}}).to_string();
        sessions.remove(&handle);
        response
    })
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn release_experimental_controlled_racing_session_json(handle: u32) -> String {
    SESSIONS.with(|sessions| {
        if sessions.borrow_mut().remove(&handle).is_none() { return json_error("unknown control handle"); }
        serde_json::json!({"schema_version":"pitgun.racing-control-release/v1", "handle":handle, "released":true}).to_string()
    })
}

#[cfg(test)]
mod tests;
