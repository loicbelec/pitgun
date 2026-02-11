use std::f64::consts::PI;

use crate::core::{Engine, Tuning};

fn linspace(start: f64, end: f64, points: usize) -> Vec<f64> {
    if points == 1 {
        return vec![start];
    }
    (0..points)
        .map(|i| {
            let t = i as f64 / (points - 1) as f64;
            start + (end - start) * t
        })
        .collect()
}

fn rpm_axis(max_rpm: f64) -> Vec<f64> {
    let count = (max_rpm / 250.0).round() as usize + 1;
    (0..count).map(|i| i as f64 * 250.0).collect()
}

fn generate_gear_ratios_total(g1_total: f64, g_last_total: f64, gear_count: u8) -> Vec<f64> {
    let denom = (gear_count - 1) as f64;
    (0..gear_count)
        .map(|k| g1_total * (g_last_total / g1_total).powf(k as f64 / denom))
        .collect()
}

fn interp_with_zero_outside(x: f64, xp: &[f64], fp: &[f64]) -> f64 {
    if xp.is_empty() || fp.is_empty() || xp.len() != fp.len() {
        return 0.0;
    }
    if x < xp[0] || x > xp[xp.len() - 1] {
        return 0.0;
    }

    let hi = xp.partition_point(|&v| v <= x);
    if hi == 0 {
        return fp[0];
    }
    if hi >= xp.len() {
        return fp[fp.len() - 1];
    }

    let i0 = hi - 1;
    let x0 = xp[i0];
    let x1 = xp[hi];
    if (x1 - x0).abs() < f64::EPSILON {
        return fp[i0];
    }
    let t = (x - x0) / (x1 - x0);
    fp[i0] + t * (fp[hi] - fp[i0])
}

fn power_kw_from_curve(rpm: f64, n: &[f64], trq_k_nm: &[f64]) -> f64 {
    let trq = interp_with_zero_outside(rpm, n, trq_k_nm);
    trq * rpm * PI / 30.0
}

fn derating_factor(temp_c: f64, t_soft: f64, beta_derate: f64) -> f64 {
    if temp_c <= t_soft {
        1.0
    } else {
        (1.0 - (temp_c - t_soft) * beta_derate).max(0.2)
    }
}

#[derive(Debug, Clone)]
pub struct V81960Engine {
    pub n: Vec<f64>,
    pub trq: Vec<f64>, // kN.m
    pub n_idle: f64,
    pub n_max: f64,
    pub n_upshift: f64,
    pub n_downshift: f64,
    pub g1_total: f64,
    pub g_last_total: f64,
    pub gear_count: u8,
    pub gear_ratios: Vec<f64>,
    pub t_amb: f64,
    pub t_init: f64,
    pub t_soft: f64,
    pub c_th: f64,
    pub alpha_heat: f64,
    pub p_cool0: f64,
    pub k_cool: f64,
    pub beta_derate: f64,
}

impl Default for V81960Engine {
    fn default() -> Self {
        let n = rpm_axis(11000.0);
        let trq = [linspace(0.11, 0.15, 40), linspace(0.15, 0.05, 5)].concat();
        let g1_total = 13.0;
        let g_last_total = 5.5;
        let gear_count = 5;
        Self {
            n,
            trq,
            n_idle: 1500.0,
            n_max: 11000.0,
            n_upshift: 11000.0,
            n_downshift: 1500.0,
            g1_total,
            g_last_total,
            gear_count,
            gear_ratios: generate_gear_ratios_total(g1_total, g_last_total, gear_count),
            t_amb: 35.0,
            t_init: 90.0,
            t_soft: 110.0,
            c_th: 100000.0,
            alpha_heat: 0.45,
            p_cool0: 0.0,
            k_cool: 45.0,
            beta_derate: 0.004,
        }
    }
}

impl Engine for V81960Engine {
    fn torque(&self, rpm: f64) -> f64 {
        interp_with_zero_outside(rpm, &self.n, &self.trq) * 1000.0
    }

    fn power_kw_from_rpm(&self, rpm: f64) -> f64 {
        power_kw_from_curve(rpm, &self.n, &self.trq)
    }

    fn derating_factor(&self, temp_c: f64) -> f64 {
        derating_factor(temp_c, self.t_soft, self.beta_derate)
    }

    fn ambient_temp_c(&self) -> f64 {
        self.t_amb
    }

    fn thermal_init_c(&self) -> f64 {
        self.t_init
    }

