//! Explicit I/II bridge from the frozen native candidate to incremental Racing.
//! Catalog resources select the physics. No coefficient, seed or pace fitting here.
use crate::thermal_career::{ThermalCareerPolicyV1, resolve_thermal_career_v1};
use crate::thermal_driver::{
    ThermalDriverPolicyV1, apply_thermal_driver_v1, resolve_thermal_driver_resource_v1,
};
use crate::tire_performance::{TirePerformanceProfileV1, compile_tire_performance_v1};
use crate::*;
use pitgun_contract::Digest;
use pitgun_racing_solver::lap_boundary::{
    BoundaryDiagnosticRequestV1, BoundaryDiagnosticVersionV1, BoundaryForecastV1,
};

const THREE_COMPOUND_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../catalogs/racing/v1.10.2/simulation/qualified-career/profile-v1.json"
));

const PROFILE_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../catalogs/racing/v1.10.1/simulation/qualified-career/profile-v1.json"
));

#[derive(Clone, Deserialize)]
pub struct QualifiedCareerProfile {
    pub schema_version: String,
    pub source_policy_digest: Digest,
    pub supported_eras: Vec<i32>,
    pub base_vehicle_id: String,
    pub initial_fuel_mass_kg: f64,
    pub thermal: ThermalCareerPolicyV1,
    pub tires: TirePerformanceProfileV1,
    #[serde(default)]
    pub tire_operating: Option<crate::tire_operating::TireOperatingPolicyV1>,
    pub roster: BTreeMap<String, QualifiedPerson>,
    pub contexts: Vec<QualifiedContext>,
    pub derating_events: physical_events::DeratingEventPolicyV1,
    pub tire_events: tire_events::TireWindowPolicyV2,
    pub installed_component_baseline_by_era: BTreeMap<String, QualifiedComponents>,
}
#[derive(Clone, Deserialize)]
pub struct QualifiedPerson {
    pub name: String,
    pub resource: RacingDriverResourceV2,
    pub mode: RacingDrivingMode,
}
#[derive(Clone, Deserialize)]
pub struct QualifiedContext {
    pub era: i32,
    pub circuit: String,
    pub cases: Vec<QualifiedPreparation>,
}
#[derive(Clone, Deserialize)]
pub struct QualifiedPreparation {
    pub actor: String,
    pub allocation: [u16; 4],
    pub budget: u16,
    pub gear: f64,
    pub downforce: f64,
    pub strategy: CompetitorStintStrategy,
}
#[derive(Clone, Deserialize)]
pub struct QualifiedComponents {
    pub engine_id: String,
    pub offsets: [u16; 4],
}

