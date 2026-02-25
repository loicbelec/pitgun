use crate::models::{
    AeroConfig, ChassisConfig, EngineConfig, EngineThermalConfig, TireConfig, TrackConfig,
    VehicleConfig,
};
use crate::profiles::{CompetitorProfile, DrivingStyle, EngineMode};
use crate::provider::InMemoryConfigProvider;

pub fn default_in_memory_provider() -> InMemoryConfigProvider {
    let mut provider = InMemoryConfigProvider::new();

    for aero in default_aero() {
        provider.insert_aero(aero);
    }
    for chassis in default_chassis() {
        provider.insert_chassis(chassis);
    }
    for tire in default_tires() {
        provider.insert_tire(tire);
    }
    for engine in default_engines() {
        provider.insert_engine(engine);
    }
    for vehicle in default_vehicles() {
        provider.insert_vehicle(vehicle);
    }
    for profile in default_profiles() {
        provider.insert_profile(profile);
    }

    for id in ["SPA", "MONZA", "SUZUKA", "MONACO", "DEFAULT"] {
        provider.insert_track(build_track(id));
    }

    provider
}

fn default_aero() -> Vec<AeroConfig> {
    vec![
        AeroConfig {
            id: "none".to_string(),
            cd_a_straight: 0.20,
            cd_a_corner: 0.25,
            cl_a_straight: 0.1,
            cl_a_corner: 0.2,
        },
        AeroConfig {
            id: "basic".to_string(),
            cd_a_straight: 0.60,
            cd_a_corner: 0.75,
            cl_a_straight: 1.8,
            cl_a_corner: 2.7,
        },
        AeroConfig {
            id: "active".to_string(),
            cd_a_straight: 0.80,
            cd_a_corner: 1.00,
            cl_a_straight: 2.6,
            cl_a_corner: 4.13,
        },
    ]
}

fn default_chassis() -> Vec<ChassisConfig> {
    vec![
        ChassisConfig {
            id: "default".to_string(),
            mass_empty_kg: 805.0,
            wheel_radius_m: 0.34,
            mu0: 1.5,
            rolling_resistance: 0.020,
            air_density: 1.225,
            gravity: 9.81,
        },
        ChassisConfig {
            id: "f1_2026".to_string(),
            mass_empty_kg: 768.0,
            wheel_radius_m: 0.36,
            mu0: 1.8,
            rolling_resistance: 0.015,
            air_density: 1.225,
            gravity: 9.81,
        },
    ]
}

fn default_tires() -> Vec<TireConfig> {
    vec![
        TireConfig {
            id: "soft".to_string(),
            mu_scale: 1.02,
            wear_per_s: 0.000002,
            wear_load_k: 0.0000002,
            wear_grip_k: 0.60,
            wear_min: 0.45,
            temp_opt_c: 100.0,
            temp_sigma_c: 42.5,
            temp_min_k: 0.825,
            heat_k: 0.0061,
            cool_k: 0.00089,
        },
        TireConfig {
            id: "medium".to_string(),
            mu_scale: 1.0,
            wear_per_s: 0.0000015,
            wear_load_k: 0.00000015,
            wear_grip_k: 0.5,
            wear_min: 0.50,
            temp_opt_c: 98.0,
            temp_sigma_c: 45.0,
            temp_min_k: 0.84,
            heat_k: 0.0058,
            cool_k: 0.00090,
        },
        TireConfig {
            id: "hard".to_string(),
            mu_scale: 0.96,
            wear_per_s: 0.000001,
            wear_load_k: 0.0000001,
            wear_grip_k: 0.4,
            wear_min: 0.60,
            temp_opt_c: 95.0,
            temp_sigma_c: 50.0,
            temp_min_k: 0.85,
            heat_k: 0.0052,
            cool_k: 0.00095,
        },
    ]
}

fn default_engines() -> Vec<EngineConfig> {
    vec![
        build_engine("v8_1960", 11000.0, 6.0, 0.46, 5, 0.018),
        build_engine("v8_1970", 12000.0, 7.0, 0.52, 6, 0.019),
        build_engine("v6t", 14500.0, 4.8, 0.56, 8, 0.021),
        build_engine("v6t_hybrid", 15000.0, 4.5, 0.59, 8, 0.022),
    ]
}

