use crate::core::{Aero, Chassis, Engine, Tuning};

pub struct Vehicle<A, C, E> {
    pub aero: A,
    pub chassis: C,
    pub engine: E,
}

impl<A: Aero, C: Chassis, E: Engine> Vehicle<A, C, E> {
    pub fn new(aero: A, chassis: C, engine: E) -> Self {
        Self { aero, chassis, engine }
    }

    pub fn apply_tuning(&mut self, tuning: &Tuning) {
        self.aero.apply_tuning(tuning);
        self.chassis.apply_tuning(tuning);
        self.engine.apply_tuning(tuning);
    }

    pub fn rpm_from_speed_gear(&self, speed_ms: f64, gear: u8) -> f64 {
        let ratio = self.engine.gear_ratio(gear);
        if ratio <= 0.0 {
            return 0.0;
        }
        speed_ms * 60.0 * ratio / (2.0 * std::f64::consts::PI * self.chassis.wheel_radius())
    }

    pub fn power_kw_from_rpm(&self, rpm: f64) -> f64 {
        self.engine.power_kw_from_rpm(rpm)
    }

    pub fn derating_factor(&self, temp_c: f64) -> f64 {
        self.engine.derating_factor(temp_c)
    }

    pub fn max_engine_power(&self, speed_ms: f64, temp_c: f64) -> (f64, f64, u8) {
        let mut pwr_max = 0.0;
        let mut rpm_at_pmax = 0.0;
        let mut best_gear = 1u8;

        for gear in 1..=self.engine.gear_count() {
            let rpm = self.rpm_from_speed_gear(speed_ms, gear);
            let pwr = self.power_kw_from_rpm(rpm);
            if pwr > pwr_max {
                pwr_max = pwr;
                rpm_at_pmax = rpm;
                best_gear = gear;
            }
        }

        pwr_max *= self.derating_factor(temp_c);
        (pwr_max, rpm_at_pmax, best_gear)
    }

    pub fn is_powerful_enough(&self, target_power_kw: f64, rpm: f64, temp_c: f64) -> bool {
        let available = self.power_kw_from_rpm(rpm) * self.derating_factor(temp_c);
        available >= target_power_kw
    }

    /// Calcule la force motrice disponible aux roues pour une vitesse et un rapport donnés
    pub fn available_drive_force(&self, speed_ms: f64, gear: u8) -> f64 {
        let r_wheel = self.chassis.wheel_radius();
        let ratio = self.engine.gear_ratio(gear);
        
        if ratio <= 0.0 { return 0.0; }

        // V = w * r  => w = V / r  (rad/s)
        // RPM = w * 60 / 2pi
        let wheel_rpm = (speed_ms / r_wheel) * (60.0 / (2.0 * std::f64::consts::PI));
        let engine_rpm = wheel_rpm * ratio;

        if engine_rpm > self.engine.max_rpm() {
            return 0.0; // Rupteur
        }

        let torque_engine = self.engine.torque(engine_rpm);
        let torque_wheel = torque_engine * ratio; // * efficacité transmission (supposée 1.0 ici)

        torque_wheel / r_wheel // Force = Couple / Rayon
    }
}
