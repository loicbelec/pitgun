use pitgun_racing_solver::{
    AERO_FULL_CORNER_CURVATURE_RAD_PER_M, AERO_FULL_STRAIGHT_CURVATURE_RAD_PER_M, AeroParams,
    ChassisParams, CurvatureAeroResponse, Driver, DriverControlParamsV3,
    DriverCorrectionCapacityModelV3, EngineParams, EngineThermalDeratingShapeV3,
    EngineThermalParamsV3, MechanicalParamsV3, PitPlan, ResolvedDriverControlLapV3,
    ResolvedSimulationRequestV3, ResolvedSimulationSessionV3, ResolvedSimulationStepV3, SimConfig,
    SimulationRequest, TireContactParamsV3, TireDegradationParamsV3, TireParams, Track, Tuning,
    TuningResponseV1, VehicleParams, VehicleState, aggregate_tire_force_capacity_v3, apply_tuning,
    apply_tuning_with_response, combined_force_utilization, curvature_aero_blend,
    derating_factor_v3, describe_circuit, remaining_longitudinal_force, run_resolved_simulation_v3,
    run_simulation, run_simulation_with_model_response, run_simulation_with_tuning_response,
};

use pitgun_racing_solver::power_telemetry::project_lap_power_v1;

fn manufactured_power_lap() -> (
    pitgun_racing_solver::SimulationSolution,
    VehicleParams,
    MechanicalParamsV3,
) {
    let mut vehicle = synthetic_request().vehicle;
    vehicle.engine.gear_ratios = vec![1.0];
    vehicle.engine.n_rpm = vec![0.0, 12_000.0];
    vehicle.engine.trq = vec![0.5, 0.5];
    vehicle.engine.n_idle = 3_000.0;
    let mechanical = MechanicalParamsV3 {
        driveline_efficiency: 0.8,
        ..MechanicalParamsV3::default()
    };
    let mut solution = pitgun_racing_solver::SimulationSolution {
        s: vec![0., 10., 20., 30.],
        t: vec![0., 1., 2., 3.],
        v: vec![0., 5., 20., 20.],
        power: vec![8., 16., 0., 0.],
        temp: vec![90.; 4],
        gear: vec![1; 4],
        engine_derating_factor: vec![1.; 4],
        shift_power_fraction: vec![0.5, 0.5, 1., 1.],
        ..Default::default()
    };
    for i in 0..3 {
        let dt = 10.0 / solution.v[i].max(1.0);
        let heat =
            vehicle.engine.alpha_heat * solution.power[i] * 1000. / mechanical.driveline_efficiency;
        let cooling = (vehicle.engine.p_cool0 + vehicle.engine.k_cool * solution.v[i].max(1.0))
            * (solution.temp[i] - vehicle.engine.t_amb);
        solution.temp[i + 1] = solution.temp[i] + (heat - cooling) * dt / vehicle.engine.c_th;
    }
    (solution, vehicle, mechanical)
}

#[test]
fn power_v1_separates_shaft_wheel_regularization_and_integration_clocks() {
    let (s, v, m) = manufactured_power_lap();
    let p = project_lap_power_v1(&s, &v, m, None).expect("power contract");
    assert_eq!(p.points[0].engine_delivered_w, 10_000.);
    assert_eq!(p.points[0].wheel_propulsion_w, 0.);
    assert_eq!(p.points[1].wheel_propulsion_w, 8_000.);
    assert_eq!(p.points[1].driveline_loss_w, 4_000.);
    assert_eq!(p.points[1].launch_regularization_w, 8_000.);
    assert_eq!(p.ledger.solver_engine_work_j, 140_000.);
    assert_eq!(p.ledger.trajectory_engine_work_j, 30_000.);
    assert_eq!(
        p.ledger.solver_engine_work_j,
        p.ledger.solver_wheel_work_j
            + p.ledger.solver_driveline_loss_j
            + p.ledger.solver_launch_regularization_j
    );
    assert!(p.ledger.engine_thermal_residual_j.abs() < 1e-7);
    assert!(
        p.points
            .iter()
            .all(|p| (0.0..=100.).contains(&p.equivalent_engine_load_pct))
    );
}

#[test]
fn power_v1_does_not_invent_pedal_or_braking_from_zero_propulsion() {
    let (s, v, m) = manufactured_power_lap();
    let p = project_lap_power_v1(&s, &v, m, None).unwrap();
    assert!(p.pedal_command_pct.is_none());
    assert!(p.brake_command_pct.is_none());
    assert_eq!(p.points[2].equivalent_engine_load_pct, 0.);
    assert_eq!(p.points[2].engine_delivered_w, 0.);
}

#[test]
fn power_v1_uses_the_committed_thermal_derating_before_normalizing_load() {
    let (mut s, v, m) = manufactured_power_lap();
    let nominal = project_lap_power_v1(&s, &v, m, None).unwrap();
    s.engine_derating_factor[0] = 0.5;
    let limited = project_lap_power_v1(&s, &v, m, None).unwrap();
    assert_eq!(
        limited.points[0].engine_available_w,
        nominal.points[0].engine_available_w / 2.0
    );
    assert_eq!(
        limited.points[0].equivalent_engine_load_pct,
        nominal.points[0].equivalent_engine_load_pct * 2.0
    );
    assert_eq!(
        limited.points[0].engine_delivered_w,
        nominal.points[0].engine_delivered_w
    );
}

#[test]
fn power_v1_export_frequency_does_not_change_native_ledger_or_invent_shift_loads() {
    let (s, v, m) = manufactured_power_lap();
    let p = project_lap_power_v1(&s, &v, m, None).unwrap();
    let original = p.clone();
    let slow = p.resample(1.).unwrap();
    let fast = p.resample(5.).unwrap();
    assert_eq!(slow.len(), 3);
    assert_eq!(fast.len(), 15);
    assert_eq!(fast[4].engine_delivered_w, 10_000.);
    assert_eq!(fast[5].engine_delivered_w, 20_000.);
    assert_eq!(p, original);
    for bad in [0., -1., f64::NAN, 21.] {
        assert!(p.resample(bad).is_err());
    }
}

#[test]
fn power_v1_export_times_are_causal_and_cannot_extend_the_computed_prefix() {
    let (s, v, m) = manufactured_power_lap();
    let p = project_lap_power_v1(&s, &v, m, None).unwrap();
    let rows = p.sample_at_times(&[0.0, 0.9, 1.0, 2.5]).unwrap();
    assert_eq!(rows[1].engine_delivered_w, rows[0].engine_delivered_w);
    assert_eq!(rows[2].engine_delivered_w, p.points[1].engine_delivered_w);
    assert_eq!(rows[3].engine_delivered_w, p.points[2].engine_delivered_w);
    for times in [&[-0.1][..], &[3.1], &[0.0, 0.0], &[1.0, 0.0], &[f64::NAN]] {
        assert!(p.sample_at_times(times).is_err());
    }
    assert!(p.sample_at_times(&[]).unwrap().is_empty());
}

#[test]
fn power_v1_rejects_capacity_mismatch_instead_of_hiding_it_with_a_clamp() {
    let (mut s, v, m) = manufactured_power_lap();
    s.power[0] = 1_000.;
    assert!(
        project_lap_power_v1(&s, &v, m, None)
            .unwrap_err()
            .contains("capacity mismatch")
    );
    s.power[0] = 8.;
    s.shift_power_fraction[0] = 0.;
    assert!(
        project_lap_power_v1(&s, &v, m, None)
            .unwrap_err()
            .contains("capacity mismatch")
    );
}

