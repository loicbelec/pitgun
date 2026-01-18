use crate::PlayerTuning;

pub mod presets;
pub mod registry;

#[derive(Clone, Debug)]
pub struct VehicleSpec {
    pub mass_kg: f32,
    pub rho: f32,
    pub g: f32,
    pub wheel_radius_m: f32,
    pub mu: f32,
    pub c_rr: f32,
    pub cd_a_x: f32,
    pub cd_a_z: f32,
    pub cl_a_x: f32,
    pub cl_a_z: f32,
    pub rpm_curve: Vec<f32>,
    pub torque_curve: Vec<f32>,
    pub n_idle: f32,
    pub n_max: f32,
    pub g1_total: f32,
    pub g_last_total: f32,
    pub gear_count: u32,
    pub n_upshift: f32,
    pub n_downshift: f32,
    pub gear_ratios: Vec<f32>,
    pub t_amb: f32,
    pub t_init: f32,
    pub t_soft: f32,
    pub c_th: f32,
    pub alpha_heat: f32,
    pub p_cool0: f32,
    pub k_cool: f32,
    pub beta_derate: f32,
}

impl VehicleSpec {
    pub fn apply_tuning(&mut self, tuning: &PlayerTuning) {
        let df = tuning.downforce_slider.clamp(0.0, 1.0);
        let gr = tuning.gear_ratio_slider.clamp(0.0, 1.0);
        let aero_k = 1.0 + 0.10 * (tuning.aero_points as f32 / 20.0);

        let drag_blend = 0.85 + 0.30 * df;
        let df_blend = 0.75 + 0.55 * df;

        self.cd_a_x *= aero_k * drag_blend * 0.95;
        self.cd_a_z *= aero_k * drag_blend * 1.05;
        self.cl_a_x *= aero_k * df_blend * 0.95;
        self.cl_a_z *= aero_k * df_blend * 1.05;

        self.mu *= 1.0 + 0.08 * (tuning.chassis_points as f32 / 20.0);

        let cool_k = 1.0 + 0.35 * (tuning.cooling_points as f32 / 20.0);
        self.p_cool0 *= cool_k;
        self.k_cool *= cool_k;

        let engine_k = 1.0 + 0.01 * (tuning.engine_points as f32 / 20.0);
        for value in &mut self.torque_curve {
            *value *= engine_k;
        }

        let scale = 1.10 - 0.20 * gr;
        self.g1_total *= scale;
        self.g_last_total *= scale;
        self.update_gear_ratios();
    }

    pub fn update_gear_ratios(&mut self) {
        self.gear_ratios = generate_gear_ratios(self.g1_total, self.g_last_total, self.gear_count);
    }

    pub fn power_kw_from_rpm(&self, rpm: f32) -> f32 {
        if rpm <= self.rpm_curve[0] {
            return 0.0;
        }
        if rpm >= *self.rpm_curve.last().unwrap() {
            return 0.0;
        }

        let idx = self
            .rpm_curve
            .partition_point(|&x| x < rpm)
            .saturating_sub(1);
        let x0 = self.rpm_curve[idx];
        let x1 = self.rpm_curve[idx + 1];
        let y0 = self.torque_curve[idx];
        let y1 = self.torque_curve[idx + 1];
        let t = (rpm - x0) / (x1 - x0);
        let torque = y0 + (y1 - y0) * t;
        torque * rpm * std::f32::consts::PI / 30.0
    }

    pub fn max_engine_power(&self, speed_mps: f32, temp_c: f32) -> (f32, f32, usize) {
        let mut gear_choice = 1;
        let mut pwr_max = 0.0;
        let mut rpm_max = 0.0;

        for (idx, ratio) in self.gear_ratios.iter().enumerate() {
            let rpm = speed_mps * 60.0 * ratio / (2.0 * std::f32::consts::PI * self.wheel_radius_m);
            let pwr = self.power_kw_from_rpm(rpm);
            if pwr > pwr_max {
                pwr_max = pwr;
                gear_choice = idx + 1;
                rpm_max = rpm;
            }
        }

        if temp_c > self.t_soft {
            let loss = (temp_c - self.t_soft) * self.beta_derate;
            let derate = (1.0 - loss).max(0.2);
            pwr_max *= derate;
        }

        (pwr_max, rpm_max, gear_choice)
    }
}

fn generate_gear_ratios(g1_total: f32, g_last_total: f32, gear_count: u32) -> Vec<f32> {
    let steps = gear_count.saturating_sub(1).max(1) as f32;
    (0..gear_count)
        .map(|k| g1_total * (g_last_total / g1_total).powf(k as f32 / steps))
        .collect()
}
