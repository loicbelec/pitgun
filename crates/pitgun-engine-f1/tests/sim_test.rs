use pitgun_engine_f1::components::aero::{ActiveAero, Aero};
use pitgun_engine_f1::components::chassis::StandardChassis;
use pitgun_engine_f1::components::engine::V6THybridEngine;
use pitgun_engine_f1::core::Tuning;
use pitgun_engine_f1::sim::{run_simulation_with_tuning, SimConfig, TrackProfile};
use pitgun_engine_f1::vehicle::Vehicle;

fn make_track() -> TrackProfile {
    let n = 500usize;
    let ds = 1.0f64;
    let mut s = Vec::with_capacity(n);
    let mut x = Vec::with_capacity(n);
    let mut y = Vec::with_capacity(n);
    let mut z = Vec::with_capacity(n);
    let mut kappa = Vec::with_capacity(n);
    let mut slope = Vec::with_capacity(n);
    let mut heading = Vec::with_capacity(n);

    for i in 0..n {
        let si = i as f64 * ds;
        s.push(si);
        x.push(si);
        y.push(0.0);
        z.push(0.0);
        let k = if (150..350).contains(&i) { 0.01 } else { 0.0 };
        kappa.push(k);
        slope.push(0.0);
        heading.push(0.0);
    }

    TrackProfile {
        s,
        x,
        y,
        z,
        kappa,
        slope,
        heading,
    }
}

#[test]
fn test_simulation_outputs_are_finite() {
    let track = make_track();
    let mut vehicle = Vehicle::new(Aero::new(), StandardChassis::new(), V6THybridEngine::default());
    let tuning = Tuning {
        engine_points: 10.0,
        cooling_points: 10.0,
        aero_points: 10.0,
        chassis_points: 10.0,
        downforce_slider: 0.5,
        gear_ratio_slider: 0.5,
    };
    let config = SimConfig::default();

    let out = run_simulation_with_tuning(&track, &mut vehicle, &tuning, &config).unwrap();
    assert_eq!(out.solution.s.len(), track.s.len());
    assert!(!out.telemetry.time_s.is_empty());

    assert!(out.solution.v.iter().all(|v| v.is_finite() && *v >= 0.0));
    assert!(out.solution.temp_c.iter().all(|t| t.is_finite()));
    assert!(out.telemetry.speed_kph.iter().all(|v| v.is_finite() && *v >= 0.0));
    assert!(out.telemetry.g_lat.iter().all(|v| v.is_finite()));
}

#[test]
fn test_downforce_increases_corner_envelope_with_active_aero() {
    let track = make_track();
    let config = SimConfig {
        lap_number: 2,
        hz: 60.0,
    };

    let tuning_low_df = Tuning {
        engine_points: 10.0,
        cooling_points: 10.0,
        aero_points: 10.0,
        chassis_points: 10.0,
        downforce_slider: 0.0,
        gear_ratio_slider: 0.5,
    };
    let tuning_high_df = Tuning {
        downforce_slider: 1.0,
        ..tuning_low_df.clone()
    };

    let mut car_low =
        Vehicle::new(ActiveAero::new(), StandardChassis::new(), V6THybridEngine::default());
    let mut car_high =
        Vehicle::new(ActiveAero::new(), StandardChassis::new(), V6THybridEngine::default());

    let out_low = run_simulation_with_tuning(&track, &mut car_low, &tuning_low_df, &config).unwrap();
    let out_high =
        run_simulation_with_tuning(&track, &mut car_high, &tuning_high_df, &config).unwrap();

    let mut corner_low = 0.0;
    let mut corner_high = 0.0;
    let mut count = 0usize;
    for i in 0..track.kappa.len() {
        if track.kappa[i].abs() > 0.0 {
            corner_low += out_low.solution.v_corner[i];
            corner_high += out_high.solution.v_corner[i];
            count += 1;
        }
    }

    let avg_low = corner_low / count as f64;
    let avg_high = corner_high / count as f64;
    assert!(
        avg_high > avg_low,
        "expected higher corner envelope with high downforce, got low={avg_low} high={avg_high}"
    );
}
