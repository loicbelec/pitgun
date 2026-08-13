use pitgun_racing_solver::{
    AERO_FULL_CORNER_CURVATURE_RAD_PER_M, AERO_FULL_STRAIGHT_CURVATURE_RAD_PER_M, AeroParams,
    ChassisParams, CurvatureAeroResponse, Driver, EngineParams, PitPlan, SimConfig,
    SimulationRequest, TireParams, Track, Tuning, TuningResponseV1, VehicleParams, VehicleState,
    apply_tuning, apply_tuning_with_response, curvature_aero_blend, describe_circuit,
    run_simulation, run_simulation_with_model_response, run_simulation_with_tuning_response,
};

fn synthetic_request() -> SimulationRequest {
    let sample_count = 21;
    let s = (0..sample_count)
        .map(|index| index as f64 * 25.0)
        .collect::<Vec<_>>();

    SimulationRequest {
        track: Track {
            s: s.clone(),
            x: s,
            y: vec![0.0; sample_count],
            z: vec![0.0; sample_count],
            kappa: vec![0.0; sample_count],
            slope: vec![0.0; sample_count],
            heading: vec![0.0; sample_count],
        },
        vehicle: VehicleParams {
            chassis: ChassisParams {
                mass_empty: 800.0,
                r_wheel: 0.33,
                mu0: 1.7,
                c_rr: 0.015,
                rho: 1.225,
                g: 9.81,
            },
            aero: AeroParams {
                cd_a_x: 0.8,
                cd_a_z: 1.0,
                cl_a_x: 2.6,
                cl_a_z: 4.13,
            },
            engine: EngineParams {
                n_rpm: vec![0.0, 2_000.0, 6_000.0, 10_000.0, 12_000.0],
                trq: vec![0.0, 0.365, 0.56, 0.50, 0.10],
                gear_ratios: vec![14.0, 10.5, 7.8, 5.9, 4.5],
                n_upshift: 0.0,
                n_downshift: 0.0,
                n_idle: 1_700.0,
                n_max: 12_000.0,
                t_amb: 35.0,
                t_init: 90.0,
                c_th: 100_000.0,
                alpha_heat: 0.45,
                p_cool0: 0.0,
                k_cool: 45.0,
                t_soft: 110.0,
                beta_derate: 0.01,
                fuel_burn_kg_per_s: 0.02,
            },
            tire: TireParams {
                mu_scale: 1.0,
                wear_per_s: 0.000_01,
                wear_load_k: 0.000_001,
                wear_grip_k: 0.3,
                wear_min: 0.7,
                temp_opt: 90.0,
                temp_sigma: 20.0,
                temp_min_k: 0.5,
                heat_k: 0.000_1,
                cool_k: 0.000_01,
            },
        },
        state: VehicleState::default(),
        config: SimConfig {
            ds: 25.0,
            max_speed: 100.0,
            pit_time_penalty_s: 20.0,
            pit_tire_temp: None,
            tire_temp_amb: 35.0,
            sim_seed: 42,
        },
        lap_count: 2,
        pit_plan: PitPlan::default(),
        driver: Driver {
            id: "test-driver".to_string(),
            display_name: "Test Driver".to_string(),
            aggressiveness: 0.5,
        },
        tuning: None,
    }
}

#[test]
fn curvature_aero_response_is_continuous_bounded_and_monotonic() {
    assert_eq!(curvature_aero_blend(0.0), 0.0);
    assert_eq!(
        curvature_aero_blend(AERO_FULL_STRAIGHT_CURVATURE_RAD_PER_M),
        0.0
    );
    assert_eq!(
        curvature_aero_blend(AERO_FULL_CORNER_CURVATURE_RAD_PER_M),
        1.0
    );
    assert_eq!(curvature_aero_blend(1.0), 1.0);
    assert_eq!(
        curvature_aero_blend(-AERO_FULL_CORNER_CURVATURE_RAD_PER_M),
        1.0
    );

    let samples = (0..=100)
        .map(|index| curvature_aero_blend(index as f64 * 0.000_1))
        .collect::<Vec<_>>();
    assert!(samples.iter().all(|value| (0.0..=1.0).contains(value)));
    assert!(samples.windows(2).all(|window| window[1] >= window[0]));
}

#[test]
fn identical_resolved_inputs_produce_identical_physical_results() {
    let request = synthetic_request();

    let first = run_simulation(&request).expect("first physical solve must succeed");
    let second = run_simulation(&request).expect("second physical solve must succeed");

    assert_eq!(first, second);
    assert_eq!(first.lap_times_s.len(), 2);
    assert!(first.total_time_s.is_finite());
    assert!(first.total_time_s > 0.0);
    assert!(!first.solution.t.is_empty());
    assert!(first.solution.v.iter().all(|value| value.is_finite()));
}

