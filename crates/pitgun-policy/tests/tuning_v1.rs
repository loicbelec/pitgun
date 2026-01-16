use pitgun_contract::game::v1::GamePlayerTuningV1;
use pitgun_policy::{PolicyError, TuningPolicyV1, normalize_tuning_v1};

#[test]
fn rejects_out_of_range_points() {
    let input = GamePlayerTuningV1 {
        aero_points: 21,
        chassis_points: 10,
        engine_points: 10,
        cooling_points: 10,
        downforce_slider: 0.5,
        gear_ratio_slider: 0.5,
    };

    let err = normalize_tuning_v1(input).unwrap_err();
    match err {
        PolicyError::PointsOutOfRange { field, value, .. } => {
            assert_eq!(field, "aero_points");
            assert_eq!(value, 21);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn clamps_sliders_outside_range() {
    let input = GamePlayerTuningV1 {
        aero_points: 10,
        chassis_points: 10,
        engine_points: 10,
        cooling_points: 10,
        downforce_slider: 1.5,
        gear_ratio_slider: -0.25,
    };

    let output = normalize_tuning_v1(input).expect("normalize");
    assert_eq!(output.downforce_slider, 1.0);
    assert_eq!(output.gear_ratio_slider, 0.0);
}

#[test]
fn normalization_is_stable() {
    let policy = TuningPolicyV1::default();
    let input = GamePlayerTuningV1 {
        aero_points: 5,
        chassis_points: 7,
        engine_points: 12,
        cooling_points: 3,
        downforce_slider: 0.25,
        gear_ratio_slider: 0.75,
    };

    let first = policy.normalize(input).expect("normalize");
    let second = policy.normalize(first).expect("normalize again");
    assert_eq!(first, second);
}
