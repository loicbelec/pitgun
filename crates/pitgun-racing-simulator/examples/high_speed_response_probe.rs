//! Offline-only probe for locating high-speed Racing response anomalies.

use std::env;
use std::fs;
use std::path::Path;

use pitgun_contract::canonical_json_digest;
use pitgun_racing_simulator::{
    RacingCatalogSnapshot, RunRaceInput, RunRaceRequest, SetupResponseDiagnosticsV1,
    TuningResponseV1, get_circuit_with_catalog, run_race_with_catalog_and_tuning_response,
};
use serde::Serialize;
use serde_json::Value;

const PARAM_SPEED_KPH: u16 = 5005;
const CORNER_THRESHOLD_RAD_PER_M: f64 = 0.001;
const CURVATURE_BANDS: [(&str, f64, f64); 4] = [
    ("near_straight", 0.0, 0.00025),
    ("low_curvature", 0.00025, 0.001),
    ("medium_curvature", 0.001, 0.005),
    ("high_curvature", 0.005, f64::INFINITY),
];

#[derive(Serialize)]
struct ExperimentalIdentity<'a> {
    schema_version: &'static str,
    scenario: &'a Value,
    tuning_response: &'a TuningResponseV1,
    seed: u64,
}

#[derive(Serialize)]
struct PeakSpeed {
    speed_kph: f64,
    distance_m: f64,
    absolute_curvature_rad_per_m: f64,
    aerodynamic_mode: &'static str,
}

#[derive(Serialize)]
struct CurvatureBand {
    id: &'static str,
    minimum_absolute_curvature_rad_per_m: f64,
    maximum_absolute_curvature_rad_per_m: Option<f64>,
    sample_count: u64,
    mean_speed_kph: f64,
    maximum_speed_kph: f64,
}

#[derive(Default)]
struct CurvatureBandAccumulator {
    sample_count: u64,
    speed_sum_kph: f64,
    maximum_speed_kph: f64,
}

#[derive(Serialize)]
struct TrackAudit {
    length_m: f64,
    elevation_range_m: f64,
    maximum_absolute_slope: f64,
    corner_aerodynamic_mode_distance_share: f64,
}

#[derive(Serialize)]
struct ProbeOutput {
    schema_version: &'static str,
    experimental_execution_id: String,
    scenario_digest: String,
    tuning_response_digest: String,
    seed: String,
    total_time_ms: u64,
    track: TrackAudit,
    peak_speed: PeakSpeed,
    curvature_bands: Vec<CurvatureBand>,
    setup_response: SetupResponseDiagnosticsV1,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let arguments = env::args().skip(1).collect::<Vec<_>>();
    if arguments.len() != 3 {
        return Err(
            "usage: high_speed_response_probe SCENARIO_JSON TUNING_RESPONSE_JSON SEED".to_string(),
        );
    }
    let seed = arguments[2]
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
    let tuning_response =
        serde_json::from_value::<TuningResponseV1>(read_json(&arguments[1], "tuning response")?)
            .map_err(|error| format!("invalid tuning response: {error}"))?;
    tuning_response
        .validate()
        .map_err(|error| format!("invalid tuning response: {error}"))?;

    let scenario_digest = canonical_json_digest(&scenario)
        .map_err(|error| format!("cannot digest scenario: {error}"))?;
    let tuning_response_digest = canonical_json_digest(&tuning_response)
        .map_err(|error| format!("cannot digest tuning response: {error}"))?;
    let experimental_execution_id = canonical_json_digest(&ExperimentalIdentity {
        schema_version: "pitgun.racing-high-speed-response-probe/v1",
        scenario: &scenario,
        tuning_response: &tuning_response,
        seed,
    })
    .map_err(|error| format!("cannot identify experimental execution: {error}"))?;

    let snapshot = RacingCatalogSnapshot::embedded()
        .map_err(|error| format!("invalid embedded Racing catalog: {error}"))?;
    let circuit = get_circuit_with_catalog(&snapshot, &input.race.track_id)?;
    let output = run_race_with_catalog_and_tuning_response(
        RunRaceRequest {
            era: Some(input.era),
            hz: Some(input.hz),
            input,
            seed,
        },
        &snapshot,
        &tuning_response,
    )?;