#[test]
fn malformed_track_is_rejected_at_the_solver_boundary() {
    let mut request = synthetic_request();
    request.track.heading.pop();

    let error = run_simulation(&request).expect_err("misaligned track vectors must be rejected");

    assert_eq!(error, "track vectors must share the same length");
}

#[test]
fn circuit_descriptors_are_derived_from_physical_samples() {
    let mut request = synthetic_request();
    request.track.kappa[5..=12].fill(0.002);
    request.track.z = (0..21)
        .map(|index| {
            if index <= 10 {
                index as f64
            } else {
                (20 - index) as f64
            }
        })
        .collect();

    let descriptors = describe_circuit(&request.track).expect("valid physical track");

    assert_eq!(descriptors.length_m, 500.0);
    assert!(descriptors.straight_distance_m > 0.0);
    assert!(descriptors.corner_distance_m > 0.0);
    assert!(descriptors.corner_distance_share > 0.25);
    assert!(descriptors.corner_distance_share < 0.75);
    assert!(descriptors.absolute_curvature_integral_rad > 0.0);
    assert!(descriptors.maximum_absolute_curvature_rad_per_m >= 0.002);
    assert_eq!(descriptors.elevation_gain_m, 10.0);
    assert_eq!(descriptors.elevation_loss_m, 10.0);
}

#[test]
fn setup_response_diagnostics_are_deterministic_and_finite() {
    let mut request = synthetic_request();
    request.track.kappa[5..=12].fill(0.002);

    let first = run_simulation(&request).expect("first diagnostic solve");
    let second = run_simulation(&request).expect("second diagnostic solve");
    let diagnostics = first.diagnostics;

    assert_eq!(diagnostics, second.diagnostics);
    assert_eq!(diagnostics.corner_curvature_threshold_rad_per_m, 0.001);
    assert_eq!(diagnostics.longitudinal_acceleration_threshold_mps2, 0.05);
    assert_eq!(diagnostics.near_max_rpm_ratio, 0.98);
    assert!(diagnostics.observed_time_s > 0.0);
    assert!(diagnostics.straight_time_s > 0.0);
    assert!(diagnostics.corner_time_s > 0.0);
    assert!(diagnostics.mean_straight_speed_kph.is_finite());
    assert!(diagnostics.mean_corner_speed_kph.is_finite());
    assert!(diagnostics.maximum_observed_rpm > 0.0);
    assert!(diagnostics.maximum_rpm_utilization > 0.0);
    assert!(diagnostics.maximum_gear_used > 0);
    assert!(diagnostics.aerodynamic_drag_work_kj > 0.0);
    assert!(diagnostics.mean_downforce_n > 0.0);
    assert!(diagnostics.maximum_downforce_n >= diagnostics.mean_downforce_n);
    assert_eq!(
        diagnostics.acceleration_time_s
            + diagnostics.braking_time_s
            + diagnostics.steady_speed_time_s,
        diagnostics.observed_time_s
    );
}

#[test]
fn applied_downforce_setting_increases_drag_and_downforce_coefficients() {
    let request = synthetic_request();
    let low = apply_tuning(
        &request.vehicle,
        &Tuning {
            downforce_slider: 0.0,
            ..Tuning::default()
        },
    );
    let high = apply_tuning(
        &request.vehicle,
        &Tuning {
            downforce_slider: 1.0,
            ..Tuning::default()
        },
    );

    assert!(high.aero.cd_a_x > low.aero.cd_a_x);
    assert!(high.aero.cd_a_z > low.aero.cd_a_z);
    assert!(high.aero.cl_a_x > low.aero.cl_a_x);
    assert!(high.aero.cl_a_z > low.aero.cl_a_z);
}

#[test]
fn default_tuning_response_encodes_the_historical_coefficients() {
    let response = TuningResponseV1::default();

    assert_eq!(response.development_points_cap, 20.0);
    assert_eq!(response.aero_development_gain, 0.10);
    assert_eq!(response.drag_base, 0.85);
    assert_eq!(response.drag_slider_gain, 0.30);
    assert_eq!(response.downforce_base, 0.75);
    assert_eq!(response.downforce_slider_gain, 0.55);
    assert_eq!(response.straight_aero_scale, 0.95);
    assert_eq!(response.corner_aero_scale, 1.05);
    assert_eq!(response.chassis_grip_development_gain, 0.08);
    assert_eq!(response.cooling_base, 0.75);
    assert_eq!(response.cooling_development_gain, 0.50);
    assert_eq!(response.engine_torque_development_gain, 0.01);
    assert_eq!(response.gear_ratio_base, 1.10);
    assert_eq!(response.gear_ratio_slider_reduction, 0.20);
    response.validate().expect("historical response is valid");
}

