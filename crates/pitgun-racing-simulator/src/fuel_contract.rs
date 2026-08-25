use pitgun_racing_solver::FuelMassParamsV3;
use serde::{Deserialize, Serialize};

pub const RACING_FUEL_CONTRACT_SCHEMA: &str = "pitgun.racing-fuel-contract/v1";
pub const RACING_FUEL_CONTRACT_ID: &str = "pitgun.racing.fuel-contract";
pub const RACING_FUEL_CONTRACT_VERSION: &str = "1.0.0";

/// Immutable published fuel semantics selected by a Racing catalog.
///
/// Offline experiments may still provide an explicit fuel load. Published
/// workloads instead resolve both their load and consumption coefficients
/// from this catalog-owned contract so browsers, Authority and Verifier use
/// the same physical boundary.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RacingFuelContractV1 {
    schema_version: RacingFuelContractVersion,
    id: RacingFuelContractId,
    version: RacingFuelContractSemanticVersion,
    pub default_initial_fuel_mass_kg: f64,
    pub minimum_finish_reserve_kg: f64,
    pub depletion_behavior: RacingFuelDepletionBehavior,
    pub consumption: FuelMassParamsV3,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
enum RacingFuelContractVersion {
    #[serde(rename = "pitgun.racing-fuel-contract/v1")]
    V1,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
enum RacingFuelContractId {
    #[serde(rename = "pitgun.racing.fuel-contract")]
    FuelContract,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
enum RacingFuelContractSemanticVersion {
    #[serde(rename = "1.0.0")]
    V1,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum RacingFuelDepletionBehavior {
    Reject,
}

impl RacingFuelContractV1 {
    pub fn from_json(json: &str) -> Result<Self, String> {
        let contract = serde_json::from_str::<Self>(json)
            .map_err(|error| format!("invalid Racing fuel contract JSON: {error}"))?;
        contract.validate()?;
        Ok(contract)
    }

    pub fn validate(&self) -> Result<(), String> {
        if !self.default_initial_fuel_mass_kg.is_finite()
            || !(1.0..=200.0).contains(&self.default_initial_fuel_mass_kg)
        {
            return Err("default initial fuel mass must be in [1, 200] kg".to_string());
        }
        if !self.minimum_finish_reserve_kg.is_finite()
            || !(0.0..self.default_initial_fuel_mass_kg).contains(&self.minimum_finish_reserve_kg)
        {
            return Err(
                "minimum finish reserve must be non-negative and below the initial load"
                    .to_string(),
            );
        }
        self.consumption.validate()
    }
}

#[cfg(test)]
mod tests {
    use super::RacingFuelContractV1;

    #[test]
    fn published_contract_is_strict_and_physically_bounded() {
        let contract = RacingFuelContractV1::from_json(
            r#"{
                "schema_version":"pitgun.racing-fuel-contract/v1",
                "id":"pitgun.racing.fuel-contract",
                "version":"1.0.0",
                "default_initial_fuel_mass_kg":110.0,
                "minimum_finish_reserve_kg":1.0,
                "depletion_behavior":"reject",
                "consumption":{
                    "brake_specific_fuel_consumption_kg_per_kwh":0.19,
                    "idle_fuel_flow_kg_per_s":0.0004
                }
            }"#,
        )
        .expect("valid fuel contract");
        assert_eq!(contract.default_initial_fuel_mass_kg, 110.0);

        assert!(
            RacingFuelContractV1::from_json(
                r#"{
                "schema_version":"pitgun.racing-fuel-contract/v1",
                "id":"pitgun.racing.fuel-contract",
                "version":"1.0.0",
                "default_initial_fuel_mass_kg":0.0,
                "minimum_finish_reserve_kg":1.0,
                "depletion_behavior":"reject",
                "consumption":{
                    "brake_specific_fuel_consumption_kg_per_kwh":0.19,
                    "idle_fuel_flow_kg_per_s":0.0004
                }
            }"#,
            )
            .is_err()
        );
    }
}