    fn thermal_capacity_j_per_c(&self) -> f64 {
        self.c_th
    }

    fn heat_alpha(&self) -> f64 {
        self.alpha_heat
    }

    fn cooling_base_w(&self) -> f64 {
        self.p_cool0
    }

    fn cooling_speed_w_per_ms(&self) -> f64 {
        self.k_cool
    }

    fn max_rpm(&self) -> f64 {
        self.n_max
    }

    fn idle_rpm(&self) -> f64 {
        self.n_idle
    }

    fn gear_ratio(&self, gear: u8) -> f64 {
        if gear == 0 || gear as usize > self.gear_ratios.len() {
            0.0
        } else {
            self.gear_ratios[gear as usize - 1]
        }
    }

    fn gear_count(&self) -> u8 {
        self.gear_count
    }

    fn upshift_rpm(&self) -> f64 {
        self.n_upshift
    }

    fn downshift_rpm(&self) -> f64 {
        self.n_downshift
    }

    fn apply_tuning(&mut self, tuning: &Tuning) {
        let n_blend = 0.9 + 0.10 * tuning.engine_points / 5.0;
        let cool_blend = 0.9 + 0.10 * tuning.cooling_points / 10.0;

        self.n.iter_mut().for_each(|v| *v *= n_blend);
        // Python behavior is cumulative (no reset to baseline).
        self.k_cool *= cool_blend;
    }
}

#[derive(Debug, Clone)]
pub struct V81970Engine {
    pub n: Vec<f64>,
    pub trq: Vec<f64>, // kN.m
    pub n_idle: f64,
    pub n_max: f64,
    pub n_upshift: f64,
    pub n_downshift: f64,
    pub g1_total: f64,
    pub g_last_total: f64,
    pub gear_count: u8,
    pub gear_ratios: Vec<f64>,
    pub t_amb: f64,
    pub t_init: f64,
    pub t_soft: f64,
    pub c_th: f64,
    pub alpha_heat: f64,
    pub p_cool0: f64,
    pub k_cool: f64,
    pub beta_derate: f64,
}

impl Default for V81970Engine {
    fn default() -> Self {
        let n = rpm_axis(13000.0);
        let trq = [linspace(0.15, 0.26, 47), vec![0.26], linspace(0.26, 0.07, 5)].concat();
        let g1_total = 13.0;
        let g_last_total = 4.5;
        let gear_count = 5;
        Self {
            n,
            trq,
            n_idle: 1500.0,
            n_max: 13000.0,
            n_upshift: 13000.0,
            n_downshift: 1500.0,
            g1_total,
            g_last_total,
            gear_count,
            gear_ratios: generate_gear_ratios_total(g1_total, g_last_total, gear_count),
            t_amb: 35.0,
            t_init: 90.0,
            t_soft: 110.0,
            c_th: 100000.0,
            alpha_heat: 0.45,
            p_cool0: 0.0,
            k_cool: 45.0,
            beta_derate: 0.004,
        }
    }
}

impl Engine for V81970Engine {
    fn torque(&self, rpm: f64) -> f64 {
        interp_with_zero_outside(rpm, &self.n, &self.trq) * 1000.0
    }

    fn power_kw_from_rpm(&self, rpm: f64) -> f64 {
        power_kw_from_curve(rpm, &self.n, &self.trq)
    }

    fn derating_factor(&self, temp_c: f64) -> f64 {
        derating_factor(temp_c, self.t_soft, self.beta_derate)
    }

    fn ambient_temp_c(&self) -> f64 {
        self.t_amb
    }

    fn thermal_init_c(&self) -> f64 {
        self.t_init
    }

    fn thermal_capacity_j_per_c(&self) -> f64 {
        self.c_th
    }

    fn heat_alpha(&self) -> f64 {
        self.alpha_heat
    }

    fn cooling_base_w(&self) -> f64 {
        self.p_cool0
    }

    fn cooling_speed_w_per_ms(&self) -> f64 {
        self.k_cool
    }

    fn max_rpm(&self) -> f64 {
        self.n_max
    }

    fn idle_rpm(&self) -> f64 {
        self.n_idle
    }

    fn gear_ratio(&self, gear: u8) -> f64 {
        if gear == 0 || gear as usize > self.gear_ratios.len() {
            0.0
        } else {
            self.gear_ratios[gear as usize - 1]
        }
    }

    fn gear_count(&self) -> u8 {
        self.gear_count
    }

