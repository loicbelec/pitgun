//! Explicit native thermal candidate resolved from actual installed components.
//! No ordinary career constructor, catalog or runtime selects this boundary.
use crate::{
    EmbeddedCatalog, RacingCatalogSnapshot, RunRaceRequest,
    resolve_career_physics_inputs_for_audit,
    thermal_driver::{
        ThermalDriverPolicyV1, ThermalDriverResolutionV1, apply_thermal_driver_v1,
        resolve_thermal_driver_v1,
    },
};
use pitgun_contract::{Digest, canonical_json_digest};
use pitgun_racing_contract::{InstalledVehicleComponentV1, RacingDrivingMode};
use pitgun_racing_solver::{
    EngineThermalParamsV3, ResolvedSimulationRequestV3, ResolvedSimulationSessionV3,
    longitudinal_energy::LongitudinalEnergyModelV1,
    thermal_physics::{
        EngineHeatParamsV2, ThermalModelV2, ThermalPhysicsV2, TireCompoundHeatV2, TireHeatParamsV2,
    },
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThermalCareerVersionV1 {
    #[serde(rename = "pitgun.racing-thermal-career/v1")]
    V1,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThermalCareerPolicyV1 {
    pub schema_version: ThermalCareerVersionV1,
    pub engine: EngineHeatParamsV2,
    pub radiator_speed_conductance_by_power_unit: BTreeMap<String, f64>,
    pub cooling_size_without_development: f64,
    pub cooling_size_gain_at_cap: f64,
    pub cooling_drag_area_m2_at_cap: f64,
    pub road_temperature_c: f64,
    pub compounds: BTreeMap<String, ThermalCareerCompoundV1>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThermalCareerCompoundV1 {
    pub heat: TireHeatParamsV2,
    pub grip_scale: f64,
    pub temperature_optimum_c: f64,
    pub temperature_width_c: f64,
    pub minimum_temperature_grip_factor: f64,
}
impl ThermalCareerPolicyV1 {
    pub fn validate(&self) -> Result<(), String> {
        if !self.cooling_size_without_development.is_finite()
            || self.cooling_size_without_development <= 0.0
            || !self.cooling_size_gain_at_cap.is_finite()
            || self.cooling_size_gain_at_cap < 0.0
            || !self.cooling_drag_area_m2_at_cap.is_finite()
            || !(0.0..=0.5).contains(&self.cooling_drag_area_m2_at_cap)
            || !self.road_temperature_c.is_finite()
            || !(0.0..=200.0).contains(&self.road_temperature_c)
            || self.radiator_speed_conductance_by_power_unit.is_empty()
            || self
                .radiator_speed_conductance_by_power_unit
                .values()
                .any(|x| !x.is_finite() || *x < 0.0)
            || self.compounds.is_empty()
        {
            return Err(
                "thermal career: invalid sizing/environment or empty component mapping".into(),
            );
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThermalCareerCarV1 {
    pub baseline_input_digest: Digest,
    pub baseline_vehicle_id: String,
    pub installed_components: Vec<InstalledVehicleComponentV1>,
    pub engine_id: String,
    /// Digest of parsed immutable catalog engine parameters, before tuning.
    pub engine_parameters_digest: Digest,
    pub cooling_development_fraction: f64,
    pub removed_legacy_cooling_drag_area_m2: f64,
    pub added_candidate_cooling_drag_area_m2: f64,
    pub driver: ThermalDriverResolutionV1,
    pub input_digest: Digest,
    pub input: ResolvedSimulationRequestV3,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThermalCareerFieldV1 {
    pub schema_version: ThermalCareerVersionV1,
    pub source_request_digest: Digest,
    pub thermal_policy_digest: Digest,
    pub driver_policy_digest: Digest,
    pub competitors: BTreeMap<String, ThermalCareerCarV1>,
}

/// No era-derived engine, legacy driver fallback or caller-supplied code/IO.
/// Global provenance is outside individual cars so an unchanged rival stays exact
/// when only the player's legal setup or plan changes.
pub fn resolve_thermal_career_v1(
    request: &RunRaceRequest,
    snapshot: &RacingCatalogSnapshot,
    thermal: &ThermalCareerPolicyV1,
    driver_policy: &ThermalDriverPolicyV1,
    modes: &BTreeMap<String, RacingDrivingMode>,
) -> Result<ThermalCareerFieldV1, String> {
    thermal.validate()?;
    driver_policy.validate()?;
    let baseline = resolve_career_physics_inputs_for_audit(request.clone(), snapshot)?;
    let normalized = pitgun_racing_policy::normalize_and_validate_race_input(
        &request.input.race,
        request.era.unwrap_or(request.input.era).max(0) as u32,
    )
    .map_err(|e| e.to_string())?;
    if modes.keys().any(|id| !baseline.contains_key(id)) {
        return Err("thermal career: mode for unknown competitor".into());
    }
    let catalog = EmbeddedCatalog::from_snapshot(snapshot)?;
    let vehicle_id = crate::resolve_vehicle_id(request.input.vehicle_id.as_deref())?;
    let old_profile = snapshot
        .power_unit_thermal_profile()
        .ok_or("thermal career: missing baseline power-unit profile")?;
    let points_cap = crate::V3CandidateExperimentProfile::default()
        .tuning_response
        .development_points_cap;
    let mut competitors = BTreeMap::new();
    for competitor in &normalized.competitors {
        let selection = request
            .input
            .competitor_vehicle_components
            .get(&competitor.id);
        let installed = catalog.resolve_installed_components(vehicle_id, selection)?;
        let engine_id = catalog.resolve_engine_id_with_components(vehicle_id, selection)?;
        let engine = catalog
            .engines
            .get(engine_id)
            .ok_or("thermal career: missing installed engine")?;
        let radiator = thermal
            .radiator_speed_conductance_by_power_unit
            .get(engine_id)
            .ok_or_else(|| format!("thermal career: missing radiator mapping for {engine_id}"))?;
        let mut input = baseline
            .get(&competitor.id)
            .ok_or("thermal career: missing baseline input")?
            .clone();
        let baseline_input_digest = canonical_json_digest(&input).map_err(|e| e.to_string())?;
        // Remove old family coefficient application only for the fields replaced
        // by the explicit V2 model. Torque/gearing/body/chassis stay resolved.
        input.vehicle.engine.c_th = engine.c_th;
        input.vehicle.engine.t_soft = engine.t_soft;
        input.vehicle.engine.beta_derate = engine.beta_derate;
        input.engine_thermal = Some(EngineThermalParamsV3::default());
        input.longitudinal_energy = Some(LongitudinalEnergyModelV1::V1);
        let cooling = competitor
            .tuning
            .cooling_points
            .round()
            .clamp(0.0, points_cap)
            / points_cap;
        let sizing =
            thermal.cooling_size_without_development + thermal.cooling_size_gain_at_cap * cooling;
        let mut heat = thermal.engine;
        heat.radiator_idle_conductance_w_per_k *= sizing;
        heat.radiator_speed_conductance_w_per_k_mps = radiator * sizing;
        let legacy_drag = old_profile
            .resolve_power_unit(engine_id)?
            .engine_thermal_resolution
            .cooling_drag_area_m2_at_cap
            * cooling;
        let candidate_drag = thermal.cooling_drag_area_m2_at_cap * cooling;
        input.mechanical.fixed_drag_area_m2 += candidate_drag - legacy_drag;
        let default_tire = catalog
            .resolve_vehicle_with_components(vehicle_id, selection)?
            .1;
        let pits = request
            .input
            .pit_strategy
            .as_ref()
            .map(|p| p.player_pit_laps.as_slice())
            .unwrap_or(&[]);
        let plan = crate::resolve_stint_plan(competitor, input.lap_count, &default_tire, pits)?;
        let mut compounds = Vec::<TireCompoundHeatV2>::new();
        let mut apply =
            |tire: &mut pitgun_racing_solver::TireParams, id: &str| -> Result<(), String> {
                let c = thermal
                    .compounds
                    .get(id)
                    .ok_or_else(|| format!("thermal career: missing compound {id}"))?;
                tire.mu_scale = c.grip_scale;
                tire.temp_opt = c.temperature_optimum_c;
                tire.temp_sigma = c.temperature_width_c;
                tire.temp_min_k = c.minimum_temperature_grip_factor;
                if !compounds.iter().any(|p| p.tire == *tire) {
                    compounds.push(TireCompoundHeatV2 {
                        tire: tire.clone(),
                        heat: c.heat,
                    });
                }
                Ok(())
            };
        apply(&mut input.vehicle.tire, &plan.tire_by_lap[0])?;
        for stop in &mut input.pit_plan.stops {
            apply(&mut stop.tire, &plan.tire_by_lap[usize::from(stop.lap)])?;
        }
        input.thermal_physics_v2 = Some(ThermalPhysicsV2 {
            schema_version: ThermalModelV2::V2,
            engine: heat,
            compounds,
            initial_carcass_temperature_c: input.state.tire_temp,
            pit_carcass_temperature_c: input.config.pit_tire_temp.unwrap_or(input.state.tire_temp),
            road_temperature_c: thermal.road_temperature_c,
        });
        let driver = resolve_thermal_driver_v1(
            snapshot,
            competitor
                .driver_id
                .as_deref()
                .ok_or("thermal career: explicit V2 driver required")?,
            modes
                .get(&competitor.id)
                .copied()
                .unwrap_or(RacingDrivingMode::Balanced),
            driver_policy,
            &format!("career-driver:{}", competitor.id),
        )?;
        apply_thermal_driver_v1(&mut input, &driver)?;
        ResolvedSimulationSessionV3::new(input.clone())?;
        let input_digest = canonical_json_digest(&input).map_err(|e| e.to_string())?;
        competitors.insert(
            competitor.id.clone(),
            ThermalCareerCarV1 {
                baseline_input_digest,
                baseline_vehicle_id: vehicle_id.into(),
                installed_components: installed
                    .into_iter()
                    .map(|(kind, component_id)| InstalledVehicleComponentV1 { kind, component_id })
                    .collect(),
                engine_id: engine_id.into(),
                engine_parameters_digest: canonical_json_digest(engine)
                    .map_err(|e| e.to_string())?,
                cooling_development_fraction: cooling,
                removed_legacy_cooling_drag_area_m2: legacy_drag,
                added_candidate_cooling_drag_area_m2: candidate_drag,
                driver,
                input_digest,
                input,
            },
        );
    }
    Ok(ThermalCareerFieldV1 {
        schema_version: ThermalCareerVersionV1::V1,
        source_request_digest: canonical_json_digest(request).map_err(|e| e.to_string())?,
        thermal_policy_digest: canonical_json_digest(thermal).map_err(|e| e.to_string())?,
        driver_policy_digest: canonical_json_digest(driver_policy).map_err(|e| e.to_string())?,
        competitors,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn setup(
        stage: &str,
    ) -> (
        RunRaceRequest,
        RacingCatalogSnapshot,
        ThermalCareerPolicyV1,
        ThermalDriverPolicyV1,
    ) {
        let fixtures: Value = serde_json::from_str(include_str!(
            "../../../experiments/opponent_physics_v2/results/career-fixtures-v1.json"
        ))
        .unwrap();
        let fixture = fixtures["fixtures"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["id"] == format!("craft_engineering:{stage}"))
            .unwrap();
        let request =
            serde_json::from_value(fixture["preparedRequests"][0]["request"].clone()).unwrap();
        let snapshot = RacingCatalogSnapshot::embedded_model_v3_timeline().unwrap();
        let thermal = serde_json::from_str(include_str!(
            "../../../experiments/career_physics_v1/profile-v1.json"
        ))
        .unwrap();
        let driver = ThermalDriverPolicyV1::from_catalog(&snapshot).unwrap();
        (request, snapshot, thermal, driver)
    }

    #[test]
    fn actual_components_select_different_engines_with_the_same_baseline_car() {
        let (request, snapshot, thermal, driver) = setup("era2-entry");
        let field =
            resolve_thermal_career_v1(&request, &snapshot, &thermal, &driver, &BTreeMap::new())
                .unwrap();
        assert_eq!(field.competitors.len(), 10);
        let player = &field.competitors["player"];
        let rival = &field.competitors["ai_1"];
        assert_eq!(player.baseline_vehicle_id, rival.baseline_vehicle_id);
        assert_eq!(player.engine_id, "v8_1960");
        assert_eq!(rival.engine_id, "v8_1970");
        assert_ne!(
            player.engine_parameters_digest,
            rival.engine_parameters_digest
        );
        for car in [player, rival] {
            let heat = car.input.thermal_physics_v2.as_ref().unwrap().engine;
            let sizing = thermal.cooling_size_without_development
                + thermal.cooling_size_gain_at_cap * car.cooling_development_fraction;
            assert_eq!(
                heat.radiator_speed_conductance_w_per_k_mps,
                thermal.radiator_speed_conductance_by_power_unit[&car.engine_id] * sizing
            );
            assert_eq!(car.input.driver.id, car.driver.controls.driver_id);
        }
        let (exit, _, _, _) = setup("era2-exit");
        let upgraded =
            resolve_thermal_career_v1(&exit, &snapshot, &thermal, &driver, &BTreeMap::new())
                .unwrap();
        assert_eq!(upgraded.competitors["player"].engine_id, "v8_1970");
        // A modern preparation convention is not imposed on historic careers.
        assert_eq!(upgraded.competitors["player"].input.state.tire_temp, 90.0);
    }

    #[test]
    fn changing_player_setup_does_not_resolve_or_reseed_a_different_rival() {
        let (mut request, snapshot, thermal, driver) = setup("era2-exit");
        let resolve = |r: &RunRaceRequest| {
            resolve_thermal_career_v1(r, &snapshot, &thermal, &driver, &BTreeMap::new()).unwrap()
        };
        let before = resolve(&request);
        let p = request
            .input
            .race
            .competitors
            .iter_mut()
            .find(|c| c.is_player)
            .unwrap();
        p.tuning.chassis_points += 2.0;
        p.tuning.cooling_points -= 2.0;
        p.tuning.gear_ratio_slider = 1.0;
        let after = resolve(&request);
        assert_ne!(before.source_request_digest, after.source_request_digest);
        assert_ne!(
            before.competitors["player"].input_digest,
            after.competitors["player"].input_digest
        );
        for (id, car) in &before.competitors {
            if id != "player" {
                assert_eq!(car, &after.competitors[id]);
            }
        }
        assert_eq!(
            canonical_json_digest(&after).unwrap(),
            canonical_json_digest(&resolve(&request)).unwrap()
        );
    }

    #[test]
    fn no_override_preserves_previously_qualified_thermal_and_mechanical_parameters() {
        let (_, snapshot, thermal, driver) = setup("era1-entry");
        let plan: Value = serde_json::from_str(include_str!(
            "../../../experiments/thermal_physics_v2/plan-v1.json"
        ))
        .unwrap();
        let frozen: Value = serde_json::from_str(include_str!(
            "../../../experiments/thermal_physics_v2/results/screen-v1.json"
        ))
        .unwrap();
        for id in ["budget-early8-monza-c0", "budget-classic15-monza-c0"] {
            let case = plan["cases"]
                .as_array()
                .unwrap()
                .iter()
                .find(|c| c["id"] == id)
                .unwrap();
            let mut request: RunRaceRequest =
                serde_json::from_value(case["request"].clone()).unwrap();
            request.input.race.competitors[0].driver_id = Some("balanced_reference".into());
            let field =
                resolve_thermal_career_v1(&request, &snapshot, &thermal, &driver, &BTreeMap::new())
                    .unwrap();
            let actual = &field.competitors["player"].input;
            let expected = &frozen["results"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["case"] == id && r["candidate"] == true)
                .unwrap()["data"]["resolved_parameters"];
            assert_eq!(
                serde_json::to_value(&actual.vehicle).unwrap(),
                expected["vehicle"]
            );
            assert_eq!(
                serde_json::to_value(&actual.mechanical).unwrap(),
                expected["mechanical"]
            );
            let old: ThermalPhysicsV2 =
                serde_json::from_value(expected["thermal_physics_v2"].clone()).unwrap();
            let new = actual.thermal_physics_v2.as_ref().unwrap();
            assert_eq!(new.engine, old.engine);
            assert_eq!(new.road_temperature_c, old.road_temperature_c);
            for c in &new.compounds {
                assert!(old.compounds.contains(c));
            }
        }
    }

    #[test]
    fn strict_resolution_rejects_missing_mapping_driver_and_unknown_mode() {
        let (mut request, snapshot, mut thermal, driver) = setup("era2-entry");
        thermal
            .radiator_speed_conductance_by_power_unit
            .remove("v8_1970");
        assert!(
            resolve_thermal_career_v1(&request, &snapshot, &thermal, &driver, &BTreeMap::new())
                .unwrap_err()
                .contains("missing radiator mapping")
        );
        thermal
            .radiator_speed_conductance_by_power_unit
            .insert("v8_1970".into(), 45.0);
        request.input.race.competitors[0].driver_id = Some("default".into());
        assert!(
            resolve_thermal_career_v1(&request, &snapshot, &thermal, &driver, &BTreeMap::new())
                .is_err()
        );
        let modes = BTreeMap::from([("not-in-race".into(), RacingDrivingMode::Attack)]);
        assert!(
            resolve_thermal_career_v1(&request, &snapshot, &thermal, &driver, &modes)
                .unwrap_err()
                .contains("unknown competitor")
        );
    }
}
