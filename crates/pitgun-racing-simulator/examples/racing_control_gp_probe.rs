//! Full-distance local control qualification. No hosted/ranked attestation.
use pitgun_contract::{Digest, IncrementalExecutionStreamEventV1, canonical_json_digest};
use pitgun_racing_contract::{
    CompetitorStintStrategy, RacingDriverInstructionTimelineV1,
    RacingDriverInstructionTimelineVersion,
};
use pitgun_racing_simulator::{
    RaceOutput, RacingCatalogSnapshot, RacingCompetitorProgressStatusV1, RacingSessionEventV1,
    RacingSessionProgressV1, RunRaceRequest,
    control_prototype::{
        Action, AppliedDecision, ControlStep, FallbackReason, JournalEntry, LocalControlConfig,
        LocalControlledRacingSession, PlayerObservation, Proposal, ProposalSource, replay,
    },
    run_race_with_catalog_and_v3_fuel_contract_candidate,
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    time::Instant,
};

#[derive(Default)]
struct ProgressAudit {
    tails: BTreeMap<String, [f64; 2]>,
    facts: BTreeSet<String>,
    last_fact_us: u64,
    grids: usize,
    frames: usize,
    finished_rival: bool,
    max_lap_spread: f64,
    lap_length: f64,
}
impl ProgressAudit {
    fn batch(
        &mut self,
        b: &pitgun_racing_simulator::RacingSessionStreamBatchV1,
    ) -> Result<(), String> {
        let mut horizon = None;
        for record in b.records() {
            if let IncrementalExecutionStreamEventV1::Progress(p) = record.event() {
                match p {
                    RacingSessionProgressV1::Grid { lap, competitors } => {
                        if competitors.len() != 10 {
                            return Err("incomplete grid".into());
                        }
                        let player = competitors
                            .iter()
                            .find(|c| c.competitor_id == "player")
                            .ok_or("missing player")?;
                        let end = player.trajectory.last().ok_or("missing player path")?;
                        horizon = Some((end[0] * 1e6).round() as u64);
                        if self.lap_length == 0.0 {
                            self.lap_length = player.distance_m / f64::from(*lap);
                        }
                        let positions = competitors
                            .iter()
                            .map(|c| c.position)
                            .collect::<BTreeSet<_>>();
                        if positions != (1..=10).collect() {
                            return Err("invalid rank set".into());
                        }
                        let mut min = f64::INFINITY;
                        let mut max = 0.0_f64;
                        for car in competitors {
                            if car.trajectory.len() < 2 {
                                return Err("missing path bracket".into());
                            }
                            if car.trajectory[0]
                                != self
                                    .tails
                                    .get(&car.competitor_id)
                                    .copied()
                                    .unwrap_or([0., 0.])
                            {
                                return Err("broken path continuity".into());
                            }
                            if car.trajectory.iter().any(|p| {
                                !p[0].is_finite() || !p[1].is_finite() || p[0] > end[0] + 1e-9
                            }) || car
                                .trajectory
                                .windows(2)
                                .any(|w| w[1][0] < w[0][0] || w[1][1] < w[0][1])
                            {
                                return Err("future/nonmonotonic path".into());
                            }
                            let tail = *car.trajectory.last().unwrap();
                            if tail[1] != car.distance_m {
                                return Err("grid/path mismatch".into());
                            }
                            if car.status == RacingCompetitorProgressStatusV1::Running
                                && (tail[0] - end[0]).abs() > 1e-9
                            {
                                return Err("unaligned running rival".into());
                            }
                            self.finished_rival |= car.competitor_id != "player"
                                && car.status == RacingCompetitorProgressStatusV1::Finished;
                            self.tails.insert(car.competitor_id.clone(), tail);
                            min = min.min(car.distance_m);
                            max = max.max(car.distance_m);
                        }
                        self.max_lap_spread =
                            self.max_lap_spread.max((max - min) / self.lap_length);
                        self.grids += 1;
                    }
                    RacingSessionProgressV1::PlayerTelemetry { frames, .. } => {
                        self.frames += frames.len()
                    }
                    RacingSessionProgressV1::Event(event) => {
                        let timing = match event {
                            RacingSessionEventV1::LapCompleted { timing, .. }
                            | RacingSessionEventV1::PitStopCompleted { timing, .. }
                            | RacingSessionEventV1::DriverInstructionApplied { timing, .. } => {
                                timing.as_ref().ok_or("missing actor clock")?
                            }
                        };
                        if timing.occurred_at_us > timing.released_at_us
                            || Some(timing.released_at_us) != horizon
                            || timing.occurred_at_us < self.last_fact_us
                            || !self.facts.insert(timing.event_id.to_string())
                        {
                            return Err("future/duplicate/unordered fact".into());
                        }
                        self.last_fact_us = timing.occurred_at_us;
                    }
                }
            }
        }
        Ok(())
    }
    fn summary(&self) -> Value {
        json!({"grids":self.grids,"frames":self.frames,"unique_facts":self.facts.len(),"max_lap_spread":self.max_lap_spread,"saw_finished_rival":self.finished_rival,"all_grids_and_events_valid":true})
    }
}
fn timeline() -> RacingDriverInstructionTimelineV1 {
    RacingDriverInstructionTimelineV1 {
        schema_version: RacingDriverInstructionTimelineVersion::V1,
        events: vec![],
    }
}
fn summary(out: &RaceOutput) -> Result<Value, String> {
    Ok(
        json!({"digest":canonical_json_digest(out).map_err(|e|e.to_string())?,"total_time_ms":out.total_time_ms,"player_time_ms":out.standings.iter().find(|s|s.competitor_id=="player").map(|s|s.total_time_ms),"pit_laps":out.player_pit_laps,"lap_times_ms":out.player_lap_times_ms,"standings":out.standings}),
    )
}
fn run(
    request: RunRaceRequest,
    snapshot: &RacingCatalogSnapshot,
    config: LocalControlConfig,
    mode: &str,
) -> Result<(RaceOutput, Vec<AppliedDecision>, Value), String> {
    let started = Instant::now();
    let mut session = LocalControlledRacingSession::start(request, snapshot, timeline(), config)?;
    let mut observations = Vec::<PlayerObservation>::new();
    let mut audit = ProgressAudit::default();
    let mut refusals = 0;
    let output = loop {
        match session.advance()? {
            ControlStep::AwaitingDecision(o) => {
                for _ in 0..3 {
                    match session.advance()? {
                        ControlStep::AwaitingDecision(again) if again == o => {}
                        _ => return Err("unstable pending window".into()),
                    }
                }
                let w = o.window.as_ref().ok_or("missing window")?;
                let trigger = o.trigger.as_ref().ok_or("missing episode")?;
                if trigger
                    .evidence
                    .iter()
                    .any(|e| e.occurred_at_us > w.opened_at_us)
                {
                    return Err("future evidence".into());
                }
                if mode == "keep" {
                    let mut late = Proposal::for_observation(
                        &o,
                        format!("late-{}", o.decision_sequence),
                        ProposalSource::ReferenceController,
                        Action::KeepPlan,
                    )?;
                    late.decision_at_us = Some(w.deadline_at_us + 1);
                    if session.propose(late).is_ok() {
                        return Err("late proposal accepted".into());
                    }
                    refusals += 1;
                    let bad = Proposal::for_observation(
                        &o,
                        format!("bad-{}", o.decision_sequence),
                        ProposalSource::ReferenceController,
                        Action::PitNextLap {
                            tire_id: "not-delegated".into(),
                        },
                    )?;
                    if session.propose(bad).is_ok() {
                        return Err("undelegated tire accepted".into());
                    }
                    refusals += 1;
                }
                let source = if mode == "timeout" {
                    ProposalSource::Fallback {
                        reason: FallbackReason::Timeout,
                    }
                } else {
                    ProposalSource::ReferenceController
                };
                let action = if mode == "pit" && observations.is_empty() {
                    Action::PitNextLap {
                        tire_id: "hard".into(),
                    }
                } else {
                    Action::KeepPlan
                };
                let mut p = Proposal::for_observation(
                    &o,
                    format!("decision-{}", o.decision_sequence),
                    source,
                    action,
                )?;
                p.decision_at_us = Some(if mode == "timeout" {
                    w.deadline_at_us
                } else {
                    w.opened_at_us + 100_000
                });
                session.propose(p)?;
                observations.push(*o);
            }
            ControlStep::Batch(batch) => {
                audit.batch(&batch)?;
                if let Some(out) = batch.records().iter().find_map(|r| match r.event() {
                    IncrementalExecutionStreamEventV1::Complete(o) => Some(o.clone()),
                    _ => None,
                }) {
                    break out;
                }
            }
        }
    };
    if output.player_lap_times_ms.len() != 53 || output.standings.len() != 10 {
        return Err("incomplete Grand Prix".into());
    }
    if audit.max_lap_spread < 1.0 || !audit.finished_rival {
        return Err("missing lapped/finished rival coverage".into());
    }
    let applied = session
        .journal()
        .iter()
        .filter_map(|e| match e {
            JournalEntry::Applied(a) => Some(a.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    let report = json!({"mode":mode,"elapsed_ms":started.elapsed().as_millis(),"observations":observations,"applied":applied,"refusals":refusals,"progress":audit.summary(),"result":summary(&output)?});
    Ok((output, applied, report))
}
fn main() -> Result<(), String> {
    let file = std::env::args()
        .nth(1)
        .ok_or("Pass the versioned game GP fixture JSON path")?;
    let raw = std::fs::read(file).map_err(|e| e.to_string())?;
    let fixture: Value = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
    if fixture["schema_version"] != "pitgun.local-control-gp-fixture/v1" {
        return Err("wrong fixture version".into());
    }
    let request: RunRaceRequest =
        serde_json::from_value(fixture["request"].clone()).map_err(|e| e.to_string())?;
    let mut config: LocalControlConfig =
        serde_json::from_value(fixture["config"].clone()).map_err(|e| e.to_string())?;
    config.runtime_digest = Digest::from_bytes(
        &std::fs::read(std::env::current_exe().map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?,
    );
    let snapshot =
        RacingCatalogSnapshot::embedded_model_v3_fuel_contract().map_err(|e| e.to_string())?;
    let started = Instant::now();
    if std::env::args().any(|a| a == "--single-keep") {
        let (_, _, report) = run(request, &snapshot, config, "keep")?;
        println!(
            "{}",
            serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
        );
        return Ok(());
    }
    let baseline = run_race_with_catalog_and_v3_fuel_contract_candidate(
        request.clone(),
        &snapshot,
        timeline(),
    )?;
    if std::env::args().any(|a| a == "--baseline-only") {
        println!(
            "{}",
            serde_json::to_string_pretty(&summary(&baseline)?).map_err(|e| e.to_string())?
        );
        return Ok(());
    }
    let expected = canonical_json_digest(&baseline).map_err(|e| e.to_string())?;
    drop(baseline);
    let mut reports = Vec::new();
    for mode in ["keep", "timeout", "pit"] {
        let (out, applied, mut report) = run(request.clone(), &snapshot, config.clone(), mode)?;
        let digest = canonical_json_digest(&out).map_err(|e| e.to_string())?;
        if mode != "pit" && digest != expected {
            return Err(format!("{mode} changed fixed-plan output"));
        }
        let windows = applied
            .iter()
            .map(|a| a.proposal.completed_laps)
            .collect::<Vec<_>>();
        if windows
            != if mode == "pit" {
                vec![16]
            } else {
                vec![16, 36]
            }
        {
            return Err(format!("unexpected {mode} windows: {windows:?}"));
        }
        drop(out);
        let replayed = replay(
            request.clone(),
            &snapshot,
            timeline(),
            config.clone(),
            &applied,
        )?;
        if canonical_json_digest(&replayed).map_err(|e| e.to_string())? != digest {
            return Err("replay changed result".into());
        }
        drop(replayed);
        report["exact_replay"] = json!(true);
        if mode == "pit" {
            let mut fixed = request.clone();
            fixed.input.race.competitors[0].stint_strategy=Some(serde_json::from_value::<CompetitorStintStrategy>(json!({"stints":[{"tire_id":"soft","laps":17},{"tire_id":"hard","laps":36}],"pit_laps":[17]})).map_err(|e|e.to_string())?);
            let fixed =
                run_race_with_catalog_and_v3_fuel_contract_candidate(fixed, &snapshot, timeline())?;
            if canonical_json_digest(&fixed).map_err(|e| e.to_string())? != digest {
                return Err("controlled result differs from equivalent fixed plan".into());
            }
            report["equivalent_fixed_plan"] = json!(true);
        }
        reports.push(report);
    }
    println!("{}",serde_json::to_string_pretty(&json!({"passed":true,"scope":"native Monza53 control qualification, fixed independent rivals; not hosted/ranked or physics calibration","fixture_digest":Digest::from_bytes(&raw),"native_artifact_digest":config.runtime_digest,"elapsed_ms":started.elapsed().as_millis(),"baseline_digest":expected,"reports":reports})).map_err(|e|e.to_string())?);
    Ok(())
}
