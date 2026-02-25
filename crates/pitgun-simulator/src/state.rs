use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SimulatorState {
    pub fuel_mass_kg: f64,
    pub tire_wear: f64,
    pub tire_temp_c: f64,
    pub engine_temp_c: f64,
}

impl Default for SimulatorState {
    fn default() -> Self {
        Self {
            fuel_mass_kg: 20.0,
            tire_wear: 0.0,
            tire_temp_c: 90.0,
            engine_temp_c: 90.0,
        }
    }
}
