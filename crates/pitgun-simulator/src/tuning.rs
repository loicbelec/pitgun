use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Tuning {
    pub engine_points: f64,
    pub cooling_points: f64,
    pub aero_points: f64,
    pub chassis_points: f64,
    pub downforce_slider: f64,
    pub gear_ratio_slider: f64,
}

impl Tuning {
    pub fn clamped(&self) -> Self {
        Self {
            engine_points: clamp(self.engine_points, 0.0, 40.0),
            cooling_points: clamp(self.cooling_points, 0.0, 40.0),
            aero_points: clamp(self.aero_points, 0.0, 40.0),
            chassis_points: clamp(self.chassis_points, 0.0, 40.0),
            downforce_slider: clamp(self.downforce_slider, 0.0, 1.0),
            gear_ratio_slider: clamp(self.gear_ratio_slider, 0.0, 1.0),
        }
    }
}

fn clamp(v: f64, lo: f64, hi: f64) -> f64 {
    v.max(lo).min(hi)
}