pub fn model_identity() -> ArtifactIdentity {
    let manifest = format!(
        "pitgun.racing-v3-candidate:0.16.1:energy-v1:gear-threshold-8eps-v1:thermal-v2:reachable-wear-v1:fixed-nine-v3:accepted-energy-power-v1:causal-export-v1:peak-grip-window-v2:{}",
        Digest::from_bytes(PROFILE_BYTES)
    );
    ArtifactIdentity {
        id: "pitgun.racing-v3-candidate".parse().unwrap(),
        version: "0.16.1".parse().unwrap(),
        digest: Digest::from_bytes(manifest.as_bytes()),
    }
}
/// Explicit local preview identity. Signed/leaderboard admission is separate.
pub fn three_compound_model_identity() -> ArtifactIdentity {
    let manifest = format!(
        "pitgun.racing-v3-candidate:0.16.2:three-compound-operating-v1:energy-gear-8eps:reachable-wear:readable-power-v3:{}",
        Digest::from_bytes(THREE_COMPOUND_BYTES)
    );
    ArtifactIdentity {
        id: "pitgun.racing-v3-candidate".parse().unwrap(),
        version: "0.16.2".parse().unwrap(),
        digest: Digest::from_bytes(manifest.as_bytes()),
    }
}
pub fn model_identity_for(snapshot: &RacingCatalogSnapshot) -> Result<ArtifactIdentity, String> {
    profile(snapshot)?;
    Ok(
        if snapshot.qualified_career_profile_bytes() == Some(THREE_COMPOUND_BYTES) {
            three_compound_model_identity()
        } else {
            model_identity()
        },
    )
}
pub fn three_compound_profile_identity() -> ArtifactIdentity {
    ArtifactIdentity {
        id: "pitgun.racing.qualified-career.profile-v1".parse().unwrap(),
        version: "1.1.0".parse().unwrap(),
        digest: canonical_json_digest(
            &serde_json::from_slice::<Value>(THREE_COMPOUND_BYTES).unwrap(),
        )
        .unwrap(),
    }
}
pub fn compile_tires(
    snapshot: &RacingCatalogSnapshot,
    p: &QualifiedCareerProfile,
) -> Result<crate::tire_performance::CompiledTirePerformanceV1, String> {
    match &p.tire_operating {
        Some(policy) => Ok(crate::tire_operating::compile_tire_operating_v1(
            snapshot, &p.thermal, policy,
        )?
        .performance),
        None => compile_tire_performance_v1(snapshot, &p.thermal, &p.tires),
    }
}

