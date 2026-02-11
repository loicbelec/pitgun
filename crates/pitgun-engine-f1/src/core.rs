
/// Configuration de tuning passée par le joueur/jeu
#[derive(Debug, Clone, Default)]
pub struct Tuning {
    pub engine_points: f64,
    pub cooling_points: f64,
    pub aero_points: f64,
    pub chassis_points: f64,
    pub downforce_slider: f64, // 0.0 - 1.0
    pub gear_ratio_slider: f64, // 0.0 - 1.0 (Short <-> Long)
}

/// Interface Moteur
pub trait Engine {
    fn torque(&self, rpm: f64) -> f64;
    fn power_kw_from_rpm(&self, rpm: f64) -> f64;
    fn derating_factor(&self, temp_c: f64) -> f64;
    fn ambient_temp_c(&self) -> f64;
    fn thermal_init_c(&self) -> f64;
    fn thermal_capacity_j_per_c(&self) -> f64;
    fn heat_alpha(&self) -> f64;
    fn cooling_base_w(&self) -> f64;
    fn cooling_speed_w_per_ms(&self) -> f64;
    fn max_rpm(&self) -> f64;
    fn idle_rpm(&self) -> f64;
    fn gear_ratio(&self, gear: u8) -> f64;
    fn gear_count(&self) -> u8;
    fn upshift_rpm(&self) -> f64;
    fn downshift_rpm(&self) -> f64;
    fn apply_tuning(&mut self, tuning: &Tuning);
}

/// Interface Aéro
pub trait Aero {
    /// Retourne (CdA (Drag), ClA (Lift)) en mode ligne droite (X)
    fn coeffs_straight(&self) -> (f64, f64);
    /// Retourne (CdA (Drag), ClA (Lift)) en mode virage/freinage (Z)
    fn coeffs_corner(&self) -> (f64, f64);
    /// Compat: garde le comportement historique (mode Z)
    fn coeffs(&self) -> (f64, f64) {
        self.coeffs_corner()
    }
    fn apply_tuning(&mut self, tuning: &Tuning);
}

/// Interface Châssis
pub trait Chassis {
    fn mass(&self) -> f64;
    fn air_density(&self) -> f64;
    fn gravity(&self) -> f64;
    fn wheel_radius(&self) -> f64;
    fn friction_mu(&self) -> f64;
    fn rolling_resistance(&self) -> f64;
    fn apply_tuning(&mut self, tuning: &Tuning);
}
