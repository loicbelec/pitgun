//! Bounded owner-feedback diagnostic. No parameter fit or runtime modification.
use pitgun_contract::canonical_json_digest;
use pitgun_racing_simulator::{RacingCatalogSnapshot, RunRaceRequest, qualified_career};
use pitgun_racing_solver::{
    ResolvedSimulationRequestV3, ResolvedSimulationSessionV3, ResolvedSimulationStepV3,
    SimulationResult,
    lap_boundary::{BoundaryDiagnosticRequestV1, BoundaryDiagnosticVersionV1, BoundaryForecastV1},
};
use serde_json::{Value, json};
fn solve(input: ResolvedSimulationRequestV3) -> Result<SimulationResult, String> {
    let mut s = ResolvedSimulationSessionV3::new_boundary_diagnostic(
        BoundaryDiagnosticRequestV1 {
            schema_version: BoundaryDiagnosticVersionV1::V1,
            forecast: BoundaryForecastV1::ReachableWearV1,
            recompute_invariant: false,
            input,
        },
        false,
    )?;
    loop {
        if let ResolvedSimulationStepV3::Complete(r) = s.advance()? {
            return Ok(*r);
        }
    }
}
fn summary(r: &SimulationResult) -> Value {
    let s = &r.solution;
    let (mut dt_sum, mut heat_sum, mut bulk_sum) = (0., 0., 0.);
    let (mut low, mut high) = (f64::INFINITY, f64::NEG_INFINITY);
    for i in 0..s.t.len().saturating_sub(1) {
        if (3..=5).contains(&s.lap_index[i]) {
            let dt = s.t[i + 1] - s.t[i];
            dt_sum += dt;
            heat_sum += 0.5 * (s.tire_temp[i] + s.tire_temp[i + 1]) * dt;
            bulk_sum += 0.5 * (s.tire_carcass_temp[i] + s.tire_carcass_temp[i + 1]) * dt;
            low = low.min(s.tire_temp[i]);
            high = high.max(s.tire_temp[i]);
        }
    }
    let service_ms =
        (r.longitudinal_energy_v1.as_ref().unwrap().service_time_s * 1000.).round() as u64;
    let total_ms = r
        .lap_times_s
        .iter()
        .map(|t| (t * 1000.).round() as u64)
        .sum::<u64>()
        + service_ms;
    json!({"total_time_ms":total_ms,"elapsed_with_service_s":r.total_time_s,"surface_laps3_5_time_mean_c":heat_sum/dt_sum,"surface_laps3_5_min_c":low,"surface_laps3_5_max_c":high,"carcass_laps3_5_time_mean_c":bulk_sum/dt_sum,"max_gear":s.gear.iter().max(),"result_digest":canonical_json_digest(r).unwrap()})
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let catalog = RacingCatalogSnapshot::embedded_qualified_career()?;
    let refs: Value = serde_json::from_str(include_str!(
        "../../../experiments/qualified_career_v1/native-reference-v1.json"
    ))?;
    let cases = [
        ("balanced", [2, 2, 2, 2], 0.5),
        ("engine_chassis", [0, 4, 4, 0], 0.5),
        ("engine_chassis_cooling", [0, 3, 3, 2], 0.5),
        ("chassis_cooling_long", [0, 6, 0, 2], 1.0),
        ("chassis_cooling_mid", [0, 6, 0, 2], 0.5),
        ("mixed_long", [2, 3, 2, 1], 1.0),
    ];
    let mut opponents = Vec::new();
    let mut players = Vec::new();
    for (name, a, gear) in cases {
        assert_eq!(a.iter().sum::<i32>(), 8);
        let mut wire = refs["vectors"][0]["request"].clone();
        wire["seed"] = json!(1960);
        let player = &mut wire["input"]["competitors"][0];
        player["budget_cap"] = json!(8);
        player["tuning"] = json!({"aero_points":a[0],"chassis_points":a[1],"engine_points":a[2],"cooling_points":a[3],"downforce_slider":0.5,"gear_ratio_slider":gear});
        let req: RunRaceRequest = serde_json::from_value(wire)?;
        let req = qualified_career::prepare_request(req, &catalog)?;
        assert!(
            req.input
                .race
                .competitors
                .iter()
                .all(|c| c.budget_cap == 8.)
        );
        let mut inputs = qualified_career::resolve_inputs(&req, &catalog)?;
        if opponents.is_empty() {
            for (id, input) in &inputs {
                if id != "player" {
                    let input_digest = canonical_json_digest(input)?;
                    let r = solve(input.clone())?;
                    opponents.push(json!({"actor":id,"physical_input_digest":input_digest,"result":summary(&r)}));
                }
            }
        }
        let input = inputs.remove("player").unwrap();
        let input_digest = canonical_json_digest(&input)?;
        let r = solve(input)?;
        let sum = summary(&r);
        let total = sum["total_time_ms"].as_u64().unwrap();
        let rank = 1 + opponents
            .iter()
            .filter(|o| o["result"]["total_time_ms"].as_u64().unwrap() < total)
            .count();
        players.push(json!({"case":name,"allocation_aero_chassis_engine_cooling":a,"gear_slider":gear,"budget":8,"physical_input_digest":input_digest,"position":rank,"result":sum}));
        eprintln!("completed {name}: P{rank}");
    }
    println!(
        "{}",
        serde_json::to_string_pretty(
            &json!({"schema":"pitgun.owner-feedback-bounded-probe/v1","metadata":serde_json::from_str::<Value>(&qualified_career::qualified_career_metadata_json())?,"scope":"15 serial native GP: fixed nine solved once plus six prespecified player setups, Monza I, seed1960, 8 effective points each, same baseline car, common 53-lap soft26/medium27 player plan. No new fit, winning-probability estimate, all-circuit claim or thermal parameter change. Laps3-5 temperatures are pre-stop soft tires.","opponents":opponents,"player_cases":players})
        )?
    );
    Ok(())
}
