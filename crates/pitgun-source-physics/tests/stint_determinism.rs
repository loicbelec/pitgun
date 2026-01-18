use pitgun_source_physics::game::{load_track, DEFAULT_TRACK_ID};
use pitgun_source_physics::{simulate_stint, PlayerTuning};

#[test]
fn stint_is_deterministic_and_temp_changes() {
    let track = load_track(DEFAULT_TRACK_ID).expect("track should load");
    let tuning = PlayerTuning {
        aero_points: 10,
        chassis_points: 10,
        engine_points: 10,
        cooling_points: 10,
        downforce_slider: 0.5,
        gear_ratio_slider: 0.5,
    };

    let first = simulate_stint(&track, tuning, 10.0, 3).expect("simulate stint");
    let second = simulate_stint(&track, tuning, 10.0, 3).expect("simulate stint");

    assert_eq!(first.len(), second.len());

    let first_temp = first.first().map(|p| p.engine_temp_c).unwrap_or(0.0);
    let last_temp = first.last().map(|p| p.engine_temp_c).unwrap_or(0.0);
    assert!((last_temp - first_temp).abs() > 0.01);

    let last_temp_b = second.last().map(|p| p.engine_temp_c).unwrap_or(0.0);
    assert!((last_temp - last_temp_b).abs() < 1e-4);
}