    let mut peak_speed = None::<PeakSpeed>;
    let mut bands = std::array::from_fn::<_, 4, _>(|_| CurvatureBandAccumulator::default());
    for frame in output.player_batches.iter().flat_map(|batch| &batch.frames) {
        let Some(distance_m) = frame.progress_m.map(f64::from) else {
            continue;
        };
        let Some(speed_kph) = frame
            .samples
            .iter()
            .find(|sample| sample.parameter_id == PARAM_SPEED_KPH)
            .and_then(|sample| sample.value.as_f64())
        else {
            continue;
        };
        let track_distance_m = distance_m.rem_euclid(circuit.s_m.last().copied().unwrap_or(1.0));
        let curvature = interpolate(track_distance_m, &circuit.s_m, &circuit.curvature_radpm).abs();
        let band_index = CURVATURE_BANDS
            .iter()
            .position(|(_, minimum, maximum)| curvature >= *minimum && curvature < *maximum)
            .expect("curvature bands cover every non-negative finite value");
        bands[band_index].sample_count += 1;
        bands[band_index].speed_sum_kph += speed_kph;
        bands[band_index].maximum_speed_kph = bands[band_index].maximum_speed_kph.max(speed_kph);

        if peak_speed
            .as_ref()
            .is_none_or(|current| speed_kph > current.speed_kph)
        {
            peak_speed = Some(PeakSpeed {
                speed_kph,
                distance_m: track_distance_m,
                absolute_curvature_rad_per_m: curvature,
                aerodynamic_mode: if curvature > CORNER_THRESHOLD_RAD_PER_M {
                    "corner"
                } else {
                    "straight"
                },
            });
        }
    }

    let length_m = circuit.s_m.last().copied().unwrap_or(0.0);
    let corner_distance_m = circuit
        .s_m
        .windows(2)
        .zip(circuit.curvature_radpm.windows(2))
        .filter_map(|(distance, curvature)| {
            let average_curvature = 0.5 * (curvature[0].abs() + curvature[1].abs());
            (average_curvature >= CORNER_THRESHOLD_RAD_PER_M).then_some(distance[1] - distance[0])
        })
        .sum::<f64>();
    let elevation_range_m = circuit.z_m.iter().copied().reduce(f64::max).unwrap_or(0.0)
        - circuit.z_m.iter().copied().reduce(f64::min).unwrap_or(0.0);
    let curvature_bands = CURVATURE_BANDS
        .iter()
        .zip(bands)
        .map(|((id, minimum, maximum), band)| CurvatureBand {
            id,
            minimum_absolute_curvature_rad_per_m: *minimum,
            maximum_absolute_curvature_rad_per_m: maximum.is_finite().then_some(*maximum),
            sample_count: band.sample_count,
            mean_speed_kph: if band.sample_count > 0 {
                band.speed_sum_kph / band.sample_count as f64
            } else {
                0.0
            },
            maximum_speed_kph: band.maximum_speed_kph,
        })
        .collect();
    let setup_response = output
        .player_diagnostics
        .ok_or_else(|| "race output contains no setup diagnostics".to_string())?;

    let probe = ProbeOutput {
        schema_version: "pitgun.racing-high-speed-response-probe/v1",
        experimental_execution_id: experimental_execution_id.to_string(),
        scenario_digest: scenario_digest.to_string(),
        tuning_response_digest: tuning_response_digest.to_string(),
        seed: seed.to_string(),
        total_time_ms: output.total_time_ms,
        track: TrackAudit {
            length_m,
            elevation_range_m,
            maximum_absolute_slope: circuit
                .slope
                .iter()
                .map(|value| value.abs())
                .reduce(f64::max)
                .unwrap_or(0.0),
            corner_aerodynamic_mode_distance_share: if length_m > 0.0 {
                corner_distance_m / length_m
            } else {
                0.0
            },
        },
        peak_speed: peak_speed
            .ok_or_else(|| "race output contains no speed telemetry".to_string())?,
        curvature_bands,
        setup_response,
    };
    println!(
        "{}",
        serde_json::to_string(&probe)
            .map_err(|error| format!("cannot serialize probe output: {error}"))?
    );
    Ok(())
}

fn interpolate(target: f64, x: &[f64], y: &[f64]) -> f64 {
    let upper = x.partition_point(|value| *value < target);
    if upper == 0 {
        return y[0];
    }
    if upper >= x.len() {
        return y[y.len() - 1];
    }
    let lower = upper - 1;
    let fraction = (target - x[lower]) / (x[upper] - x[lower]);
    y[lower] + fraction * (y[upper] - y[lower])
}

fn read_json(path: impl AsRef<Path>, label: &str) -> Result<Value, String> {
    let path = path.as_ref();
    let bytes = fs::read(path)
        .map_err(|error| format!("cannot read {label} {}: {error}", path.display()))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid {label} JSON {}: {error}", path.display()))
}
