use std::fmt;
use std::path::PathBuf;

use clap::Args;
use pitgun_contract::{
    ArtifactIdentity, ContractVersion, DerivedMetricProcessorV1, DerivedMetricStatisticV1,
    DerivedMetricV1, DerivedMetricsV1, DeterministicRunContractV1, Digest, EventOrderingV1,
    Identifier, InputCanonicalization, InputIdentity, InputMediaType, LogicalClockV1,
    RandomAlgorithm, RandomContractV1, RuntimeProfile, ScenarioIdentity, Seed, StreamDerivation,
    canonical_json_bytes, canonical_json_digest,
};
use pitgun_core::{
    TelemetryAggregateConfig, TelemetryAggregateKind, aggregate_telemetry_parameter,
};
use pitgun_racing_simulator::evidence::RacingRunEvidenceV1;
use pitgun_racing_simulator::{RaceOutput, RacingWorkload, RunRaceInput};
use pitgun_runtime::execute_linked;
use serde::{Deserialize, Serialize};

const DEFAULT_SCENARIO: &str = include_str!("../../scenarios/racing-demo-v1.json");
const PARAM_SPEED_KPH: u16 = 5005;
const OBSERVED_MAXIMUM_SPEED_ID: &str = "racing.observed-maximum-speed";

#[derive(Args, Clone, Debug, Eq, PartialEq)]
pub(crate) struct RacingArgs {
    /// Deterministic root seed recorded in the run contract
    #[arg(long, default_value_t = 42)]
    pub(crate) seed: u64,

    /// Exact destination directory for the immutable run bundle
    #[arg(long, value_name = "PATH")]
    pub(crate) output: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
enum RacingScenarioVersion {
    #[serde(rename = "pitgun.racing-demo-scenario/v1")]
    V1,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RacingScenarioV1 {
    schema_version: RacingScenarioVersion,
    scenario: ScenarioIdentity,
    model: ArtifactIdentity,
    data_pack: ArtifactIdentity,
    clock: ScenarioClock,
    request: RunRaceInput,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ScenarioClock {
    tick_numerator_us: u64,
    tick_denominator: u64,
}

#[derive(Debug)]
pub(crate) struct RacingDemoRun {
    pub(crate) scenario: ScenarioIdentity,
    pub(crate) seed: Seed,
    pub(crate) run_id: Digest,
    pub(crate) output_digest: Digest,
    pub(crate) telemetry_summary_digest: Digest,
    pub(crate) contract: DeterministicRunContractV1,
    pub(crate) output: RaceOutput,
    pub(crate) evidence: RacingRunEvidenceV1,
    pub(crate) scenario_json: Vec<u8>,
    pub(crate) metrics: DerivedMetricsV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RacingDemoPhase {
    Contract,
    Simulation,
}

#[derive(Debug)]
pub(crate) struct RacingDemoError {
    phase: RacingDemoPhase,
    message: String,
}

impl RacingDemoError {
    fn contract(error: impl fmt::Display) -> Self {
        Self {
            phase: RacingDemoPhase::Contract,
            message: error.to_string(),
        }
    }

    fn simulation(error: impl fmt::Display) -> Self {
        Self {
            phase: RacingDemoPhase::Simulation,
            message: error.to_string(),
        }
    }

    pub(crate) const fn exit_code(&self) -> u8 {
        match self.phase {
            RacingDemoPhase::Contract => 10,
            RacingDemoPhase::Simulation => 20,
        }
    }
}

impl fmt::Display for RacingDemoError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.phase {
            RacingDemoPhase::Contract => {
                write!(formatter, "Racing contract failed: {}", self.message)
            }
            RacingDemoPhase::Simulation => {
                write!(formatter, "Racing simulation failed: {}", self.message)
            }
        }
    }
}

impl std::error::Error for RacingDemoError {}

pub(crate) fn run(args: &RacingArgs) -> Result<RacingDemoRun, RacingDemoError> {
    let scenario: RacingScenarioV1 =
        serde_json::from_str(DEFAULT_SCENARIO).map_err(RacingDemoError::contract)?;
    if scenario.schema_version != RacingScenarioVersion::V1 {
        return Err(RacingDemoError::contract(
            "unsupported Racing scenario version",
        ));
    }
    let scenario_json = canonical_json_bytes(&scenario).map_err(RacingDemoError::contract)?;

    let input_digest =
        canonical_json_digest(&scenario.request).map_err(RacingDemoError::contract)?;
    let seed = Seed::new(args.seed);
    let contract = DeterministicRunContractV1 {
        contract_version: ContractVersion::V1,
        scenario: scenario.scenario.clone(),
        model: scenario.model,
        data_pack: scenario.data_pack,
        runtime_profile: RuntimeProfile::PortableExactV1,
        random: RandomContractV1 {
            seed,
            algorithm: RandomAlgorithm::PitgunSplitMix64V1,
            stream_derivation: StreamDerivation::Sha256LabelV1,
        },
        clock: LogicalClockV1::new(
            0,
            scenario.clock.tick_numerator_us,
            scenario.clock.tick_denominator,
        )
        .map_err(RacingDemoError::contract)?,
        event_ordering: EventOrderingV1::v1(),
        input: InputIdentity {
            media_type: InputMediaType::ApplicationJson,
            canonicalization: InputCanonicalization::JcsRfc8785,
            digest: input_digest,
        },
    };
    let workload = RacingWorkload::v1();
    let executed = execute_linked(&workload, &contract, scenario.request)
        .map_err(RacingDemoError::simulation)?;
    let metrics = calculate_metrics(&executed.output).map_err(RacingDemoError::simulation)?;

    Ok(RacingDemoRun {
        scenario: contract.scenario.clone(),
        seed,
        run_id: executed.run_id,
        output_digest: executed.output_digest,
        telemetry_summary_digest: executed.telemetry_summary_digest,
        contract,
        output: executed.output,
        evidence: executed.evidence,
        scenario_json,
        metrics,
    })
}

fn calculate_metrics(output: &RaceOutput) -> Result<DerivedMetricsV1, Box<dyn std::error::Error>> {
    let frames = output
        .player_batches
        .iter()
        .flat_map(|batch| batch.frames.iter());
    let maximum_speed = aggregate_telemetry_parameter(
        frames,
        TelemetryAggregateConfig {
            parameter_id: PARAM_SPEED_KPH,
            kind: TelemetryAggregateKind::Maximum,
        },
    )?;

    Ok(DerivedMetricsV1::new(vec![DerivedMetricV1 {
        id: Identifier::new(OBSERVED_MAXIMUM_SPEED_ID)?,
        processor: DerivedMetricProcessorV1::TelemetryAggregateV1,
        parameter_id: PARAM_SPEED_KPH,
        statistic: DerivedMetricStatisticV1::Maximum,
        unit: "km/h".to_owned(),
        sample_count: maximum_speed.sample_count,
        value: maximum_speed.value,
    }])?)
}

#[cfg(test)]
mod tests {
    use pitgun_contract::{SampleValue, canonical_json_digest};

