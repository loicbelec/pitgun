use serde::{Deserialize, Serialize};

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
    Dnf(String), // Reason
    Dsq(String), // Reason
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
    pub display_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub country_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub laps: Option<u16>,
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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DriverCatalogEntry {
    pub id: String,
    pub display_name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TireCatalogEntry {
    pub id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct VehicleCatalogEntry {
    pub id: String,
    pub engine_id: String,
    pub default_tire_id: String,
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
    use super::{VehicleClass, resolve_vehicle_class};

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
}
