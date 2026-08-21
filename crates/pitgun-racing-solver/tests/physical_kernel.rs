use pitgun_racing_solver::{
    AERO_FULL_CORNER_CURVATURE_RAD_PER_M, AERO_FULL_STRAIGHT_CURVATURE_RAD_PER_M, AeroParams,
    ChassisParams, CurvatureAeroResponse, Driver, DriverControlParamsV3, EngineParams,
    EngineThermalDeratingShapeV3, EngineThermalParamsV3, MechanicalParamsV3, PitPlan,
    ResolvedSimulationRequestV3, SimConfig, SimulationRequest, TireContactParamsV3,
    TireDegradationParamsV3, TireParams, Track, Tuning, TuningResponseV1, VehicleParams,
    VehicleState, aggregate_tire_force_capacity_v3, apply_tuning, apply_tuning_with_response,
    combined_force_utilization, curvature_aero_blend, derating_factor_v3, describe_circuit,
    remaining_longitudinal_force, run_resolved_simulation_v3, run_simulation,
    run_simulation_with_model_response, run_simulation_with_tuning_response,
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

fn resolved_v3_request(request: &SimulationRequest) -> ResolvedSimulationRequestV3 {
    ResolvedSimulationRequestV3 {
        track: request.track.clone(),
        vehicle: request.vehicle.clone(),
        state: request.state.clone(),
        config: request.config.clone(),
        lap_count: request.lap_count,
        pit_plan: request.pit_plan.clone(),
        driver: request.driver.clone(),
        tire_contact: TireContactParamsV3::default(),
        mechanical: MechanicalParamsV3 {
            fixed_drag_area_m2: 0.5 * (request.vehicle.aero.cd_a_x + request.vehicle.aero.cd_a_z),
            fixed_downforce_area_m2: 0.5
                * (request.vehicle.aero.cl_a_x + request.vehicle.aero.cl_a_z),
            ..MechanicalParamsV3::default()
        },
        driver_control: DriverControlParamsV3::default(),
        fuel_mass: None,
        tire_degradation: None,
        engine_thermal: None,
    }
}

#[test]
fn v3_combined_contact_budget_is_bounded_and_reserves_longitudinal_force() {
    let contact = TireContactParamsV3::default();
    let nominal_load = contact.reference_normal_load_n;
    let available = aggregate_tire_force_capacity_v3(1.7, nominal_load, &contact);

    assert_eq!(available, 1.7 * nominal_load);
    assert_eq!(remaining_longitudinal_force(available, 0.0), available);
    assert_eq!(remaining_longitudinal_force(available, available), 0.0);
    assert!(remaining_longitudinal_force(available, available * 0.8) < available * 0.61);
    assert_eq!(
        combined_force_utilization(available, available, available),
        1.0
    );
    assert_eq!(
        combined_force_utilization(available * 0.3, available * 0.4, available),
        0.5
    );
}

#[test]
fn v3_load_sensitivity_makes_added_load_sublinear() {
    let contact = TireContactParamsV3::default();
    let reference =
        aggregate_tire_force_capacity_v3(1.7, contact.reference_normal_load_n, &contact);
    let doubled =
        aggregate_tire_force_capacity_v3(1.7, contact.reference_normal_load_n * 2.0, &contact);

    assert!(doubled > reference);
    assert!(doubled < reference * 2.0);
}

#[test]
fn v3_tire_state_affects_the_whole_force_envelope_and_stays_bounded() {
    let mut request = synthetic_request();
    request.track.kappa[3..18].fill(0.003);
    request.lap_count = 12;

    let healthy = run_resolved_simulation_v3(&resolved_v3_request(&request))
        .expect("healthy V3 contact solve");
    let mut compromised_request = resolved_v3_request(&request);
    compromised_request.state.tire_temp = 45.0;
    compromised_request.state.tire_wear = 0.75;
    let compromised =
        run_resolved_simulation_v3(&compromised_request).expect("compromised V3 contact solve");

    assert!(compromised.total_time_s > healthy.total_time_s);
    assert!(
        healthy
            .solution
            .tire_force_utilization
            .iter()
            .all(|value| value.is_finite() && (0.0..=1.0).contains(value))
    );
    assert!(
        healthy
            .solution
            .tire_temp
            .iter()
            .all(|value| value.is_finite() && (0.0..=250.0).contains(value))
    );
    assert!(
        healthy
            .solution
            .tire_wear
            .iter()
            .all(|value| value.is_finite() && (0.0..=1.0).contains(value))
    );

    let diagnostics = healthy.tire_diagnostics_v3.expect("V3 tire diagnostics");
    assert!(diagnostics.maximum_combined_utilization <= 1.0);
    assert!(diagnostics.maximum_combined_utilization > 0.0);
    assert!(diagnostics.maximum_normal_load_n >= diagnostics.minimum_normal_load_n);
    assert!(diagnostics.maximum_available_force_n >= diagnostics.minimum_available_force_n);
    assert!(diagnostics.generated_heat_kj > 0.0);
    assert!(diagnostics.contact_workload_mj > 0.0);
}

#[test]
fn historical_models_do_not_emit_v3_contact_diagnostics() {
    let result = run_simulation(&synthetic_request()).expect("historical solve");

    assert!(result.tire_diagnostics_v3.is_none());
    assert!(result.solution.tire_force_utilization.is_empty());
    assert!(result.solution.tire_normal_load_n.is_empty());
    assert!(result.solution.tire_available_force_n.is_empty());
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
fn v3_resolved_boundary_is_deterministic_without_gameplay_tuning() {
    let request = resolved_v3_request(&synthetic_request());

    let first = run_resolved_simulation_v3(&request).expect("first V3 candidate solve");
    let second = run_resolved_simulation_v3(&request).expect("second V3 candidate solve");

    assert_eq!(first, second);
    assert_eq!(first.applied_vehicle.chassis, request.vehicle.chassis);
    assert_eq!(
        first.applied_vehicle.aero.cd_a_x,
        request.mechanical.fixed_drag_area_m2
    );
    assert_eq!(
        first.applied_vehicle.aero.cd_a_z,
        request.mechanical.fixed_drag_area_m2
    );
    assert_eq!(
        first.applied_vehicle.aero.cl_a_x,
        request.mechanical.fixed_downforce_area_m2
    );
    assert_eq!(
        first.applied_vehicle.aero.cl_a_z,
        request.mechanical.fixed_downforce_area_m2
    );
    assert_eq!(first.applied_vehicle.engine, request.vehicle.engine);
    assert!(first.total_time_s.is_finite());
    assert!(first.total_time_s > 0.0);
}

#[test]
fn v3_gearbox_is_sequential_and_exposes_shift_cost() {
    let mut request = resolved_v3_request(&synthetic_request());
    request.mechanical.upshift_rpm = 6_000.0;
    request.mechanical.downshift_rpm = 2_000.0;
    request.mechanical.shift_duration_s = 0.20;

    let shifted = run_resolved_simulation_v3(&request).expect("sequential gearbox solve");
    let diagnostics = shifted
        .mechanical_diagnostics_v3
        .expect("V3 mechanical diagnostics");

    assert!(diagnostics.sequential_shift_count > 0);
    assert!(diagnostics.shift_interruption_time_s > 0.0);
    assert!(
        shifted
            .solution
            .gear
            .windows(2)
            .all(|gears| gears[0].abs_diff(gears[1]) <= 1),
        "a sequential gearbox may only select an adjacent ratio"
    );
    assert!(
        shifted
            .solution
            .shift_power_fraction
            .iter()
            .any(|fraction| *fraction < 1.0)
    );

    let mut instant = request;
    instant.mechanical.shift_duration_s = 0.0;
    let instant = run_resolved_simulation_v3(&instant).expect("instant gearbox solve");
    assert!(shifted.total_time_s >= instant.total_time_s);
}

#[test]
fn v3_braking_is_bounded_by_the_named_system_limit() {
    let mut request = resolved_v3_request(&synthetic_request());
    request.track.kappa[7..15].fill(0.004);
    request.mechanical.maximum_brake_force_n = 4_000.0;
    let weak = run_resolved_simulation_v3(&request).expect("limited brake solve");

    assert!(
        weak.solution
            .brake_force_budget_n
            .iter()
            .all(|force| *force <= 4_000.0 + f64::EPSILON)
    );
    assert!(
        weak.mechanical_diagnostics_v3
            .expect("V3 mechanical diagnostics")
            .brake_limit_activation_count
            > 0
    );

    request.mechanical.maximum_brake_force_n = 18_000.0;
    let strong = run_resolved_simulation_v3(&request).expect("strong brake solve");
    assert!(weak.total_time_s >= strong.total_time_s);
}

#[test]
fn v3_driver_operates_physical_limits_without_post_solve_time_scaling() {
    let mut first_request = resolved_v3_request(&synthetic_request());
    first_request.driver_control.control_error = 0.0;
    let mut second_request = first_request.clone();
    second_request.driver.id = "another-driver".to_string();
    second_request.driver.display_name = "Another Driver".to_string();
    second_request.driver.aggressiveness = 1.0;

    let first = run_resolved_simulation_v3(&first_request).expect("first driver solve");
    let second = run_resolved_simulation_v3(&second_request).expect("second driver solve");

    assert_eq!(first.solution, second.solution);
    assert_eq!(first.lap_times_s, second.lap_times_s);
    assert_ne!(first.applied_driver, second.applied_driver);
}

#[test]
fn v3_fixed_aero_is_independent_of_curvature_and_drag_cannot_help_terminal_speed() {
    let mut low_drag_request = resolved_v3_request(&synthetic_request());
    low_drag_request.lap_count = 1;
    low_drag_request.mechanical.fixed_drag_area_m2 = 0.50;
    let low_drag = run_resolved_simulation_v3(&low_drag_request).expect("low drag solve");

    let mut high_drag_request = low_drag_request;
    high_drag_request.mechanical.fixed_drag_area_m2 = 1.50;
    let high_drag = run_resolved_simulation_v3(&high_drag_request).expect("high drag solve");

    let low_terminal = low_drag.solution.v.iter().copied().fold(0.0, f64::max);
    let high_terminal = high_drag.solution.v.iter().copied().fold(0.0, f64::max);
    assert!(high_terminal <= low_terminal);
    assert!(high_drag.total_time_s >= low_drag.total_time_s);
    assert_eq!(
        low_drag
            .mechanical_diagnostics_v3
            .expect("low drag diagnostics")
            .fixed_drag_area_m2,
        0.50
    );
    assert_eq!(low_drag.applied_vehicle.aero.cd_a_x, 0.50);
    assert_eq!(low_drag.applied_vehicle.aero.cd_a_z, 0.50);
    assert_eq!(
        low_drag.applied_vehicle.aero.cl_a_x,
        low_drag.applied_vehicle.aero.cl_a_z
    );
}

#[test]
fn v3_cooling_is_observable_and_only_acts_through_temperature() {
    let mut weak_request = resolved_v3_request(&synthetic_request());
    weak_request.lap_count = 20;
    weak_request.vehicle.engine.t_soft = 92.0;
    weak_request.vehicle.engine.p_cool0 = 0.0;
    weak_request.vehicle.engine.k_cool = 0.0;
    let weak = run_resolved_simulation_v3(&weak_request).expect("weak cooling solve");

    let mut strong_request = weak_request;
    strong_request.vehicle.engine.p_cool0 = 500.0;
    strong_request.vehicle.engine.k_cool = 100.0;
    let strong = run_resolved_simulation_v3(&strong_request).expect("strong cooling solve");

    let weak_diagnostics = weak
        .mechanical_diagnostics_v3
        .expect("weak cooling diagnostics");
    let strong_diagnostics = strong
        .mechanical_diagnostics_v3
        .expect("strong cooling diagnostics");
    assert!(weak_diagnostics.generated_engine_heat_kj > 0.0);
    assert_eq!(weak_diagnostics.removed_engine_heat_kj, 0.0);
    assert!(strong_diagnostics.removed_engine_heat_kj > 0.0);
    assert!(
        strong_diagnostics.maximum_engine_temperature_c
            <= weak_diagnostics.maximum_engine_temperature_c
    );
    assert!(strong_diagnostics.engine_derated_time_s <= weak_diagnostics.engine_derated_time_s);
}

#[test]
fn v3_thermal_minimum_power_guard_is_explicit_and_bounded() {
    let mut baseline_request = resolved_v3_request(&synthetic_request());
    baseline_request.lap_count = 20;
    baseline_request.vehicle.engine.t_soft = 90.0;
    baseline_request.vehicle.engine.p_cool0 = 0.0;
    baseline_request.vehicle.engine.k_cool = 0.0;
    let baseline = run_resolved_simulation_v3(&baseline_request).expect("baseline thermal solve");

    let mut guarded_request = baseline_request;
    guarded_request.engine_thermal = Some(EngineThermalParamsV3 {
        minimum_power_fraction: 0.60,
        ..EngineThermalParamsV3::default()
    });
    let guarded = run_resolved_simulation_v3(&guarded_request).expect("guarded thermal solve");

    assert!(guarded.total_time_s < baseline.total_time_s);
    assert!(
        guarded
            .solution
            .engine_derating_factor
            .iter()
            .copied()
            .fold(1.0, f64::min)
            >= 0.60
    );

    guarded_request
        .engine_thermal
        .as_mut()
        .expect("thermal boundary")
        .minimum_power_fraction = 0.0;
    assert!(run_resolved_simulation_v3(&guarded_request).is_err());
}

#[test]
fn v3_smooth_thermal_knee_is_continuous_and_less_abrupt() {
    let request = synthetic_request();
    let engine = &request.vehicle.engine;
    let linear = EngineThermalParamsV3::default();
    let smooth = EngineThermalParamsV3 {
        derating_shape: EngineThermalDeratingShapeV3::SmoothKnee,
        smooth_knee_width_c: 10.0,
        ..linear
    };

    assert_eq!(derating_factor_v3(engine.t_soft, engine, smooth), 1.0);
    assert!(
        derating_factor_v3(engine.t_soft + 2.0, engine, smooth)
            > derating_factor_v3(engine.t_soft + 2.0, engine, linear)
    );
    assert_eq!(
        derating_factor_v3(engine.t_soft + 10.0, engine, smooth),
        derating_factor_v3(engine.t_soft + 10.0, engine, linear)
    );
    assert!(
        derating_factor_v3(engine.t_soft + 10.001, engine, smooth)
            <= derating_factor_v3(engine.t_soft + 10.0, engine, smooth)
    );
}

#[test]
fn v3_uses_explicit_non_uniform_segments_and_ignores_compatibility_ds() {
    let mut request = resolved_v3_request(&synthetic_request());
    request.track.s = vec![
        0.0, 8.0, 21.0, 39.0, 62.0, 90.0, 123.0, 161.0, 204.0, 252.0, 305.0, 363.0, 426.0, 494.0,
        567.0, 645.0, 728.0, 816.0, 909.0, 1_007.0, 1_110.0,
    ];
    request.track.x = request.track.s.clone();
    request.track.slope = request
        .track
        .s
        .iter()
        .map(|distance| 0.01 * (distance / 1_110.0))
        .collect();

    request.config.ds = 1.0;
    let first = run_resolved_simulation_v3(&request).expect("first non-uniform V3 solve");
    request.config.ds = 999.0;
    let second = run_resolved_simulation_v3(&request).expect("second non-uniform V3 solve");

    assert_eq!(first, second);
    assert_eq!(first.solution.s.last().copied(), Some(2_220.0));
}

#[test]
fn v3_rejects_invalid_resolved_physics_before_solving() {
    let mut bad_track = resolved_v3_request(&synthetic_request());
    bad_track.track.s[5] = bad_track.track.s[4];
    assert_eq!(
        run_resolved_simulation_v3(&bad_track).unwrap_err(),
        "track.s must be strictly increasing"
    );

    let mut bad_vehicle = resolved_v3_request(&synthetic_request());
    bad_vehicle.vehicle.chassis.mass_empty = -1.0;
    assert_eq!(
        run_resolved_simulation_v3(&bad_vehicle).unwrap_err(),
        "vehicle.chassis.mass_empty_kg must be finite and positive"
    );

    let mut bad_contact = resolved_v3_request(&synthetic_request());
    bad_contact.tire_contact.load_sensitivity_exponent = 0.31;
    assert_eq!(
        run_resolved_simulation_v3(&bad_contact).unwrap_err(),
        "tire_contact.load_sensitivity_exponent must be <= 0.3"
    );

    let mut bad_mechanical = resolved_v3_request(&synthetic_request());
    bad_mechanical.mechanical.driveline_efficiency = 0.0;
    assert_eq!(
        run_resolved_simulation_v3(&bad_mechanical).unwrap_err(),
        "mechanical.driveline_efficiency must be greater than 0"
    );

    let mut bad_driver = resolved_v3_request(&synthetic_request());
    bad_driver.driver_control.braking_utilization = 0.49;
    assert_eq!(
        run_resolved_simulation_v3(&bad_driver).unwrap_err(),
        "driver_control.braking_utilization must be finite and in [0.5, 1]"
    );

    let mut bad_degradation = resolved_v3_request(&synthetic_request());
    bad_degradation.tire_degradation = Some(TireDegradationParamsV3 {
        maximum_thermal_wear_multiplier: 0.5,
        ..TireDegradationParamsV3::default()
    });
    assert_eq!(
        run_resolved_simulation_v3(&bad_degradation).unwrap_err(),
        "V3 maximum thermal-wear multiplier must be in [1, 20]"
    );
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