pub fn profile_identity() -> ArtifactIdentity {
    ArtifactIdentity {
        id: "pitgun.racing.qualified-career.profile-v1".parse().unwrap(),
        version: "1.0.0".parse().unwrap(),
        digest: canonical_json_digest(&serde_json::from_slice::<Value>(PROFILE_BYTES).unwrap())
            .unwrap(),
    }
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn qualified_career_metadata_json() -> String {
    serialize_json(
        &serde_json::json!({"model":model_identity(),"profile":profile_identity(),
        "control_policy":crate::service_control::qualified_service_control_policy_identity(),
        "telemetry_policy":crate::power_stream::energy_power_telemetry_policy_identity(),
        "physics":"pitgun.local-qualified-career/v1","scope":"local I/II candidate, not deployed"}),
    )
}
/// Local unranked preview metadata. Public signed-service admission stays closed.
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn three_compound_career_metadata_json() -> String {
    let control = crate::service_control::three_compound_local_control_policy_identity();
    serialize_json(
        &serde_json::json!({"model":three_compound_model_identity(),"profile":three_compound_profile_identity(),"control_policy":control,
        "telemetry_policy":crate::power_stream::readable_power_telemetry_policy_identity(),"physics":"pitgun.local-qualified-career/v1",
        "scope":"local I/II three-compound preview; provisional AI; not leaderboard admitted","signed_service_admitted":false}),
    )
}

pub fn profile(snapshot: &RacingCatalogSnapshot) -> Result<QualifiedCareerProfile, String> {
    let bytes = snapshot
        .qualified_career_profile_bytes()
        .ok_or("qualified career profile missing")?;
    if bytes != PROFILE_BYTES && bytes != THREE_COMPOUND_BYTES {
        return Err("qualified career profile differs from the compiled candidate".into());
    }
    let p: QualifiedCareerProfile = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    if p.schema_version != "pitgun.racing-qualified-career/v1"
        || p.supported_eras != [1, 2]
        || p.roster.len() != 9
    {
        return Err("qualified career schema/scope/roster mismatch".into());
    }
    p.thermal.validate()?;
    p.tires.validate()?;
    if (bytes == THREE_COMPOUND_BYTES) != p.tire_operating.is_some() {
        return Err("operating policy/profile identity mismatch".into());
    }
    if p.tire_operating
        .as_ref()
        .is_some_and(|t| t.performance != p.tires)
    {
        return Err("conflicting tire performance policy".into());
    }
    Ok(p)
}

/// Resolve exact public inputs. The player's setup/components remain their own.
/// Short practices retain each rival's opening compound, with no mandatory stop.
pub fn prepare_request(
    mut request: RunRaceRequest,
    snapshot: &RacingCatalogSnapshot,
) -> Result<RunRaceRequest, String> {
    let p = profile(snapshot)?;
    // The qualified field was measured on the catalog's physical geometry and
    // service loss. Presentation-path reconstruction must not replace either.
    if request.input.track_profile.is_some()
        || request
            .input
            .pit_strategy
            .as_ref()
            .is_some_and(|plan| plan.pit_loss_ms.is_some())
    {
        return Err("qualified career requires catalog geometry and pit loss".into());
    }
    let era = request.era.unwrap_or(request.input.era);
    let context = p
        .contexts
        .iter()
        .find(|c| {
            c.era == era
                && normalize_track_id(&c.circuit)
                    == normalize_track_id(&request.input.race.track_id)
        })
        .ok_or("qualified career supports only the recorded I/II circuit contexts")?;
    if request.input.vehicle_id.as_deref() != Some(p.base_vehicle_id.as_str()) {
        return Err("qualified career baseline vehicle mismatch".into());
    }
    let players: Vec<_> = request
        .input
        .race
        .competitors
        .iter()
        .filter(|c| c.is_player)
        .cloned()
        .collect();
    if players.len() != 1 || players[0].id != "player" {
        return Err("qualified career requires the explicit player identity".into());
    }
    let full_laps: u16 = context.cases[0]
        .strategy
        .stints
        .iter()
        .map(|s| s.laps)
        .sum();
    let laps = request.input.race.laps;
    if laps == 0 || laps > full_laps || (laps != full_laps && laps > 5) {
        return Err("qualified career requires a complete GP or1..5practice laps".into());
    }
    let mut player = players[0].clone();
    player.driver_id = Some("balanced_reference".into());
    let mut cars = vec![player];
    request
        .input
        .competitor_vehicle_components
        .retain(|id, _| id == "player");
    for case in &context.cases {
        let person = &p.roster[&case.actor];
        let base = &p.installed_component_baseline_by_era[&era.to_string()];
        let points: Vec<f64> = case
            .allocation
            .iter()
            .zip(base.offsets)
            .map(|(a, b)| f64::from(a + b))
            .collect();
        let strategy = if laps == full_laps {
            case.strategy.clone()
        } else {
            serde_json::from_value(serde_json::json!({"stints":[{"tire_id":case.strategy.stints[0].tire_id,"laps":laps}],"pit_laps":[]})).map_err(|e|e.to_string())?
        };
        let competitor: CompetitorSpec = serde_json::from_value(serde_json::json!({
            "id":case.actor,"name":person.name,"team_id":format!("FIXED_{}",case.actor),"is_player":false,
            "driver_id":"balanced_reference","budget_cap":points.iter().sum::<f64>(),
            "tuning":{"aero_points":points[0],"chassis_points":points[1],"engine_points":points[2],"cooling_points":points[3],"gear_ratio_slider":case.gear,"downforce_slider":case.downforce},"stint_strategy":strategy
        })).map_err(|e| e.to_string())?;
        if era == 2 {
            let selection = serde_json::from_value(serde_json::json!({"schema_version":"pitgun.racing-vehicle-components/v1","engine_id":base.engine_id,"aero_id":"basic"})).map_err(|e|e.to_string())?;
            request
                .input
                .competitor_vehicle_components
                .insert(case.actor.clone(), selection);
        }
        cars.push(competitor);
    }
    request.input.race.competitors = cars;
    request.input.competitor_profiles.clear();
    request.input.era = era;
    request.input.initial_fuel_mass_kg = Some(p.initial_fuel_mass_kg);
    Ok(request)
}

/// Starts the already qualified resolved inputs inside the ordinary stream host.
/// No race is run during preparation, and no accepted prefix is rewritten.
pub fn resolve_inputs(
    request: &RunRaceRequest,
    snapshot: &RacingCatalogSnapshot,
) -> Result<BTreeMap<String, ResolvedSimulationRequestV3>, String> {
    let p = profile(snapshot)?;
    let driver_policy = ThermalDriverPolicyV1::from_catalog(snapshot)?;
    let field = resolve_thermal_career_v1(
        request,
        snapshot,
        &p.thermal,
        &driver_policy,
        &BTreeMap::new(),
    )?;
    let tires = compile_tires(snapshot, &p)?;
    field
        .competitors
        .into_iter()
        .map(|(id, car)| {
            let mut input = car.input;
            if id != "player" {
                let person = p.roster.get(&id).ok_or("unknown fixed driver")?;
                let driver = resolve_thermal_driver_resource_v1(
                    &person.resource,
                    person.mode,
                    &driver_policy,
                    &format!("fixed-field:{id}"),
                )?;
                apply_thermal_driver_v1(&mut input, &driver)?;
            }
            let competitor = request
                .input
                .race
                .competitors
                .iter()
                .find(|c| c.id == id)
                .ok_or("missing competitor")?;
            let strategy = competitor
                .stint_strategy
                .as_ref()
                .ok_or("qualified career requires explicit stint identities")?;
            let ids: Vec<_> = strategy.stints.iter().map(|s| s.tire_id.clone()).collect();
            tires.apply_initial_to_input(&mut input, &ids)?;
            Ok((id, input))
        })
        .collect()
}

pub fn start(
    request: RunRaceRequest,
    snapshot: &RacingCatalogSnapshot,
    service: bool,
) -> Result<IncrementalRacingSession, String> {
    let p = profile(snapshot)?;
    // Canonical input must already contain the fixed field before signing/replay.
    let prepared = prepare_request(request.clone(), snapshot)?;
    if canonical_json_digest(&prepared.input).map_err(|e| e.to_string())?
        != canonical_json_digest(&request.input).map_err(|e| e.to_string())?
    {
        return Err("qualified career input must be prepared before execution".into());
    }
    let mut inputs = resolve_inputs(&request, snapshot)?;
    let baseline_profile = V3CandidateExperimentProfile {
        schema_version: V3CandidateExperimentProfileVersion::V9,
        engine_thermal_resolution: None,
        ..Default::default()
    };
    let mut session = start_incremental_race_with_catalog_and_v3_component_profile(
        request,
        snapshot,
        &baseline_profile,
        snapshot
            .power_unit_thermal_profile()
            .ok_or("missing baseline power unit profile")?,
        None,
        service,
    )?;
    for car in &mut session.competitors {
        let input = inputs
            .remove(&car.competitor_id)
            .ok_or("missing qualified physical input")?;
        car.physical_vehicle = input.vehicle.clone();
        car.engine_thermal = input.engine_thermal;
        car.power_projection_mechanical = input.mechanical;
        car.power_projection_fuel = input.fuel_mass;
        car.physics_audit_context = None;
        car.request.vehicle = input.vehicle.clone();
        car.request.state = input.state.clone();
        car.request.config = input.config.clone();
        car.request.pit_plan = input.pit_plan.clone();
        car.request.driver = input.driver.clone();
        car.solver = ResolvedSimulationSessionV3::new_boundary_diagnostic(
            BoundaryDiagnosticRequestV1 {
                schema_version: BoundaryDiagnosticVersionV1::V1,
                forecast: BoundaryForecastV1::ReachableWearV1,
                recompute_invariant: false,
                input,
            },
            service && car.is_player,
        )?;
        car.physical_event_detector = Some(physical_events::NativeDeratingEventDetector::new(
            car.competitor_id.clone(),
            p.derating_events.clone(),
        )?);
        car.tire_event_detector = Some(
            tire_events::NativeTireWindowEventDetector::new_with_peak_grip_window(
                car.competitor_id.clone(),
                p.tire_events.clone(),
            )?,
        );
    }
    session.player_power_unit_thermal_resolution_v3 = None;
    session.competitor_power_unit_thermal_resolutions_v3.clear();
    session.align_player_horizon = true;
    session.enable_power_telemetry_v1()?;
    session.readable_power_telemetry = p.tire_operating.is_some();
    Ok(session)
}

pub fn run(
    request: RunRaceRequest,
    snapshot: &RacingCatalogSnapshot,
) -> Result<RaceOutput, String> {
    start(request, snapshot, false)?.collect()
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn run_qualified_career_with_catalog_json(input_json: String, catalog_json: String) -> String {
    let result = (|| {
        let snapshot =
            RacingCatalogSnapshot::from_bundle_json(&catalog_json).map_err(|e| e.to_string())?;
        run(parse_run_race_request(&input_json)?, &snapshot)
    })();
    match result {
        Ok(output) => serialize_json(&output),
        Err(e) => json_error(&e),
    }
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn prepare_qualified_career_request_json(input_json: String, catalog_json: String) -> String {
    let result = (|| {
        let snapshot =
            RacingCatalogSnapshot::from_bundle_json(&catalog_json).map_err(|e| e.to_string())?;
        prepare_request(parse_run_race_request(&input_json)?, &snapshot)
    })();
    match result {
        Ok(request) => serialize_json(&request),
        Err(e) => json_error(&e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn operating_catalog_inputs_equal_the_frozen_offline_compilation() {
        let old = RacingCatalogSnapshot::embedded_qualified_career().unwrap();
        let next = RacingCatalogSnapshot::embedded_three_compound().unwrap();
        assert_eq!(
            model_identity().digest.to_string(),
            "sha256:fcce2427ec1f0d27b5d8a04be8664f1c90ea8be222ba988028b6080cdb2b6abe"
        );
        assert_eq!(model_identity_for(&old).unwrap(), model_identity());
        assert_eq!(
            model_identity_for(&next).unwrap(),
            three_compound_model_identity()
        );
        let candidate = profile(&next).unwrap();
        let compiled = compile_tires(&next, &candidate).unwrap();
        let fixtures: Value = serde_json::from_str(include_str!(
            "../../../experiments/opponent_physics_v2/results/career-fixtures-v1.json"
        ))
        .unwrap();
        for id in [
            "craft_engineering:era1-entry",
            "craft_engineering:era2-entry",
            "craft_engineering:era2-exit",
        ] {
            let f = fixtures["fixtures"]
                .as_array()
                .unwrap()
                .iter()
                .find(|f| f["id"] == id)
                .unwrap();
            let req: RunRaceRequest =
                serde_json::from_value(f["preparedRequests"][0]["request"].clone()).unwrap();
            let req = prepare_request(req, &old).unwrap();
            let mut baseline = resolve_inputs(&req, &old).unwrap();
            let actual = resolve_inputs(&req, &next).unwrap();
            for (actor, input) in &mut baseline {
                let c = req
                    .input
                    .race
                    .competitors
                    .iter()
                    .find(|c| c.id == *actor)
                    .unwrap();
                let ids: Vec<_> = c
                    .stint_strategy
                    .as_ref()
                    .unwrap()
                    .stints
                    .iter()
                    .map(|s| s.tire_id.clone())
                    .collect();
                compiled.apply_initial_to_input(input, &ids).unwrap();
                assert_eq!(
                    canonical_json_digest(input).unwrap(),
                    canonical_json_digest(&actual[actor]).unwrap(),
                    "{id}/{actor}"
                );
            }
        }
    }

    fn request(era: i32, short: bool) -> RunRaceRequest {
        let fixtures: Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../experiments/opponent_physics_v2/results/career-fixtures-v1.json"
        )))
        .unwrap();
        let id = format!("craft_engineering:era{era}-exit");
        let fixture = fixtures["fixtures"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["id"] == id)
            .unwrap();
        let mut request: RunRaceRequest =
            serde_json::from_value(fixture["preparedRequests"][0]["request"].clone()).unwrap();
        if short {
            request.input.race.laps = 2;
            request.input.pit_strategy = None;
            let player = request
                .input
                .race
                .competitors
                .iter_mut()
                .find(|c| c.is_player)
                .unwrap();
            player.stint_strategy = Some(
                serde_json::from_value(
                    serde_json::json!({"stints":[{"tire_id":"medium","laps":2}],"pit_laps":[]}),
                )
                .unwrap(),
            );
        }
        request
    }
    #[test]
    fn fixed_field_is_idempotent_independent_and_fully_compiled() {
        let catalog = RacingCatalogSnapshot::embedded_qualified_career().unwrap();
        let first = prepare_request(request(1, true), &catalog).unwrap();
        assert_eq!(
            canonical_json_digest(&first).unwrap(),
            canonical_json_digest(&prepare_request(first.clone(), &catalog).unwrap()).unwrap()
        );
        assert_eq!(first.input.race.competitors.len(), 10);
        let initial = resolve_inputs(&first, &catalog).unwrap();
        let mut changed = first;
        let player = changed
            .input
            .race
            .competitors
            .iter_mut()
            .find(|c| c.is_player)
            .unwrap();
        player.tuning.engine_points -= 1.0;
        player.tuning.chassis_points += 1.0;
        let other = resolve_inputs(&changed, &catalog).unwrap();
        for (id, input) in &initial {
            assert!(input.longitudinal_energy.is_some());
            assert_eq!(input.state.tire_temp, 70.0);
            assert_eq!(
                input.thermal_physics_v2.as_ref().unwrap().compounds.len(),
                3
            );
            assert!(input.vehicle.tire.wear_grip_curve.is_some());
            if id != "player" {
                assert_eq!(
                    canonical_json_digest(input).unwrap(),
                    canonical_json_digest(&other[id]).unwrap()
                );
            }
        }
    }
    #[test]
    fn short_incremental_stream_has_real_energy_channels_and_all_ten_cars() {
        let catalog = RacingCatalogSnapshot::embedded_qualified_career().unwrap();
        let req = prepare_request(request(1, true), &catalog).unwrap();
        let mut session = start(req, &catalog, false).unwrap();
        session.advance().unwrap();
        let lap = session
            .competitors
            .iter()
            .find(|c| c.is_player)
            .unwrap()
            .latest_lap
            .as_ref()
            .unwrap();
        let shaft: f64 = lap
            .solution
            .energy_power_points
            .iter()
            .zip(lap.solution.t.windows(2))
            .map(|(p, t)| p.shaft_w * (t[1] - t[0]))
            .sum();
        assert!(
            (shaft
                - lap
                    .longitudinal_energy_v1
                    .as_ref()
                    .unwrap()
                    .engine_shaft_work_j)
                .abs()
                < 1e-5
        );
        let digest = canonical_json_digest(&lap.solution).unwrap();
        let mut copy = lap.solution.clone();
        copy.energy_power_points.clear();
        assert_eq!(digest, canonical_json_digest(&copy).unwrap());
        let output = session.collect().unwrap();
        assert_eq!(output.standings.len(), 10);
        assert!(output.standings.iter().all(|r| r.laps_completed == 2));
        assert!(
            output
                .player_batches
                .iter()
                .flat_map(|b| &b.frames)
                .all(|f| f
                    .samples
                    .iter()
                    .all(|s| ![5008, 5009].contains(&s.parameter_id)))
        );
    }
    #[test]
    fn pit_instruction_uses_compiled_compound_and_replays_exactly() {
        use crate::control_prototype::{
            ControlStep, LocalControlConfig, LocalControlledRacingSession, reference_proposal,
        };
        let catalog = RacingCatalogSnapshot::embedded_qualified_career().unwrap();
        let mut input = request(1, true);
        input.input.race.laps = 3;
        input
            .input
            .race
            .competitors
            .iter_mut()
            .find(|c| c.is_player)
            .unwrap()
            .stint_strategy
            .as_mut()
            .unwrap()
            .stints[0]
            .laps = 3;
        let input = prepare_request(input, &catalog).unwrap();
        let config:LocalControlConfig=serde_json::from_value(serde_json::json!({"execution_id":"qualified-pit-test","runtime_digest":Digest::from_bytes(b"qualified-test-runtime"),
            "physics":"pitgun.local-qualified-career/v1","decision_laps":[1],"allowed_tire_ids":["hard","medium","soft"]})).unwrap();
        let mut session = LocalControlledRacingSession::start(
            input.clone(),
            &catalog,
            crate::service_control::empty_timeline(),
            config.clone(),
        )
        .unwrap();
        let mut history = Vec::new();
        let output = loop {
            match session.advance().unwrap() {
                ControlStep::AwaitingDecision(observation) => history.push(
                    session
                        .propose(reference_proposal(&observation, 1, "hard").unwrap())
                        .unwrap(),
                ),
                ControlStep::Batch(batch) => {
                    if let Some(out) = batch.records().iter().find_map(|r| match r.event() {
                        IncrementalExecutionStreamEventV1::Complete(out) => Some(out.clone()),
                        _ => None,
                    }) {
                        break out;
                    }
                }
            }
        };
        assert_eq!(output.player_pit_laps, vec![2]);
        let replay = crate::control_prototype::replay(
            input,
            &catalog,
            crate::service_control::empty_timeline(),
            config,
            &history,
        )
        .unwrap();
        assert_eq!(
            canonical_json_digest(&output).unwrap(),
            canonical_json_digest(&replay).unwrap()
        );
    }
    #[test]
    fn physical_inputs_match_the_sealed_native_campaign_exactly() {
        let catalog = RacingCatalogSnapshot::embedded_qualified_career().unwrap();
        let vectors: Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../experiments/qualified_career_v1/native-reference-v1.json"
        )))
        .unwrap();
        for vector in vectors["vectors"].as_array().unwrap() {
            let request = prepare_request(
                serde_json::from_value(vector["request"].clone()).unwrap(),
                &catalog,
            )
            .unwrap();
            for (actor, input) in resolve_inputs(&request, &catalog).unwrap() {
                assert_eq!(
                    canonical_json_digest(&input).unwrap().to_string(),
                    vector["expected"][&actor]["physical_input_digest"]
                        .as_str()
                        .unwrap(),
                    "{} {actor}",
                    vector["circuit"]
                );
            }
        }
    }
    #[test]
    fn unsupported_era_is_rejected_before_computation() {
        let catalog = RacingCatalogSnapshot::embedded_qualified_career().unwrap();
        let mut req = request(1, true);
        req.era = Some(3);
        assert!(prepare_request(req, &catalog).is_err());
    }
    #[test]
    fn presentation_geometry_and_service_loss_cannot_override_qualification() {
        let catalog = RacingCatalogSnapshot::embedded_qualified_career().unwrap();
        let mut req = request(1, true);
        req.input.pit_strategy = Some(PitStrategyConfig {
            player_pit_laps: vec![],
            pit_loss_ms: Some(0),
        });
        assert!(
            prepare_request(req, &catalog)
                .unwrap_err()
                .contains("catalog geometry")
        );
        let mut wire = serde_json::to_value(request(1, true)).unwrap();
        wire["input"]["track_profile"] = serde_json::json!({
            "s":[0.,100.],"x":[0.,100.],"y":[0.,0.]
        });
        let req = serde_json::from_value(wire).unwrap();
        assert!(
            prepare_request(req, &catalog)
                .unwrap_err()
                .contains("catalog geometry")
        );
    }
}
