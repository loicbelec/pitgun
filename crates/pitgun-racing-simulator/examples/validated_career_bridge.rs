//! Resolve old native evidence through the new runtime, without rerunning a GP.
use pitgun_contract::canonical_json_digest;
use pitgun_racing_simulator::{RacingCatalogSnapshot, RunRaceRequest, qualified_career};
use serde_json::{Value, json};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proof: Value = serde_json::from_str(include_str!(
        "../../../experiments/validated_runtime_v1/bridge-inputs-v1.json"
    ))?;
    let catalog = RacingCatalogSnapshot::embedded_validated_career()?;
    let mut vectors = Vec::new();
    let mut checked = 0;
    let inspect = std::env::args().nth(1);
    if inspect.as_deref() == Some("limits") {
        let p = qualified_career::profile(&catalog)?;
        let mut limits = Vec::new();
        for (era, circuit) in [(1, "MONZA"), (2, "MEXICO")] {
            let ctx = p
                .contexts
                .iter()
                .find(|c| c.era == era && c.circuit == circuit)
                .unwrap();
            let cap = ctx.cases.iter().map(|c| c.budget).max().unwrap();
            let case = ctx
                .cases
                .iter()
                .find(|c| c.budget == cap && c.actor == "franz_hermann")
                .unwrap();
            let vector = proof["vectors"]
                .as_array()
                .unwrap()
                .iter()
                .find(|v| {
                    v["full_race_qa"] == true
                        && v["id"].as_str().unwrap().starts_with(&era.to_string())
                })
                .unwrap();
            let mut request: RunRaceRequest = serde_json::from_value(vector["request"].clone())?;
            let tuning = serde_json::from_value(
                json!({"aero_points":case.allocation[0],"chassis_points":case.allocation[1],"engine_points":case.allocation[2],"cooling_points":case.allocation[3],"gear_ratio_slider":case.gear,"downforce_slider":case.downforce}),
            )?;
            let player = request
                .input
                .race
                .competitors
                .iter_mut()
                .find(|c| c.is_player)
                .unwrap();
            player.budget_cap = f64::from(cap);
            player.tuning = tuning;
            player.stint_strategy = Some(case.strategy.clone());
            request.input.pit_strategy = None;
            let request = qualified_career::prepare_request(request, &catalog)?;
            let digests: std::collections::BTreeMap<_, _> =
                qualified_career::resolve_inputs(&request, &catalog)?
                    .iter()
                    .map(|(id, input)| canonical_json_digest(input).map(|d| (id.clone(), d)))
                    .collect::<Result<_, _>>()?;
            let started = std::time::Instant::now();
            let result = qualified_career::run(request.clone(), &catalog)?;
            if result.standings.len() != 10 {
                return Err("incomplete upper-budget field".into());
            }
            limits.push(json!({"id":format!("era{era}-cap{cap}"),"request":request,"physical_inputs":digests,"expected":{"standings":result.standings,"player_lap_times_ms":result.player_lap_times_ms},"native_wall_s":started.elapsed().as_secs_f64(),"full_race_qa":true}));
        }
        println!(
            "{}",
            serde_json::to_string_pretty(
                &json!({"schema":"pitgun.validated-runtime-upper-envelope-smoke/v1","model":qualified_career::validated_model_identity(),"vectors":limits})
            )?
        );
        return Ok(());
    }
    for vector in proof["vectors"].as_array().unwrap() {
        let request: RunRaceRequest = serde_json::from_value(vector["request"].clone())?;
        let request = qualified_career::prepare_request(request, &catalog)?;
        let inputs = qualified_career::resolve_inputs(&request, &catalog)?;
        if inspect.as_deref() == vector["id"].as_str() {
            println!("{}", serde_json::to_string(&inputs)?);
            return Ok(());
        }
        for (id, expected) in vector["expected"].as_object().unwrap() {
            if canonical_json_digest(&inputs[id])?.to_string()
                != expected["physical_input_digest"].as_str().unwrap()
            {
                return Err(
                    format!("retained physical input differs: {} {id}", vector["id"]).into(),
                );
            }
            checked += 1;
        }
        vectors.push(json!({"id":vector["id"],"request":request,"expected":vector["expected"],"full_race_qa":vector["full_race_qa"]}));
    }
    println!(
        "{}",
        serde_json::to_string_pretty(
            &json!({"schema":"pitgun.validated-runtime-native-bridge/v1","model":qualified_career::validated_model_identity(),"checked_inputs":checked,"vectors":vectors})
        )?
    );
    Ok(())
}