    fn upshift_rpm(&self) -> f64 {
        self.n_upshift
    }

    fn downshift_rpm(&self) -> f64 {
        self.n_downshift
    }

    fn apply_tuning(&mut self, tuning: &Tuning) {
        let n_blend = 0.9 + 0.10 * tuning.engine_points / 10.0;
        let trq_blend = 1.0 + 0.20 * (tuning.engine_points / 20.0);
        let cool_blend = 1.0 + 0.35 * tuning.cooling_points / 20.0;
        let gear_blend = 1.10 - 0.20 * tuning.gear_ratio_slider;

        self.n.iter_mut().for_each(|v| *v *= n_blend);
        self.trq.iter_mut().for_each(|v| *v *= trq_blend);
        // Python behavior is cumulative (no reset to baseline).
        self.k_cool *= cool_blend;
        self.g1_total *= gear_blend;
        self.g_last_total *= gear_blend;
        self.gear_ratios.iter_mut().for_each(|v| *v *= gear_blend);
    }
}

#[derive(Debug, Clone)]
pub struct V6TEngine {
    pub n: Vec<f64>,
    pub trq: Vec<f64>, // kN.m
    pub n_idle: f64,
    pub n_max: f64,
    pub n_upshift: f64,
    pub n_downshift: f64,
    pub g1_total: f64,
    pub g_last_total: f64,
    pub gear_count: u8,
    pub gear_ratios: Vec<f64>,
    pub t_amb: f64,
    pub t_init: f64,
    pub t_soft: f64,
    pub c_th: f64,
    pub alpha_heat: f64,
    pub p_cool0: f64,
    pub k_cool: f64,
    pub beta_derate: f64,
}

impl Default for V6TEngine {
    fn default() -> Self {
        let n = rpm_axis(12000.0);
        let trq = [
            linspace(0.365, 0.56, 41),
            linspace(0.56, 0.51, 4),
            linspace(0.5, 0.1, 4),
        ]
        .concat();
        let g1_total = 14.0;
        let g_last_total = 4.5;
        let gear_count = 5;
        Self {
            n,
            trq,
            n_idle: 1700.0,
            n_max: 12000.0,
            n_upshift: 12000.0,
            n_downshift: 1700.0,
            g1_total,
            g_last_total,
            gear_count,
            gear_ratios: generate_gear_ratios_total(g1_total, g_last_total, gear_count),
            t_amb: 35.0,
            t_init: 90.0,
            t_soft: 110.0,
            c_th: 100000.0,
            alpha_heat: 0.45,
            p_cool0: 0.0,
            k_cool: 45.0,
            beta_derate: 0.004,
        }
    }
}

impl Engine for V6TEngine {
    fn torque(&self, rpm: f64) -> f64 {
        interp_with_zero_outside(rpm, &self.n, &self.trq) * 1000.0
    }

    fn power_kw_from_rpm(&self, rpm: f64) -> f64 {
        power_kw_from_curve(rpm, &self.n, &self.trq)
    }

    fn derating_factor(&self, temp_c: f64) -> f64 {
        derating_factor(temp_c, self.t_soft, self.beta_derate)
    }

    fn ambient_temp_c(&self) -> f64 {
        self.t_amb
    }

    fn thermal_init_c(&self) -> f64 {
        self.t_init
    }

    fn thermal_capacity_j_per_c(&self) -> f64 {
        self.c_th
    }

    fn heat_alpha(&self) -> f64 {
        self.alpha_heat
    }

    fn cooling_base_w(&self) -> f64 {
        self.p_cool0
    }

    fn cooling_speed_w_per_ms(&self) -> f64 {
        self.k_cool
    }

    fn max_rpm(&self) -> f64 {
        self.n_max
    }

    fn idle_rpm(&self) -> f64 {
        self.n_idle
    }

    fn gear_ratio(&self, gear: u8) -> f64 {
        if gear == 0 || gear as usize > self.gear_ratios.len() {
            0.0
        } else {
            self.gear_ratios[gear as usize - 1]
        }
    }

    fn gear_count(&self) -> u8 {
        self.gear_count
    }

    fn upshift_rpm(&self) -> f64 {
        self.n_upshift
    }

    fn downshift_rpm(&self) -> f64 {
        self.n_downshift
    }

