use pitgun_engine_f1::components::aero::{ActiveAero, Aero, NoAero};
use pitgun_engine_f1::components::chassis::StandardChassis;
use pitgun_engine_f1::components::engine::{V6TEngine, V6THybridEngine, V81960Engine, V81970Engine};
use pitgun_engine_f1::core::{Aero as AeroTrait, Engine, Tuning};
use pitgun_engine_f1::vehicle::Vehicle;

fn assert_close(lhs: f64, rhs: f64, eps: f64) {
    let diff = (lhs - rhs).abs();
    assert!(
        diff <= eps,
        "values differ: left={lhs}, right={rhs}, abs_diff={diff}, eps={eps}"
    );
}

#[test]
fn test_components_exist_and_construct() {
    let _ = NoAero::default();
    let _ = Aero::default();
    let _ = ActiveAero::default();
    let _ = StandardChassis::default();
    let _ = V81960Engine::default();
    let _ = V81970Engine::default();
    let _ = V6TEngine::default();
    let _ = V6THybridEngine::default();
}

#[test]
fn test_vehicle_python_methods() {
    let aero = Aero::new();
    let chassis = StandardChassis::new();
    let engine = V6THybridEngine::default();
    let car = Vehicle::new(aero, chassis, engine);

    let speed = 50.0;
    let rpm = car.rpm_from_speed_gear(speed, 4);
    assert!(rpm > 0.0);

    let p_kw = car.power_kw_from_rpm(rpm);
    assert!(p_kw > 0.0);

    let derate_hot = car.derating_factor(160.0);
    assert!(derate_hot < 1.0);

    let (p_max, rpm_at_pmax, best_gear) = car.max_engine_power(speed, 90.0);
    assert!(p_max > 0.0);
    assert!(rpm_at_pmax > 0.0);
    assert!((1..=8).contains(&best_gear));

    assert!(car.is_powerful_enough(p_max * 0.5, rpm_at_pmax, 90.0));
}

#[test]
fn test_power_interp_is_zero_outside_range() {
    let engine = V6THybridEngine::default();

    assert_close(engine.power_kw_from_rpm(-1.0), 0.0, 1e-12);
    assert_close(engine.power_kw_from_rpm(20000.0), 0.0, 1e-12);
    assert!(engine.power_kw_from_rpm(8000.0) > 0.0);
}

#[test]
fn test_tuning_is_cumulative_like_python() {
    let aero = Aero::new();
    let chassis = StandardChassis::new();
    let engine = V6THybridEngine::default();
    let mut car = Vehicle::new(aero, chassis, engine);

    let tuning = Tuning {
        engine_points: 10.0,
        cooling_points: 10.0,
        aero_points: 10.0,
        chassis_points: 10.0,
        downforce_slider: 0.5,
        gear_ratio_slider: 0.5,
    };

    let grip_blend = 1.0 + 0.08 * (tuning.chassis_points / 20.0);
    let gear_blend = 1.15 - 0.15 * tuning.gear_ratio_slider;

    let mu0 = car.chassis.mu;
    let gr0 = car.engine.gear_ratio(1);

    car.apply_tuning(&tuning);
    let mu1 = car.chassis.mu;
    let gr1 = car.engine.gear_ratio(1);

    car.apply_tuning(&tuning);
    let mu2 = car.chassis.mu;
    let gr2 = car.engine.gear_ratio(1);

    assert_close(mu1, mu0 * grip_blend, 1e-12);
    assert_close(mu2, mu1 * grip_blend, 1e-12);
    assert_close(gr1, gr0 * gear_blend, 1e-12);
    assert_close(gr2, gr1 * gear_blend, 1e-12);
}

#[test]
fn test_aero_mode_specific_coeffs() {
    let mut aero = Aero::new();
    let tuning = Tuning {
        engine_points: 0.0,
        cooling_points: 0.0,
        aero_points: 5.0,
        chassis_points: 0.0,
        downforce_slider: 0.7,
        gear_ratio_slider: 0.0,
    };
    aero.apply_tuning(&tuning);

    let (cdx, clx) = aero.coeffs_straight();
    let (cdz, clz) = aero.coeffs_corner();
    let (cd_default, cl_default) = aero.coeffs();

    assert!(cdx > 0.0 && clx > 0.0);
    assert!(cdz > 0.0 && clz > 0.0);
    // Backward-compatible default returns corner mode.
    assert_close(cd_default, cdz, 1e-12);
    assert_close(cl_default, clz, 1e-12);
}

#[test]
fn test_active_aero_2026_behavior() {
    let mut active = ActiveAero::new();
    let mut classic = Aero::new();

    let tuning = Tuning {
        engine_points: 0.0,
        cooling_points: 0.0,
        aero_points: 10.0,
        chassis_points: 0.0,
        downforce_slider: 1.0,
        gear_ratio_slider: 0.0,
    };

    active.apply_tuning(&tuning);
    classic.apply_tuning(&tuning);

    let (active_cdx, _) = active.coeffs_straight();
    let (classic_cdx, _) = classic.coeffs_straight();
    // Active aero should penalize drag less than the classic aero law.
    assert!(active_cdx < classic_cdx);
}
