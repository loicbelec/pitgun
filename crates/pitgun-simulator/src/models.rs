use serde::{Deserialize, Serialize};

use crate::errors::SimulatorError;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AeroConfig {
    pub id: String,
    pub cd_a_straight: f64,
    pub cd_a_corner: f64,
    pub cl_a_straight: f64,
    pub cl_a_corner: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChassisConfig {
    pub id: String,
    pub mass_empty_kg: f64,
    pub wheel_radius_m: f64,
    pub mu0: f64,
    pub rolling_resistance: f64,
    pub air_density: f64,
    pub gravity: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EngineThermalConfig {
    pub ambient_temp_c: f64,
    pub initial_temp_c: f64,
    pub capacity_j_per_c: f64,
    pub heat_alpha: f64,
    pub cooling_base_w: f64,
    pub cooling_speed_w_per_ms: f64,
    pub soft_temp_c: f64,
    pub derate_per_c: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EngineConfig {
    pub id: String,
    pub rpm_samples: Vec<f64>,
    pub torque_samples: Vec<f64>,
    pub gear_ratios: Vec<f64>,
    pub idle_rpm: f64,
    pub max_rpm: f64,
    pub thermal: EngineThermalConfig,
    pub fuel_burn_kg_per_s: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TireConfig {
    pub id: String,
    pub mu_scale: f64,
    pub wear_per_s: f64,
    pub wear_load_k: f64,
    pub wear_grip_k: f64,
    pub wear_min: f64,
    pub temp_opt_c: f64,
    pub temp_sigma_c: f64,
    pub temp_min_k: f64,
    pub heat_k: f64,
    pub cool_k: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VehicleConfig {
    pub id: String,
    pub aero_id: String,
    pub chassis_id: String,
    pub engine_id: String,
    pub tire_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrackConfig {
    pub id: String,
    pub s_m: Vec<f64>,
    pub x_m: Vec<f64>,
    pub y_m: Vec<f64>,
    pub z_m: Vec<f64>,
    pub curvature_radpm: Vec<f64>,
    pub slope: Vec<f64>,
    pub heading_rad: Vec<f64>,
}

impl AeroConfig {
    pub fn validate(&self) -> Result<(), SimulatorError> {
        if self.cd_a_straight <= 0.0 || self.cd_a_corner <= 0.0 {
            return Err(SimulatorError::InvalidConfig {
                kind: "aero",
                id: self.id.clone(),
                reason: "drag coefficients must be > 0".to_string(),
            });
        }
        Ok(())
    }
}

impl ChassisConfig {
    pub fn validate(&self) -> Result<(), SimulatorError> {
        if self.mass_empty_kg <= 0.0 || self.wheel_radius_m <= 0.0 {
            return Err(SimulatorError::InvalidConfig {
                kind: "chassis",
                id: self.id.clone(),
                reason: "mass and wheel radius must be > 0".to_string(),
            });
        }
        Ok(())
    }
}

impl EngineConfig {
    pub fn validate(&self) -> Result<(), SimulatorError> {
        if self.rpm_samples.len() < 2
            || self.torque_samples.len() != self.rpm_samples.len()
            || self.gear_ratios.is_empty()
        {
            return Err(SimulatorError::InvalidConfig {
                kind: "engine",
                id: self.id.clone(),
                reason: "invalid curve/gear dimensions".to_string(),
            });
        }
        if !self.rpm_samples.windows(2).all(|w| w[1] > w[0]) {
            return Err(SimulatorError::InvalidConfig {
                kind: "engine",
                id: self.id.clone(),
                reason: "rpm_samples must be strictly increasing".to_string(),
            });
        }
        if self.max_rpm <= self.idle_rpm || self.fuel_burn_kg_per_s < 0.0 {
            return Err(SimulatorError::InvalidConfig {
                kind: "engine",
                id: self.id.clone(),
                reason: "rpm/fuel constraints are invalid".to_string(),
            });
        }
        Ok(())
    }
}

impl TireConfig {
    pub fn validate(&self) -> Result<(), SimulatorError> {
        if self.mu_scale <= 0.0 || self.temp_sigma_c <= 0.0 {
            return Err(SimulatorError::InvalidConfig {
                kind: "tire",
                id: self.id.clone(),
                reason: "mu_scale and temp_sigma must be > 0".to_string(),
            });
        }
        Ok(())
    }
}

impl TrackConfig {
    pub fn validate(&self) -> Result<(), SimulatorError> {
        let n = self.s_m.len();
        if n < 3
            || self.x_m.len() != n
            || self.y_m.len() != n
            || self.z_m.len() != n
            || self.curvature_radpm.len() != n
            || self.slope.len() != n
            || self.heading_rad.len() != n
        {
            return Err(SimulatorError::InvalidConfig {
                kind: "track",
                id: self.id.clone(),
                reason: "track vectors must share the same length >= 3".to_string(),
            });
        }
        if !self.s_m.windows(2).all(|w| w[1] > w[0]) {
            return Err(SimulatorError::InvalidConfig {
                kind: "track",
                id: self.id.clone(),
                reason: "s_m must be strictly increasing".to_string(),
            });
        }
        Ok(())
    }
}
