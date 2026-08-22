//! Offline-only probe for Racing Model V3 driver-control screening.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::Path;

use pitgun_contract::{ArtifactIdentity, canonical_json_digest};
use pitgun_racing_simulator::{
    DriverControlDiagnosticsV3, RacingCatalogSnapshot, ResolvedV3DriverControlV1, RunRaceInput,
    RunRaceRequest, TireDegradationDiagnosticsV3, TireDiagnosticsV3, V3CandidateExperimentProfile,
    V3DriverControlExperimentV1, run_race_with_catalog_and_v3_driver_control_profile,
};
use serde::Serialize;
use serde_json::Value;

const PARAM_TIRE_TEMP_C: u16 = 5015;
const PARAM_TIRE_WEAR_PCT: u16 = 5016;

#[derive(Serialize)]
struct ExperimentalIdentity<'a> {
    schema_version: &'static str,
    model: &'a ArtifactIdentity,
    scenario: &'a Value,
    profile: &'a V3CandidateExperimentProfile,
    driver_experiment: &'a V3DriverControlExperimentV1,
    seed: u64,
}

#[derive(Serialize)]
struct ProbeOutput {
    schema_version: &'static str,
    experimental_execution_id: String,
    model: ArtifactIdentity,
    scenario_digest: String,
    profile_digest: String,
    driver_experiment_digest: String,
    seed: String,
    total_time_ms: u64,
    player_lap_times_ms: Vec<u64>,
    player_pit_laps: Vec<u16>,
    final_tire_temperature_c: f64,
    final_tire_wear_pct: f64,
    driver_control_resolutions: BTreeMap<String, ResolvedV3DriverControlV1>,
    driver_control_diagnostics: BTreeMap<String, DriverControlDiagnosticsV3>,
    tire_diagnostics: TireDiagnosticsV3,
    tire_degradation_diagnostics: TireDegradationDiagnosticsV3,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let arguments = env::args().skip(1).collect::<Vec<_>>();
    if arguments.len() != 4 {
        return Err(
            "usage: v3_driver_control_probe SCENARIO_JSON V3_PROFILE_JSON DRIVER_EXPERIMENT_JSON SEED"
                .to_string(),
        );
    }
    let seed = arguments[3]
        .parse::<u64>()
        .map_err(|error| format!("invalid seed: {error}"))?;
    let scenario = read_json(&arguments[0], "scenario")?;
    if scenario.get("schema_version").and_then(Value::as_str)
        != Some("pitgun.racing-resolved-scenario/v1")
    {
        return Err("unsupported Racing scenario version".to_string());
    }
    let input = serde_json::from_value::<RunRaceInput>(
        scenario
            .get("request")
            .cloned()
            .ok_or_else(|| "scenario request is missing".to_string())?,
    )
    .map_err(|error| format!("invalid Racing request: {error}"))?;
    let profile = serde_json::from_value::<V3CandidateExperimentProfile>(read_json(
        &arguments[1],
        "V3 experiment profile",
    )?)
    .map_err(|error| format!("invalid V3 experiment profile: {error}"))?;
    let driver_experiment = serde_json::from_value::<V3DriverControlExperimentV1>(read_json(
        &arguments[2],
        "driver experiment",
    )?)
    .map_err(|error| format!("invalid driver experiment: {error}"))?;

    profile
        .validate()
        .map_err(|error| format!("invalid V3 experiment profile: {error}"))?;
    let model = profile.model_identity();
    let scenario_digest = canonical_json_digest(&scenario)
        .map_err(|error| format!("cannot digest scenario: {error}"))?;
    let profile_digest = canonical_json_digest(&profile)
        .map_err(|error| format!("cannot digest V3 experiment profile: {error}"))?;
    let driver_experiment_digest = canonical_json_digest(&driver_experiment)
        .map_err(|error| format!("cannot digest driver experiment: {error}"))?;
    let experimental_execution_id = canonical_json_digest(&ExperimentalIdentity {
        schema_version: "pitgun.racing-v3-driver-control-probe/v1",
        model: &model,
        scenario: &scenario,
        profile: &profile,
        driver_experiment: &driver_experiment,
        seed,
    })
    .map_err(|error| format!("cannot identify experimental execution: {error}"))?;

    let snapshot = RacingCatalogSnapshot::embedded_model_v3_component()
        .map_err(|error| format!("invalid embedded component Racing catalog: {error}"))?;
    let thermal = snapshot
        .power_unit_thermal_profile()
        .ok_or_else(|| "embedded Racing catalog has no power-unit thermal profile".to_string())?;
    let output = run_race_with_catalog_and_v3_driver_control_profile(
        RunRaceRequest {
            era: Some(input.era),
            hz: Some(input.hz),
            input,
            seed,
        },
        &snapshot,
        &profile,
        thermal,
        &driver_experiment,
    )?;
    let final_tire_temperature_c = final_sample(&output, PARAM_TIRE_TEMP_C, "tire temperature")?;
    let final_tire_wear_pct = final_sample(&output, PARAM_TIRE_WEAR_PCT, "tire wear")?;

    let probe = ProbeOutput {
        schema_version: "pitgun.racing-v3-driver-control-probe/v1",
        experimental_execution_id: experimental_execution_id.to_string(),
        model,
        scenario_digest: scenario_digest.to_string(),
        profile_digest: profile_digest.to_string(),
        driver_experiment_digest: driver_experiment_digest.to_string(),
        seed: seed.to_string(),
        total_time_ms: output.total_time_ms,
        player_lap_times_ms: output.player_lap_times_ms,
        player_pit_laps: output.player_pit_laps,
        final_tire_temperature_c,
        final_tire_wear_pct,
        driver_control_resolutions: output.competitor_driver_control_resolutions_v3,
        driver_control_diagnostics: output.competitor_driver_control_diagnostics_v3,
        tire_diagnostics: output
            .player_tire_diagnostics_v3
            .ok_or_else(|| "race output contains no V3 tire diagnostics".to_string())?,
        tire_degradation_diagnostics: output
            .player_tire_degradation_diagnostics_v3
            .ok_or_else(|| "race output contains no V3 tire-degradation diagnostics".to_string())?,
    };
    println!(
        "{}",
        serde_json::to_string(&probe)
            .map_err(|error| format!("cannot serialize probe output: {error}"))?
    );
    Ok(())
}

fn final_sample(
    output: &pitgun_racing_simulator::RaceOutput,
    parameter_id: u16,
    label: &str,
) -> Result<f64, String> {
    output
        .player_batches
        .iter()
        .flat_map(|batch| &batch.frames)
        .flat_map(|frame| &frame.samples)
        .filter(|sample| sample.parameter_id == parameter_id)
        .filter_map(|sample| sample.value.as_f64())
        .next_back()
        .ok_or_else(|| format!("race output contains no player {label} telemetry"))
}

fn read_json(path: impl AsRef<Path>, label: &str) -> Result<Value, String> {
    let path = path.as_ref();
    let bytes = fs::read(path)
        .map_err(|error| format!("cannot read {label} {}: {error}", path.display()))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid {label} JSON {}: {error}", path.display()))
}
