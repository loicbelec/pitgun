//! Offline lot-0 diagnostic: existing career physics, no authorization or model changes.
use std::{collections::BTreeMap, env, fs, time::Instant};

use pitgun_contract::{Digest, IncrementalExecutionStreamEventV1, canonical_json_digest};
use pitgun_racing_simulator::{
    RaceOutput, RacingCatalogSnapshot, RunRaceRequest,
    control_prototype::{
        CareerControlPhysics, ControlStep, LocalControlConfig, LocalControlledRacingSession,
        ServiceWindowPolicy, ServiceWindowVersion,
    },
    racing_model_v3_component_candidate_identity,
    service_control::empty_timeline,
};
use serde::Serialize;
use serde_json::json;

#[derive(Serialize)]
struct ChannelRange {
    samples: usize,
    minimum: f64,
    maximum: f64,
    last_exported_sample: f64,
    above_100_count: usize,
}

fn summarize(output: &RaceOutput) -> Result<serde_json::Value, String> {
    let mut channels = BTreeMap::<u16, ChannelRange>::new();
    for sample in output
        .player_batches
        .iter()
        .flat_map(|b| &b.frames)
        .flat_map(|f| &f.samples)
    {
        let Some(value) = sample.value.as_f64() else {
            continue;
        };
        if !value.is_finite() {
            return Err(format!(
                "non-finite exported channel {}",
                sample.parameter_id
            ));
        }
        let range = channels.entry(sample.parameter_id).or_insert(ChannelRange {
            samples: 0,
            minimum: value,
            maximum: value,
            last_exported_sample: value,
            above_100_count: 0,
        });
        range.samples += 1;
        range.minimum = range.minimum.min(value);
        range.maximum = range.maximum.max(value);
        range.last_exported_sample = value;
        range.above_100_count += usize::from(value > 100.0);
    }
    // This hashes the existing canonical output; it is not a new physical invariant.
    let digest = canonical_json_digest(output).map_err(|e| e.to_string())?;
    Ok(json!({
        "output_digest":digest,
        "total_time_ms":output.total_time_ms,
        "standings":output.standings,
        "lap_times_ms":output.player_lap_times_ms,
        "pit_laps":output.player_pit_laps,
        "exported_channels":channels,
        "setup":output.player_diagnostics,
        "tires":output.player_tire_diagnostics_v3,
        "wear":output.player_tire_degradation_diagnostics_v3,
        "mechanical":output.player_mechanical_diagnostics_v3,
        "fuel":output.player_fuel_mass_diagnostics_v3,
        "thermal_resolution":output.competitor_power_unit_thermal_resolutions_v3,
        "vehicle_resolution":output.competitor_vehicle_capabilities_v3,
        "driver_control_resolution":output.competitor_driver_control_resolutions_v3,
    }))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    let power_v1 = args.len() == 3 && args[2] == "--power-v1";
    if args.len() != 2 && !power_v1 {
        return Err(
            "usage: physics_baseline_probe REQUEST_JSON CATALOG_RELEASE_DIR [--power-v1]".into(),
        );
    }
    let began = Instant::now();
    let request: RunRaceRequest = serde_json::from_slice(&fs::read(&args[0])?)?;
    if request.input.race.laps > 100 || request.input.race.competitors.len() > 10 {
        return Err("lot-0 probe is bounded to100 laps and10 cars".into());
    }
    let snapshot = RacingCatalogSnapshot::from_release_dir(&args[1])?;
    let native_digest = Digest::from_bytes(&fs::read(env::current_exe()?)?);
    // Embed source bytes at compilation so an old executable cannot be silently
    // associated with newly edited equations by the local runner.
    let compiled_sources = BTreeMap::from([
        (
            "crates/pitgun-racing-solver/src/power_telemetry.rs",
            Digest::from_bytes(include_bytes!(
                "../../pitgun-racing-solver/src/power_telemetry.rs"
            )),
        ),
        (
            "crates/pitgun-racing-solver/src/lib.rs",
            Digest::from_bytes(include_bytes!("../../pitgun-racing-solver/src/lib.rs")),
        ),
        (
            "crates/pitgun-racing-simulator/src/lib.rs",
            Digest::from_bytes(include_bytes!("../src/lib.rs")),
        ),
        (
            "crates/pitgun-racing-simulator/src/control_prototype.rs",
            Digest::from_bytes(include_bytes!("../src/control_prototype.rs")),
        ),
        (
            "crates/pitgun-racing-simulator/src/thermal_profile.rs",
            Digest::from_bytes(include_bytes!("../src/thermal_profile.rs")),
        ),
        (
            "crates/pitgun-racing-simulator/src/workload.rs",
            Digest::from_bytes(include_bytes!("../src/workload.rs")),
        ),
        (
            "crates/pitgun-racing-simulator/examples/physics_baseline_probe.rs",
            Digest::from_bytes(include_bytes!("physics_baseline_probe.rs")),
        ),
    ]);
    let request_digest = canonical_json_digest(&request)?;
    let config = LocalControlConfig {
        execution_id: "physics-lot0-public-baseline".into(),
        runtime_digest: native_digest,
        physics: Some(CareerControlPhysics::PowerUnitThermalV1),
        decision_laps: vec![],
        allowed_tire_ids: vec!["soft".into(), "medium".into(), "hard".into()],
        service_windows: Some(ServiceWindowPolicy {
            schema_version: ServiceWindowVersion::V1,
            closing_margin_us: 5_000_000,
        }),
        event_policy: None,
        planned_stop_reviews: None,
        strategy_observation: None,
    };
    let config_json = serde_json::to_value(&config)?;
    let mut session =
        LocalControlledRacingSession::start(request, &snapshot, empty_timeline(), config)?;
    let prepare_ms = began.elapsed().as_secs_f64() * 1000.0;
    let mut advance_ms = 0.0;
    let mut advance_calls = Vec::new();
    let mut power_laps = BTreeMap::new();
    let mut power_projection_ms = 0.0;
    let (summary, summary_ms) = loop {
        let start = Instant::now();
        let step = session.advance()?;
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;
        advance_ms += elapsed;
        advance_calls.push(elapsed);
        if power_v1 {
            let start = Instant::now();
            if let Some((lap, projection)) = session.latest_player_power_telemetry_v1()? {
                let maximum_load = projection
                    .points
                    .iter()
                    .map(|p| p.equivalent_engine_load_pct)
                    .fold(0.0_f64, f64::max);
                power_laps.insert(
                    lap,
                    json!({
                        "lap":lap,"schema_version":projection.schema_version,
                        "pedal_command_pct":projection.pedal_command_pct,
                        "brake_command_pct":projection.brake_command_pct,
                        "native_nodes":projection.points.len(),
                        "maximum_equivalent_engine_load_pct":maximum_load,
                    "export_1hz_samples":projection.resample(1.0)?.len(),
                    "export_5hz_samples":projection.resample(5.0)?.len(),
                    "maximum_port_residual_w":projection.points.iter().map(|p|
                        (p.engine_delivered_w-p.wheel_propulsion_w-p.driveline_loss_w-p.launch_regularization_w).abs()
                    ).fold(0.0_f64,f64::max),
                        "projection_digest":canonical_json_digest(&projection)?,
                        "ledger":projection.ledger,
                    }),
                );
            }
            power_projection_ms += start.elapsed().as_secs_f64() * 1000.0;
        }
        match step {
            ControlStep::AwaitingDecision(_) => {
                return Err("baseline unexpectedly requested a controller".into());
            }
            ControlStep::Batch(batch) => {
                if let Some(output) = batch.records().iter().find_map(|r| match r.event() {
                    IncrementalExecutionStreamEventV1::Complete(o) => Some(o),
                    _ => None,
                }) {
                    let start = Instant::now();
                    let summary = summarize(output)?;
                    break (summary, start.elapsed().as_secs_f64() * 1000.0);
                }
            }
        }
    };
    println!(
        "{}",
        json!({
            "schema":"pitgun.physics-baseline-probe/v1",
            "identity":{
                "request_digest":request_digest,
                "native_binary_digest":native_digest,
                "compiled_source_digests":compiled_sources,
                "catalog":snapshot.release_identity(),
                "physical_core":racing_model_v3_component_candidate_identity(),
                "config":config_json,
            },
            "result":summary,
            "power_projection":power_v1.then_some(power_laps),
            "timings":{
                "prepare_ms":prepare_ms,
                "advance_ms":advance_ms,
                "advance_calls_ms":advance_calls,
                "summary_and_digest_ms":summary_ms,
                "power_projection_ms":power_projection_ms,
            }
        })
    );
    Ok(())
}
