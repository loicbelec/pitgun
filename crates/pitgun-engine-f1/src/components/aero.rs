use crate::core::{Aero as AeroTrait, Tuning};

#[derive(Debug, Clone)]
pub struct NoAero {
    pub cda_x: f64,
    pub cda_z: f64,
    pub cla_x: f64,
    pub cla_z: f64,
}

impl Default for NoAero {
    fn default() -> Self {
        Self::new()
    }
}

impl NoAero {
    pub fn new() -> Self {
        Self {
            cda_x: 0.3,
            cda_z: 0.3,
            cla_x: 0.0,
            cla_z: 0.0,
        }
    }
}

impl AeroTrait for NoAero {
    fn coeffs_straight(&self) -> (f64, f64) {
        (self.cda_x, self.cla_x)
    }

    fn coeffs_corner(&self) -> (f64, f64) {
        (self.cda_z, self.cla_z)
    }

    fn apply_tuning(&mut self, _tuning: &Tuning) {}
}

#[derive(Debug, Clone)]
pub struct Aero {
    pub cda_x: f64,
    pub cda_z: f64,
    pub cla_x: f64,
    pub cla_z: f64,
}

impl Default for Aero {
    fn default() -> Self {
        Self::new()
    }
}

impl Aero {
    pub fn new() -> Self {
        Self {
            cda_x: 0.9,
            cda_z: 0.9,
            cla_x: 4.0,
            cla_z: 4.0,
        }
    }
}

impl AeroTrait for Aero {
    fn coeffs_straight(&self) -> (f64, f64) {
        (self.cda_x, self.cla_x)
    }

    fn coeffs_corner(&self) -> (f64, f64) {
        (self.cda_z, self.cla_z)
    }

    fn apply_tuning(&mut self, tuning: &Tuning) {
        let df = tuning.downforce_slider.clamp(0.0, 1.0);
        let aero_k = 1.0 + 0.10 * (tuning.aero_points / 20.0);

        let drag_blend = 0.80 + 0.70 * df;
        let df_blend = 0.75 + 0.55 * df;

        // Python behavior is cumulative (no reset to baseline).
        self.cda_x *= drag_blend * aero_k * 0.95;
        self.cda_z *= drag_blend * aero_k * 1.05;
        self.cla_x *= df_blend * aero_k * 0.95;
        self.cla_z *= df_blend * aero_k * 1.05;
    }
}

#[derive(Debug, Clone)]
pub struct ActiveAero {
    pub cda_x: f64,
    pub cda_z: f64,
    pub cla_x: f64,
    pub cla_z: f64,
}

impl Default for ActiveAero {
    fn default() -> Self {
        Self::new()
    }
}

impl ActiveAero {
    pub fn new() -> Self {
        Self {
            cda_x: 0.8,
            cda_z: 1.0,
            cla_x: 2.6,
            cla_z: 4.13,
        }
    }
}

impl AeroTrait for ActiveAero {
    fn coeffs_straight(&self) -> (f64, f64) {
        (self.cda_x, self.cla_x)
    }

    fn coeffs_corner(&self) -> (f64, f64) {
        (self.cda_z, self.cla_z)
    }

    fn apply_tuning(&mut self, tuning: &Tuning) {
        let df = tuning.downforce_slider.clamp(0.0, 1.0);
        let aero_k = 0.9 + 0.10 * (tuning.aero_points / 20.0);

        let drag_blend = 0.85 + 0.30 * df;
        let df_blend = 0.75 + 0.55 * df;

        // Python behavior is cumulative (no reset to baseline).
        self.cda_x *= drag_blend * aero_k / 2.0;
        self.cda_z *= drag_blend * aero_k / 2.0;
        self.cla_x *= df_blend * aero_k;
        self.cla_z *= df_blend * aero_k;
    }
}
