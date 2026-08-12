use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use clap::Args;
use pitgun_contract::{
    canonical_json_bytes, canonical_json_digest, ArtifactIdentity, DerivedMetricsV1, Digest,
    ScenarioIdentity, Seed,
};
use pitgun_racing_simulator::{SetupResponseDiagnosticsV1, StandingEntry};
use serde::Serialize;

use crate::demo::{bundle, racing};

#[derive(Args, Clone, Debug, Eq, PartialEq)]
pub(crate) struct RacingBatchArgs {
    /// Resolved Racing scenario JSON document
    #[arg(long, value_name = "PATH")]
    pub(crate) scenario: PathBuf,

    /// Deterministic root seed recorded in the run contract
    #[arg(long)]
    pub(crate) seed: u64,

    /// Optional file for the canonical compact JSON result; stdout by default
    #[arg(long, value_name = "PATH")]
    pub(crate) result: Option<PathBuf>,

    /// Optional directory for a complete immutable Run Bundle V1
    #[arg(long, value_name = "PATH")]
    pub(crate) bundle: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
enum BatchRunResultVersion {
    #[serde(rename = "pitgun.batch-run-result/v1")]
    V1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
enum BatchRunErrorVersion {
    #[serde(rename = "pitgun.batch-run-error/v1")]
    V1,
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct RuntimeDescriptor<'a> {
    name: &'a str,
    version: &'a str,
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct RacingBatchSummary<'a> {
    total_time_ms: u64,
    standings: &'a [StandingEntry],
    player_pit_laps: &'a [u16],
    player_lap_times_ms: &'a [u64],
    telemetry_frame_count: u64,
    telemetry_batch_count: u64,
    metrics: &'a DerivedMetricsV1,
    setup_response: Option<&'a SetupResponseDiagnosticsV1>,
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct BatchRunResult<'a> {
    schema_version: BatchRunResultVersion,
    runtime: RuntimeDescriptor<'static>,
    scenario: &'a ScenarioIdentity,
    scenario_digest: Digest,
    model: &'a ArtifactIdentity,
    data_pack: &'a ArtifactIdentity,
    seed: Seed,
    configuration_id: Digest,
    run_id: Digest,
    contract_digest: Digest,
    output_digest: Digest,
    telemetry_summary_digest: Digest,
    metrics_digest: Digest,
    summary: RacingBatchSummary<'a>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum BatchErrorPhase {
    Input,
    Contract,
    Simulation,
    Bundle,
    Output,
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct BatchRunErrorDocument<'a> {
    schema_version: BatchRunErrorVersion,
    phase: BatchErrorPhase,
    code: &'a str,
    message: &'a str,
}

#[derive(Debug)]
pub(crate) struct BatchRunError {
    exit_code: u8,
    phase: BatchErrorPhase,
    code: &'static str,
    message: String,
}

impl BatchRunError {
    fn new(
        exit_code: u8,
        phase: BatchErrorPhase,
        code: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self {
            exit_code,
            phase,
            code,
            message: message.into(),
        }
    }

    fn from_racing(error: racing::RacingDemoError) -> Self {
        let exit_code = error.exit_code();
        let (phase, code) = if exit_code == 10 {
            (BatchErrorPhase::Contract, "invalid_scenario")
        } else {
            (BatchErrorPhase::Simulation, "simulation_failed")
        };
        Self::new(exit_code, phase, code, error.to_string())
    }

    pub(crate) const fn exit_code(&self) -> u8 {
        self.exit_code
    }
}

impl fmt::Display for BatchRunError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let document = BatchRunErrorDocument {
            schema_version: BatchRunErrorVersion::V1,
            phase: self.phase,
            code: self.code,
            message: &self.message,
        };
        match canonical_json_bytes(&document) {
            Ok(bytes) => formatter.write_str(&String::from_utf8_lossy(&bytes)),
            Err(_) => formatter.write_str(
                r#"{"schema_version":"pitgun.batch-run-error/v1","phase":"output","code":"error_encoding_failed","message":"batch error could not be encoded"}"#,
            ),
        }
    }
}

impl std::error::Error for BatchRunError {}

pub(crate) fn run_racing(args: &RacingBatchArgs) -> Result<(), BatchRunError> {
    let scenario_bytes = fs::read(&args.scenario).map_err(|error| {
        BatchRunError::new(
            10,
            BatchErrorPhase::Input,
            "scenario_unreadable",
            format!("cannot read {}: {error}", args.scenario.display()),
        )
    })?;
    let run =
        racing::run_scenario(&scenario_bytes, args.seed).map_err(BatchRunError::from_racing)?;

    if let Some(bundle_path) = args.bundle.as_deref() {
        bundle::persist(&run, Some(bundle_path)).map_err(|error| {
            BatchRunError::new(
                error.exit_code(),
                BatchErrorPhase::Bundle,
                "bundle_persistence_failed",
                error.to_string(),
            )
        })?;
    }

    let contract_digest = canonical_json_digest(&run.contract).map_err(|error| {
        BatchRunError::new(
            30,
            BatchErrorPhase::Output,
            "result_encoding_failed",
            error.to_string(),
        )
    })?;
    let metrics_digest = canonical_json_digest(&run.metrics).map_err(|error| {
        BatchRunError::new(
            30,
            BatchErrorPhase::Output,
            "result_encoding_failed",
            error.to_string(),
        )
    })?;
    let result = BatchRunResult {
        schema_version: BatchRunResultVersion::V1,
        runtime: RuntimeDescriptor {
            name: "pitgun-cli",
            version: env!("CARGO_PKG_VERSION"),
        },
        scenario: &run.contract.scenario,
        scenario_digest: Digest::from_bytes(&run.scenario_json),
        model: &run.contract.model,
        data_pack: &run.contract.data_pack,
        seed: run.seed,
        configuration_id: run.contract.input.digest,
        run_id: run.run_id,
        contract_digest,
        output_digest: run.output_digest,
        telemetry_summary_digest: run.telemetry_summary_digest,
        metrics_digest,
        summary: RacingBatchSummary {
            total_time_ms: run.output.total_time_ms,
            standings: &run.output.standings,
            player_pit_laps: &run.output.player_pit_laps,
            player_lap_times_ms: &run.output.player_lap_times_ms,
            telemetry_frame_count: run.evidence.telemetry_summary.frame_count(),
            telemetry_batch_count: run.evidence.telemetry_summary.batch_count(),
            metrics: &run.metrics,
            setup_response: run.output.player_diagnostics.as_ref(),
        },
    };
    let mut bytes = canonical_json_bytes(&result).map_err(|error| {
        BatchRunError::new(
            30,
            BatchErrorPhase::Output,
            "result_encoding_failed",
            error.to_string(),
        )
    })?;
    bytes.push(b'\n');

    match args.result.as_deref() {
        Some(path) => write_result(path, &bytes),
        None => io::stdout().write_all(&bytes).map_err(|error| {
            BatchRunError::new(
                30,
                BatchErrorPhase::Output,
                "result_write_failed",
                format!("cannot write stdout: {error}"),
            )
        }),
    }
}

fn write_result(path: &Path, bytes: &[u8]) -> Result<(), BatchRunError> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|error| {
            BatchRunError::new(
                30,
                BatchErrorPhase::Output,
                "result_write_failed",
                format!("cannot create {}: {error}", parent.display()),
            )
        })?;
    }
    fs::write(path, bytes).map_err(|error| {
        BatchRunError::new(
            30,
            BatchErrorPhase::Output,
            "result_write_failed",
            format!("cannot write {}: {error}", path.display()),
        )
    })
}
