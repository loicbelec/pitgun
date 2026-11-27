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
    for vector in proof["vectors"].as_array().unwrap() {
        let request: RunRaceRequest = serde_json::from_value(vector["request"].clone())?;
        let request = qualified_career::prepare_request(request, &catalog)?;
        let inputs = qualified_career::resolve_inputs(&request, &catalog)?;
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
