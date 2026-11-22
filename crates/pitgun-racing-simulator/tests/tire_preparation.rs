#[path = "../examples/support/frozen_thermal_input.rs"]
mod frozen;
use pitgun_contract::canonical_json_digest;
use pitgun_racing_simulator::{
    RacingCatalogSnapshot,
    tire_preparation::{TirePreparationPolicyV1, prepare_tires_v1},
};
use pitgun_racing_solver::{ResolvedSimulationSessionV3, ResolvedSimulationStepV3};
use std::path::Path;

fn policy() -> TirePreparationPolicyV1 {
    serde_json::from_str(include_str!(
        "../../../experiments/tyre_operating_policy_v1/profile-v1.json"
    ))
    .unwrap()
}
#[test]
fn preparation_is_explicit_atomic_and_does_not_inherit_a_diagnostic_start_at_pit() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let catalog =
        RacingCatalogSnapshot::from_release_dir(root.join("catalogs/racing/v1.8.1")).unwrap();
    let (source, _) = frozen::frozen_input(&root, "soft-medium-pit10", &catalog).unwrap();
    let original = source.clone();
    let p = policy();
    let (prepared, receipt) = prepare_tires_v1(&source, &p).unwrap();
    assert_eq!(source, original);
    assert_eq!(
        receipt.source_input_digest,
        canonical_json_digest(&source).unwrap()
    );
    assert_eq!(
        receipt.prepared_input_digest,
        canonical_json_digest(&prepared).unwrap()
    );
    assert_eq!(receipt.policy_digest, canonical_json_digest(&p).unwrap());
    let mut expected = original.clone();
    expected.state.tire_temp = 70.0;
    expected.config.pit_tire_temp = Some(70.0);
    let thermal = expected.thermal_physics_v2.as_mut().unwrap();
    thermal.initial_carcass_temperature_c = 70.0;
    thermal.pit_carcass_temperature_c = 70.0;
    assert_eq!(prepared, expected);
    let mut invalid = p.clone();
    invalid.mount_surface_temperature_c = 80.0;
    assert!(prepare_tires_v1(&source, &invalid).is_err());
    let mut historical = source.clone();
    historical.thermal_physics_v2 = None;
    assert!(prepare_tires_v1(&historical, &p).is_err());

    // A cold grid diagnostic must not silently make the replacement set cold.
    let mut cold = prepared;
    cold.state.tire_temp = 35.0;
    cold.thermal_physics_v2
        .as_mut()
        .unwrap()
        .initial_carcass_temperature_c = 35.0;
    cold.lap_count = 2;
    cold.pit_plan.stops[0].lap = 1;
    let mut session = ResolvedSimulationSessionV3::new(cold).unwrap();
    let ResolvedSimulationStepV3::Lap(first) = session.advance().unwrap() else {
        panic!("lap")
    };
    assert!(first.pit_stop_completed);
    assert_eq!(first.solution.tire_temp[0], 35.0);
    assert_eq!(first.state_after_lap.tire_temp, 70.0);
    assert_eq!(first.thermal_v2.unwrap().carcass_temperature_c, 70.0);
    let ResolvedSimulationStepV3::Lap(second) = session.advance().unwrap() else {
        panic!("lap")
    };
    assert_eq!(second.solution.tire_temp[0], 70.0);
    assert_eq!(second.solution.tire_carcass_temp[0], 70.0);
}
