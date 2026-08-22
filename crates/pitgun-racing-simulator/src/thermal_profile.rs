use std::collections::{BTreeMap, BTreeSet};

use pitgun_contract::{ArtifactIdentity, Digest};
use serde::{Deserialize, Serialize};

use crate::{
    V3EngineThermalResolutionParams, racing_model_v3_component_candidate_identity,
    racing_model_v3_thermal_candidate_identity,
};

pub const V3_THERMAL_FAMILY_PROFILE_SCHEMA: &str =
    "pitgun.racing-v3-thermal-family-profile-candidate/v1";
pub const V3_THERMAL_FAMILY_PROFILE_ID: &str = "pitgun.racing-v3-thermal-family-profile";
pub const V3_THERMAL_FAMILY_PROFILE_VERSION: &str = "1.0.0-rc.1";
pub const V3_THERMAL_FAMILY_PROFILE_DIGEST: &str =
    "sha256:8aefd230da307e3439eef115fbfcd1117c8a8bbb1128c2c4b00138d6026f2f57";
pub const V3_POWER_UNIT_THERMAL_PROFILE_SCHEMA: &str =
    "pitgun.racing-v3-thermal-family-profile-candidate/v2";
pub const V3_POWER_UNIT_THERMAL_PROFILE_VERSION: &str = "2.0.0-rc.1";
pub const V3_POWER_UNIT_THERMAL_PROFILE_DIGEST: &str =
    "sha256:47c54c5fb79327d1937c84decab79bfbd9cd72b933f15710515670dc61315c84";

const REVIEWED_STATUS: &str = "REVIEWED_CANDIDATE";
const HISTORICAL_V8: &str = "historical_v8";
const MODERN_V6T: &str = "modern_v6t";
const F1_2026: &str = "f1_2026";

/// Immutable evidence pointers retained by the reviewed thermal candidate.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct V3ThermalFamilySourceEvidenceV1 {
    pub campaign_id: String,
    pub campaign_digest: Digest,
    pub review_id: String,
    pub review_digest: Digest,
    pub databricks_job_id: String,
    pub databricks_run_id: String,
    pub read_only_review_run_id: String,
    pub evidence_versions: BTreeMap<String, u64>,
}

