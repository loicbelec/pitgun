//! Native observation-only event bank; reconstructs and checks frozen V2 inputs.
//! Normal and pre-service/checkpoint execution must keep both physics and facts.
use pitgun_contract::{Digest, canonical_json_digest};
use pitgun_racing_simulator::physical_events::{
    DeratingTransitionV1, NativeDeratingEventDetector, RacingPhysicsEventV1,
};
use pitgun_racing_simulator::{
    RacingCatalogSnapshot, RacingSessionEventV1, ResolvedSimulationSessionV3,
    ResolvedSimulationStepV3, RunRaceRequest, resolve_career_physics_inputs_for_audit,
};
use pitgun_racing_solver::longitudinal_energy::LongitudinalEnergyModelV1;
use pitgun_racing_solver::{
    ResolvedServiceStepV3, ResolvedSimulationLapV3, ResolvedSimulationRequestV3, SimulationResult,
};
use serde_json::{Value, json};
use std::{env, fs, path::Path};

fn time_us(t: f64) -> u64 {
    (t * 1e6).round() as u64
}
fn bind_driving(
    detector: &mut NativeDeratingEventDetector,
    lap: &ResolvedSimulationLapV3,
    input: &ResolvedSimulationRequestV3,
    offset: f64,
    queue: &mut Vec<(u64, RacingSessionEventV1)>,
) -> Result<(), String> {
    queue.extend(
        detector
            .observe_driving_lap(
                lap,
                &input.vehicle.engine,
                input.engine_thermal.unwrap_or_default(),
            )?
            .into_iter()
            .map(|f| f.into_unreleased()),
    );
    queue.push((
        time_us(offset + lap.solution.t.last().unwrap()),
        RacingSessionEventV1::LapCompleted {
            competitor_id: "player".into(),
            lap: lap.lap_index,
            timing: None,
        },
    ));
    Ok(())
}
fn bind_boundary(
    detector: &mut NativeDeratingEventDetector,
    lap: &ResolvedSimulationLapV3,
    input: &ResolvedSimulationRequestV3,
    queue: &mut Vec<(u64, RacingSessionEventV1)>,
    journal: &mut Vec<RacingSessionEventV1>,
) -> Result<(), String> {
    if lap.pit_stop_completed {
        queue.push((
            time_us(lap.cumulative_time_s),
            RacingSessionEventV1::PitStopCompleted {
                competitor_id: "player".into(),
                lap: lap.lap_index,
                timing: None,
            },
        ));
    }
    queue.extend(
        detector
            .observe_lap_boundary(
                lap,
                &input.vehicle.engine,
                input.engine_thermal.unwrap_or_default(),
            )?
            .into_iter()
            .map(|f| f.into_unreleased()),
    );
    let phase = |e: &RacingSessionEventV1| match e {
        RacingSessionEventV1::LapCompleted { .. } => 0,
        RacingSessionEventV1::PitStopCompleted { .. } => 1,
        RacingSessionEventV1::DriverInstructionApplied { .. } => 2,
        RacingSessionEventV1::Physical { .. } => 3,
    };
    queue.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| phase(&a.1).cmp(&phase(&b.1))));
    for (time, event) in queue.drain(..) {
        journal.push(event.release_at_horizon(time, time_us(lap.cumulative_time_s))?);
    }
    Ok(())
}
fn round_trip_checkpoint(
    detector: &mut NativeDeratingEventDetector,
    queue: &mut Vec<(u64, RacingSessionEventV1)>,
    journal: &mut Vec<RacingSessionEventV1>,
) -> Result<(), String> {
    let bytes = serde_json::to_vec(&(detector.checkpoint(), &*queue, &*journal))
        .map_err(|e| e.to_string())?;
    let (state, pending, persisted): (
        pitgun_racing_simulator::physical_events::DeratingDetectorCheckpointV1,
        Vec<(u64, RacingSessionEventV1)>,
        Vec<RacingSessionEventV1>,
    ) = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    *detector = NativeDeratingEventDetector::restore(state)?;
    *queue = pending;
    *journal = persisted;
    Ok(())
}
fn execute(
    input: &ResolvedSimulationRequestV3,
    checkpoint_service: bool,
) -> Result<(SimulationResult, Vec<RacingSessionEventV1>), String> {
    let mut session = if checkpoint_service {
        ResolvedSimulationSessionV3::new_with_service_boundaries(input.clone())?
    } else {
        ResolvedSimulationSessionV3::new(input.clone())?
    };
    let mut detector = NativeDeratingEventDetector::new("player".into(), Default::default())?;
    let mut queue = Vec::new();
    let mut journal = Vec::new();
    let mut offset = 0.0;
    loop {
        if checkpoint_service {
            match session.advance_service()? {
                ResolvedServiceStepV3::BeforeService(lap) => {
                    bind_driving(&mut detector, &lap, input, offset, &mut queue)?;
                    // Restore while the driving prefix is committed but service is open.
                    round_trip_checkpoint(&mut detector, &mut queue, &mut journal)?;
                    session.resolve_service_boundary(None)?;
                }
                ResolvedServiceStepV3::Lap(lap) => {
                    bind_boundary(&mut detector, &lap, input, &mut queue, &mut journal)?;
                    offset = lap.cumulative_time_s;
                    round_trip_checkpoint(&mut detector, &mut queue, &mut journal)?;
                }
                ResolvedServiceStepV3::Complete(r) => return Ok((*r, journal)),
            }
        } else {
            match session.advance()? {
                ResolvedSimulationStepV3::Lap(lap) => {
                    bind_driving(&mut detector, &lap, input, offset, &mut queue)?;
                    bind_boundary(&mut detector, &lap, input, &mut queue, &mut journal)?;
                    offset = lap.cumulative_time_s;
                }
                ResolvedSimulationStepV3::Complete(r) => return Ok((*r, journal)),
            }
        }
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 1 {
        return Err("usage: native_derating_events_probe REPOSITORY_ROOT".into());
    }
    let root = Path::new(&args[0]);
    let exp = root.join("experiments/thermal_physics_v2");
    let frozen_bytes = fs::read(exp.join("results/screen-v1.json"))?;
    let frozen: Value = serde_json::from_slice(&frozen_bytes)?;
    let plan: Value = serde_json::from_slice(&fs::read(exp.join("plan-v1.json"))?)?;
    let params: Value = serde_json::from_slice(&fs::read(exp.join("profile-v1.json"))?)?;
    let catalog = RacingCatalogSnapshot::from_release_dir(root.join("catalogs/racing/v1.8.1"))?;
    let source = [
        (
            "crates/pitgun-racing-solver/src/lib.rs",
            include_bytes!("../../pitgun-racing-solver/src/lib.rs").as_slice(),
        ),
        (
            "crates/pitgun-racing-solver/src/longitudinal_energy.rs",
            include_bytes!("../../pitgun-racing-solver/src/longitudinal_energy.rs").as_slice(),
        ),
        (
            "crates/pitgun-racing-solver/src/thermal_physics.rs",
            include_bytes!("../../pitgun-racing-solver/src/thermal_physics.rs").as_slice(),
        ),
    ];
    for (name, bytes) in source {
        if frozen["identity"]["source_files"][name] != json!(Digest::from_bytes(bytes)) {
            return Err(format!("physical source changed: {name}").into());
        }
    }
    let mut cases = Vec::new();
    for id in [
        "budget-early8-monza-c0",
        "budget-early8-monza-c2",
        "budget-modern20-monza-c0",
        "budget-modern20-monza-c4",
        "full-classic15-c0",
    ] {
        let case = plan["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["id"] == id)
            .ok_or("missing plan case")?;
        let baseline = &frozen["results"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["case"] == id && c["candidate"] == true)
            .ok_or("missing frozen result")?["data"];
        let request: RunRaceRequest = serde_json::from_value(case["request"].clone())?;
        let mut inputs = resolve_career_physics_inputs_for_audit(request, &catalog)?;
        let mut input = inputs.remove("player").ok_or("missing player")?;
        let p = &baseline["resolved_parameters"];
        input.vehicle = serde_json::from_value(p["vehicle"].clone())?;
        input.mechanical = serde_json::from_value(p["mechanical"].clone())?;
        input.tire_contact = serde_json::from_value(p["contact"].clone())?;
        input.fuel_mass = serde_json::from_value(p["fuel_mass"].clone())?;
        input.state = serde_json::from_value(p["initial_state"].clone())?;
        input.config = serde_json::from_value(p["config"].clone())?;
        input.driver_control = serde_json::from_value(p["driver_control"].clone())?;
        input.thermal_physics_v2 = serde_json::from_value(p["thermal_physics_v2"].clone())?;
        input.engine_thermal = Some(Default::default());
        input.longitudinal_energy = Some(LongitudinalEnergyModelV1::V1);
        let stints = case["request"]["input"]["competitors"][0]["stint_strategy"]["stints"]
            .as_array()
            .ok_or("missing stints")?;
        for (stop, stint) in input.pit_plan.stops.iter_mut().zip(stints.iter().skip(1)) {
            let c = &params["compounds"][stint["tire_id"].as_str().ok_or("missing tire id")?];
            stop.tire.mu_scale = c["grip_scale"].as_f64().ok_or("grip")?;
            stop.tire.temp_opt = c["temperature_optimum_c"].as_f64().ok_or("opt")?;
            stop.tire.temp_sigma = c["temperature_width_c"].as_f64().ok_or("width")?;
            stop.tire.temp_min_k = c["minimum_temperature_grip_factor"]
                .as_f64()
                .ok_or("floor")?;
        }
        let input_digest = canonical_json_digest(&input)?;
        if json!(input_digest) != baseline["input_digest"] {
            return Err(format!("frozen resolved input differs: {id}").into());
        }
        let (result, journal) = execute(&input, false)?;
        let result_digest = canonical_json_digest(&result)?;
        if json!(result_digest) != baseline["outcome"]["digest"] {
            return Err(format!("physical output changed: {id}").into());
        }
        let (resumed, resumed_journal) = execute(&input, true)?;
        if resumed != result || resumed_journal != journal {
            return Err(format!("service/checkpoint replay differs: {id}").into());
        }
        let mut ids = std::collections::BTreeSet::new();
        let mut lap_count = 0;
        let mut pit_count = 0;
        let mut starts = 0;
        let mut recoveries = 0;
        for event in &journal {
            let timing = match event {
                RacingSessionEventV1::LapCompleted { timing, .. } => {
                    lap_count += 1;
                    timing
                }
                RacingSessionEventV1::PitStopCompleted { timing, .. } => {
                    pit_count += 1;
                    timing
                }
                RacingSessionEventV1::DriverInstructionApplied { timing, .. } => timing,
                RacingSessionEventV1::Physical {
                    event: RacingPhysicsEventV1::EngineDerating(data),
                    timing,
                } => {
                    match data.transition {
                        DeratingTransitionV1::Started => starts += 1,
                        DeratingTransitionV1::Recovered => recoveries += 1,
                    };
                    timing
                }
            }
            .as_ref()
            .ok_or("missing release timing")?;
            if timing.occurred_at_us > timing.released_at_us
                || !ids.insert(timing.event_id.to_string())
            {
                return Err("future or duplicate event".into());
            }
        }
        if lap_count != usize::from(input.lap_count) || pit_count != input.pit_plan.stops.len() {
            return Err("lost lap/pit events".into());
        }
        cases.push(json!({"id":id,"input_digest":input_digest,"physical_result_digest":result_digest,
            "physical_result_matches_frozen":true,"service_and_checkpoint_equal":true,"lap_events":lap_count,
            "pit_events":pit_count,"derating_starts":starts,"derating_recoveries":recoveries,
            "journal_digest":canonical_json_digest(&journal)?,"journal":journal}));
    }
    println!(
        "{}",
        json!({"schema":"pitgun.native-derating-events-qualification/v1","frozen_screen_digest":Digest::from_bytes(&frozen_bytes),"physical_sources_match_frozen":true,
        "binary_digest":Digest::from_bytes(&fs::read(env::current_exe()?)?),"catalog":catalog.release_identity(),
        "sources":{"simulator":Digest::from_bytes(include_bytes!("../src/lib.rs")),"detector":Digest::from_bytes(include_bytes!("../src/physical_events.rs")),"probe":Digest::from_bytes(include_bytes!("native_derating_events_probe.rs"))},
        "policy":pitgun_racing_simulator::physical_events::DeratingEventPolicyV1::default(),"cases":cases,"scope":"Native audit only, exact frozen physical inputs/results. No UI/provider/Authority admission; raw state transitions, no relevance calibration."})
    );
    Ok(())
}
