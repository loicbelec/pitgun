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
        physics: None,
        execution_id: "local-control-fixture-1".into(),
        runtime_digest: Digest::from_bytes(b"test runtime, not a distributed attestation"),
        decision_laps: vec![1, 3, 4],
        service_windows: None,
        event_policy: None,
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
            ControlStep::AwaitingDecision(observation) => return *observation,
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
fn career_thermal_keep_and_pit_use_the_ordinary_workload_and_replay_exactly() {
    let snapshot = RacingCatalogSnapshot::embedded_model_v3_timeline().unwrap();
    let thermal = snapshot.power_unit_thermal_profile().unwrap();
    let mut cfg = config();
    cfg.physics = Some(CareerControlPhysics::PowerUnitThermalV1);
    cfg.service_windows = Some(ServiceWindowPolicy {
        schema_version: ServiceWindowVersion::V1,
        closing_margin_us: 5_000_000,
    });
    let mut classic = request();
    classic.input.vehicle_id = Some("classic_v8_1960".into());
    classic.input.era = 1;
    classic.era = Some(1);
    let expected_classic = crate::run_race_with_catalog_and_v3_power_unit_thermal_profile(
        classic.clone(),
        &snapshot,
        thermal,
    )
    .unwrap();
    let mut classic_session =
        LocalControlledRacingSession::start(classic, &snapshot, timeline(), cfg.clone()).unwrap();
    assert_eq!(
        canonical_json_bytes(&finish(&mut classic_session, false)).unwrap(),
        canonical_json_bytes(&expected_classic).unwrap()
    );
    let expected = crate::run_race_with_catalog_and_v3_power_unit_thermal_profile(
        request(),
        &snapshot,
        thermal,
    )
    .unwrap();
    let mut keep =
        LocalControlledRacingSession::start(request(), &snapshot, timeline(), cfg.clone()).unwrap();
    let actual = finish(&mut keep, false);
    assert_eq!(
        canonical_json_bytes(&actual).unwrap(),
        canonical_json_bytes(&expected).unwrap()
    );

    let mut changed =
        LocalControlledRacingSession::start(request(), &snapshot, timeline(), cfg.clone()).unwrap();
    let boundary = observe(&mut changed);
    let mut proposal = reference_proposal(&boundary, 1, "hard").unwrap();
    proposal.decision_at_us = Some(boundary.window.as_ref().unwrap().opened_at_us);
    changed.propose(proposal).unwrap();
    let output = finish(&mut changed, false);
    assert_ne!(
        canonical_json_bytes(&output).unwrap(),
        canonical_json_bytes(&actual).unwrap()
    );
    let reproduced = replay(request(), &snapshot, timeline(), cfg, &applied(&changed)).unwrap();
    assert_eq!(
        canonical_json_bytes(&output).unwrap(),
        canonical_json_bytes(&reproduced).unwrap()
    );
    assert_eq!(output.player_pit_laps, vec![2]);
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
                        assert_eq!(observe(&mut session), *observation);
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

#[test]
fn physical_events_release_only_observed_rival_facts_and_separate_crossing_from_service() {
    use crate::{RacingSessionEventV1, RacingSessionProgressV1};
    let mut request = request();
    let base = request.input.race.competitors[0].clone();
    for (id, engine, cooling) in [("fast", 55.0, 5.0), ("slow", 5.0, 55.0)] {
        let mut rival = base.clone();
        rival.id = id.into();
        rival.is_player = false;
        rival.tuning.engine_points = engine;
        rival.tuning.cooling_points = cooling;
        rival.tuning.aero_points = 20.0;
        rival.tuning.chassis_points = 20.0;
        request.input.race.competitors.push(rival);
    }
    let snapshot = RacingCatalogSnapshot::embedded_model_v3_fuel_contract().unwrap();
    let event_timeline: RacingDriverInstructionTimelineV1 = serde_json::from_value(serde_json::json!({
        "schema_version":"pitgun.racing-driver-instruction-timeline/v1",
        "events":[{"sequence":0,"competitor_id":"fast","effective_at":{"lap_index":1,"segment_index":0},"mode":"attack"}]
    })).unwrap();
    let mut baseline = start_incremental_race_with_catalog_and_v3_fuel_contract_candidate(
        request.clone(),
        &snapshot,
        event_timeline.clone(),
    )
    .unwrap();
    let mut expected = BTreeMap::new();
    for _ in 0..5 {
        baseline.advance().unwrap();
        for car in &baseline.competitors {
            let lap = car.latest_lap.as_ref().unwrap();
            if car.competitor_id == "fast" && lap.lap_index == 1 {
                expected.insert(
                    (car.competitor_id.clone(), 2, "instruction"),
                    crate::physical_time_us(lap.cumulative_time_s),
                );
            }
            let service = if lap.pit_stop_completed {
                baseline.pit_loss_ms as f64 / 1000.0
            } else {
                0.0
            };
            expected.insert(
                (car.competitor_id.clone(), lap.lap_index, "lap"),
                crate::physical_time_us(lap.cumulative_time_s - service),
            );
            if lap.pit_stop_completed {
                expected.insert(
                    (car.competitor_id.clone(), lap.lap_index, "pit"),
                    crate::physical_time_us(lap.cumulative_time_s),
                );
            }
        }
    }
    let mut controlled =
        LocalControlledRacingSession::start(request, &snapshot, event_timeline, config()).unwrap();
    let mut seen = BTreeMap::new();
    let mut ids = std::collections::BTreeSet::new();
    let mut saw_future_pending = false;
    let mut saw_prefetched_event = false;
    let mut last_time = 0;
    let mut last_phase = 0;
    loop {
        match controlled.advance().unwrap() {
            ControlStep::AwaitingDecision(o) => {
                controlled
                    .propose(reference_proposal(&o, 0, "hard").unwrap())
                    .unwrap();
            }
            ControlStep::Batch(batch) => {
                let mut completed = false;
                for record in batch.records() {
                    match record.event() {
                        IncrementalExecutionStreamEventV1::Complete(_) => completed = true,
                        IncrementalExecutionStreamEventV1::Progress(
                            RacingSessionProgressV1::Event(event),
                        ) => {
                            let (id, lap, kind, timing) = match event {
                                RacingSessionEventV1::LapCompleted {
                                    competitor_id,
                                    lap,
                                    timing,
                                } => (competitor_id, *lap, "lap", timing),
                                RacingSessionEventV1::PitStopCompleted {
                                    competitor_id,
                                    lap,
                                    timing,
                                } => (competitor_id, *lap, "pit", timing),
                                RacingSessionEventV1::DriverInstructionApplied {
                                    competitor_id,
                                    lap,
                                    timing,
                                } => (competitor_id, *lap, "instruction", timing),
                            };
                            let timing = timing.as_ref().expect("native timing required");
                            let horizon = crate::physical_time_us(
                                controlled
                                    .session
                                    .competitors
                                    .iter()
                                    .find(|c| c.is_player)
                                    .unwrap()
                                    .latest_lap
                                    .as_ref()
                                    .unwrap()
                                    .cumulative_time_s,
                            );
                            assert_eq!(timing.released_at_us, horizon);
                            assert!(timing.occurred_at_us <= horizon);
                            assert!(timing.occurred_at_us >= last_time);
                            let phase = match kind {
                                "lap" => 0,
                                "pit" => 1,
                                _ => 2,
                            };
                            if timing.occurred_at_us == last_time {
                                assert!(phase >= last_phase);
                            }
                            last_phase = phase;
                            last_time = timing.occurred_at_us;
                            assert!(ids.insert(timing.event_id.to_string()));
                            let key = (id.clone(), lap, kind);
                            assert_eq!(timing.occurred_at_us, expected[&key]);
                            assert!(seen.insert(key, timing.occurred_at_us).is_none());
                            let car = controlled
                                .session
                                .competitors
                                .iter()
                                .find(|c| c.competitor_id == *id)
                                .unwrap();
                            saw_prefetched_event |=
                                car.latest_lap.as_ref().unwrap().lap_index < lap;
                        }
                        _ => {}
                    }
                }
                if completed {
                    break;
                }
                let horizon = crate::physical_time_us(
                    controlled
                        .session
                        .competitors
                        .iter()
                        .find(|c| c.is_player)
                        .unwrap()
                        .latest_lap
                        .as_ref()
                        .unwrap()
                        .cumulative_time_s,
                );
                saw_future_pending |= controlled
                    .session
                    .competitors
                    .iter()
                    .any(|c| c.pending_events.iter().any(|(t, _)| *t > horizon));
            }
        }
    }
    assert!(
        saw_future_pending,
        "slow rival events must wait for their physical time"
    );
    assert!(
        saw_prefetched_event,
        "fast rival facts must not wait for the same lap-number checkpoint"
    );
    let crossing = seen[&("player".into(), 3, "lap")];
    let service = seen[&("player".into(), 3, "pit")];
    assert_eq!(service - crossing, controlled.session.pit_loss_ms * 1000);
    // A slow rival's last crossing after the player's finish is intentionally
    // not disclosed by this player-horizon progress stream.
    assert!(
        expected
            .keys()
            .all(|key| seen.contains_key(key) || key.0 == "slow")
    );
}

fn live_start() -> LocalControlledRacingSession {
    let mut cfg = config();
    cfg.service_windows = Some(ServiceWindowPolicy {
        schema_version: ServiceWindowVersion::V1,
        closing_margin_us: 5_000_000,
    });
    LocalControlledRacingSession::start(
        request(),
        &RacingCatalogSnapshot::embedded_model_v3_fuel_contract().unwrap(),
        timeline(),
        cfg,
    )
    .unwrap()
}

#[test]
fn service_window_observes_the_committed_past_and_modifies_only_service_after_driving() {
    let mut session = live_start();
    let observation = observe(&mut session);
    let window = observation.window.as_ref().unwrap();
    assert_eq!(observation.completed_laps, 1);
    assert_eq!(window.service_lap, 2);
    assert!(window.opened_at_us < window.deadline_at_us);
    assert_eq!(window.service_at_us - window.deadline_at_us, 5_000_000);
    let prefix = session.session.competitors[0].latest_lap.clone().unwrap();
    assert_eq!(prefix.lap_index, 2);
    assert!(!prefix.pit_stop_completed);
    assert!(
        observation.fuel_mass_kg > prefix.state_after_lap.fuel_mass,
        "observation must not expose the solved future state"
    );
    for _ in 0..3 {
        assert_eq!(observe(&mut session), observation);
    }
    let mut p = reference_proposal(&observation, 1, "hard").unwrap();
    p.decision_at_us = Some(window.opened_at_us + 1_000_000);
    let receipt = session.propose(p.clone()).unwrap();
    assert_eq!(receipt.pit_after_lap, Some(2));
    assert_eq!(session.propose(p).unwrap(), receipt);
    assert_eq!(
        session.session.competitors[0].latest_lap.as_ref(),
        Some(&prefix),
        "receipt must precede future mutation"
    );
    let actual = finish(&mut session, false);
    let mut old = start();
    let expected = finish(&mut old, true);
    assert_eq!(
        canonical_json_bytes(&actual).unwrap(),
        canonical_json_bytes(&expected).unwrap()
    );
    let replayed = replay(
        request(),
        &RacingCatalogSnapshot::embedded_model_v3_fuel_contract().unwrap(),
        timeline(),
        session.config.clone(),
        &applied(&session),
    )
    .unwrap();
    assert_eq!(
        canonical_json_bytes(&replayed).unwrap(),
        canonical_json_bytes(&actual).unwrap()
    );
}

#[test]
fn service_window_deadline_rejects_late_commands_and_records_an_explicit_keep_fallback() {
    let mut session = live_start();
    let observation = observe(&mut session);
    let w = observation.window.as_ref().unwrap();
    let mut p = reference_proposal(&observation, 1, "hard").unwrap();
    p.decision_at_us = Some(w.deadline_at_us);
    assert!(session.propose(p.clone()).unwrap_err().contains("outside"));
    assert_eq!(observe(&mut session), observation);
    p.decision_at_us = None;
    assert!(session.propose(p.clone()).is_err());
    p.decision_at_us = Some(w.opened_at_us - 1);
    assert!(session.propose(p).is_err());
    let mut fallback = Proposal::for_observation(
        &observation,
        "deadline-0".into(),
        ProposalSource::Fallback {
            reason: FallbackReason::Timeout,
        },
        Action::KeepPlan,
    )
    .unwrap();
    fallback.decision_at_us = Some(w.deadline_at_us);
    session.propose(fallback.clone()).unwrap();
    let actual = finish(&mut session, false);
    assert_eq!(session.propose(fallback).unwrap().pit_after_lap, None);
    let expected = run_race_with_catalog_and_v3_fuel_contract_candidate(
        request(),
        &RacingCatalogSnapshot::embedded_model_v3_fuel_contract().unwrap(),
        timeline(),
    )
    .unwrap();
    assert_eq!(
        canonical_json_bytes(&actual).unwrap(),
        canonical_json_bytes(&expected).unwrap()
    );
}

fn rival_policy() -> RivalPitPolicy {
    RivalPitPolicy {
        schema_version: RivalPitPolicyVersion::V1,
        max_pre_stop_gap_us: 8_000_000,
        max_event_age_us: 120_000_000,
        planned_stop_within_laps: 2,
        cooldown_laps: 2,
        max_windows: 3,
    }
}
fn event_config() -> LocalControlConfig {
    let mut c = config();
    c.decision_laps.clear();
    c.event_policy = Some(rival_policy());
    c.service_windows = Some(ServiceWindowPolicy {
        schema_version: ServiceWindowVersion::V1,
        closing_margin_us: 5_000_000,
    });
    c
}
fn event_request() -> RunRaceRequest {
    let mut v: serde_json::Value =
        serde_json::from_str(include_str!("../../fixtures/control/monza-five-laps.json")).unwrap();
    v["input"]["laps"] = serde_json::json!(7);
    v["input"]["competitors"][0]["stint_strategy"] = serde_json::json!({"stints":[{"tire_id":"soft","laps":4},{"tire_id":"medium","laps":3}],"pit_laps":[4]});
    let mut cars = vec![v["input"]["competitors"][0].clone()];
    for i in 0..9 {
        let mut car = cars[0].clone();
        car["id"] = serde_json::json!(format!("rival-{i}"));
        car["is_player"] = serde_json::json!(false);
        car["stint_strategy"] = serde_json::json!({"stints":[{"tire_id":"soft","laps":1},{"tire_id":"hard","laps":6}],"pit_laps":[1]});
        cars.push(car);
    }
    v["input"]["competitors"] = serde_json::json!(cars);
    serde_json::from_value(v).unwrap()
}
#[test]
fn released_nearby_stops_group_once_and_keep_replays_without_changing_result() {
    let snapshot = RacingCatalogSnapshot::embedded_model_v3_fuel_contract().unwrap();
    let request = event_request();
    let config = event_config();
    let expected = run_race_with_catalog_and_v3_fuel_contract_candidate(
        request.clone(),
        &snapshot,
        timeline(),
    )
    .unwrap();
    let mut session =
        LocalControlledRacingSession::start(request.clone(), &snapshot, timeline(), config.clone())
            .unwrap();
    let observation = observe(&mut session);
    assert_eq!(observation.completed_laps, 2);
    let trigger = observation.trigger.as_ref().unwrap();
    assert_eq!(trigger.evidence.len(), 9);
    assert_eq!(trigger.next_planned_pit_lap, 4);
    assert_eq!(
        trigger.observed_at_us,
        observation.window.as_ref().unwrap().opened_at_us
    );
    assert!(trigger.evidence.iter().all(|e| e.lap == 1
        && e.pre_stop_gap_us == 0
        && e.occurred_at_us <= trigger.observed_at_us));
    for _ in 0..5 {
        assert_eq!(observe(&mut session), observation);
    }
    let actual = finish(&mut session, false);
    let decisions = applied(&session);
    assert_eq!(decisions.len(), 1);
    assert_eq!(
        canonical_json_bytes(&actual).unwrap(),
        canonical_json_bytes(&expected).unwrap()
    );
    assert_eq!(
        canonical_json_bytes(&replay(request, &snapshot, timeline(), config, &decisions).unwrap())
            .unwrap(),
        canonical_json_bytes(&actual).unwrap()
    );
}
#[test]
fn event_policy_changes_actual_service_and_binds_the_frozen_episode() {
    let snapshot = RacingCatalogSnapshot::embedded_model_v3_fuel_contract().unwrap();
    let mut session =
        LocalControlledRacingSession::start(event_request(), &snapshot, timeline(), event_config())
            .unwrap();
    let o = observe(&mut session);
    let p = reference_proposal(&o, 2, "hard").unwrap();
    let mut forged = p.clone();
    let mut changed = o.clone();
    changed.trigger.as_mut().unwrap().evidence.clear();
    forged.observation_digest = changed.digest().unwrap();
    assert!(session.propose(forged).is_err());
    assert_eq!(observe(&mut session), o);
    assert_eq!(session.propose(p).unwrap().pit_after_lap, Some(3));
    let out = finish(&mut session, false);
    assert_eq!(applied(&session).len(), 1);
    assert_eq!(out.player_pit_laps, vec![3]);
}
#[test]
fn event_policy_rejects_ambiguous_or_unbounded_config_and_omits_default_fields() {
    let snapshot = RacingCatalogSnapshot::embedded_model_v3_fuel_contract().unwrap();
    let mut c = event_config();
    c.decision_laps.push(1);
    assert!(
        LocalControlledRacingSession::start(event_request(), &snapshot, timeline(), c).is_err()
    );
    let mut c = event_config();
    c.service_windows = None;
    assert!(
        LocalControlledRacingSession::start(event_request(), &snapshot, timeline(), c).is_err()
    );
    let mut p = rival_policy();
    p.max_windows = 65;
    assert!(p.validate().is_err());
    assert!(
        !serde_json::to_string(&config())
            .unwrap()
            .contains("event_policy")
    );
}