fn build_engine(
    id: &str,
    max_rpm: f64,
    g_last_total: f64,
    tq_peak: f64,
    gear_count: usize,
    fuel_burn_kg_per_s: f64,
) -> EngineConfig {
    let mut rpm_samples = Vec::new();
    let mut torque_samples = Vec::new();

    let step = 250.0;
    let mut rpm = 0.0;
    while rpm <= max_rpm + step * 0.5 {
        rpm_samples.push(rpm);
        let normalized = if max_rpm > 0.0 { rpm / max_rpm } else { 0.0 };
        let tq = if normalized < 0.7 {
            tq_peak * (0.70 + 0.40 * normalized)
        } else {
            tq_peak * (1.0 - 0.65 * (normalized - 0.7))
        }
        .max(0.12);
        torque_samples.push(tq);
        rpm += step;
    }

    let g1_total = 14.0;
    let mut gear_ratios = Vec::with_capacity(gear_count);
    for idx in 0..gear_count {
        let a = idx as f64 / (gear_count - 1) as f64;
        gear_ratios.push(g1_total * (g_last_total / g1_total).powf(a));
    }

    EngineConfig {
        id: id.to_string(),
        rpm_samples,
        torque_samples,
        gear_ratios,
        idle_rpm: 400.0,
        max_rpm,
        thermal: EngineThermalConfig {
            ambient_temp_c: 35.0,
            initial_temp_c: 90.0,
            capacity_j_per_c: 100000.0,
            heat_alpha: 0.45,
            cooling_base_w: 0.0,
            cooling_speed_w_per_ms: 45.0,
            soft_temp_c: 110.0,
            derate_per_c: 0.02,
        },
        fuel_burn_kg_per_s,
    }
}

fn default_vehicles() -> Vec<VehicleConfig> {
    vec![
        VehicleConfig {
            id: "classic_v8_1960".to_string(),
            engine_id: "v8_1960".to_string(),
            aero_id: "none".to_string(),
            chassis_id: "default".to_string(),
            tire_id: "medium".to_string(),
        },
        VehicleConfig {
            id: "classic_v8_1970".to_string(),
            engine_id: "v8_1970".to_string(),
            aero_id: "basic".to_string(),
            chassis_id: "default".to_string(),
            tire_id: "medium".to_string(),
        },
        VehicleConfig {
            id: "modern_v6t".to_string(),
            engine_id: "v6t".to_string(),
            aero_id: "basic".to_string(),
            chassis_id: "f1_2026".to_string(),
            tire_id: "medium".to_string(),
        },
        VehicleConfig {
            id: "f1_2026".to_string(),
            engine_id: "v6t_hybrid".to_string(),
            aero_id: "active".to_string(),
            chassis_id: "f1_2026".to_string(),
            tire_id: "medium".to_string(),
        },
    ]
}

fn default_profiles() -> Vec<CompetitorProfile> {
    vec![
        CompetitorProfile {
            id: "conservative".to_string(),
            display_name: "Conservative".to_string(),
            style: DrivingStyle::Conservative,
            engine_mode: EngineMode::Economy,
            tire_id: "hard".to_string(),
            downforce_bias: 0.10,
            gear_ratio_bias: -0.05,
            pace_variance_ms: 20.0,
        },
        CompetitorProfile {
            id: "balanced".to_string(),
            display_name: "Balanced".to_string(),
            style: DrivingStyle::Balanced,
            engine_mode: EngineMode::Balanced,
            tire_id: "medium".to_string(),
            downforce_bias: 0.0,
            gear_ratio_bias: 0.0,
            pace_variance_ms: 35.0,
        },
        CompetitorProfile {
            id: "aggressive".to_string(),
            display_name: "Aggressive".to_string(),
            style: DrivingStyle::Aggressive,
            engine_mode: EngineMode::Push,
            tire_id: "soft".to_string(),
            downforce_bias: -0.03,
            gear_ratio_bias: 0.07,
            pace_variance_ms: 70.0,
        },
    ]
}

#[derive(Clone, Copy)]
struct TrackTemplate {
    distance_m: f64,
    radius_x: f64,
    radius_y: f64,
    wobble_x: f64,
    wobble_y: f64,
    slope_amp_m: f64,
}

