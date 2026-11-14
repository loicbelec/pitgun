//! Racing service permissions and completed-history identity. No physics or
//! provider metadata belongs in this domain-owned wire contract.
use pitgun_contract::{
    ArtifactIdentity, DeterministicRunContractV1, Digest, InputCanonicalization, InputIdentity,
    InputMediaType, RunAttemptAuthorizationV1, canonical_json_digest,
};
use serde::{Deserialize, Serialize};

pub const SERVICE_CONTROL_SCENARIO: &str = "racing.service-control";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum RacingServiceAuthorizationVersion {
    #[serde(rename = "pitgun.racing-service-authorization/v1")]
    V1,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RacingServiceAuthorizationV1 {
    pub schema_version: RacingServiceAuthorizationVersion,
    pub authorized_input: InputIdentity,
    pub control_policy: ArtifactIdentity,
    pub runtime_digest: Digest,
    pub player_id: String,
    pub allowed_tire_ids: Vec<String>,
    pub lap_count: u16,
    pub max_decisions: u16,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum RacingCompletedServiceInputVersion {
    #[serde(rename = "pitgun.racing-completed-service-input/v1")]
    V1,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RacingCompletedServiceInputV1 {
    pub schema_version: RacingCompletedServiceInputVersion,
    pub authorized_input: InputIdentity,
    pub decision_envelope: ArtifactIdentity,
    /// Ordered native observation/applied-receipt pairs, supplied separately.
    pub applied_history_digest: Digest,
}

impl RacingServiceAuthorizationV1 {
    pub fn artifact_identity(&self) -> Result<ArtifactIdentity, String> {
        Ok(ArtifactIdentity {
            id: "pitgun.racing-service-authorization"
                .parse()
                .map_err(|e| format!("{e}"))?,
            version: "1.0.0".parse().map_err(|e| format!("{e}"))?,
            digest: canonical_json_digest(self).map_err(|e| e.to_string())?,
        })
    }
    pub fn validate(
        &self,
        initial: &DeterministicRunContractV1,
        policy: &ArtifactIdentity,
    ) -> Result<(), String> {
        if initial.scenario.id.as_str() != SERVICE_CONTROL_SCENARIO
            || initial.scenario.version != "1.0.0".parse().expect("static scenario version")
            || self.authorized_input != initial.input
            || &self.control_policy != policy
            || self.player_id.is_empty()
            || self.player_id.len() > 64
            || self.player_id.trim() != self.player_id
            || self.lap_count < 2
            || self.lap_count > 128
            || self.max_decisions == 0
            || self.max_decisions > 35
            || self.allowed_tire_ids.is_empty()
            || self.allowed_tire_ids.len() > 16
            || self.allowed_tire_ids.iter().any(|s| {
                s.is_empty()
                    || s.len() > 32
                    || !s.bytes().all(|b| {
                        b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-'
                    })
            })
            || self.allowed_tire_ids.windows(2).any(|w| w[0] >= w[1])
        {
            return Err("invalid Racing service authorization".into());
        }
        Ok(())
    }
    pub fn validate_attempt(
        &self,
        attempt: &RunAttemptAuthorizationV1,
        policy: &ArtifactIdentity,
    ) -> Result<(), String> {
        attempt.validate_integrity().map_err(|e| e.to_string())?;
        self.validate(&attempt.initial_contract, policy)?;
        if attempt.decision_envelope != self.artifact_identity()? {
            return Err("service envelope is not signed into this attempt".into());
        }
        Ok(())
    }
    pub fn completed_input(
        &self,
        history: Digest,
    ) -> Result<RacingCompletedServiceInputV1, String> {
        Ok(RacingCompletedServiceInputV1 {
            schema_version: RacingCompletedServiceInputVersion::V1,
            authorized_input: self.authorized_input.clone(),
            decision_envelope: self.artifact_identity()?,
            applied_history_digest: history,
        })
    }
    pub fn final_contract(
        &self,
        attempt: &RunAttemptAuthorizationV1,
        completed: &RacingCompletedServiceInputV1,
        history: Digest,
        policy: &ArtifactIdentity,
    ) -> Result<DeterministicRunContractV1, String> {
        self.validate_attempt(attempt, policy)?;
        if *completed != self.completed_input(history)? {
            return Err("completed service history does not match the authorized attempt".into());
        }
        let mut contract = attempt.initial_contract.clone();
        contract.input = InputIdentity {
            media_type: InputMediaType::ApplicationJson,
            canonicalization: InputCanonicalization::JcsRfc8785,
            digest: canonical_json_digest(completed).map_err(|e| e.to_string())?,
        };
        attempt
            .validate_final_contract(&contract)
            .map_err(|e| e.to_string())?;
        Ok(contract)
    }
}