    fn apply_tuning(&mut self, tuning: &Tuning) {
        let trq_blend = 0.95 + 0.05 * tuning.engine_points / 20.0;
        let n_blend = 0.95 + 0.05 * tuning.engine_points / 15.0;
        let cool_blend = 0.9 + 0.10 * tuning.cooling_points / 10.0;

        self.trq.iter_mut().for_each(|v| *v *= trq_blend);
        self.n.iter_mut().for_each(|v| *v *= n_blend);
        // Python behavior is cumulative (no reset to baseline).
        self.n_max *= n_blend;
        self.k_cool *= cool_blend;
    }
}

#[derive(Debug, Clone)]
pub struct V6THybridEngine {
    pub n: Vec<f64>,
    pub trq: Vec<f64>, // kN.m
    pub n_idle: f64,
    pub n_max: f64,
    pub n_upshift: f64,
    pub n_downshift: f64,
    pub g1_total: f64,
    pub g_last_total: f64,
    pub gear_count: u8,
    pub gear_ratios: Vec<f64>,
    pub t_amb: f64,
    pub t_init: f64,
    pub t_soft: f64,
    pub c_th: f64,
    pub alpha_heat: f64,
    pub p_cool0: f64,
    pub k_cool: f64,
    pub beta_derate: f64,
}

impl Default for V6THybridEngine {
    fn default() -> Self {
        let n = rpm_axis(15000.0);
        let trq = [
            linspace(0.44, 0.59, 43),
            linspace(0.57, 0.455, 8),
            linspace(0.44, 0.32, 9),
            vec![0.16],
        ]
        .concat();
        let g1_total = 14.0;
        let g_last_total = 4.5;
        let gear_count = 8;
        Self {
            n,
            trq,
            n_idle: 400.0,
            n_max: 15000.0,
            // Not explicitly set in Python V6T_Hybrid: keep neutral defaults.
            n_upshift: 15000.0,
            n_downshift: 400.0,
            g1_total,
            g_last_total,
            gear_count,
            gear_ratios: generate_gear_ratios_total(g1_total, g_last_total, gear_count),
            t_amb: 35.0,
            t_init: 90.0,
            t_soft: 110.0,
            c_th: 100000.0,
            alpha_heat: 0.45,
            p_cool0: 0.0,
            k_cool: 45.0,
            beta_derate: 0.004,
        }
    }
}

impl Engine for V6THybridEngine {
    fn torque(&self, rpm: f64) -> f64 {
        interp_with_zero_outside(rpm, &self.n, &self.trq) * 1000.0
    }

    fn power_kw_from_rpm(&self, rpm: f64) -> f64 {
        power_kw_from_curve(rpm, &self.n, &self.trq)
    }

    fn derating_factor(&self, temp_c: f64) -> f64 {
        derating_factor(temp_c, self.t_soft, self.beta_derate)
    }

    fn ambient_temp_c(&self) -> f64 {
        self.t_amb
    }

    fn thermal_init_c(&self) -> f64 {
        self.t_init
    }

    fn thermal_capacity_j_per_c(&self) -> f64 {
        self.c_th
    }

    fn heat_alpha(&self) -> f64 {
        self.alpha_heat
    }

    fn cooling_base_w(&self) -> f64 {
        self.p_cool0
    }

    fn cooling_speed_w_per_ms(&self) -> f64 {
        self.k_cool
    }

    fn max_rpm(&self) -> f64 {
        self.n_max
    }

    fn idle_rpm(&self) -> f64 {
        self.n_idle
    }

    fn gear_ratio(&self, gear: u8) -> f64 {
        if gear == 0 || gear as usize > self.gear_ratios.len() {
            0.0
        } else {
            self.gear_ratios[gear as usize - 1]
        }
    }

    fn gear_count(&self) -> u8 {
        self.gear_count
    }

    fn upshift_rpm(&self) -> f64 {
        self.n_upshift
    }

    fn downshift_rpm(&self) -> f64 {
        self.n_downshift
    }

    fn apply_tuning(&mut self, tuning: &Tuning) {
        let trq_blend = 0.9 + 0.10 * tuning.engine_points / 20.0;
        let cool_blend = 0.9 + 0.10 * tuning.cooling_points / 10.0;
        let gear_blend = 1.15 - 0.15 * tuning.gear_ratio_slider;

        self.trq.iter_mut().for_each(|v| *v *= trq_blend);
        // Python behavior is cumulative (no reset to baseline).
        self.p_cool0 *= cool_blend;
        self.g1_total *= gear_blend;
        self.g_last_total *= gear_blend;
        self.gear_ratios.iter_mut().for_each(|v| *v *= gear_blend);
    }
}