fn build_track(track_id: &str) -> TrackConfig {
    let tpl = match normalize_track_id(track_id).as_str() {
        "SPA" => TrackTemplate {
            distance_m: 7004.0,
            radius_x: 930.0,
            radius_y: 600.0,
            wobble_x: 260.0,
            wobble_y: 180.0,
            slope_amp_m: 9.0,
        },
        "MONZA" => TrackTemplate {
            distance_m: 5793.0,
            radius_x: 980.0,
            radius_y: 430.0,
            wobble_x: 120.0,
            wobble_y: 95.0,
            slope_amp_m: 2.0,
        },
        "SUZUKA" => TrackTemplate {
            distance_m: 5807.0,
            radius_x: 760.0,
            radius_y: 620.0,
            wobble_x: 290.0,
            wobble_y: 250.0,
            slope_amp_m: 5.0,
        },
        "MONACO" => TrackTemplate {
            distance_m: 3337.0,
            radius_x: 500.0,
            radius_y: 390.0,
            wobble_x: 170.0,
            wobble_y: 120.0,
            slope_amp_m: 6.0,
        },
        _ => TrackTemplate {
            distance_m: 5200.0,
            radius_x: 760.0,
            radius_y: 520.0,
            wobble_x: 180.0,
            wobble_y: 130.0,
            slope_amp_m: 4.0,
        },
    };

    let points = 420usize;
    let mut s = Vec::with_capacity(points);
    let mut x = Vec::with_capacity(points);
    let mut y = Vec::with_capacity(points);
    let mut z = Vec::with_capacity(points);

    for i in 0..points {
        let t = i as f64 / (points - 1) as f64;
        let theta = t * std::f64::consts::TAU;
        s.push(t * tpl.distance_m);
        x.push(
            tpl.radius_x * theta.cos()
                + tpl.wobble_x * (2.6 * theta).cos() * 0.55
                + tpl.wobble_x * (4.2 * theta).sin() * 0.15,
        );
        y.push(
            tpl.radius_y * theta.sin()
                + tpl.wobble_y * (1.8 * theta).sin() * 0.60
                + tpl.wobble_y * (3.3 * theta).cos() * 0.20,
        );
        z.push(
            tpl.slope_amp_m * (1.7 * theta).sin() * 0.5
                + tpl.slope_amp_m * (0.4 * theta).cos() * 0.2,
        );
    }

    let mut heading = vec![0.0; points];
    for i in 0..points {
        let i0 = i.saturating_sub(1);
        let i1 = (i + 1).min(points - 1);
        let dx = x[i1] - x[i0];
        let dy = y[i1] - y[i0];
        heading[i] = dy.atan2(dx);
    }
    for i in 1..points {
        heading[i] = unwrap_angle(heading[i], heading[i - 1]);
    }

    let mut curvature = vec![0.0; points];
    let mut slope = vec![0.0; points];
    for i in 0..points {
        let i0 = i.saturating_sub(1);
        let i1 = (i + 1).min(points - 1);
        let ds = (s[i1] - s[i0]).max(1e-6);
        curvature[i] = (heading[i1] - heading[i0]) / ds;
        slope[i] = (z[i1] - z[i0]) / ds;
    }

    TrackConfig {
        id: normalize_track_id(track_id),
        s_m: s,
        x_m: x,
        y_m: y,
        z_m: z,
        curvature_radpm: curvature,
        slope,
        heading_rad: heading,
    }
}

fn unwrap_angle(mut value: f64, reference: f64) -> f64 {
    while value - reference > std::f64::consts::PI {
        value -= std::f64::consts::TAU;
    }
    while value - reference < -std::f64::consts::PI {
        value += std::f64::consts::TAU;
    }
    value
}

fn normalize_track_id(track_id: &str) -> String {
    track_id
        .chars()
        .filter(|ch| !matches!(ch, '-' | '_' | ' '))
        .flat_map(char::to_uppercase)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::ConfigProvider;

    #[test]
    fn defaults_are_resolvable() {
        let provider = default_in_memory_provider();
        let track = provider.get_track("SPA").expect("spa track");
        let vehicle = provider.get_vehicle("f1_2026").expect("f1 vehicle");
        assert!(track.s_m.len() > 100);
        assert_eq!(vehicle.engine_id, "v6t_hybrid");
    }
}
