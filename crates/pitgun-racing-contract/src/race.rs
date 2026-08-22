use serde::{Deserialize, Serialize};

/// Wire version of one explicit Racing vehicle-component selection.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum VehicleComponentSelectionVersion {
    /// First component override contract layered over a catalog vehicle.
    #[serde(rename = "pitgun.racing-vehicle-components/v1")]
    V1,
}

/// Optional physical component overrides for one Racing competitor.
///
/// The catalog vehicle selected by the workload remains the compatibility
/// baseline. Every populated field replaces exactly one component of that
/// baseline. Game progression and HQ upgrade identities intentionally stay
/// outside this domain contract.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VehicleComponentSelectionV1 {
    /// Exact wire semantics for this component selection.
    pub schema_version: VehicleComponentSelectionVersion,
    /// Optional aerodynamic package resource identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aero_id: Option<String>,
    /// Optional chassis resource identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chassis_id: Option<String>,
    /// Optional engine or power-unit resource identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine_id: Option<String>,
    /// Optional default tire resource identity used when no stint overrides it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tire_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RunPackage {
    pub input: RaceInput,
    pub output: RaceOutput,
    /// The seed used for the deterministic RNG.
    pub seed: u64,
    /// Git hash or version string of the engine used.
    pub engine_version: String,
    /// Version of the policy used for validation.
    pub policy_version: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RaceInput {
    pub track_id: String,
    pub laps: u16,
    pub competitors: Vec<CompetitorSpec>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RaceStint {
    pub tire_id: String,
    pub laps: u16,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CompetitorStintStrategy {
    pub stints: Vec<RaceStint>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pit_laps: Vec<u16>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CompetitorSpec {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub driver_id: Option<String>,
    pub name: String,
    pub team_id: String,
    pub is_player: bool,
    pub tuning: TuningSpec,
    /// Total point budget used by this competitor (for validation).
    pub budget_cap: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stint_strategy: Option<CompetitorStintStrategy>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct TuningSpec {
    pub engine_points: f64,
    pub cooling_points: f64,
    pub aero_points: f64,
    pub chassis_points: f64,
    pub downforce_slider: f64,  // 0.0 - 1.0
    pub gear_ratio_slider: f64, // 0.0 - 1.0
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
pub struct RaceOutput {
    pub standings: Vec<StandingEntry>,
    pub total_time_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct StandingEntry {
    pub competitor_id: String,
    pub position: u8,
    pub total_time_ms: u64,
    pub best_lap_ms: u64,
    pub laps_completed: u16,
    pub gap_to_leader_ms: u64,
    pub status: CompetitorStatus,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CompetitorStatus {
    Finished,
    Dnf(String),
    Dsq(String),
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VehicleClass {
    Legacy1960,
    GroundEffect1970,
    HybridModern,
    ActiveAero2026,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CircuitCatalogEntry {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub country_code: Option<String>,
    pub sample_count: usize,
    pub distance_m: f64,
    pub pit_loss_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct EngineCatalogEntry {
    pub id: String,
    pub idle_rpm: f64,
    pub max_rpm: f64,
    pub gear_count: usize,
}

/// Canonical mapping from game-era (or explicit year) to vehicle class.
///
/// Game-era mapping:
/// - era 1-2 -> Legacy1960
/// - era 3-4 -> GroundEffect1970
/// - era 5   -> HybridModern
/// - era 6+  -> ActiveAero2026
///
/// Year fallback:
/// - >= 2026 -> ActiveAero2026
/// - >= 2014 -> HybridModern
/// - >= 1970 -> GroundEffect1970
/// - else    -> Legacy1960
pub fn resolve_vehicle_class(era: i32) -> VehicleClass {
    if era > 0 && era <= 10 {
        return match era {
            1 | 2 => VehicleClass::Legacy1960,
            3 | 4 => VehicleClass::GroundEffect1970,
            5 => VehicleClass::HybridModern,
            _ => VehicleClass::ActiveAero2026,
        };
    }

    if era >= 2026 {
        VehicleClass::ActiveAero2026
    } else if era >= 2014 {
        VehicleClass::HybridModern
    } else if era >= 1970 {
        VehicleClass::GroundEffect1970
    } else {
        VehicleClass::Legacy1960
    }
}

#[cfg(test)]
mod tests {
    use super::{
        VehicleClass, VehicleComponentSelectionV1, VehicleComponentSelectionVersion,
        resolve_vehicle_class,
    };

    #[test]
    fn maps_game_eras_to_expected_vehicle_class() {
        assert_eq!(resolve_vehicle_class(1), VehicleClass::Legacy1960);
        assert_eq!(resolve_vehicle_class(2), VehicleClass::Legacy1960);
        assert_eq!(resolve_vehicle_class(3), VehicleClass::GroundEffect1970);
        assert_eq!(resolve_vehicle_class(4), VehicleClass::GroundEffect1970);
        assert_eq!(resolve_vehicle_class(5), VehicleClass::HybridModern);
        assert_eq!(resolve_vehicle_class(6), VehicleClass::ActiveAero2026);
        assert_eq!(resolve_vehicle_class(7), VehicleClass::ActiveAero2026);
    }

    #[test]
    fn maps_year_fallbacks_to_expected_vehicle_class() {
        assert_eq!(resolve_vehicle_class(1960), VehicleClass::Legacy1960);
        assert_eq!(resolve_vehicle_class(1970), VehicleClass::GroundEffect1970);
        assert_eq!(resolve_vehicle_class(2025), VehicleClass::HybridModern);
        assert_eq!(resolve_vehicle_class(2026), VehicleClass::ActiveAero2026);
    }

    #[test]
    fn vehicle_component_selection_has_strict_versioned_wire_semantics() {
        let selection = VehicleComponentSelectionV1 {
            schema_version: VehicleComponentSelectionVersion::V1,
            aero_id: Some("basic".to_string()),
            chassis_id: None,
            engine_id: Some("v8_1970".to_string()),
            tire_id: None,
        };

        assert_eq!(
            serde_json::to_value(&selection).expect("component selection"),
            serde_json::json!({
                "schema_version": "pitgun.racing-vehicle-components/v1",
                "aero_id": "basic",
                "engine_id": "v8_1970"
            })
        );
        assert!(
            serde_json::from_value::<VehicleComponentSelectionV1>(serde_json::json!({
                "schema_version": "pitgun.racing-vehicle-components/v1",
                "aero_id": "basic",
                "unreviewed_component": "forged"
            }))
            .is_err()
        );
    }
}
