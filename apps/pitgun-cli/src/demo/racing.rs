use std::fmt;

use clap::Args;
use pitgun_contract::{
    canonical_json_digest, ArtifactIdentity, ContractVersion, DeterministicRunContractV1, Digest,
    EventOrderingV1, InputCanonicalization, InputIdentity, InputMediaType, LogicalClockV1,
    RandomAlgorithm, RandomContractV1, RuntimeProfile, ScenarioIdentity, Seed, StreamDerivation,
};
use pitgun_simulator::racing::{
    run_race, RaceOutput, RacingRunEvidenceV1, RunRaceInput, RunRaceRequest,
};
use serde::Deserialize;

const DEFAULT_SCENARIO: &str = include_str!("../../scenarios/racing-demo-v1.json");

#[derive(Args, Clone, Debug, Eq, PartialEq)]
pub(crate) struct RacingArgs {
    /// Deterministic root seed recorded in the run contract
    #[arg(long, default_value_t = 42)]
    pub(crate) seed: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize)]
enum RacingScenarioVersion {
    #[serde(rename = "pitgun.racing-demo-scenario/v1")]
    V1,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RacingScenarioV1 {
    schema_version: RacingScenarioVersion,
    scenario: ScenarioIdentity,
    model: ArtifactIdentity,
    data_pack: ArtifactIdentity,
    clock: ScenarioClock,
    request: RunRaceInput,
}

#[derive(Clone, Copy, Debug, Deserialize)]
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
    let run_id = contract.run_id().map_err(RacingDemoError::contract)?;

    let output = run_race(RunRaceRequest {
        era: Some(scenario.request.era),
        hz: Some(scenario.request.hz),
        input: scenario.request,
        seed: args.seed,
    })
    .map_err(RacingDemoError::simulation)?;
    let evidence =
        RacingRunEvidenceV1::from_race_output(&output).map_err(RacingDemoError::simulation)?;
    let output_digest = evidence
        .output_digest()
        .map_err(RacingDemoError::simulation)?;
    let telemetry_summary_digest = evidence
        .telemetry_summary_digest()
        .map_err(RacingDemoError::simulation)?;

    Ok(RacingDemoRun {
        scenario: contract.scenario.clone(),
        seed,
        run_id,
        output_digest,
        telemetry_summary_digest,
        contract,
        output,
        evidence,
    })
}

#[cfg(test)]
mod tests {
    use super::{run, RacingArgs};

    #[test]
    fn identical_seed_repeats_logical_results() {
        let first = run(&RacingArgs { seed: 42 }).expect("first Racing demo");
        let second = run(&RacingArgs { seed: 42 }).expect("second Racing demo");

        assert_eq!(first.contract, second.contract);
        assert_eq!(first.run_id, second.run_id);
        assert_eq!(first.evidence.output, second.evidence.output);
        assert_eq!(
            first.evidence.telemetry_summary,
            second.evidence.telemetry_summary
        );
    }

    #[test]
    fn changing_seed_changes_run_identity_and_output() {
        let first = run(&RacingArgs { seed: 42 }).expect("seed 42 Racing demo");
        let changed = run(&RacingArgs { seed: 43 }).expect("seed 43 Racing demo");

        assert_ne!(first.run_id, changed.run_id);
        assert_ne!(first.evidence.output, changed.evidence.output);
        assert_ne!(first.contract.random.seed, changed.contract.random.seed);
        assert_eq!(first.contract.input, changed.contract.input);
    }

    #[test]
    fn built_in_scenario_emits_typed_telemetry() {
        let result = run(&RacingArgs { seed: 42 }).expect("Racing demo");

        assert!(!result.output.player_batches.is_empty());
        assert!(result.evidence.telemetry_summary.frame_count() > 0);
        assert!(!result.evidence.telemetry_summary.parameter_ids().is_empty());
    }

    #[test]
    fn seed_42_matches_the_versioned_scenario_vectors() {
        let result = run(&RacingArgs { seed: 42 }).expect("Racing demo");

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
    }
}
