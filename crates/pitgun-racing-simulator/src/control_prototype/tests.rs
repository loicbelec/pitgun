use super::*;
use crate::{CompetitorStintStrategy, run_race_with_catalog_and_v3_fuel_contract_candidate};
use pitgun_contract::canonical_json_bytes;
use pitgun_racing_contract::RacingDriverInstructionTimelineVersion;

fn request() -> RunRaceRequest {
    serde_json::from_str(include_str!("../../fixtures/control/monza-five-laps.json")).unwrap()
}

fn timeline() -> RacingDriverInstructionTimelineV1 {
    RacingDriverInstructionTimelineV1 {
        schema_version: RacingDriverInstructionTimelineVersion::V1,
        events: vec![],
    }
}

fn config() -> LocalControlConfig {
    LocalControlConfig {
        execution_id: "local-control-fixture-1".into(),
        runtime_digest: Digest::from_bytes(b"test runtime, not a distributed attestation"),
        decision_laps: vec![1, 3, 4],
        allowed_tire_ids: vec!["hard".into(), "soft".into()],
    }
}

fn start() -> LocalControlledRacingSession {
    LocalControlledRacingSession::start(
        request(),
        &RacingCatalogSnapshot::embedded_model_v3_fuel_contract().unwrap(),
        timeline(),
        config(),
    )
    .unwrap()
}

fn observe(session: &mut LocalControlledRacingSession) -> PlayerObservation {
    loop {
        match session.advance().unwrap() {
            ControlStep::AwaitingDecision(observation) => return observation,
            ControlStep::Batch(batch) => assert!(batch.records().iter().all(|record| !matches!(
                record.event(),
                IncrementalExecutionStreamEventV1::Complete(_)
            ))),
        }
    }
}

fn finish(session: &mut LocalControlledRacingSession, pit: bool) -> RaceOutput {
    loop {
        match session.advance().unwrap() {
            ControlStep::AwaitingDecision(observation) => {
                let proposal =
                    reference_proposal(&observation, if pit { 1 } else { 0 }, "hard").unwrap();
                session.propose(proposal).unwrap();
            }
            ControlStep::Batch(batch) => {
                for record in batch.records() {
                    if let IncrementalExecutionStreamEventV1::Complete(output) = record.event() {
                        return output.clone();
                    }
                }
            }
        }
    }
}

