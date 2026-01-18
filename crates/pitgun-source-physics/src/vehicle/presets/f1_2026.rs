use crate::vehicle::VehicleSpec;

pub const VEHICLE_ID: &str = "f1_2026";

pub fn build() -> VehicleSpec {
    let rpm_curve = rpm_curve();
    let torque_curve = torque_curve();

    let mut spec = VehicleSpec {
        mass_kg: 768.0,
        rho: 1.225,
        g: 9.81,
        wheel_radius_m: 0.36,
        mu: 1.7,
        c_rr: 0.015,
        cd_a_x: 0.85,
        cd_a_z: 1.50,
        cl_a_x: 2.6,
        cl_a_z: 4.13,
        rpm_curve,
        torque_curve,
        n_idle: 4000.0,
        n_max: 15000.0,
        g1_total: 14.0,
        g_last_total: 4.7,
        gear_count: 8,
        n_upshift: 12300.0,
        n_downshift: 5500.0,
        gear_ratios: Vec::new(),
        t_amb: 35.0,
        t_init: 90.0,
        t_soft: 110.0,
        c_th: 500000.0,
        alpha_heat: 0.45,
        p_cool0: 15000.0,
        k_cool: 1100.0,
        beta_derate: 0.004,
    };

    spec.update_gear_ratios();
    spec
}

fn rpm_curve() -> Vec<f32> {
    (0..=15000).step_by(250).map(|value| value as f32).collect()
}

fn torque_curve() -> Vec<f32> {
    let mut curve = Vec::new();
    curve.extend(linspace(0.44, 0.59, 43));
    curve.extend(linspace(0.57, 0.455, 8));
    curve.extend(linspace(0.44, 0.32, 9));
    curve.push(0.16);
    curve
}

fn linspace(start: f32, end: f32, count: usize) -> Vec<f32> {
    if count == 0 {
        return Vec::new();
    }
    if count == 1 {
        return vec![start];
    }
    let step = (end - start) / (count as f32 - 1.0);
    (0..count).map(|i| start + step * i as f32).collect()
}
