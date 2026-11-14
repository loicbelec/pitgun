//! Native contract preparation/receipt projection for the isolated HTTP/WASM
//! integration recipe. Does not execute controllers or hold signing material.
use pitgun_racing_simulator::{
    RaceOutput, RacingCatalogSnapshot, RunRaceRequest,
    control_prototype::{JournalEntry, PlayerObservation},
    service_control::{
        RacingServiceAuthorizationResponseV1, RacingServiceDecisionV1, empty_timeline,
        service_control_evidence, validate_service_start,
    },
};
use std::{env, fs};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() < 5 {
        return Err(
            "Usage: probe prepare AUTH CATALOG OUTPUT | evidence AUTH CATALOG OUTPUT COMPLETION"
                .into(),
        );
    }
    let response: RacingServiceAuthorizationResponseV1 =
        serde_json::from_slice(&fs::read(&args[2])?)?;
    let catalog = RacingCatalogSnapshot::from_release_dir(&args[3])?;
    let config = validate_service_start(&response, &catalog)?;
    let value = match args[1].as_str() {
        "observation" => {
            let mut session = pitgun_racing_simulator::service_control::start_service_control(
                &response, &catalog,
            )?;
            loop {
                if let pitgun_racing_simulator::control_prototype::ControlStep::AwaitingDecision(
                    o,
                ) = session.advance()?
                {
                    break serde_json::to_value(o)?;
                }
            }
        }
        "prepare" => {
            let input = response.canonical_input;
            let request = RunRaceRequest {
                era: Some(input.era),
                hz: Some(input.hz),
                input,
                seed: response
                    .signed
                    .authorization
                    .initial_contract
                    .random
                    .seed
                    .get(),
            };
            serde_json::json!({"requestJson":serde_json::to_string(&request)?,"catalogBundleJson":serde_json::to_string(&catalog.to_bundle()?)?,
                "timelineJson":serde_json::to_string(&empty_timeline())?,"configJson":serde_json::to_string(&config)?})
        }
        "evidence" => {
            let completion: serde_json::Value =
                serde_json::from_slice(&fs::read(args.get(5).ok_or("completion path required")?)?)?;
            let output: RaceOutput = serde_json::from_value(completion["result"].clone())?;
            let entries: Vec<JournalEntry> = serde_json::from_value(completion["journal"].clone())?;
            let mut observation: Option<PlayerObservation> = None;
            let mut history = vec![];
            for e in entries {
                match e {
                    JournalEntry::Observation(o) => {
                        if observation.replace(o).is_some() {
                            return Err("unresolved observation in completion".into());
                        }
                    }
                    JournalEntry::Applied(applied) => history.push(RacingServiceDecisionV1 {
                        observation_digest: {
                            observation.take().ok_or("receipt without observation")?;
                            applied.proposal.observation_digest
                        },
                        applied,
                    }),
                    _ => {}
                }
            }
            if observation.is_some() {
                return Err("unfinished service history".into());
            }
            // Optional hostile-client vector for the isolated verification recipe.
            // The Verifier must reject it even after all hashes are recomputed.
            if let Some(path) = args.get(6) {
                history = serde_json::from_slice(&fs::read(path)?)?;
            }
            serde_json::to_value(service_control_evidence(
                response, &catalog, history, &output,
            )?)?
        }
        _ => return Err("unknown operation".into()),
    };
    fs::write(&args[4], serde_json::to_vec(&value)?)?;
    Ok(())
}