    use super::{OBSERVED_MAXIMUM_SPEED_ID, PARAM_SPEED_KPH, RacingArgs, calculate_metrics, run};

    #[test]
    fn identical_seed_repeats_logical_results() {
        let first = run(&RacingArgs {
            seed: 42,
            output: None,
        })
        .expect("first Racing demo");
        let second = run(&RacingArgs {
            seed: 42,
            output: None,
        })
        .expect("second Racing demo");

        assert_eq!(first.contract, second.contract);
        assert_eq!(first.run_id, second.run_id);
        assert_eq!(first.evidence.output, second.evidence.output);
        assert_eq!(first.metrics, second.metrics);
        assert_eq!(
            first.evidence.telemetry_summary,
            second.evidence.telemetry_summary
        );
    }

    #[test]
    fn changing_seed_changes_run_identity_and_output() {
        let first = run(&RacingArgs {
            seed: 42,
            output: None,
        })
        .expect("seed 42 Racing demo");
        let changed = run(&RacingArgs {
            seed: 43,
            output: None,
        })
        .expect("seed 43 Racing demo");

        assert_ne!(first.run_id, changed.run_id);
        assert_ne!(first.evidence.output, changed.evidence.output);
        assert_ne!(first.contract.random.seed, changed.contract.random.seed);
        assert_eq!(first.contract.input, changed.contract.input);
    }

    #[test]
    fn built_in_scenario_emits_typed_telemetry() {
        let result = run(&RacingArgs {
            seed: 42,
            output: None,
        })
        .expect("Racing demo");

        assert!(!result.output.player_batches.is_empty());
        assert!(result.evidence.telemetry_summary.frame_count() > 0);
        assert!(!result.evidence.telemetry_summary.parameter_ids().is_empty());
    }

    #[test]
    fn seed_42_matches_the_versioned_scenario_vectors() {
        let result = run(&RacingArgs {
            seed: 42,
            output: None,
        })
        .expect("Racing demo");

        assert_eq!(
            result.run_id.to_string(),
            "sha256:89dc458a7460056dd519f5cda74c55c2b2b47f7091f1309ae10d11a2eb46a64a"
        );
        assert_eq!(
            result.contract.input.digest.to_string(),
            "sha256:12a4207b2c26c814763a2a488054f7421e7cc3836a35e26fc16d96477c8744d7"
        );
        assert_eq!(
            result.output_digest.to_string(),
            "sha256:c16d23af721b33a4e0919cd99c8fc74aa1f680dfa4b143a92b57e566b8e8d1e3"
        );
        assert_eq!(
            result.telemetry_summary_digest.to_string(),
            "sha256:b69f733ae44dd87a21fe1767b95b19e9e6eaa4bfc73f3a53a9203d33ef920e80"
        );
        let metric = &result.metrics.metrics[0];
        assert_eq!(metric.id.as_str(), OBSERVED_MAXIMUM_SPEED_ID);
        assert_eq!(metric.parameter_id, PARAM_SPEED_KPH);
        assert_eq!(metric.sample_count, 427);
        assert_eq!(metric.value, 355.59540920059794);
        assert_eq!(
            canonical_json_digest(&result.metrics)
                .expect("canonical metrics")
                .to_string(),
            "sha256:f360dc83d186a259a6d168b8fb75ee7237fdef09877d9306099dcdc4de44d76d"
        );
    }

    #[test]
    fn changing_a_recorded_speed_changes_the_derived_metric() {
        let result = run(&RacingArgs {
            seed: 42,
            output: None,
        })
        .expect("Racing demo");
        let mut mutated = result.output.clone();
        let speed = mutated
            .player_batches
            .iter_mut()
            .flat_map(|batch| batch.frames.iter_mut())
            .flat_map(|frame| frame.samples.iter_mut())
            .find(|sample| sample.parameter_id == PARAM_SPEED_KPH)
            .expect("recorded speed sample");
        speed.value = SampleValue::F64(999.0);

        let changed = calculate_metrics(&mutated).expect("mutated metrics");

        assert_eq!(result.metrics.metrics[0].value, 355.59540920059794);
        assert_eq!(changed.metrics[0].value, 999.0);
    }
}
