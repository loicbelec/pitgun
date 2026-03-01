use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SimulatorState {
    pub fuel_mass_kg: f64,
    pub tire_wear: f64,
    pub tire_temp_c: f64,
    pub engine_temp_c: f64,
    #[serde(default)]
    pub exit_speed_mps: f64,
    #[serde(default = "default_exit_gear")]
    pub exit_gear: u8,
}

impl Default for SimulatorState {
    fn default() -> Self {
        Self {
            fuel_mass_kg: 100.0,
            tire_wear: 0.0,
            tire_temp_c: 90.0,
            engine_temp_c: 90.0,
            exit_speed_mps: 0.0,
            exit_gear: default_exit_gear(),
        }
    }
}

const fn default_exit_gear() -> u8 {
    1
}
