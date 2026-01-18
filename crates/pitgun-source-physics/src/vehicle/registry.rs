use crate::vehicle::presets;
use crate::vehicle::VehicleSpec;

pub const DEFAULT_VEHICLE_ID: &str = presets::f1_2026::VEHICLE_ID;

#[derive(Debug)]
pub enum VehicleLoadError {
    UnknownVehicle(String),
}

impl std::fmt::Display for VehicleLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VehicleLoadError::UnknownVehicle(id) => write!(f, "unknown vehicle_id '{id}'"),
        }
    }
}

impl std::error::Error for VehicleLoadError {}

pub fn load_vehicle(vehicle_id: &str) -> Result<VehicleSpec, VehicleLoadError> {
    let resolved = if vehicle_id.trim().is_empty() {
        DEFAULT_VEHICLE_ID
    } else {
        vehicle_id
    };

    match resolved {
        presets::f1_2026::VEHICLE_ID => Ok(presets::f1_2026::build()),
        other => Err(VehicleLoadError::UnknownVehicle(other.to_string())),
    }
}