fn applied(session: &LocalControlledRacingSession) -> Vec<AppliedDecision> {
    session
        .journal()
        .iter()
        .filter_map(|entry| match entry {
            JournalEntry::Applied(decision) => Some(decision.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn keep_preserves_the_existing_canonical_output() {
    let snapshot = RacingCatalogSnapshot::embedded_model_v3_fuel_contract().unwrap();
    let expected =
        run_race_with_catalog_and_v3_fuel_contract_candidate(request(), &snapshot, timeline())
            .unwrap();
    let mut session = start();
    let actual = finish(&mut session, false);
    assert_eq!(
        canonical_json_bytes(&actual).unwrap(),
        canonical_json_bytes(&expected).unwrap()
    );
    assert_eq!(applied(&session).len(), 3);
}

#[test]
fn open_boundary_never_computes_a_future_lap_or_repeats_the_observation_log() {
    let mut session = start();
    let observation = observe(&mut session);
    let prefix = canonical_json_bytes(&session.session.competitors[0].latest_lap).unwrap();
    let entries = session.journal().len();
    for _ in 0..25 {
        assert_eq!(observe(&mut session), observation);
        assert_eq!(session.journal().len(), entries);
        assert_eq!(
            canonical_json_bytes(&session.session.competitors[0].latest_lap).unwrap(),
            prefix
        );
    }
    let visible = serde_json::to_string(&observation).unwrap();
    assert!(!visible.contains("competitors"));
    assert_eq!(observation.completed_laps, 1);
}

#[test]
fn pit_changes_real_service_and_tire_state_without_rewriting_the_prefix_and_replays_exactly() {
    let snapshot = RacingCatalogSnapshot::embedded_model_v3_fuel_contract().unwrap();
    let mut session = start();
    let observation = observe(&mut session);
    let prefix = canonical_json_bytes(&session.session.competitors[0].latest_lap).unwrap();
    let proposal = reference_proposal(&observation, 1, "hard").unwrap();
    let receipt = session.propose(proposal).unwrap();
    assert_eq!(receipt.pit_after_lap, Some(2));
    assert_eq!(
        canonical_json_bytes(&session.session.competitors[0].latest_lap).unwrap(),
        prefix
    );
    assert!(matches!(session.advance().unwrap(), ControlStep::Batch(_)));
    let player = &session.session.competitors[0];
    let service = player.latest_lap.as_ref().unwrap();
    assert_eq!(service.lap_index, 2);
    assert!(service.pit_stop_completed);
    assert_eq!(service.state_after_lap.tire_wear, 0.0);
    assert_eq!(service.end_speed_mps, None);
    assert_eq!(player.stint_plan.pit_laps, [2]);
    assert_eq!(
        player.stint_plan.tire_by_lap,
        ["soft", "soft", "hard", "hard", "hard"]
    );
    let output = finish(&mut session, false);
    let mut unchanged = start();
    let unchanged_output = finish(&mut unchanged, false);
    assert_ne!(
        canonical_json_bytes(&output).unwrap(),
        canonical_json_bytes(&unchanged_output).unwrap()
    );
    // A predeclared equivalent plan must have exactly the same physical result.
    let mut fixed = request();
    fixed.input.race.competitors[0].stint_strategy = Some(
        serde_json::from_value::<CompetitorStintStrategy>(
            serde_json::json!({"stints": [{"tire_id": "soft", "laps": 2},
            {"tire_id": "hard", "laps": 3}], "pit_laps": [2]}),
        )
        .unwrap(),
    );
    let expected =
        run_race_with_catalog_and_v3_fuel_contract_candidate(fixed, &snapshot, timeline()).unwrap();
    assert_eq!(
        canonical_json_bytes(&output).unwrap(),
        canonical_json_bytes(&expected).unwrap()
    );
    let log = applied(&session);
    let replayed = replay(request(), &snapshot, timeline(), config(), &log).unwrap();
    assert_eq!(
        canonical_json_bytes(&output).unwrap(),
        canonical_json_bytes(&replayed).unwrap()
    );
}

#[test]
fn duplicate_receipt_is_stable_and_a_conflicting_retry_cannot_change_the_action() {
    let mut session = start();
    let observation = observe(&mut session);
    let proposal = reference_proposal(&observation, 1, "hard").unwrap();
    let receipt = session.propose(proposal.clone()).unwrap();
    assert_eq!(session.propose(proposal.clone()).unwrap(), receipt);
    session.advance().unwrap();
    assert_eq!(session.propose(proposal.clone()).unwrap(), receipt);
    let mut conflicting = proposal;
    conflicting.action = Action::KeepPlan;
    assert!(
        session
            .propose(conflicting)
            .unwrap_err()
            .contains("conflict")
    );
    assert_eq!(applied(&session), vec![receipt]);
    assert_eq!(session.session.competitors[0].stint_plan.pit_laps, [2]);
}

#[test]
fn bad_bindings_and_undelegated_tires_leave_the_same_boundary_open() {
    let mut session = start();
    let observation = observe(&mut session);
    let good = reference_proposal(&observation, 1, "hard").unwrap();
    let mut invalid = vec![];
    let mut p = good.clone();
    p.run_digest = Digest::from_bytes(b"other run");
    invalid.push(p);
    let mut p = good.clone();
    p.decision_sequence += 1;
    invalid.push(p);
    let mut p = good.clone();
    p.completed_laps = 0;
    invalid.push(p);
    let mut p = good.clone();
    p.observation_digest = Digest::from_bytes(b"other observation");
    invalid.push(p);
    let mut p = good.clone();
    p.idempotency_key.clear();
    invalid.push(p);
    let mut p = good.clone();
    p.action = Action::PitNextLap {
        tire_id: "medium".into(),
    };
    invalid.push(p);
    let mut p = good.clone();
    p.source = ProposalSource::Fallback {
        reason: FallbackReason::Timeout,
    };
    invalid.push(p);
    for proposal in invalid {
        assert!(session.propose(proposal).is_err());
        assert_eq!(observe(&mut session), observation);
    }
    assert_eq!(
        session
            .journal()
            .iter()
            .filter(|entry| matches!(entry, JournalEntry::Rejected { .. }))
            .count(),
        7
    );
    assert!(applied(&session).is_empty());
    session.propose(good).unwrap();
}

#[test]
fn unexposed_closed_and_last_lap_boundaries_reject_new_actions() {
    let mut session = start();
    let observation = observe(&mut session);
    let good = reference_proposal(&observation, 0, "hard").unwrap();
    let mut unopened = start();
    assert!(unopened.propose(good.clone()).is_err());
    session.propose(good.clone()).unwrap();
    let mut late = good;
    late.idempotency_key = "late".into();
    assert!(session.propose(late.clone()).is_err());
    let third = observe(&mut session);
    assert_eq!(third.completed_laps, 3);
    assert!(session.propose(late).is_err());
    session
        .propose(reference_proposal(&third, 0, "hard").unwrap())
        .unwrap();
    let last = observe(&mut session);
    assert_eq!(last.completed_laps, 4);
    assert!(!last.can_pit_next_lap);
    assert!(
        session
            .propose(reference_proposal(&last, 4, "hard").unwrap())
            .is_err()
    );
    let fallback = Proposal::for_observation(
        &last,
        "fallback".into(),
        ProposalSource::Fallback {
            reason: FallbackReason::Timeout,
        },
        Action::KeepPlan,
    )
    .unwrap();
    session.propose(fallback).unwrap();
    finish(&mut session, false);
    assert!(session.advance().is_err());
}

#[test]
fn repeated_waits_and_transport_roundtrips_preserve_the_entire_stream_and_journal() {
    let run = |extra_pulls: usize| {
        let mut session = start();
        let mut batches = vec![];
        loop {
            match session.advance().unwrap() {
                ControlStep::AwaitingDecision(observation) => {
                    for _ in 0..extra_pulls {
                        assert_eq!(observe(&mut session), observation);
                    }
                    let proposal = reference_proposal(&observation, 1, "hard").unwrap();
                    let proposal =
                        serde_json::from_slice(&serde_json::to_vec(&proposal).unwrap()).unwrap();
                    session.propose(proposal).unwrap();
                }
                ControlStep::Batch(batch) => {
                    let done = batch.records().iter().any(|record| {
                        matches!(
                            record.event(),
                            IncrementalExecutionStreamEventV1::Complete(_)
                        )
                    });
                    batches.push(batch);
                    if done {
                        break;
                    }
                }
            }
        }
        (
            canonical_json_bytes(&batches).unwrap(),
            canonical_json_bytes(&session.journal()).unwrap(),
        )
    };
    assert_eq!(run(0), run(11));
}

#[test]
fn replay_rejects_missing_extra_reordered_and_tampered_decisions_or_inputs() {
    let snapshot = RacingCatalogSnapshot::embedded_model_v3_fuel_contract().unwrap();
    let mut session = start();
    finish(&mut session, true);
    let log = applied(&session);
    assert!(replay(request(), &snapshot, timeline(), config(), &log[..2]).is_err());
    let mut extra = log.clone();
    extra.push(log[0].clone());
    assert!(replay(request(), &snapshot, timeline(), config(), &extra).is_err());
    let mut swapped = log.clone();
    swapped.swap(0, 1);
    assert!(replay(request(), &snapshot, timeline(), config(), &swapped).is_err());
    let mut changed = log.clone();
    changed[0].pit_after_lap = Some(3);
    assert!(replay(request(), &snapshot, timeline(), config(), &changed).is_err());
    let mut other_input = request();
    other_input.seed += 1;
    assert!(replay(other_input, &snapshot, timeline(), config(), &log).is_err());
    let mut other_runtime = config();
    other_runtime.runtime_digest = Digest::from_bytes(b"other runtime");
    assert!(replay(request(), &snapshot, timeline(), other_runtime, &log).is_err());
}

#[test]
fn configuration_and_wire_actions_fail_closed() {
    let snapshot = RacingCatalogSnapshot::embedded_model_v3_fuel_contract().unwrap();
    for decisions in [vec![0], vec![5], vec![2, 1], vec![1, 1]] {
        let mut bad = config();
        bad.decision_laps = decisions;
        assert!(
            LocalControlledRacingSession::start(request(), &snapshot, timeline(), bad).is_err()
        );
    }
    for tires in [vec!["unknown".into()], vec!["soft".into(), "soft".into()]] {
        let mut bad = config();
        bad.allowed_tire_ids = tires;
        assert!(
            LocalControlledRacingSession::start(request(), &snapshot, timeline(), bad).is_err()
        );
    }
    assert!(
        serde_json::from_str::<Action>(r#"{"kind":"pit_next_lap","tire_id":"hard","lap":1}"#)
            .is_err()
    );
    assert!(serde_json::from_str::<Action>(r#"{"kind":"change_aggression","value":1}"#).is_err());
    assert!(
        serde_json::from_str::<PrototypeVersion>(r#""pitgun.racing-control-prototype/v2""#)
            .is_err()
    );
}

#[test]
fn a_second_request_preserves_completed_service_and_replaces_only_the_remaining_stint() {
    let snapshot = RacingCatalogSnapshot::embedded_model_v3_fuel_contract().unwrap();
    let mut session = start();
    let first = observe(&mut session);
    session
        .propose(reference_proposal(&first, 1, "hard").unwrap())
        .unwrap();
    let third = observe(&mut session);
    let prefix = canonical_json_bytes(&session.session.competitors[0].latest_lap).unwrap();
    session
        .propose(reference_proposal(&third, 3, "soft").unwrap())
        .unwrap();
    assert_eq!(
        canonical_json_bytes(&session.session.competitors[0].latest_lap).unwrap(),
        prefix
    );
    assert_eq!(session.session.competitors[0].stint_plan.pit_laps, [2, 4]);
    let output = finish(&mut session, false);
    assert_eq!(output.player_pit_laps, [2, 4]);
    let mut fixed = request();
    fixed.input.race.competitors[0].stint_strategy = Some(
        serde_json::from_value(
            serde_json::json!({"stints": [{"tire_id": "soft", "laps": 2},
            {"tire_id": "hard", "laps": 2}, {"tire_id": "soft", "laps": 1}],
            "pit_laps": [2, 4]}),
        )
        .unwrap(),
    );
    let expected =
        run_race_with_catalog_and_v3_fuel_contract_candidate(fixed, &snapshot, timeline()).unwrap();
    assert_eq!(
        canonical_json_bytes(&output).unwrap(),
        canonical_json_bytes(&expected).unwrap()
    );
    let replayed = replay(
        request(),
        &snapshot,
        timeline(),
        config(),
        &applied(&session),
    )
    .unwrap();
    assert_eq!(
        canonical_json_bytes(&output).unwrap(),
        canonical_json_bytes(&replayed).unwrap()
    );
}

#[test]
fn ten_car_control_changes_only_the_delegated_players_physics() {
    let snapshot = RacingCatalogSnapshot::embedded_model_v3_fuel_contract().unwrap();
    let mut request = request();
    let player = request.input.race.competitors[0].clone();
    for i in 1..10 {
        let mut rival = player.clone();
        rival.id = format!("rival-{i}");
        rival.name = format!("Rival {i}");
        rival.is_player = false;
        request.input.race.competitors.push(rival);
    }
    let expected = run_race_with_catalog_and_v3_fuel_contract_candidate(
        request.clone(),
        &snapshot,
        timeline(),
    )
    .unwrap();
    let mut session =
        LocalControlledRacingSession::start(request.clone(), &snapshot, timeline(), config())
            .unwrap();
    let output = finish(&mut session, true);
    assert_eq!(output.standings.len(), 10);
    for before in expected
        .standings
        .iter()
        .filter(|row| row.competitor_id != "player")
    {
        let after = output
            .standings
            .iter()
            .find(|row| row.competitor_id == before.competitor_id)
            .unwrap();
        assert_eq!(
            (
                before.total_time_ms,
                before.best_lap_ms,
                before.laps_completed,
                &before.status
            ),
            (
                after.total_time_ms,
                after.best_lap_ms,
                after.laps_completed,
                &after.status
            )
        );
    }
    for entry in session.journal() {
        if let JournalEntry::Observation(observation) = entry {
            let visible = serde_json::to_string(observation).unwrap();
            assert!(!visible.contains("rival"));
        }
    }
    let replayed = replay(request, &snapshot, timeline(), config(), &applied(&session)).unwrap();
    assert_eq!(
        canonical_json_bytes(&output).unwrap(),
        canonical_json_bytes(&replayed).unwrap()
    );
}

#[test]
fn common_time_grid_covers_fast_slow_pit_and_finished_rivals_without_changing_results() {
    use crate::{RacingCompetitorProgressStatusV1, RacingSessionProgressV1};
    let mut request = request();
    let player = request.input.race.competitors[0].clone();
    let mut fast = player.clone();
    fast.id = "fast".into();
    fast.is_player = false;
    fast.tuning.engine_points = 55.0;
    fast.tuning.cooling_points = 5.0;
    fast.tuning.aero_points = 20.0;
    fast.tuning.chassis_points = 20.0;
    let mut slow = player.clone();
    slow.id = "slow".into();
    slow.is_player = false;
    slow.tuning.engine_points = 5.0;
    slow.tuning.cooling_points = 55.0;
    slow.tuning.aero_points = 20.0;
    slow.tuning.chassis_points = 20.0;
    request.input.race.competitors = vec![player, fast, slow];
    let snapshot = RacingCatalogSnapshot::embedded_model_v3_fuel_contract().unwrap();
    let expected = run_race_with_catalog_and_v3_fuel_contract_candidate(
        request.clone(),
        &snapshot,
        timeline(),
    )
    .unwrap();
    let mut session =
        LocalControlledRacingSession::start(request, &snapshot, timeline(), config()).unwrap();
    let mut previous = BTreeMap::<String, [f64; 2]>::new();
    let mut saw_lookahead = false;
    let mut saw_finished_rival = false;
    let mut saw_service = false;
    let output = loop {
        match session.advance().unwrap() {
            ControlStep::AwaitingDecision(observation) => {
                let player = session
                    .session
                    .competitors
                    .iter()
                    .find(|c| c.is_player)
                    .unwrap();
                assert!(player.prefetched_laps.is_empty());
                assert_eq!(
                    player.latest_lap.as_ref().unwrap().lap_index,
                    observation.completed_laps
                );
                session
                    .propose(reference_proposal(&observation, 0, "hard").unwrap())
                    .unwrap();
            }
            ControlStep::Batch(batch) => {
                let mut result = None;
                for record in batch.records() {
                    match record.event() {
                        IncrementalExecutionStreamEventV1::Complete(output) => {
                            result = Some(output.clone())
                        }
                        IncrementalExecutionStreamEventV1::Progress(
                            RacingSessionProgressV1::Grid { competitors, .. },
                        ) => {
                            let horizon = session
                                .session
                                .competitors
                                .iter()
                                .find(|c| c.is_player)
                                .unwrap()
                                .latest_lap
                                .as_ref()
                                .unwrap()
                                .cumulative_time_s;
                            for car in competitors {
                                assert!(car.trajectory.len() >= 2);
                                assert!(car.trajectory.iter().all(|p| p[0] <= horizon));
                                assert!(
                                    car.trajectory
                                        .windows(2)
                                        .all(|w| w[0][0] <= w[1][0] && w[0][1] <= w[1][1])
                                );
                                let start = *car.trajectory.first().unwrap();
                                assert_eq!(
                                    start,
                                    previous
                                        .get(&car.competitor_id)
                                        .copied()
                                        .unwrap_or([0.0, 0.0])
                                );
                                let end = *car.trajectory.last().unwrap();
                                assert_eq!(end[1], car.distance_m);
                                if car.status == RacingCompetitorProgressStatusV1::Running {
                                    assert_eq!(end[0], horizon);
                                } else if car.competitor_id != "player" {
                                    saw_finished_rival = true;
                                }
                                saw_service |= car
                                    .trajectory
                                    .windows(2)
                                    .any(|w| w[1][0] - w[0][0] > 10.0 && w[1][1] == w[0][1]);
                                previous.insert(car.competitor_id.clone(), end);
                            }
                            saw_lookahead |= session
                                .session
                                .competitors
                                .iter()
                                .any(|c| !c.prefetched_laps.is_empty());
                            assert!(session.session.competitors.iter().all(|c| c.is_player
                                || c.prefetched_laps.iter().all(|p| p.solution.t.is_empty())));
                        }
                        _ => {}
                    }
                }
                if let Some(result) = result {
                    break result;
                }
            }
        }
    };
    assert!(saw_lookahead, "fixture must require rival look-ahead");
    assert!(
        saw_finished_rival,
        "a faster rival must finish before the player"
    );
    assert!(
        saw_service,
        "native pit time must remain stationary in the solved path"
    );
    assert_eq!(
        canonical_json_bytes(&output).unwrap(),
        canonical_json_bytes(&expected).unwrap()
    );
}

#[test]
fn common_time_prefix_keeps_the_unemitted_bracket_and_stationary_service() {
    let mut path = vec![[0.0, 0.0], [10.0, 100.0], [30.0, 100.0], [40.0, 200.0]];
    assert_eq!(
        crate::take_playback_prefix(&mut path, 20.0),
        vec![[0.0, 0.0], [10.0, 100.0], [20.0, 100.0]]
    );
    assert_eq!(
        crate::take_playback_prefix(&mut path, 35.0),
        vec![[20.0, 100.0], [30.0, 100.0], [35.0, 150.0]]
    );
    assert_eq!(
        crate::take_playback_prefix(&mut path, 50.0),
        vec![[35.0, 150.0], [40.0, 200.0]]
    );
    assert_eq!(
        crate::take_playback_prefix(&mut path, 60.0),
        vec![[40.0, 200.0], [40.0, 200.0]]
    );
}
