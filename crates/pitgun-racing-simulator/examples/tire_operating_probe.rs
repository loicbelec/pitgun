//! Frozen native tire-policy qualification; bounded chart data, full native metrics.
#[path = "support/frozen_thermal_input.rs"]
mod frozen;
use pitgun_contract::{Digest, canonical_json_digest};
use pitgun_racing_contract::RacingDrivingMode;
use pitgun_racing_simulator::{
    RacingCatalogSnapshot,
    thermal_driver::{ThermalDriverPolicyV1, apply_thermal_driver_v1, resolve_thermal_driver_v1},
    tire_events::{NativeTireWindowEventDetector, TireEventContextV1},
    tire_preparation::{TirePreparationPolicyV1, prepare_tires_v1},
};
use pitgun_racing_solver::{
    ResolvedSimulationSessionV3, ResolvedSimulationStepV3, effective_mu,
    tire_temperature_grip_factor,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{env, fs, path::Path};

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Case {
    id: String,
    frozen_case: String,
    baseline: bool,
    initial_temperature_c: Option<f64>,
    mode: Option<RacingDrivingMode>,
    air_temperature_c: Option<f64>,
    road_temperature_c: Option<f64>,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().skip(1).collect();
    if args.len() != 3 {
        return Err("usage: tire_operating_probe ROOT CASE POLICY".into());
    }
    let root = Path::new(&args[0]);
    let case: Case = serde_json::from_slice(&fs::read(&args[1])?)?;
    let policy: TirePreparationPolicyV1 = serde_json::from_slice(&fs::read(&args[2])?)?;
    policy.validate()?;
    let catalog = RacingCatalogSnapshot::from_release_dir(root.join("catalogs/racing/v1.8.1"))?;
    let (source, frozen_outcome) = frozen::frozen_input(root, &case.frozen_case, &catalog)?;
    let source_digest = canonical_json_digest(&source)?;
    let (mut input, receipt) = if case.baseline {
        if case.initial_temperature_c.is_some()
            || case.mode.is_some()
            || case.air_temperature_c.is_some()
            || case.road_temperature_c.is_some()
        {
            return Err("baseline forbids candidate overrides".into());
        }
        (source.clone(), None)
    } else {
        let (input, receipt) = prepare_tires_v1(&source, &policy)?;
        (input, Some(receipt))
    };
    let driver = if case.baseline {
        None
    } else {
        let dp: ThermalDriverPolicyV1 = serde_json::from_slice(&fs::read(
            root.join("experiments/thermal_driver_v1/profile-v1.json"),
        )?)?;
        let driver = resolve_thermal_driver_v1(
            &catalog,
            "balanced_reference",
            case.mode.unwrap_or(RacingDrivingMode::Balanced),
            &dp,
            "thermal-driver:player",
        )?;
        apply_thermal_driver_v1(&mut input, &driver)?;
        Some(driver)
    };
    for t in [
        case.initial_temperature_c,
        case.air_temperature_c,
        case.road_temperature_c,
    ]
    .into_iter()
    .flatten()
    {
        if !t.is_finite() || !(0.0..=140.0).contains(&t) {
            return Err("diagnostic temperature outside0..140C".into());
        }
    }
    // Explicit diagnostics change only the initial tire set. New pit sets stay prepared.
    if let Some(t) = case.initial_temperature_c {
        input.state.tire_temp = t;
        input
            .thermal_physics_v2
            .as_mut()
            .unwrap()
            .initial_carcass_temperature_c = t;
    }
    if let Some(t) = case.air_temperature_c {
        input.config.tire_temp_amb = t;
        input.vehicle.engine.t_amb = t;
    }
    if let Some(t) = case.road_temperature_c {
        input
            .thermal_physics_v2
            .as_mut()
            .unwrap()
            .road_temperature_c = t;
    }
    let input_digest = canonical_json_digest(&input)?;
    let thermal = input.thermal_physics_v2.as_ref().unwrap();
    let mut context = TireEventContextV1 {
        tire_id: "compound-0".into(),
        tire: input.vehicle.tire.clone(),
    };
    let mut detector = NativeTireWindowEventDetector::new_with_peak_grip_window(
        "player".into(),
        policy.window.clone(),
    )?;
    let mut session = ResolvedSimulationSessionV3::new(input.clone())?;
    let mut expected_surface = input.state.tire_temp;
    let mut expected_carcass = thermal.initial_carcass_temperature_c;
    let mut max_surface_jump: f64 = 0.0;
    let mut max_carcass_jump: f64 = 0.0;
    let mut events = Vec::new();
    let mut laps = Vec::new();
    let mut chart: Vec<[f64; 9]> = Vec::new();
    let mut next_chart_s = 0.0;
    let mut elapsed = 0.0;
    let mut distance = 0.0;
    let mut stint = 0u16;
    let outcome = loop {
        match session.advance() {
            Ok(ResolvedSimulationStepV3::Lap(lap)) => {
                let s = &lap.solution;
                max_surface_jump = max_surface_jump.max((s.tire_temp[0] - expected_surface).abs());
                max_carcass_jump =
                    max_carcass_jump.max((s.tire_carcass_temp[0] - expected_carcass).abs());
                let (low, high) = policy.window.bounds(&context.tire)?;
                let mut bands = [0.0; 3];
                let mut heating = 0.0;
                let mut cooling = 0.0;
                let mut floor_time = 0.0;
                for i in 0..s.t.len() {
                    let t = elapsed + s.t[i];
                    let grip = tire_temperature_grip_factor(s.tire_temp[i], &context.tire);
                    if t >= next_chart_s || i == 0 || i + 1 == s.t.len() {
                        chart.push([
                            t,
                            distance + s.s[i],
                            f64::from(lap.lap_index),
                            f64::from(stint),
                            s.tire_temp[i],
                            s.tire_carcass_temp[i],
                            s.tire_wear[i],
                            grip,
                            effective_mu(1.0, s.tire_wear[i], s.tire_temp[i], &context.tire),
                        ]);
                        if t >= next_chart_s {
                            let interval = if t < 300.0 { 1.0 } else { 10.0 };
                            next_chart_s = (t / interval).floor() * interval + interval;
                        }
                    }
                    if i + 1 < s.t.len() {
                        let dt = s.t[i + 1] - s.t[i];
                        let delta = s.tire_temp[i + 1] - s.tire_temp[i];
                        if delta > 1e-8 {
                            heating += dt;
                        }
                        if delta < -1e-8 {
                            cooling += dt;
                        }
                        bands[if s.tire_temp[i] < low {
                            0
                        } else if s.tire_temp[i] > high {
                            2
                        } else {
                            1
                        }] += dt;
                        if grip <= context.tire.temp_min_k + 1e-12 {
                            floor_time += dt;
                        }
                    }
                }
                events.extend(detector.observe_driving_lap(&lap, &context)?);
                let before = json!({"surface_c":s.tire_temp.last(),"carcass_c":s.tire_carcass_temp.last(),"wear":s.tire_wear.last()});
                let range = |v: &[f64]| {
                    [
                        v.iter().copied().fold(f64::INFINITY, f64::min),
                        v.iter().copied().fold(f64::NEG_INFINITY, f64::max),
                    ]
                };
                let mut next_context = context.clone();
                if lap.pit_stop_completed {
                    let stop = input
                        .pit_plan
                        .stops
                        .iter()
                        .find(|p| p.lap == lap.lap_index)
                        .ok_or("missing pit")?;
                    stint += 1;
                    next_context = TireEventContextV1 {
                        tire_id: format!("compound-{stint}"),
                        tire: stop.tire.clone(),
                    };
                }
                events.extend(detector.observe_lap_boundary(&lap, &next_context)?);
                laps.push(json!({"lap":lap.lap_index,"time_s":lap.lap_time_s,"elapsed_s":lap.cumulative_time_s,"compound":context,"window_c":[low,high],"surface_range_c":range(&s.tire_temp),"carcass_range_c":range(&s.tire_carcass_temp),"heating_s":heating,"cooling_s":cooling,"band_occupancy_s":{"cold":bands[0],"near_peak":bands[1],"hot":bands[2]},"grip_floor_s":floor_time,"before_service":before,"after_service":lap.state_after_lap,"pit":lap.pit_stop_completed,"thermal":lap.thermal_v2}));
                expected_surface = lap.state_after_lap.tire_temp;
                expected_carcass = lap.thermal_v2.unwrap().carcass_temperature_c;
                elapsed = lap.cumulative_time_s;
                distance = lap.cumulative_distance_m;
                context = next_context;
            }
            Ok(ResolvedSimulationStepV3::Complete(r)) => {
                let digest = canonical_json_digest(&r)?;
                break json!({"ok":true,"result_digest":digest,"frozen_outcome_equal":case.baseline.then(||json!(digest)==frozen_outcome["digest"]),"time_s":r.total_time_s,"final_state":r.final_state,"thermal":r.thermal_v2});
            }
            Err(error) => break json!({"ok":false,"error":error,"accepted_laps":laps.len()}),
        }
    };
    let events: Vec<_> = events
        .iter()
        .map(|e| json!({"occurred_at_us":e.occurred_at_us,"event":e.event}))
        .collect();
    println!(
        "{}",
        json!({"schema":"pitgun.tire-operating-probe/v1","case":case,"catalog":catalog.release_identity(),"source_input_digest":source_digest,"preparation":receipt,"diagnostic_input_digest":canonical_json_digest(&(&case,&policy,&input_digest))?,"input_digest":input_digest,"resolved_initial_conditions":{"surface_c":input.state.tire_temp,"carcass_c":thermal.initial_carcass_temperature_c,"pit_surface_c":input.config.pit_tire_temp.unwrap_or(input.state.tire_temp),"pit_carcass_c":thermal.pit_carcass_temperature_c,"air_c":input.config.tire_temp_amb,"road_c":thermal.road_temperature_c},"policy":policy,"driver":driver,"laps":laps,"outcome":outcome,"maximum_surface_jump_c":max_surface_jump,"maximum_carcass_jump_c":max_carcass_jump,"events":events,"chart":{"initial_interval_s":1,"initial_duration_s":300,"interval_s":10,"columns":["time_s","distance_m","lap","stint","surface_c","carcass_c","wear_fraction","temperature_grip_factor","effective_mu_over_mu0"],"rows":chart},"binary_digest":Digest::from_bytes(&fs::read(env::current_exe()?)?),"sources":{"probe":Digest::from_bytes(include_bytes!("tire_operating_probe.rs")),"preparation":Digest::from_bytes(include_bytes!("../src/tire_preparation.rs")),"detector":Digest::from_bytes(include_bytes!("../src/tire_events.rs")),"helper":Digest::from_bytes(include_bytes!("support/frozen_thermal_input.rs")),"solver":Digest::from_bytes(include_bytes!("../../pitgun-racing-solver/src/lib.rs")),"thermal":Digest::from_bytes(include_bytes!("../../pitgun-racing-solver/src/thermal_physics.rs"))}})
    );
    Ok(())
}
