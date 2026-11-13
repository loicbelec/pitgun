//! A bounded native proof, not a Grand Prix benchmark or Hosted submission.
use pitgun_contract::{Digest, IncrementalExecutionStreamEventV1, canonical_json_digest};
use pitgun_racing_contract::{
    RacingDriverInstructionTimelineV1, RacingDriverInstructionTimelineVersion,
};
use pitgun_racing_simulator::{
    RaceOutput, RacingCatalogSnapshot, RunRaceRequest,
    control_prototype::{
        AppliedDecision, ControlStep, JournalEntry, LocalControlConfig,
        LocalControlledRacingSession, reference_proposal, replay,
    },
    run_race_with_catalog_and_v3_fuel_contract_candidate,
};
use serde_json::{Value, json};

fn summary(output: &RaceOutput) -> Result<Value, String> {
    Ok(json!({
        "canonical_result_digest": canonical_json_digest(output).map_err(|e| e.to_string())?,
        "total_time_ms": output.total_time_ms,
        "player_lap_times_ms": output.player_lap_times_ms,
        "player_pit_laps": output.player_pit_laps,
        "standings": output.standings,
    }))
}

fn main() -> Result<(), String> {
    let request: RunRaceRequest =
        serde_json::from_str(include_str!("../fixtures/control/monza-five-laps.json"))
            .map_err(|e| e.to_string())?;
    let snapshot =
        RacingCatalogSnapshot::embedded_model_v3_fuel_contract().map_err(|e| e.to_string())?;
    let timeline = RacingDriverInstructionTimelineV1 {
        schema_version: RacingDriverInstructionTimelineVersion::V1,
        events: vec![],
    };
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    let runtime_digest = Digest::from_bytes(&std::fs::read(executable).map_err(|e| e.to_string())?);
    let config = LocalControlConfig {
        physics: None,
        execution_id: "native-local-control-probe".into(),
        runtime_digest,
        service_windows: None,
        event_policy: None,
        decision_laps: vec![1, 3, 4],
        allowed_tire_ids: vec!["hard".into(), "soft".into()],
    };
    let baseline = run_race_with_catalog_and_v3_fuel_contract_candidate(
        request.clone(),
        &snapshot,
        timeline.clone(),
    )?;
    let mut session = LocalControlledRacingSession::start(
        request.clone(),
        &snapshot,
        timeline.clone(),
        config.clone(),
    )?;
    let output = loop {
        match session.advance()? {
            ControlStep::AwaitingDecision(observation) => {
                // Reference policy: pit after lap 2, hard tires to the finish.
                session.propose(reference_proposal(&observation, 1, "hard")?)?;
            }
            ControlStep::Batch(batch) => {
                if let Some(output) =
                    batch
                        .records()
                        .iter()
                        .find_map(|record| match record.event() {
                            IncrementalExecutionStreamEventV1::Complete(output) => {
                                Some(output.clone())
                            }
                            _ => None,
                        })
                {
                    break output;
                }
            }
        }
    };
    let applied: Vec<AppliedDecision> = session
        .journal()
        .iter()
        .filter_map(|entry| match entry {
            JournalEntry::Applied(decision) => Some(decision.clone()),
            _ => None,
        })
        .collect();
    let replayed = replay(
        request.clone(),
        &snapshot,
        timeline.clone(),
        config.clone(),
        &applied,
    )?;
    let controlled = summary(&output)?;
    let replayed = summary(&replayed)?;
    if controlled != replayed {
        return Err("offline replay differs".into());
    }
    let mut report = json!({
        "schema_version": "pitgun.racing-control-probe-report/v1",
        "scope": "native local protocol fixture, not a Grand Prix benchmark or Hosted evidence",
        "catalog_release": snapshot.release_identity(),
        "request": request, "timeline": timeline, "config": config,
        "journal": session.journal(), "applied_decisions": applied,
        "baseline": summary(&baseline)?, "controlled": controlled,
        "replayed": replayed, "exact_replay": true,
    });
    if std::env::args().any(|arg| arg == "--browser-fixture") {
        report["browser_fixture"] = json!({
            "requestJson": serde_json::to_string(&request).map_err(|e|e.to_string())?,
            "catalogBundleJson": serde_json::to_string(&snapshot.to_bundle().map_err(|e|e.to_string())?).map_err(|e|e.to_string())?,
            "timelineJson": serde_json::to_string(&timeline).map_err(|e|e.to_string())?,
            "configJson": serde_json::to_string(&config).map_err(|e|e.to_string())?,
        });
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
    );
    Ok(())
}