#[test]
fn explicit_default_response_is_exactly_compatible() {
    let mut request = synthetic_request();
    request.tuning = Some(Tuning {
        aero_points: 20,
        chassis_points: 20,
        cooling_points: 20,
        engine_points: 20,
        downforce_slider: 0.65,
        gear_ratio_slider: 0.35,
    });

    let compatibility = run_simulation(&request).expect("compatibility solve");
    let explicit = run_simulation_with_tuning_response(&request, &TuningResponseV1::default())
        .expect("explicit default solve");

    assert_eq!(explicit, compatibility);
    assert_eq!(
        apply_tuning(&request.vehicle, request.tuning.as_ref().unwrap()),
        apply_tuning_with_response(
            &request.vehicle,
            request.tuning.as_ref().unwrap(),
            &TuningResponseV1::default(),
        )
        .expect("explicit default tuning"),
    );
}

#[test]
fn explicit_legacy_model_response_is_exactly_compatible() {
    let mut request = synthetic_request();
    request.track.kappa[5..=12].fill(0.002);
    request.tuning = Some(Tuning {
        downforce_slider: 0.65,
        gear_ratio_slider: 0.35,
        ..Tuning::default()
    });

    let compatibility = run_simulation_with_tuning_response(&request, &TuningResponseV1::default())
        .expect("compatibility solve");
    let explicit = run_simulation_with_model_response(
        &request,
        &TuningResponseV1::default(),
        CurvatureAeroResponse::LegacyBinary,
    )
    .expect("explicit legacy solve");

    assert_eq!(explicit, compatibility);
}

#[test]
fn continuous_model_response_is_deterministic_and_distinct_from_legacy() {
    let mut request = synthetic_request();
    request.track.kappa.fill(0.000_5);
    request.tuning = Some(Tuning {
        downforce_slider: 0.75,
        gear_ratio_slider: 0.25,
        ..Tuning::default()
    });

    let legacy = run_simulation_with_model_response(
        &request,
        &TuningResponseV1::default(),
        CurvatureAeroResponse::LegacyBinary,
    )
    .expect("legacy solve");
    let first = run_simulation_with_model_response(
        &request,
        &TuningResponseV1::default(),
        CurvatureAeroResponse::ContinuousV1,
    )
    .expect("first continuous solve");
    let second = run_simulation_with_model_response(
        &request,
        &TuningResponseV1::default(),
        CurvatureAeroResponse::ContinuousV1,
    )
    .expect("second continuous solve");

    assert_eq!(first, second);
    assert_ne!(first, legacy);
}

#[test]
fn candidate_response_changes_only_the_selected_transform() {
    let request = synthetic_request();
    let tuning = Tuning {
        downforce_slider: 1.0,
        gear_ratio_slider: 1.0,
        ..Tuning::default()
    };
    let baseline = apply_tuning(&request.vehicle, &tuning);

    let mut reduced_downforce = TuningResponseV1::default();
    reduced_downforce.downforce_slider_gain = 0.20;
    let reduced = apply_tuning_with_response(&request.vehicle, &tuning, &reduced_downforce)
        .expect("reduced downforce candidate");
    assert!(reduced.aero.cl_a_x < baseline.aero.cl_a_x);
    assert_eq!(reduced.aero.cd_a_x, baseline.aero.cd_a_x);

    let mut increased_drag = TuningResponseV1::default();
    increased_drag.drag_slider_gain = 0.60;
    let drag = apply_tuning_with_response(&request.vehicle, &tuning, &increased_drag)
        .expect("increased drag candidate");
    assert!(drag.aero.cd_a_x > baseline.aero.cd_a_x);
    assert_eq!(drag.aero.cl_a_x, baseline.aero.cl_a_x);

    let mut wider_gearing = TuningResponseV1::default();
    wider_gearing.gear_ratio_slider_reduction = 0.40;
    let gearing = apply_tuning_with_response(&request.vehicle, &tuning, &wider_gearing)
        .expect("wider gearing candidate");
    assert!(
        gearing
            .engine
            .gear_ratios
            .iter()
            .zip(&baseline.engine.gear_ratios)
            .all(|(candidate, current)| candidate < current)
    );
}

#[test]
fn invalid_tuning_responses_fail_before_simulation() {
    let request = synthetic_request();
    let mut non_finite = TuningResponseV1::default();
    non_finite.drag_slider_gain = f64::NAN;
    assert_eq!(
        run_simulation_with_tuning_response(&request, &non_finite).unwrap_err(),
        "tuning response coefficients must be finite"
    );

    let mut inverted_gearing = TuningResponseV1::default();
    inverted_gearing.gear_ratio_slider_reduction = inverted_gearing.gear_ratio_base;
    assert_eq!(
        inverted_gearing.validate().unwrap_err(),
        "gear_ratio_slider_reduction must remain below gear_ratio_base"
    );
}
