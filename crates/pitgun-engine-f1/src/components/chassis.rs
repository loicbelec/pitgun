use crate::core::{Chassis, Tuning};

#[derive(Debug, Clone)]
pub struct StandardChassis {
    pub m: f64,
    pub rho: f64,
    pub g: f64,
    pub r_wheel: f64,
    pub mu: f64,
    pub c_rr: f64,
}

impl Default for StandardChassis {
    fn default() -> Self {
        Self::new()
    }
}

impl StandardChassis {
    pub fn new() -> Self {
        Self {
            m: 800.0,
            rho: 1.225,
            g: 9.81,
            r_wheel: 0.34,
            mu: 1.5,
            c_rr: 0.02,
        }
    }
}

impl Chassis for StandardChassis {
    fn mass(&self) -> f64 {
        self.m
    }

    fn air_density(&self) -> f64 {
        self.rho
    }

    fn gravity(&self) -> f64 {
        self.g
    }

    fn wheel_radius(&self) -> f64 {
        self.r_wheel
    }

    fn friction_mu(&self) -> f64 {
        self.mu
    }
    
    fn rolling_resistance(&self) -> f64 {
        self.c_rr
    }

    fn apply_tuning(&mut self, tuning: &Tuning) {
        let grip_blend = 1.0 + 0.08 * (tuning.chassis_points / 20.0);
        // Python behavior is cumulative (no reset to baseline).
        self.mu *= grip_blend;
    }
}