#[test]
fn power_v1_rejects_missing_or_nonfinite_native_inputs() {
    let (s, v, m) = manufactured_power_lap();
    for kind in 0..6 {
        let mut bad = s.clone();
        match kind {
            0 => bad.power[0] = f64::NAN,
            1 => bad.t[1] = bad.t[0],
            2 => bad.engine_derating_factor.clear(),
            3 => bad.gear[0] = 0,
            4 => bad.shift_power_fraction[0] = 1.1,
            _ => bad.lap_index = vec![1, 1, 2, 2],
        }
        assert!(project_lap_power_v1(&bad, &v, m, None).is_err());
    }
}

#[test]
fn power_v1_audits_real_incremental_laps_without_changing_the_physical_result() {
    let request = resolved_v3_request(&synthetic_request());
    let expected = run_resolved_simulation_v3(&request).unwrap();
    let mechanical = request.mechanical;
    let vehicle = request.vehicle.clone();
    let fuel = request.fuel_mass;
    let mut session = ResolvedSimulationSessionV3::new(request).unwrap();
    let mut work = 0.;
    let mut losses = 0.;
    let mut burned = 0.;
    let actual = loop {
        match session.advance().unwrap() {
            ResolvedSimulationStepV3::Lap(lap) => {
                let p = project_lap_power_v1(&lap.solution, &vehicle, mechanical, fuel).unwrap();
                work += p.ledger.solver_engine_work_j;
                losses += p.ledger.solver_driveline_loss_j;
                burned += p.ledger.combustion_fuel_consumption_kg.unwrap_or(0.);
                assert!(p.ledger.engine_thermal_residual_j.abs() < 1e-5);
            }
            ResolvedSimulationStepV3::Complete(result) => break *result,
        }
    };
    assert_eq!(actual, expected);
    assert!(
        (losses / 1000. - actual.mechanical_diagnostics_v3.unwrap().driveline_loss_kj).abs() < 1e-7
    );
    if let Some(f) = actual.fuel_mass_diagnostics_v3 {
        assert!((work / 1000. - f.engine_output_work_kj).abs() < 1e-7);
        assert!((burned - f.fuel_consumed_kg).abs() < 1e-9);
    }
}

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
        driver_correction_workload_multiplier: None,
        driver_control_schedule: Vec::new(),
        driver_correction_capacity_model: None,
        fuel_mass: None,
        tire_degradation: None,
        engine_thermal: None,
        longitudinal_energy: None,
        thermal_physics_v2: None,
        driver_lapses_v1: None,
    }
}

