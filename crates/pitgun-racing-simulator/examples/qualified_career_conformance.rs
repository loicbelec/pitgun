//! Bounded bridge qualification, not a new fitting campaign.
use pitgun_contract::canonical_json_digest;
use pitgun_racing_simulator::{RacingCatalogSnapshot, RunRaceRequest, qualified_career};
use pitgun_racing_solver::{
    ResolvedSimulationSessionV3, ResolvedSimulationStepV3,
    lap_boundary::{BoundaryDiagnosticRequestV1, BoundaryDiagnosticVersionV1, BoundaryForecastV1},
};
use serde_json::{Value, json};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let catalog = RacingCatalogSnapshot::embedded_qualified_career()?;
    let refs: Value = serde_json::from_str(include_str!(
        "../../../experiments/qualified_career_v1/native-reference-v1.json"
    ))?;
    let mut rows = Vec::new();
    for vector in refs["vectors"].as_array().unwrap() {
        let request: RunRaceRequest = serde_json::from_value(vector["request"].clone())?;
        let prepared = qualified_career::prepare_request(request, &catalog)?;
        for (actor, input) in qualified_career::resolve_inputs(&prepared, &catalog)? {
            let input_digest = canonical_json_digest(&input)?;
            let began = std::time::Instant::now();
            let mut session = ResolvedSimulationSessionV3::new_boundary_diagnostic(
                BoundaryDiagnosticRequestV1 {
                    schema_version: BoundaryDiagnosticVersionV1::V1,
                    forecast: BoundaryForecastV1::ReachableWearV1,
                    recompute_invariant: false,
                    input,
                },
                false,
            )?;
            let result = loop {
                if let ResolvedSimulationStepV3::Complete(result) = session.advance()? {
                    break result;
                }
            };
            let digest = canonical_json_digest(&result)?;
            let expected = &vector["expected"][&actor];
            let passed = input_digest.to_string()
                == expected["physical_input_digest"].as_str().unwrap()
                && digest.to_string() == expected["outcome"]["result_digest"].as_str().unwrap();
            rows.push(json!({"circuit":vector["circuit"],"era":vector["era"],"actor":actor,"passed":passed,"physical_input_digest":input_digest,"result_digest":digest,"time_s":result.total_time_s,"elapsed_ms":began.elapsed().as_secs_f64()*1000.0}));
        }
    }
    let passed = rows.iter().all(|r| r["passed"] == true);
    println!(
        "{}",
        serde_json::to_string_pretty(
            &json!({"schema":"pitgun.qualified-career-native-conformance/v1","passed":passed,"metadata":serde_json::from_str::<Value>(&qualified_career::qualified_career_metadata_json())?,"checks":rows})
        )?
    );
    if !passed {
        return Err("native input/result differs from the sealed campaign".into());
    }
    Ok(())
}
