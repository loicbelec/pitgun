//! Pull facade for the existing local power-unit thermal model (V9).
//! No Authority authorization or verification evidence is created here.

use super::*;

struct LocalSession {
    engine: IncrementalRacingSession,
    output: Option<RaceOutput>,
}

#[derive(Serialize)]
struct LocalCompletion {
    schema_version: &'static str,
    handle: u32,
    result: RaceOutput,
}

thread_local! {
    static LOCAL_SESSIONS: RefCell<BTreeMap<u32, LocalSession>> =
        const { RefCell::new(BTreeMap::new()) };
    static NEXT_LOCAL_HANDLE: Cell<u32> = const { Cell::new(1) };
}

/// Starts the same model/profile as the corresponding synchronous export.
/// Initialization does not advance any lap. Handles belong to this local API.
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn start_local_racing_session_with_catalog_and_v3_power_unit_thermal_profile_json(
    input_json: String,
    catalog_bundle_json: String,
    candidate_json: String,
) -> String {
    let create = || -> Result<IncrementalRacingSession, String> {
        let request = parse_run_race_request(&input_json)?;
        let catalog = RacingCatalogSnapshot::from_bundle_json(&catalog_bundle_json)
            .map_err(|error| format!("invalid Racing catalog: {error}"))?;
        let candidate = V3PowerUnitThermalProfileCandidateV2::from_exact_json(&candidate_json)?;
        candidate.validate()?;
        let profile = V3CandidateExperimentProfile {
            schema_version: V3CandidateExperimentProfileVersion::V9,
            engine_thermal_resolution: None,
            ..V3CandidateExperimentProfile::default()
        };
        start_incremental_race_with_catalog_and_v3_component_profile(
            request, &catalog, &profile, &candidate, None,
        )
    };
    let mut engine = match create() {
        Ok(engine) => engine,
        Err(error) => return json_error(&error),
    };
    // Emit the same solved playback samples as authorized sessions.
    engine.include_playback_trajectories = true;
    LOCAL_SESSIONS.with(|sessions| {
        NEXT_LOCAL_HANDLE.with(|next| {
            let mut sessions = sessions.borrow_mut();
            let first = next.get().max(1);
            let mut handle = first;
            loop {
                if let std::collections::btree_map::Entry::Vacant(entry) = sessions.entry(handle) {
                    entry.insert(LocalSession {
                        engine,
                        output: None,
                    });
                    next.set(handle.wrapping_add(1).max(1));
                    return serialize_json(&RacingSessionStreamStartV1 {
                        schema_version: "pitgun.racing-session-stream-start/v1",
                        handle,
                    });
                }
                handle = handle.wrapping_add(1).max(1);
                if handle == first {
                    return json_error("local Racing session handle space exhausted");
                }
            }
        })
    })
}

/// Advances all competitors by one deterministic lap boundary.
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn pull_local_racing_session_json(handle: u32) -> String {
    LOCAL_SESSIONS.with(|sessions| {
        let mut sessions = sessions.borrow_mut();
        let Some(session) = sessions.get_mut(&handle) else {
            return json_error(&format!("unknown local Racing session handle {handle}"));
        };
        if session.output.is_some() {
            return json_error("local Racing session completed; call complete or release");
        }
        match session.engine.advance() {
            Ok(batch) => {
                session.output = batch
                    .records()
                    .iter()
                    .find_map(|record| match record.event() {
                        IncrementalExecutionStreamEventV1::Complete(output) => Some(output.clone()),
                        IncrementalExecutionStreamEventV1::Progress(_) => None,
                    });
                serialize_json(&RacingSessionStreamPullV1 {
                    schema_version: "pitgun.racing-session-stream-pull/v1",
                    handle,
                    complete: session.output.is_some(),
                    batch,
                })
            }
            Err(error) => json_error(&error),
        }
    })
}

/// Consumes a completed local handle and returns the unchanged RaceOutput.
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn complete_local_racing_session_json(handle: u32) -> String {
    LOCAL_SESSIONS.with(|sessions| {
        let mut sessions = sessions.borrow_mut();
        let Some(session) = sessions.get(&handle) else {
            return json_error(&format!("unknown local Racing session handle {handle}"));
        };
        if session.output.is_none() {
            return json_error("local Racing session is not complete; pull another batch");
        }
        let session = sessions.remove(&handle).expect("checked local session");
        serialize_json(&LocalCompletion {
            schema_version: "pitgun.racing-local-session-completion/v1",
            handle,
            result: session.output.expect("checked terminal output"),
        })
    })
}

/// Releases local state on cancellation or error without producing a result.
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn release_local_racing_session_json(handle: u32) -> String {
    LOCAL_SESSIONS.with(|sessions| {
        if sessions.borrow_mut().remove(&handle).is_none() {
            return json_error(&format!("unknown local Racing session handle {handle}"));
        }
        serialize_json(&RacingSessionStreamReleaseV1 {
            schema_version: "pitgun.racing-session-stream-release/v1",
            handle,
            released: true,
        })
    })
}