#[test]
fn incremental_v3_session_preserves_the_monolithic_result() {
    let request = resolved_v3_request(&synthetic_request());
    let expected = run_resolved_simulation_v3(&request).expect("monolithic V3 solve");
    let mut session = ResolvedSimulationSessionV3::new(request).expect("incremental V3 session");
    let mut laps = Vec::new();

    let actual = loop {
        match session.advance().expect("incremental V3 step") {
            ResolvedSimulationStepV3::Lap(lap) => laps.push(lap),
            ResolvedSimulationStepV3::Complete(result) => break *result,
        }
    };

    assert_eq!(laps.len(), 2);
    assert_eq!(laps[0].lap_index, 1);
    assert_eq!(laps[1].lap_index, 2);
    assert_eq!(laps[1].state_after_lap, actual.final_state);
    assert_eq!(laps[1].cumulative_time_s, actual.total_time_s);
    assert_eq!(actual, expected);
    assert!(session.advance().is_err());
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
fn v3_driver_corrections_add_explainable_heat_and_wear_deterministically() {
    let mut request = resolved_v3_request(&synthetic_request());
    request.lap_count = 3;
    request.tire_degradation = Some(TireDegradationParamsV3::default());
    request.driver_correction_workload_multiplier = Some(1.20);

    let first = run_resolved_simulation_v3(&request).expect("first corrected solve");
    let second = run_resolved_simulation_v3(&request).expect("second corrected solve");
    assert_eq!(first, second);

    let tire = first.tire_diagnostics_v3.expect("tire diagnostics");
    let base = tire
        .base_contact_workload_mj
        .expect("base contact workload");
    let correction = tire
        .correction_contact_workload_mj
        .expect("correction contact workload");
    assert!(base > 0.0);
    assert!(correction > 0.0);
    assert!((tire.contact_workload_mj - base - correction).abs() < 1e-9);
    assert!(tire.correction_generated_heat_kj.expect("correction heat") > 0.0);
    assert!(
        first
            .tire_degradation_diagnostics_v3
            .expect("wear diagnostics")
            .requested_correction_wear_fraction
            .expect("correction wear")
            > 0.0
    );
}

#[test]
fn v3_driver_corrections_can_reserve_the_shared_friction_budget() {
    let mut workload_only = resolved_v3_request(&synthetic_request());
    workload_only.lap_count = 3;
    workload_only.tire_degradation = Some(TireDegradationParamsV3::default());
    workload_only.driver_control = DriverControlParamsV3 {
        cornering_utilization: 0.99,
        braking_utilization: 0.99,
        traction_utilization: 0.99,
        control_error: 0.03,
    };
    workload_only.driver_correction_workload_multiplier = Some(1.20);

    let legacy = run_resolved_simulation_v3(&workload_only).expect("workload-only solve");
    let mut friction_budget = workload_only;
    friction_budget.driver_correction_capacity_model =
        Some(DriverCorrectionCapacityModelV3::FrictionBudgetV1);
    let first = run_resolved_simulation_v3(&friction_budget).expect("friction-budget solve");
    let second = run_resolved_simulation_v3(&friction_budget).expect("repeat solve");
    assert_eq!(first, second);
    assert!(first.total_time_s >= legacy.total_time_s);

    let legacy_driver = legacy
        .driver_control_diagnostics_v3
        .expect("legacy driver diagnostics");
    let reserved_driver = first
        .driver_control_diagnostics_v3
        .expect("reserved driver diagnostics");
    assert_eq!(legacy_driver.correction_force_capacity_fraction, None);
    let capacity_fraction = reserved_driver
        .correction_force_capacity_fraction
        .expect("correction force capacity");
    assert!((capacity_fraction - 1.20_f64.sqrt().recip()).abs() < 1e-12);
    assert!(reserved_driver.cornering.mean_realized < legacy_driver.cornering.mean_realized);
    assert!(reserved_driver.braking.mean_realized < legacy_driver.braking.mean_realized);
    assert!(reserved_driver.traction.mean_realized < legacy_driver.traction.mean_realized);
}

#[test]
fn v3_driver_control_schedule_is_deterministic_and_preserves_lap_state() {
    let mut request = resolved_v3_request(&synthetic_request());
    request.track.kappa[5..=12].fill(0.002);
    request.lap_count = 4;
    request.tire_degradation = Some(TireDegradationParamsV3::default());
    request.driver_control = DriverControlParamsV3 {
        cornering_utilization: 0.76,
        braking_utilization: 0.75,
        traction_utilization: 0.77,
        control_error: 0.015,
    };
    request.driver_correction_workload_multiplier = Some(1.05);
    request.driver_correction_capacity_model =
        Some(DriverCorrectionCapacityModelV3::FrictionBudgetV1);
    request.driver_control_schedule = vec![
        ResolvedDriverControlLapV3 {
            lap_index: 1,
            driver_control: DriverControlParamsV3 {
                cornering_utilization: 0.99,
                braking_utilization: 0.98,
                traction_utilization: 0.99,
                control_error: 0.04,
            },
            correction_workload_multiplier: Some(1.30),
        },
        ResolvedDriverControlLapV3 {
            lap_index: 3,
            driver_control: DriverControlParamsV3 {
                cornering_utilization: 0.88,
                braking_utilization: 0.87,
                traction_utilization: 0.89,
                control_error: 0.025,
            },
            correction_workload_multiplier: Some(1.12),
        },
    ];

    let mut static_baseline = request.clone();
    static_baseline.driver_control_schedule.clear();
    let baseline = run_resolved_simulation_v3(&static_baseline).expect("static baseline");
    let first = run_resolved_simulation_v3(&request).expect("first scheduled solve");
    let second = run_resolved_simulation_v3(&request).expect("second scheduled solve");

    assert_eq!(first, second);
    assert_eq!(first.lap_times_s[0], baseline.lap_times_s[0]);
    let scheduled_second_lap_cornering = first
        .solution
        .lap_index
        .iter()
        .zip(&first.solution.driver_cornering_utilization)
        .filter_map(|(lap, utilization)| (*lap == 2).then_some(*utilization))
        .collect::<Vec<_>>();
    let baseline_second_lap_cornering = baseline
        .solution
        .lap_index
        .iter()
        .zip(&baseline.solution.driver_cornering_utilization)
        .filter_map(|(lap, utilization)| (*lap == 2).then_some(*utilization))
        .collect::<Vec<_>>();
    assert_ne!(
        scheduled_second_lap_cornering,
        baseline_second_lap_cornering
    );
    assert_ne!(first.final_state, baseline.final_state);
    assert_eq!(first.driver_control_diagnostics_v3, None);
    assert_eq!(
        first.applied_driver_control_schedule_v3,
        vec![
            ResolvedDriverControlLapV3 {
                lap_index: 0,
                driver_control: request.driver_control,
                correction_workload_multiplier: request.driver_correction_workload_multiplier,
            },
            request.driver_control_schedule[0],
            request.driver_control_schedule[1],
        ]
    );
}

#[test]
fn v3_driver_control_schedule_fails_closed_and_empty_wire_stays_compatible() {
    let request = resolved_v3_request(&synthetic_request());
    let encoded = serde_json::to_value(&request).expect("resolved V3 request JSON");
    assert!(encoded.get("driver_control_schedule").is_none());

    let transition = ResolvedDriverControlLapV3 {
        lap_index: 1,
        driver_control: DriverControlParamsV3::default(),
        correction_workload_multiplier: None,
    };

    let mut at_start = request.clone();
    at_start.driver_control_schedule = vec![ResolvedDriverControlLapV3 {
        lap_index: 0,
        ..transition
    }];
    assert!(
        run_resolved_simulation_v3(&at_start)
            .unwrap_err()
            .contains("start after zero-based lap 0")
    );

    let mut duplicate = request.clone();
    duplicate.driver_control_schedule = vec![transition, transition];
    assert!(
        run_resolved_simulation_v3(&duplicate)
            .unwrap_err()
            .contains("strictly increasing lap indexes")
    );

    let mut outside = request.clone();
    outside.driver_control_schedule = vec![ResolvedDriverControlLapV3 {
        lap_index: request.lap_count,
        ..transition
    }];
    assert!(
        run_resolved_simulation_v3(&outside)
            .unwrap_err()
            .contains("outside lap_count")
    );

    let mut missing_workload = request;
    missing_workload.driver_correction_workload_multiplier = Some(1.10);
    missing_workload.driver_correction_capacity_model =
        Some(DriverCorrectionCapacityModelV3::FrictionBudgetV1);
    missing_workload.driver_control_schedule = vec![transition];
    assert!(
        run_resolved_simulation_v3(&missing_workload)
            .unwrap_err()
            .contains("requires a workload multiplier")
    );
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

#[test]
fn service_boundary_keep_preserves_canonical_output_and_emits_driving_once() {
    use pitgun_racing_solver::{PitStop, ResolvedServiceStepV3};
    let mut input = resolved_v3_request(&synthetic_request());
    input.lap_count = 4;
    input.fuel_mass = Some(pitgun_racing_solver::FuelMassParamsV3::default());
    input.tire_degradation = Some(TireDegradationParamsV3::default());
    input.pit_plan.stops = vec![PitStop {
        lap: 2,
        tire: input.vehicle.tire.clone(),
    }];
    let expected = run_resolved_simulation_v3(&input).unwrap();
    let mut session = ResolvedSimulationSessionV3::new_with_service_boundaries(input).unwrap();
    assert!(session.advance().is_err());
    let mut prefixes = 0;
    let mut commits = 0;
    let actual = loop {
        match session.advance_service().unwrap() {
            ResolvedServiceStepV3::BeforeService(prefix) => {
                prefixes += 1;
                assert_eq!(prefix.lap_index, prefixes);
                assert!(!prefix.solution.t.is_empty());
                assert!(!prefix.pit_stop_completed);
                for _ in 0..5 {
                    assert!(
                        session
                            .advance_service()
                            .unwrap_err()
                            .contains("awaits resolution")
                    );
                }
                session.resolve_service_boundary(None).unwrap();
                assert!(session.resolve_service_boundary(None).is_err());
            }
            ResolvedServiceStepV3::Lap(lap) => {
                commits += 1;
                assert_eq!(lap.lap_index, commits);
                assert!(lap.solution.t.is_empty());
            }
            ResolvedServiceStepV3::Complete(result) => break *result,
        }
    };
    assert_eq!((prefixes, commits), (4, 4));
    assert_eq!(actual, expected);
    assert!(session.advance_service().is_err());
}

#[test]
fn service_boundary_pit_changes_service_without_recomputing_its_driving_prefix() {
    use pitgun_racing_solver::{PitStop, ResolvedServiceStepV3};
    let mut input = resolved_v3_request(&synthetic_request());
    input.lap_count = 4;
    input.fuel_mass = Some(pitgun_racing_solver::FuelMassParamsV3::default());
    input.tire_degradation = Some(TireDegradationParamsV3::default());
    input.config.pit_time_penalty_s = 20.0;
    input.pit_plan.stops = vec![PitStop {
        lap: 2,
        tire: input.vehicle.tire.clone(),
    }];
    let mut replacement = input.vehicle.tire.clone();
    replacement.mu_scale *= 0.9;
    let mut equivalent = input.clone();
    equivalent.pit_plan.stops = vec![PitStop {
        lap: 1,
        tire: replacement.clone(),
    }];
    let expected = run_resolved_simulation_v3(&equivalent).unwrap();
    let mut original =
        ResolvedSimulationSessionV3::new_with_service_boundaries(input.clone()).unwrap();
    let first = match original.advance_service().unwrap() {
        ResolvedServiceStepV3::BeforeService(p) => p,
        _ => panic!("prefix required"),
    };
    let mut controlled = ResolvedSimulationSessionV3::new_with_service_boundaries(input).unwrap();
    let actual_prefix = match controlled.advance_service().unwrap() {
        ResolvedServiceStepV3::BeforeService(p) => p,
        _ => panic!("prefix required"),
    };
    assert_eq!(actual_prefix, first);
    let saved = actual_prefix.clone();
    let mut bad = replacement.clone();
    bad.mu_scale = f64::NAN;
    assert!(controlled.resolve_service_boundary(Some(bad)).is_err());
    assert!(controlled.advance_service().is_err());
    assert_eq!(
        controlled.resolve_service_boundary(Some(replacement)),
        Ok(1)
    );
    let committed = match controlled.advance_service().unwrap() {
        ResolvedServiceStepV3::Lap(p) => p,
        _ => panic!("commit required"),
    };
    assert_eq!(committed.cumulative_time_s, first.cumulative_time_s + 20.0);
    assert!(committed.pit_stop_completed);
    assert_eq!(committed.state_after_lap.tire_wear, 0.0);
    assert_eq!(committed.end_speed_mps, None);
    assert_eq!(actual_prefix, saved);
    let actual = loop {
        match controlled.advance_service().unwrap() {
            ResolvedServiceStepV3::BeforeService(p) => {
                if p.lap_index == 4 {
                    assert!(
                        controlled
                            .resolve_service_boundary(Some(expected.applied_vehicle.tire.clone()))
                            .is_err()
                    );
                }
                controlled.resolve_service_boundary(None).unwrap();
            }
            ResolvedServiceStepV3::Lap(p) => assert!(
                !p.pit_stop_completed,
                "remaining original stop must have been replaced"
            ),
            ResolvedServiceStepV3::Complete(result) => break *result,
        }
    };
    assert_eq!(actual, expected);
    let mut normal = ResolvedSimulationSessionV3::new(equivalent).unwrap();
    assert!(normal.advance_service().is_err());
    assert!(normal.resolve_service_boundary(None).is_err());
    assert!(matches!(
        normal.advance().unwrap(),
        ResolvedSimulationStepV3::Lap(_)
    ));
}

fn energy_analytic_request(step_m: f64) -> ResolvedSimulationRequestV3 {
    use pitgun_racing_solver::longitudinal_energy::LongitudinalEnergyModelV1;
    let mut r = resolved_v3_request(&synthetic_request());
    let count = (100.0 / step_m).round() as usize + 1;
    let s = (0..count).map(|i| i as f64 * step_m).collect::<Vec<_>>();
    r.track = Track {
        s: s.clone(),
        x: s,
        y: vec![0.; count],
        z: vec![0.; count],
        kappa: vec![0.; count],
        slope: vec![0.; count],
        heading: vec![0.; count],
    };
    r.config.ds = step_m;
    r.config.max_speed = 400.;
    r.lap_count = 1;
    r.vehicle.chassis.mass_empty = 900.;
    r.state.fuel_mass = 100.;
    r.vehicle.chassis.r_wheel = 0.5;
    r.vehicle.chassis.rho = 1.;
    r.vehicle.chassis.c_rr = 0.;
    r.vehicle.chassis.mu0 = 100.;
    r.vehicle.engine.n_rpm = vec![0., 60000.];
    r.vehicle.engine.trq = vec![0.1, 0.1];
    r.vehicle.engine.gear_ratios = vec![10.];
    r.vehicle.engine.n_idle = 1000.;
    r.vehicle.engine.n_max = 60000.;
    r.vehicle.engine.alpha_heat = 0.;
    r.vehicle.engine.p_cool0 = 0.;
    r.vehicle.engine.k_cool = 0.;
    r.vehicle.engine.fuel_burn_kg_per_s = 0.;
    r.vehicle.tire.wear_per_s = 0.;
    r.vehicle.tire.wear_load_k = 0.;
    r.vehicle.tire.wear_min = 1.;
    r.vehicle.tire.temp_min_k = 1.;
    r.mechanical.driveline_efficiency = 1.;
    r.mechanical.chassis_force_transfer_efficiency = 1.;
    r.mechanical.fixed_drag_area_m2 = 0.;
    r.mechanical.fixed_downforce_area_m2 = 0.;
    r.driver_control = DriverControlParamsV3 {
        cornering_utilization: 1.,
        braking_utilization: 1.,
        traction_utilization: 1.,
        control_error: 0.,
    };
    r.tire_contact.heat_generation_fraction = 0.;
    r.tire_contact.baseline_wear_per_s = 0.;
    r.longitudinal_energy = Some(LongitudinalEnergyModelV1::V1);
    r
}

#[test]
fn energy_v1_constant_torque_matches_analytic_motion_and_work_on_four_meshes() {
    //100 Nm, total ratio10, r=0.5m, eta=1 ->2000N. m=1000kg ->a=2.
    for step in [1., 2., 5., 10.] {
        let r = run_resolved_simulation_v3(&energy_analytic_request(step)).unwrap();
        assert!(
            (r.total_time_s - 10.).abs() < 1e-7,
            "step{step}: {}",
            r.total_time_s
        );
        assert!((r.solution.v.last().unwrap() - 20.).abs() < 1e-7);
        let e = r.longitudinal_energy_v1.unwrap();
        assert!((e.kinetic_change_j - 200000.).abs() < 0.01);
        let lock = 1000. * std::f64::consts::TAU * 0.5 / (60. * 10.);
        assert!((e.clutch_slip_loss_j - 2000. * lock * lock / 4.).abs() < 0.01);
        assert!(e.vehicle_balance_residual_j().abs() < 1e-6);
        assert!((e.driving_time_s - r.total_time_s).abs() < 1e-10);
    }
}

#[test]
fn energy_v1_inclined_straight_uses_height_work_and_the_same_elapsed_time() {
    let mut input = energy_analytic_request(2.);
    input.track.z = input.track.s.iter().map(|s| s * 0.01).collect();
    input.track.slope.fill(0.01);
    let r = run_resolved_simulation_v3(&input).unwrap();
    let e = r.longitudinal_energy_v1.unwrap();
    let a: f64 = (2000. - 1000. * 9.81 * 0.01) / 1000.;
    let expected = (200. / a).sqrt();
    assert!((r.total_time_s - expected).abs() < 1e-7);
    assert!((e.potential_change_j - 9810.).abs() < 1e-6);
    assert!(e.vehicle_balance_residual_j().abs() < 1e-6);
}

#[test]
fn energy_v1_incremental_continuity_fuel_and_terminal_ledger_agree() {
    use pitgun_racing_solver::longitudinal_energy::{EnergyLedgerV1, LongitudinalEnergyModelV1};
    let mut input = resolved_v3_request(&synthetic_request());
    input.longitudinal_energy = Some(LongitudinalEnergyModelV1::V1);
    input.fuel_mass = Some(Default::default());
    let direct = run_resolved_simulation_v3(&input).unwrap();
    let mut session = ResolvedSimulationSessionV3::new(input.clone()).unwrap();
    let mut ledger = EnergyLedgerV1::default();
    let mut previous_speed = None;
    let terminal = loop {
        match session.advance().unwrap() {
            ResolvedSimulationStepV3::Lap(lap) => {
                if let Some(v) = previous_speed {
                    assert_eq!(Some(&v), lap.solution.v.first());
                }
                previous_speed = lap.solution.v.last().copied();
                ledger.accumulate(&lap.longitudinal_energy_v1.unwrap());
                assert_eq!(lap.solution.longitudinal_model, input.longitudinal_energy);
            }
            ResolvedSimulationStepV3::Complete(result) => break result,
        }
    };
    assert_eq!(*terminal, direct);
    assert_eq!(Some(ledger), direct.longitudinal_energy_v1);
    let fuel = direct.fuel_mass_diagnostics_v3.unwrap();
    assert!((fuel.engine_output_work_kj - ledger.engine_shaft_work_j / 1000.).abs() < 1e-8);
    let expected = ledger.engine_shaft_work_j / 3600000.
        * input
            .fuel_mass
            .unwrap()
            .brake_specific_fuel_consumption_kg_per_kwh
        + input.fuel_mass.unwrap().idle_fuel_flow_kg_per_s * ledger.driving_time_s;
    assert!((fuel.fuel_consumed_kg - expected).abs() < 1e-10);
    assert!(
        project_lap_power_v1(
            &direct.solution,
            &direct.applied_vehicle,
            input.mechanical,
            input.fuel_mass
        )
        .is_err()
    );
    assert!(
        pitgun_racing_solver::resample_telemetry(
            &input.track,
            &direct.solution,
            &direct.applied_vehicle,
            5.
        )
        .is_err()
    );
    let legacy = run_resolved_simulation_v3(&ResolvedSimulationRequestV3 {
        longitudinal_energy: None,
        thermal_physics_v2: None,
        driver_lapses_v1: None,
        ..input
    })
    .unwrap();
    let value = serde_json::to_value(&legacy).unwrap();
    assert!(value.get("longitudinal_energy_v1").is_none());
    assert!(value["solution"].get("longitudinal_model").is_none());
}

#[test]
fn energy_v1_service_has_its_own_clock_cooling_fuel_and_visible_velocity_reset() {
    use pitgun_racing_solver::longitudinal_energy::LongitudinalEnergyModelV1;
    let mut input = resolved_v3_request(&synthetic_request());
    input.longitudinal_energy = Some(LongitudinalEnergyModelV1::V1);
    input.fuel_mass = Some(Default::default());
    input.vehicle.engine.p_cool0 = 100.;
    input.pit_plan.stops.push(pitgun_racing_solver::PitStop {
        lap: 1,
        tire: input.vehicle.tire.clone(),
    });
    let r = run_resolved_simulation_v3(&input).unwrap();
    let e = r.longitudinal_energy_v1.unwrap();
    assert_eq!(e.service_time_s, input.config.pit_time_penalty_s);
    assert!((e.driving_time_s + e.service_time_s - r.total_time_s).abs() < 1e-9);
    assert!(e.unresolved_service_velocity_reset_j > 0.);
    assert!(
        e.boundary_balance_residual_j().abs() < 1e-6,
        "{}",
        e.boundary_balance_residual_j()
    );
    let f = input.fuel_mass.unwrap();
    let expected = e.engine_shaft_work_j / 3600000. * f.brake_specific_fuel_consumption_kg_per_kwh
        + (e.driving_time_s + e.service_time_s) * f.idle_fuel_flow_kg_per_s;
    assert!((r.fuel_mass_diagnostics_v3.unwrap().fuel_consumed_kg - expected).abs() < 1e-10);
    let heat_storage =
        input.vehicle.engine.c_th * (r.final_state.engine_temp - input.state.engine_temp);
    assert!(
        (heat_storage - (e.generated_engine_heat_j - e.rejected_engine_heat_signed_j)).abs() < 1e-6
    );
}

fn thermal_v2_request() -> ResolvedSimulationRequestV3 {
    use pitgun_racing_solver::{
        longitudinal_energy::LongitudinalEnergyModelV1, thermal_physics::*,
    };
    let mut r = resolved_v3_request(&synthetic_request());
    let profile: serde_json::Value = serde_json::from_str(include_str!(
        "../../../experiments/thermal_physics_v2/profile-v1.json"
    ))
    .unwrap();
    r.longitudinal_energy = Some(LongitudinalEnergyModelV1::V1);
    r.fuel_mass = Some(Default::default());
    r.tire_degradation = Some(Default::default());
    r.thermal_physics_v2 = Some(ThermalPhysicsV2 {
        schema_version: ThermalModelV2::V2,
        engine: serde_json::from_value(profile["engine"].clone()).unwrap(),
        initial_carcass_temperature_c: 80.0,
        pit_carcass_temperature_c: 70.0,
        road_temperature_c: 42.0,
        compounds: vec![TireCompoundHeatV2 {
            tire: r.vehicle.tire.clone(),
            heat: serde_json::from_value(profile["compounds"]["medium"]["heat"].clone()).unwrap(),
        }],
    });
    r
}

#[test]
fn thermal_v2_derating_reduces_delivered_work_and_power_against_analytic_launch() {
    use pitgun_racing_solver::thermal_physics::TireCompoundHeatV2;
    // Manufactured constant-torque engine:100Nm, ratio10, eta1, wheel0.5m,
    // mass1000kg. No road load or tire-force bottleneck. Hold engine temperature
    // to isolate thermal derating from changes to heat generation/radiator sizing.
    for mesh_m in [1.0, 5.0] {
        let mut previous_mean_power = f64::INFINITY;
        for factor in [1.0_f64, 0.9, 0.5] {
            let mut input = energy_analytic_request(mesh_m);
            let mut thermal = thermal_v2_request().thermal_physics_v2.unwrap();
            thermal.engine.coolant_waste_fraction = 0.0;
            thermal.engine.passive_conductance_w_per_k = 0.0;
            thermal.engine.radiator_idle_conductance_w_per_k = 0.0;
            thermal.engine.radiator_speed_conductance_w_per_k_mps = 0.0;
            let mut heat = thermal.compounds[0].heat;
            heat.deformation_resistance_coefficient = 0.0;
            thermal.compounds = vec![TireCompoundHeatV2 {
                tire: input.vehicle.tire.clone(),
                heat,
            }];
            input.vehicle.engine.t_soft = 100.0;
            input.vehicle.engine.beta_derate = 0.01;
            input.state.engine_temp = 100.0 + (1.0 - factor) / 0.01;
            input.engine_thermal = Some(Default::default());
            input.fuel_mass = Some(Default::default());
            input.tire_degradation = Some(Default::default());
            input.thermal_physics_v2 = Some(thermal);

            let result = run_resolved_simulation_v3(&input).unwrap();
            let ledger = result.longitudinal_energy_v1.unwrap();
            let acceleration = 2.0 * factor;
            let expected_time = (200.0 / acceleration).sqrt();
            let lock_speed = 1000.0 * std::f64::consts::TAU * 0.5 / (60.0 * 10.0);
            let expected_wheel_work = 200000.0 * factor;
            // Ideal slipping-clutch launch loses half*m*v_lock², independent of
            // force here; it must remain distinct from the derated wheel work.
            let expected_shaft_work = expected_wheel_work + 0.5 * 1000.0 * lock_speed.powi(2);
            let observed_power_integral = result
                .solution
                .t
                .windows(2)
                .zip(&result.solution.power)
                .map(|(t, power_kw)| 1000.0 * power_kw * (t[1] - t[0]))
                .sum::<f64>();
            assert!((result.total_time_s - expected_time).abs() < 1e-7);
            assert!((ledger.wheel_propulsion_work_j - expected_wheel_work).abs() < 0.01);
            assert!((ledger.engine_shaft_work_j - expected_shaft_work).abs() < 0.01);
            assert!((observed_power_integral - expected_shaft_work).abs() < 0.01);
            assert!(
                result
                    .solution
                    .engine_derating_factor
                    .iter()
                    .all(|v| (v - factor).abs() < 1e-12)
            );
            let mean_power = observed_power_integral / result.total_time_s;
            assert!((mean_power - expected_shaft_work / expected_time).abs() < 0.01);
            assert!(mean_power < previous_mean_power);
            previous_mean_power = mean_power;
        }
    }
}

#[test]
fn thermal_v2_laps_keep_carcass_state_and_commit_both_states_at_service() {
    let mut input = thermal_v2_request();
    input.pit_plan.stops.push(pitgun_racing_solver::PitStop {
        lap: 1,
        tire: input.vehicle.tire.clone(),
    });
    let direct = run_resolved_simulation_v3(&input).unwrap();
    let mut session = ResolvedSimulationSessionV3::new(input.clone()).unwrap();
    let mut previous = None;
    let mut chemical = 0.0;
    let mut emitted_laps = 0;
    loop {
        match session.advance().unwrap() {
            ResolvedSimulationStepV3::Lap(lap) => {
                emitted_laps += 1;
                assert_eq!(lap.lap_index, emitted_laps);
                let thermal = lap.thermal_v2.unwrap();
                assert_eq!(
                    lap.solution.thermal_model,
                    Some(input.thermal_physics_v2.as_ref().unwrap().schema_version)
                );
                assert_eq!(lap.solution.t.len(), lap.solution.tire_carcass_temp.len());
                if let Some(t) = previous {
                    assert_eq!(lap.solution.tire_carcass_temp[0], t);
                }
                previous = Some(thermal.carcass_temperature_c);
                chemical += thermal.ledger.fuel_chemical_energy_j;
                if lap.lap_index == 1 {
                    assert_eq!(thermal.carcass_temperature_c, 70.0);
                    assert_eq!(
                        lap.state_after_lap.tire_temp,
                        input.config.pit_tire_temp.unwrap_or(input.state.tire_temp)
                    );
                }
            }
            ResolvedSimulationStepV3::Complete(result) => {
                assert_eq!(*result, direct);
                break;
            }
        }
    }
    let thermal = direct.thermal_v2.unwrap();
    assert_eq!(emitted_laps, input.lap_count);
    assert_eq!(direct.lap_times_s.len(), usize::from(input.lap_count));
    assert_eq!(previous, Some(thermal.final_carcass_temperature_c));
    assert!((chemical - thermal.ledger.fuel_chemical_energy_j).abs() < 1e-6);
    let f = input.fuel_mass.unwrap();
    let e = direct.longitudinal_energy_v1.unwrap();
    let expected_fuel = e.engine_shaft_work_j / 3_600_000.0
        * f.brake_specific_fuel_consumption_kg_per_kwh
        + f.idle_fuel_flow_kg_per_s * direct.total_time_s;
    assert!(
        (expected_fuel
            * input
                .thermal_physics_v2
                .unwrap()
                .engine
                .fuel_lower_heating_value_j_per_kg
            - chemical)
            .abs()
            < 1e-5
    );
    assert!(thermal.ledger.maximum_engine_balance_residual_j < 1e-5);
    assert!(thermal.ledger.maximum_tire_balance_residual_j < 1e-5);
    assert!(thermal.ledger.maximum_relative_grip_coupling_error <= 1e-3);
    assert!(thermal.ledger.maximum_contact_utilization_upper_bound <= 1.0002);
}

#[test]
fn thermal_v2_rejects_missing_energy_compounds_and_impossible_efficiency() {
    let input = thermal_v2_request();
    let mut invalid = input.clone();
    invalid.longitudinal_energy = None;
    assert!(ResolvedSimulationSessionV3::new(invalid).is_err());
    let mut invalid = input.clone();
    invalid
        .thermal_physics_v2
        .as_mut()
        .unwrap()
        .compounds
        .clear();
    assert!(ResolvedSimulationSessionV3::new(invalid).is_err());
    let mut invalid = input.clone();
    invalid
        .fuel_mass
        .as_mut()
        .unwrap()
        .brake_specific_fuel_consumption_kg_per_kwh = 0.05;
    assert!(ResolvedSimulationSessionV3::new(invalid).is_err());
    let mut session = ResolvedSimulationSessionV3::new(input.clone()).unwrap();
    let mut unknown = input.vehicle.tire.clone();
    unknown.mu_scale += 0.01;
    assert!(session.request_next_lap_pit(unknown).is_err());
    assert!(session.advance().is_ok());
}

#[test]
fn thermal_v2_deformation_heat_costs_real_mechanical_work() {
    let mut input = energy_analytic_request(2.0);
    let profile = thermal_v2_request().thermal_physics_v2.unwrap();
    input.vehicle.chassis.c_rr = 0.02;
    input.vehicle.chassis.mu0 = 2.0;
    input.fuel_mass = Some(Default::default());
    input.tire_degradation = Some(Default::default());
    input.thermal_physics_v2 = Some(pitgun_racing_solver::thermal_physics::ThermalPhysicsV2 {
        compounds: vec![pitgun_racing_solver::thermal_physics::TireCompoundHeatV2 {
            tire: input.vehicle.tire.clone(),
            heat: profile.compounds[0].heat,
        }],
        ..profile
    });
    let loss = run_resolved_simulation_v3(&input).unwrap();
    input.thermal_physics_v2.as_mut().unwrap().compounds[0]
        .heat
        .deformation_resistance_coefficient = 0.0;
    let no_loss = run_resolved_simulation_v3(&input).unwrap();
    assert!(loss.total_time_s > no_loss.total_time_s);
    let e = loss.longitudinal_energy_v1.unwrap();
    let t = loss.thermal_v2.unwrap().ledger;
    assert!(t.tire_deformation_dissipation_j > 0.0);
    assert!(
        (t.tire_rolling_dissipation_j + t.tire_deformation_dissipation_j - e.rolling_work_j).abs()
            < 1e-6
    );
    assert!(e.maximum_vehicle_balance_residual_j < 1e-5);
    assert!(
        no_loss
            .thermal_v2
            .unwrap()
            .ledger
            .tire_deformation_dissipation_j
            < 1e-6
    );
}

#[test]
fn accounted_driver_corrections_pay_real_loss_heat_and_wear() {
    let mut input = thermal_v2_request();
    input.driver_correction_capacity_model =
        Some(DriverCorrectionCapacityModelV3::EnergyAccountedV1);
    input.driver_correction_workload_multiplier = Some(1.2);
    let result = run_resolved_simulation_v3(&input).unwrap();
    let thermal = result.thermal_v2.as_ref().unwrap().ledger;
    let energy = result.longitudinal_energy_v1.unwrap();
    let contact = result.tire_diagnostics_v3.unwrap();
    assert!(thermal.tire_deformation_dissipation_j > 0.0);
    assert!(contact.correction_contact_workload_mj.unwrap() > 0.0);
    assert!(contact.correction_generated_heat_kj.unwrap() > 0.0);
    assert!(
        result
            .tire_degradation_diagnostics_v3
            .unwrap()
            .requested_correction_wear_fraction
            .unwrap()
            > 0.0
    );
    assert!(
        (thermal.tire_rolling_dissipation_j + thermal.tire_deformation_dissipation_j
            - energy.rolling_work_j)
            .abs()
            < 1e-5
    );
    assert!(energy.maximum_vehicle_balance_residual_j < 1e-5);
    assert!(thermal.maximum_tire_balance_residual_j < 1e-5);
    assert!(thermal.maximum_contact_utilization_upper_bound <= 1.0002);
    let control = result.driver_control_diagnostics_v3.unwrap();
    assert!(
        (control.correction_force_capacity_fraction.unwrap() - 1.0 / 1.2_f64.sqrt()).abs() < 1e-12
    );
    input.driver_correction_workload_multiplier = Some(1.0);
    let clean = run_resolved_simulation_v3(&input).unwrap();
    assert_eq!(
        clean
            .tire_diagnostics_v3
            .unwrap()
            .correction_contact_workload_mj,
        Some(0.0)
    );
    assert_ne!(clean.total_time_s, result.total_time_s);
}

#[test]
fn driver_hesitations_change_the_solved_motion_and_replay_across_service() {
    use pitgun_racing_solver::ResolvedServiceStepV3;
    use pitgun_racing_solver::driver_lapses::{DriverLapseParamsV1, DriverLapseVersion};
    let mut input = thermal_v2_request();
    input.lap_count = 3;
    input.driver_correction_capacity_model =
        Some(DriverCorrectionCapacityModelV3::EnergyAccountedV1);
    input.driver_correction_workload_multiplier = Some(1.0);
    // Manufactured near-certain events, not the gameplay policy's rate.
    input.driver_lapses_v1 = Some(DriverLapseParamsV1 {
        schema_version: DriverLapseVersion::V1,
        noise_stream_id: "same-actor".into(),
        hazard_per_100km: 1000.0,
        exposure_cell_m: 500.0,
        recovery_distance_m: 250.0,
        minimum_engine_request_fraction: 0.15,
    });
    input.pit_plan.stops.push(pitgun_racing_solver::PitStop {
        lap: 1,
        tire: input.vehicle.tire.clone(),
    });
    let expected = run_resolved_simulation_v3(&input).unwrap();
    assert!(!expected.solution.driver_lapses_v1.is_empty());
    let mut unique = std::collections::BTreeSet::new();
    for event in &expected.solution.driver_lapses_v1 {
        assert!(unique.insert(event.exposure_index));
    }
    let restored: ResolvedSimulationRequestV3 =
        serde_json::from_slice(&serde_json::to_vec(&input).unwrap()).unwrap();
    assert_eq!(run_resolved_simulation_v3(&restored).unwrap(), expected);
    let mut session =
        ResolvedSimulationSessionV3::new_with_service_boundaries(input.clone()).unwrap();
    let mut committed = Vec::new();
    loop {
        match session.advance_service().unwrap() {
            ResolvedServiceStepV3::BeforeService(lap) => {
                committed.extend(lap.solution.driver_lapses_v1.clone());
                session.resolve_service_boundary(None).unwrap();
            }
            ResolvedServiceStepV3::Lap(lap) => assert!(lap.solution.driver_lapses_v1.is_empty()),
            ResolvedServiceStepV3::Complete(result) => {
                assert_eq!(*result, expected);
                break;
            }
        }
    }
    assert_eq!(committed, expected.solution.driver_lapses_v1);
    input.driver_lapses_v1.as_mut().unwrap().hazard_per_100km = 0.0;
    let clean = run_resolved_simulation_v3(&input).unwrap();
    assert!(clean.solution.driver_lapses_v1.is_empty());
    assert!(expected.total_time_s > clean.total_time_s);
    assert_ne!(expected.solution.v, clean.solution.v);
    assert!(
        expected
            .longitudinal_energy_v1
            .unwrap()
            .maximum_vehicle_balance_residual_j
            < 1e-5
    );
}

#[test]
fn energy_accounted_driver_fields_require_the_explicit_new_profile() {
    use pitgun_racing_solver::driver_lapses::{DriverLapseParamsV1, DriverLapseVersion};
    let mut r = thermal_v2_request();
    r.driver_correction_workload_multiplier = Some(1.1);
    r.driver_correction_capacity_model = Some(DriverCorrectionCapacityModelV3::FrictionBudgetV1);
    assert!(
        run_resolved_simulation_v3(&r)
            .unwrap_err()
            .contains("historical correction heat proxy")
    );
    r.driver_correction_capacity_model = Some(DriverCorrectionCapacityModelV3::EnergyAccountedV1);
    r.thermal_physics_v2 = None;
    assert!(run_resolved_simulation_v3(&r).is_err());
    r.driver_correction_capacity_model = None;
    r.driver_correction_workload_multiplier = None;
    r.driver_lapses_v1 = Some(DriverLapseParamsV1 {
        schema_version: DriverLapseVersion::V1,
        noise_stream_id: "test".into(),
        hazard_per_100km: 0.0,
        exposure_cell_m: 500.0,
        recovery_distance_m: 250.0,
        minimum_engine_request_fraction: 0.15,
    });
    assert!(run_resolved_simulation_v3(&r).is_err());
    let old = resolved_v3_request(&synthetic_request());
    let json = serde_json::to_value(&old).unwrap();
    assert!(json.get("driver_lapses_v1").is_none());
    let output = serde_json::to_value(run_resolved_simulation_v3(&old).unwrap()).unwrap();
    assert!(output["solution"].get("driver_lapses_v1").is_none());
    assert!(output["solution"].get("driver_lapse_profile_v1").is_none());
}

#[test]
fn fuel_domain_outcome_preserves_accepted_prefix_without_inventing_a_finish() {
    use pitgun_racing_solver::fuel_outcome::{
        FuelAwareSessionV1, FuelAwareStepV1, FuelShortfallPhaseV1,
    };
    let mut input = thermal_v2_request();
    input.lap_count = 3;
    let expected = run_resolved_simulation_v3(&input).unwrap();
    let mut session = FuelAwareSessionV1::new(input.clone()).unwrap();
    let complete = loop {
        match session.advance().unwrap() {
            FuelAwareStepV1::Lap { .. } => {}
            FuelAwareStepV1::Finished { result } => break result,
            other => panic!("unexpected {other:?}"),
        }
    };
    assert_eq!(*complete, expected);
    assert!(session.advance().is_err());
    input.state.fuel_mass = 0.000001;
    let legacy_error = run_resolved_simulation_v3(&input).unwrap_err();
    let mut session = FuelAwareSessionV1::new(input.clone()).unwrap();
    let outcome = session.advance().unwrap();
    match &outcome {
        FuelAwareStepV1::FuelNotCompleted {
            progress,
            shortfall,
        } => {
            assert_eq!(progress.completed_laps, 0);
            assert_eq!(progress.elapsed_s, 0.0);
            assert_eq!(progress.state, input.state);
            assert_eq!(shortfall.phase, FuelShortfallPhaseV1::DrivingLap);
            assert_eq!(shortfall.to_string(), legacy_error);
            assert!(shortfall.required_kg > shortfall.available_kg);
        }
        other => panic!("unexpected {other:?}"),
    }
    let decoded = serde_json::from_str(&serde_json::to_string(&input).unwrap()).unwrap();
    assert_eq!(
        FuelAwareSessionV1::new(decoded).unwrap().advance().unwrap(),
        outcome
    );
}

#[test]
fn fuel_outcome_distinguishes_service_shortfall_and_technical_failure() {
    use pitgun_racing_solver::fuel_outcome::{
        FuelAwareSessionV1, FuelAwareStepV1, FuelShortfallPhaseV1,
    };
    let mut input = thermal_v2_request();
    input.state.fuel_mass = 2.0;
    input.pit_plan.stops.push(pitgun_racing_solver::PitStop {
        lap: 1,
        tire: input.vehicle.tire.clone(),
    });
    input.config.pit_time_penalty_s = 20000.0;
    let legacy_error = run_resolved_simulation_v3(&input).unwrap_err();
    match FuelAwareSessionV1::new(input.clone())
        .unwrap()
        .advance()
        .unwrap()
    {
        FuelAwareStepV1::FuelNotCompleted {
            progress,
            shortfall,
        } => {
            assert_eq!(shortfall.phase, FuelShortfallPhaseV1::Service);
            assert_eq!(shortfall.to_string(), legacy_error);
            assert_eq!(progress.completed_laps, 0); // Incomplete service never counts as a complete lap boundary.
        }
        other => panic!("unexpected {other:?}"),
    }
    input.pit_plan.stops.clear();
    input.vehicle.engine.trq.fill(0.0);
    assert!(FuelAwareSessionV1::new(input).unwrap().advance().is_err());
}

#[test]
fn invariant_lap_forecast_cache_preserves_complete_result_and_service_continuity() {
    use pitgun_racing_solver::lap_boundary::{
        BoundaryDiagnosticRequestV1, BoundaryDiagnosticVersionV1, BoundaryForecastV1,
    };
    let mut input = thermal_v2_request();
    input.lap_count = 3;
    input.pit_plan.stops.push(pitgun_racing_solver::PitStop {
        lap: 1,
        tire: input.vehicle.tire.clone(),
    });
    let expected = run_resolved_simulation_v3(&input).unwrap();
    let run = |recompute, service| {
        let request = BoundaryDiagnosticRequestV1 {
            schema_version: BoundaryDiagnosticVersionV1::V1,
            forecast: BoundaryForecastV1::MinimumGrip,
            recompute_invariant: recompute,
            input: input.clone(),
        };
        let mut session =
            ResolvedSimulationSessionV3::new_boundary_diagnostic(request, service).unwrap();
        let result = loop {
            if service {
                match session.advance_service().unwrap() {
                    pitgun_racing_solver::ResolvedServiceStepV3::BeforeService(_) => {
                        session.resolve_service_boundary(None).unwrap();
                    }
                    pitgun_racing_solver::ResolvedServiceStepV3::Lap(_) => {}
                    pitgun_racing_solver::ResolvedServiceStepV3::Complete(result) => break result,
                }
            } else {
                match session.advance().unwrap() {
                    ResolvedSimulationStepV3::Lap(_) => {}
                    ResolvedSimulationStepV3::Complete(result) => break result,
                }
            }
        };
        (*result, session.boundary_work().unwrap())
    };
    let (cached, work) = run(false, false);
    let (recomputed, repeated) = run(true, false);
    let (service, service_work) = run(false, true);
    assert_eq!(cached, expected);
    assert_eq!(recomputed, expected);
    assert_eq!(service, expected);
    assert_eq!(work, service_work);
    assert_eq!(work.laps.len(), 2);
    assert!(work.laps.iter().all(|l| l.forecast_evaluations == 1));
    assert!(work.laps.iter().map(|l| l.cache_hits).sum::<u32>() > 0);
    assert!(repeated.laps.iter().all(|l| l.cache_hits == 0));
    assert!(
        repeated
            .laps
            .iter()
            .map(|l| l.forecast_evaluations)
            .sum::<u32>()
            > 2
    );
    assert!(
        ResolvedSimulationSessionV3::new(input)
            .unwrap()
            .boundary_work()
            .is_none()
    );
}

#[test]
fn reachable_forecast_certifies_before_commit_preserves_prefix_and_replays_service() {
    use pitgun_racing_solver::lap_boundary::{
        BoundaryDiagnosticRequestV1, BoundaryDiagnosticVersionV1, BoundaryForecastV1,
    };
    let mut input = thermal_v2_request();
    input.lap_count = 4;
    input.pit_plan.stops.push(pitgun_racing_solver::PitStop {
        lap: 2,
        tire: input.vehicle.tire.clone(),
    });
    let request = BoundaryDiagnosticRequestV1 {
        schema_version: BoundaryDiagnosticVersionV1::V1,
        forecast: BoundaryForecastV1::ReachableWearV1,
        recompute_invariant: false,
        input,
    };
    let run = |service: bool| {
        let decoded = serde_json::from_str(&serde_json::to_string(&request).unwrap()).unwrap();
        let mut s = ResolvedSimulationSessionV3::new_boundary_diagnostic(decoded, service).unwrap();
        let mut previous = None;
        let mut prefix = None;
        let mut service_solution = None;
        loop {
            let step = if service {
                match s.advance_service().unwrap() {
                    pitgun_racing_solver::ResolvedServiceStepV3::BeforeService(lap) => {
                        service_solution = Some(lap.solution);
                        s.resolve_service_boundary(None).unwrap();
                        continue;
                    }
                    pitgun_racing_solver::ResolvedServiceStepV3::Lap(mut l) => {
                        assert!(l.solution.v.is_empty());
                        l.solution = service_solution.take().unwrap();
                        ResolvedSimulationStepV3::Lap(l)
                    }
                    pitgun_racing_solver::ResolvedServiceStepV3::Complete(r) => {
                        ResolvedSimulationStepV3::Complete(r)
                    }
                }
            } else {
                s.advance().unwrap()
            };
            match step {
                ResolvedSimulationStepV3::Lap(l) => {
                    if let Some((speed, ts, tb, wear)) = previous {
                        assert_eq!(l.solution.v[0], speed);
                        assert_eq!(l.solution.tire_temp[0], ts);
                        assert_eq!(l.solution.tire_carcass_temp[0], tb);
                        assert_eq!(l.solution.tire_wear[0], wear);
                    }
                    previous = Some((
                        l.end_speed_mps.unwrap_or(0.0),
                        l.state_after_lap.tire_temp,
                        l.thermal_v2.unwrap().carcass_temperature_c,
                        l.state_after_lap.tire_wear,
                    ));
                    if prefix.is_none() {
                        prefix = Some(l.solution.clone());
                    }
                }
                ResolvedSimulationStepV3::Complete(r) => {
                    let p = prefix.unwrap();
                    assert_eq!(&r.solution.v[..p.v.len()], p.v.as_slice());
                    assert_eq!(
                        &r.solution.tire_temp[..p.tire_temp.len()],
                        p.tire_temp.as_slice()
                    );
                    let w = s.boundary_work().unwrap();
                    assert!(
                        w.laps
                            .iter()
                            .filter_map(|l| l.reachable.as_ref())
                            .all(|l| l.service_reset
                                || l.committed_end_speed_mps <= l.certificate_cap_mps + 1e-9)
                    );
                    return (*r, w);
                }
            }
        }
    };
    assert_eq!(run(false), run(true));
}

#[test]
fn held_state_diagnostic_requires_thermal_profile_and_agrees_at_constant_grip() {
    use pitgun_racing_solver::lap_boundary::{
        BoundaryDiagnosticRequestV1, BoundaryDiagnosticVersionV1, BoundaryForecastV1,
    };
    let mut input = thermal_v2_request();
    input.lap_count = 3;
    input.vehicle.tire.temp_min_k = 1.0;
    input.vehicle.tire.wear_min = 1.0;
    input.thermal_physics_v2.as_mut().unwrap().compounds[0].tire = input.vehicle.tire.clone();
    let expected = run_resolved_simulation_v3(&input).unwrap();
    let request = BoundaryDiagnosticRequestV1 {
        schema_version: BoundaryDiagnosticVersionV1::V1,
        forecast: BoundaryForecastV1::EndStateHeldDiagnostic,
        recompute_invariant: false,
        input: input.clone(),
    };
    let restored = serde_json::from_str(&serde_json::to_string(&request).unwrap()).unwrap();
    let mut session =
        ResolvedSimulationSessionV3::new_boundary_diagnostic(restored, false).unwrap();
    let mut end = None;
    loop {
        match session.advance().unwrap() {
            ResolvedSimulationStepV3::Lap(lap) => {
                if let Some(previous) = end {
                    assert_eq!(lap.solution.v[0], previous);
                }
                end = lap.solution.v.last().copied();
            }
            ResolvedSimulationStepV3::Complete(result) => {
                assert_eq!(*result, expected);
                break;
            }
        }
    }
    input.thermal_physics_v2 = None;
    let request = BoundaryDiagnosticRequestV1 {
        schema_version: BoundaryDiagnosticVersionV1::V1,
        forecast: BoundaryForecastV1::EndStateHeldDiagnostic,
        recompute_invariant: false,
        input,
    };
    assert!(ResolvedSimulationSessionV3::new_boundary_diagnostic(request, false).is_err());
}

#[test]
fn absent_thermal_candidate_does_not_add_historical_fields() {
    let input = resolved_v3_request(&synthetic_request());
    let input_json = serde_json::to_value(&input).unwrap();
    assert!(input_json.get("thermal_physics_v2").is_none());
    let result = serde_json::to_value(run_resolved_simulation_v3(&input).unwrap()).unwrap();
    assert!(result.get("thermal_v2").is_none());
    assert!(result["solution"].get("thermal_model").is_none());
    assert!(result["solution"].get("tire_carcass_temp").is_none());
}

#[test]
fn thermal_v2_service_boundary_reports_driving_before_committing_replacement() {
    use pitgun_racing_solver::ResolvedServiceStepV3;
    let mut input = thermal_v2_request();
    input.pit_plan.stops.push(pitgun_racing_solver::PitStop {
        lap: 1,
        tire: input.vehicle.tire.clone(),
    });
    let direct = run_resolved_simulation_v3(&input).unwrap();
    let mut session = ResolvedSimulationSessionV3::new_with_service_boundaries(input).unwrap();
    let mut before_first_service = None;
    loop {
        match session.advance_service().unwrap() {
            ResolvedServiceStepV3::BeforeService(lap) => {
                assert_eq!(
                    lap.thermal_v2.unwrap().carcass_temperature_c,
                    *lap.solution.tire_carcass_temp.last().unwrap()
                );
                if lap.lap_index == 1 {
                    before_first_service = Some(lap.thermal_v2.unwrap());
                }
                session.resolve_service_boundary(None).unwrap();
            }
            ResolvedServiceStepV3::Lap(lap) if lap.lap_index == 1 => {
                let before = before_first_service.unwrap();
                let after = lap.thermal_v2.unwrap();
                assert_eq!(after.carcass_temperature_c, 70.0);
                assert!(after.ledger.fuel_chemical_energy_j > before.ledger.fuel_chemical_energy_j);
                assert_eq!(
                    after.ledger.tire_generated_heat_j,
                    before.ledger.tire_generated_heat_j
                );
            }
            ResolvedServiceStepV3::Lap(_) => {}
            ResolvedServiceStepV3::Complete(result) => {
                assert_eq!(*result, direct);
                break;
            }
        }
    }
}
