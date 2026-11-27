//! Bounded native/browser bridge reference, no fitting or signed admission.
use pitgun_racing_simulator::{RacingCatalogSnapshot, RunRaceRequest, qualified_career};
use serde_json::{Value, json};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let catalog = RacingCatalogSnapshot::embedded_three_compound()?;
    let fixtures: Value = serde_json::from_str(include_str!(
        "../../../experiments/opponent_physics_v2/results/career-fixtures-v1.json"
    ))?;
    let mut vectors = Vec::new();
    for id in [
        "craft_engineering:era1-entry",
        "craft_engineering:era2-exit",
    ] {
        let f = fixtures["fixtures"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["id"] == id)
            .unwrap();
        let mut request: RunRaceRequest =
            serde_json::from_value(f["preparedRequests"][0]["request"].clone())?;
        request.input.race.laps = 3;
        request.input.pit_strategy = None;
        let player = request
            .input
            .race
            .competitors
            .iter_mut()
            .find(|c| c.is_player)
            .unwrap();
        player.stint_strategy = Some(serde_json::from_value(
            json!({"stints":[{"tire_id":"hard","laps":1},{"tire_id":"medium","laps":1},{"tire_id":"soft","laps":1}],"pit_laps":[1,2]}),
        )?);
        let prepared = qualified_career::prepare_request(request, &catalog)?;
        let output = qualified_career::run(prepared.clone(), &catalog)?;
        vectors.push(json!({"id":id,"request":prepared,"expected":{"standings":output.standings,"player_lap_times_ms":output.player_lap_times_ms}}));
    }
    println!(
        "{}",
        serde_json::to_string_pretty(
            &json!({"schema":"pitgun.three-compound-native-bridge/v1","model":qualified_career::three_compound_model_identity(),"vectors":vectors})
        )?
    );
    Ok(())
}