/// Selection semantics owned by the Racing Simulator.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct V3ThermalFamilyResolutionContractV1 {
    pub resolver_owner: String,
    pub solver_input: String,
    pub selection_key: String,
    pub unknown_vehicle_behavior: String,
    pub era_only_selection_forbidden: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct V3ThermalFamilyBindingsV1 {
    pub vehicle_ids: Vec<String>,
    pub eras: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct V3ThermalFamilyEntryV1 {
    pub parameter_set_id: String,
    pub validated_profile_ref: Digest,
    pub bindings: V3ThermalFamilyBindingsV1,
    pub engine_thermal_resolution: V3EngineThermalResolutionParams,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct V3ThermalExcludedFuelControlV1 {
    pub experimental_fuel_reservoir_kg: f64,
    pub reason: String,
    pub owner_issue: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct V3ThermalPromotionBoundaryV1 {
    pub candidate_creation_authorized: bool,
    pub rust_wasm_integration_authorized: bool,
    pub catalog_publication_authorized: bool,
    pub authority_verifier_promotion_authorized: bool,
    pub game_staging_promotion_authorized: bool,
    pub production_promotion_authorized: bool,
    pub automatic_promotion: bool,
}

/// Reviewed, content-addressed family profile consumed by native Rust and WASM.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct V3ThermalFamilyProfileCandidateV1 {
    schema_version: String,
    id: String,
    version: String,
    status: String,
    model: ArtifactIdentity,
    source_evidence: V3ThermalFamilySourceEvidenceV1,
    resolution_contract: V3ThermalFamilyResolutionContractV1,
    profiles: BTreeMap<String, V3ThermalFamilyEntryV1>,
    excluded_from_candidate: V3ThermalExcludedFuelControlV1,
    promotion: V3ThermalPromotionBoundaryV1,
}

/// Exact profile identity selected for one deterministic execution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct V3ThermalFamilyResolutionV1 {
    pub schema_version: String,
    pub candidate: ArtifactIdentity,
    pub model: ArtifactIdentity,
    pub family: String,
    pub vehicle_id: String,
    pub parameter_set_id: String,
    pub validated_profile_ref: Digest,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedV3ThermalFamilyProfileV1 {
    pub evidence: V3ThermalFamilyResolutionV1,
    pub engine_thermal_resolution: V3EngineThermalResolutionParams,
}

impl V3ThermalFamilyProfileCandidateV1 {
    pub fn from_exact_json(candidate_json: &str) -> Result<Self, String> {
        let digest = Digest::from_bytes(candidate_json.as_bytes());
        if digest.to_string() != V3_THERMAL_FAMILY_PROFILE_DIGEST {
            return Err(format!(
                "unsupported thermal family profile digest {digest}; expected {V3_THERMAL_FAMILY_PROFILE_DIGEST}"
            ));
        }
        let candidate = serde_json::from_str::<Self>(candidate_json)
            .map_err(|error| format!("invalid thermal family profile JSON: {error}"))?;
        candidate.validate()?;
        Ok(candidate)
    }

    pub fn identity(&self) -> ArtifactIdentity {
        ArtifactIdentity {
            id: self.id.parse().expect("reviewed profile id is valid"),
            version: self
                .version
                .parse()
                .expect("reviewed profile version is valid"),
            digest: V3_THERMAL_FAMILY_PROFILE_DIGEST
                .parse()
                .expect("reviewed profile digest is valid"),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != V3_THERMAL_FAMILY_PROFILE_SCHEMA
            || self.id != V3_THERMAL_FAMILY_PROFILE_ID
            || self.version != V3_THERMAL_FAMILY_PROFILE_VERSION
            || self.status != REVIEWED_STATUS
        {
            return Err("unsupported thermal family profile identity".to_string());
        }
        if self.model != racing_model_v3_thermal_candidate_identity() {
            return Err("thermal family profile targets an unsupported model".to_string());
        }
        let contract = &self.resolution_contract;
        if contract.resolver_owner != "pitgun-racing-simulator"
            || contract.solver_input != "one resolved engine_thermal_resolution per execution"
            || contract.selection_key != "vehicle_id"
            || contract.unknown_vehicle_behavior != "reject"
            || !contract.era_only_selection_forbidden
        {
            return Err("unsupported thermal family resolution contract".to_string());
        }

        let expected_families = BTreeSet::from([HISTORICAL_V8, MODERN_V6T, F1_2026]);
        let actual_families = self
            .profiles
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        if actual_families != expected_families {
            return Err("thermal family profile coverage is incomplete".to_string());
        }
        let expected_bindings = BTreeMap::from([
            (
                HISTORICAL_V8,
                (
                    ["classic_v8_1960", "classic_v8_1970"].as_slice(),
                    [1, 4].as_slice(),
                ),
            ),
            (MODERN_V6T, (["modern_v6t"].as_slice(), [5].as_slice())),
            (F1_2026, (["f1_2026"].as_slice(), [5].as_slice())),
        ]);
        let mut bound_vehicles = BTreeSet::new();
        for (family, profile) in &self.profiles {
            profile.engine_thermal_resolution.validate()?;
            let (vehicle_ids, eras) = expected_bindings
                .get(family.as_str())
                .expect("family coverage checked above");
            if profile
                .bindings
                .vehicle_ids
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
                != *vehicle_ids
                || profile.bindings.eras != *eras
            {
                return Err(format!("unexpected thermal bindings for family {family:?}"));
            }
            for vehicle_id in &profile.bindings.vehicle_ids {
                if !bound_vehicles.insert(vehicle_id.as_str()) {
                    return Err(format!(
                        "vehicle {vehicle_id:?} has multiple thermal profiles"
                    ));
                }
            }
        }

        if self.excluded_from_candidate.experimental_fuel_reservoir_kg != 130.0
            || self.excluded_from_candidate.owner_issue != 246
        {
            return Err("experimental fuel exclusion is not preserved".to_string());
        }
        let promotion = &self.promotion;
        if !promotion.candidate_creation_authorized
            || !promotion.rust_wasm_integration_authorized
            || promotion.catalog_publication_authorized
            || promotion.authority_verifier_promotion_authorized
            || promotion.game_staging_promotion_authorized
            || promotion.production_promotion_authorized
            || promotion.automatic_promotion
        {
            return Err("thermal candidate exceeds its reviewed promotion boundary".to_string());
        }
        Ok(())
    }

    pub fn resolve_vehicle(
        &self,
        vehicle_id: &str,
    ) -> Result<ResolvedV3ThermalFamilyProfileV1, String> {
        self.validate()?;
        let vehicle_id = vehicle_id.trim();
        let (family, profile) = self
            .profiles
            .iter()
            .find(|(_, profile)| {
                profile
                    .bindings
                    .vehicle_ids
                    .iter()
                    .any(|candidate| candidate == vehicle_id)
            })
            .ok_or_else(|| {
                format!(
                    "unknown vehicle {vehicle_id:?} for thermal family profile {}@{}",
                    self.id, self.version
                )
            })?;
        Ok(ResolvedV3ThermalFamilyProfileV1 {
            evidence: V3ThermalFamilyResolutionV1 {
                schema_version: "pitgun.racing-v3-thermal-family-resolution/v1".to_string(),
                candidate: self.identity(),
                model: self.model.clone(),
                family: family.clone(),
                vehicle_id: vehicle_id.to_string(),
                parameter_set_id: profile.parameter_set_id.clone(),
                validated_profile_ref: profile.validated_profile_ref,
            },
            engine_thermal_resolution: profile.engine_thermal_resolution,
        })
    }
}

/// Per-competitor thermal selection semantics introduced with component
/// composition. The stable selection key names the installed power unit, not
/// a vehicle shell or an era.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct V3PowerUnitThermalResolutionContractV2 {
    pub resolver_owner: String,
    pub solver_input: String,
    pub selection_key: String,
    pub unknown_power_unit_behavior: String,
    pub era_only_selection_forbidden: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct V3PowerUnitThermalBindingsV2 {
    pub power_unit_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct V3PowerUnitThermalEntryV2 {
    pub parameter_set_id: String,
    pub validated_profile_ref: Digest,
    pub bindings: V3PowerUnitThermalBindingsV2,
    pub engine_thermal_resolution: V3EngineThermalResolutionParams,
}

/// Exact reviewed V2 profile. It reuses the reviewed numeric coefficients but
/// changes their deterministic binding from a monolithic vehicle to the power
/// unit installed for each competitor.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct V3PowerUnitThermalProfileCandidateV2 {
    schema_version: String,
    id: String,
    version: String,
    status: String,
    model: ArtifactIdentity,
    source_evidence: V3ThermalFamilySourceEvidenceV1,
    resolution_contract: V3PowerUnitThermalResolutionContractV2,
    profiles: BTreeMap<String, V3PowerUnitThermalEntryV2>,
    promotion: V3ThermalPromotionBoundaryV1,
}

/// Exact power-unit thermal resource selected for one competitor.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct V3PowerUnitThermalResolutionV2 {
    pub schema_version: String,
    pub candidate: ArtifactIdentity,
    pub model: ArtifactIdentity,
    pub family: String,
    pub power_unit_id: String,
    pub parameter_set_id: String,
    pub validated_profile_ref: Digest,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedV3PowerUnitThermalProfileV2 {
    pub evidence: V3PowerUnitThermalResolutionV2,
    pub engine_thermal_resolution: V3EngineThermalResolutionParams,
}

impl V3PowerUnitThermalProfileCandidateV2 {
    pub fn from_exact_json(candidate_json: &str) -> Result<Self, String> {
        let digest = Digest::from_bytes(candidate_json.as_bytes());
        if digest.to_string() != V3_POWER_UNIT_THERMAL_PROFILE_DIGEST {
            return Err(format!(
                "unsupported power-unit thermal profile digest {digest}; expected {V3_POWER_UNIT_THERMAL_PROFILE_DIGEST}"
            ));
        }
        let candidate = serde_json::from_str::<Self>(candidate_json)
            .map_err(|error| format!("invalid power-unit thermal profile JSON: {error}"))?;
        candidate.validate()?;
        Ok(candidate)
    }

    #[must_use]
    pub fn identity(&self) -> ArtifactIdentity {
        ArtifactIdentity {
            id: self.id.parse().expect("reviewed profile id is valid"),
            version: self
                .version
                .parse()
                .expect("reviewed profile version is valid"),
            digest: V3_POWER_UNIT_THERMAL_PROFILE_DIGEST
                .parse()
                .expect("reviewed profile digest is valid"),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != V3_POWER_UNIT_THERMAL_PROFILE_SCHEMA
            || self.id != V3_THERMAL_FAMILY_PROFILE_ID
            || self.version != V3_POWER_UNIT_THERMAL_PROFILE_VERSION
            || self.status != REVIEWED_STATUS
        {
            return Err("unsupported power-unit thermal profile identity".to_string());
        }
        if self.model != racing_model_v3_component_candidate_identity() {
            return Err("power-unit thermal profile targets an unsupported model".to_string());
        }
        let contract = &self.resolution_contract;
        if contract.resolver_owner != "pitgun-racing-simulator"
            || contract.solver_input
                != "one resolved engine_thermal_resolution per competitor execution"
            || contract.selection_key != "installed_power_unit_id"
            || contract.unknown_power_unit_behavior != "reject"
            || !contract.era_only_selection_forbidden
        {
            return Err("unsupported power-unit thermal resolution contract".to_string());
        }

        let expected_families = BTreeSet::from([HISTORICAL_V8, MODERN_V6T, F1_2026]);
        let actual_families = self
            .profiles
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        if actual_families != expected_families {
            return Err("power-unit thermal profile coverage is incomplete".to_string());
        }
        let expected_bindings = BTreeMap::from([
            (HISTORICAL_V8, ["v8_1960", "v8_1970"].as_slice()),
            (MODERN_V6T, ["v6t"].as_slice()),
            (F1_2026, ["v6t_hybrid"].as_slice()),
        ]);
        let mut bound_power_units = BTreeSet::new();
        for (family, profile) in &self.profiles {
            profile.engine_thermal_resolution.validate()?;
            let expected = expected_bindings
                .get(family.as_str())
                .expect("family coverage checked above");
            if profile
                .bindings
                .power_unit_ids
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
                != *expected
            {
                return Err(format!(
                    "unexpected power-unit thermal bindings for family {family:?}"
                ));
            }
            for power_unit_id in &profile.bindings.power_unit_ids {
                if !bound_power_units.insert(power_unit_id.as_str()) {
                    return Err(format!(
                        "power unit {power_unit_id:?} has multiple thermal profiles"
                    ));
                }
            }
        }

        let promotion = &self.promotion;
        if !promotion.candidate_creation_authorized
            || !promotion.rust_wasm_integration_authorized
            || promotion.catalog_publication_authorized
            || promotion.authority_verifier_promotion_authorized
            || promotion.game_staging_promotion_authorized
            || promotion.production_promotion_authorized
            || promotion.automatic_promotion
        {
            return Err(
                "power-unit thermal candidate exceeds its reviewed promotion boundary".to_string(),
            );
        }
        Ok(())
    }

    pub fn resolve_power_unit(
        &self,
        power_unit_id: &str,
    ) -> Result<ResolvedV3PowerUnitThermalProfileV2, String> {
        self.validate()?;
        let power_unit_id = power_unit_id.trim();
        let (family, profile) = self
            .profiles
            .iter()
            .find(|(_, profile)| {
                profile
                    .bindings
                    .power_unit_ids
                    .iter()
                    .any(|candidate| candidate == power_unit_id)
            })
            .ok_or_else(|| {
                format!(
                    "unknown power unit {power_unit_id:?} for thermal family profile {}@{}",
                    self.id, self.version
                )
            })?;
        Ok(ResolvedV3PowerUnitThermalProfileV2 {
            evidence: V3PowerUnitThermalResolutionV2 {
                schema_version: "pitgun.racing-v3-thermal-family-resolution/v2".to_string(),
                candidate: self.identity(),
                model: self.model.clone(),
                family: family.clone(),
                power_unit_id: power_unit_id.to_string(),
                parameter_set_id: profile.parameter_set_id.clone(),
                validated_profile_ref: profile.validated_profile_ref,
            },
            engine_thermal_resolution: profile.engine_thermal_resolution,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const POWER_UNIT_CANDIDATE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../experiments/racing_v3_thermal_refinement/candidates/thermal-family-profile-v2.json"
    ));

    fn parsed_candidate() -> V3PowerUnitThermalProfileCandidateV2 {
        serde_json::from_str(POWER_UNIT_CANDIDATE).expect("power-unit candidate JSON")
    }

    #[test]
    fn power_unit_profile_rejects_missing_and_duplicate_bindings() {
        let mut missing = parsed_candidate();
        missing
            .profiles
            .get_mut(F1_2026)
            .expect("F1 2026 family")
            .bindings
            .power_unit_ids
            .clear();
        assert!(
            missing
                .validate()
                .expect_err("missing binding must fail")
                .contains("unexpected power-unit thermal bindings")
        );

        let mut duplicate = parsed_candidate();
        duplicate
            .profiles
            .get_mut(F1_2026)
            .expect("F1 2026 family")
            .bindings
            .power_unit_ids
            .push("v6t".to_string());
        assert!(
            duplicate
                .validate()
                .expect_err("duplicate binding must fail")
                .contains("unexpected power-unit thermal bindings")
        );
    }
}
