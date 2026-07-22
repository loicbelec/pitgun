use pitgun_racing_solver::{
    AeroParams, ChassisParams, Driver, EngineParams, PitPlan, SimConfig, SimulationRequest,
    TireParams, Track, VehicleParams, VehicleState, run_simulation,
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
