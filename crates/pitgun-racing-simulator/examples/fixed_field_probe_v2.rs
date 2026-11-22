//! Native fixed-personality field calibration. No ordinary runtime selects it.
use pitgun_contract::{Digest, canonical_json_digest};
use pitgun_racing_contract::{
    CompetitorStintStrategy, RacingDriverResourceV2, RacingDrivingMode, VehicleComponentSelectionV1,
};
use pitgun_racing_simulator::{
    RacingCatalogSnapshot, RunRaceRequest,
    thermal_career::{ThermalCareerPolicyV1, resolve_thermal_career_v1},
    thermal_driver::{
        ThermalDriverPolicyV1, apply_thermal_driver_v1, resolve_thermal_driver_resource_v1,
    },
};
use pitgun_racing_solver::{
    ResolvedSimulationSessionV3, ResolvedSimulationStepV3,
    lap_boundary::{BoundaryDiagnosticRequestV1, BoundaryDiagnosticVersionV1, BoundaryForecastV1},
    thermal_physics::ThermalLedgerV2,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::BTreeMap, env, fs, path::Path};

#[derive(Deserialize)]
struct Roster {
    schema: String,
    status: String,
    drivers: BTreeMap<String, Personality>,
    front_runners: Vec<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Personality {
    name: String,
    resource: RacingDriverResourceV2,
    mode: RacingDrivingMode,
    style: String,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Case {
    id: String,
    fixture_id: String,
    circuit: String,
    actor: String,
    kind: String,
    budget: u16,
    allocation: [u16; 4],
    gear: f64,
    downforce: f64,
    strategy: CompetitorStintStrategy,
    seed: u64,
    environment: Option<Environment>,
    #[serde(default)]
    elite_level: u8,
}
#[derive(Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Environment {
    air_c: f64,
    road_c: f64,
}
struct Context {
    fixtures: Value,
    catalog: RacingCatalogSnapshot,
    thermal: ThermalCareerPolicyV1,
    driver_policy: ThermalDriverPolicyV1,
    roster: Roster,
    roster_digest: Digest,
    binary_digest: Digest,
}
impl Context {
    fn load(root: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let catalog = RacingCatalogSnapshot::from_release_dir(root.join("catalogs/racing/v1.8.1"))?;
        let bytes = fs::read(root.join("experiments/fixed_opponents_v2/roster-v2.json"))?;
        let roster: Roster = serde_json::from_slice(&bytes)?;
        let driver_policy = ThermalDriverPolicyV1::from_catalog(&catalog)?;
        if roster.schema != "pitgun.fixed-opponent-roster/v1"
            || roster.status.is_empty()
            || roster.drivers.len() != 9
            || roster.front_runners.len() != 2
        {
            return Err("fixed field: invalid roster identity/count".into());
        }
        for (id, p) in &roster.drivers {
            if id != &p.resource.id || p.name.is_empty() || p.style.is_empty() {
                return Err("fixed field: personality identity mismatch".into());
            }
            resolve_thermal_driver_resource_v1(
                &p.resource,
                p.mode,
                &driver_policy,
                &format!("fixed-field:{id}"),
            )?;
        }
        if roster
            .front_runners
            .iter()
            .any(|id| !roster.drivers.contains_key(id))
        {
            return Err("fixed field: missing front-runner".into());
        }
        Ok(Self {
            fixtures: serde_json::from_slice(&fs::read(
                root.join("experiments/opponent_physics_v2/results/career-fixtures-v1.json"),
            )?)?,
            thermal: serde_json::from_slice(&fs::read(
                root.join("experiments/career_physics_v1/profile-v1.json"),
            )?)?,
            catalog,
            driver_policy,
            roster,
            roster_digest: Digest::from_bytes(&bytes),
            binary_digest: Digest::from_bytes(&fs::read(env::current_exe()?)?),
        })
    }
    fn resolve(
        &self,
        c: &Case,
    ) -> Result<(pitgun_racing_solver::ResolvedSimulationRequestV3, Value), String> {
        let fixture = self.fixtures["fixtures"]
            .as_array()
            .ok_or("fixtures")?
            .iter()
            .find(|f| f["id"] == c.fixture_id)
            .ok_or("unknown fixture")?;
        let prepared = fixture["preparedRequests"]
            .as_array()
            .ok_or("requests")?
            .iter()
            .find(|p| p["circuitId"] == c.circuit)
            .ok_or("circuit not in authored career")?;
        let era = fixture["era"].as_u64().ok_or("era")?;
        if !(1..=2).contains(&era) {
            return Err("fixed field: unqualified era".into());
        }
        if !c.gear.is_finite()
            || !(0.0..=1.0).contains(&c.gear)
            || !c.downforce.is_finite()
            || !(0.0..=1.0).contains(&c.downforce)
        {
            return Err("fixed field: slider bounds".into());
        }
        if c.allocation.iter().map(|v| u32::from(*v)).sum::<u32>() != u32::from(c.budget) {
            return Err("fixed field: allocation does not match explicit budget".into());
        }
        let career = c.kind == "career_player";
        if !career && c.kind != "field" {
            return Err("fixed field: unknown case kind".into());
        }
        if career
            && (c.actor != "player"
                || u64::from(c.budget)
                    != fixture["discretionaryPoints"]
                        .as_u64()
                        .ok_or("career budget")?)
        {
            return Err("fixed field: player identity/budget".into());
        }
        if !career && c.actor != "reference" && !self.roster.drivers.contains_key(&c.actor) {
            return Err("fixed field: unknown named driver".into());
        }
        if c.elite_level > 2 {
            return Err("fixed field: unknown fixed elite calibration".into());
        }
        let mut request: RunRaceRequest =
            serde_json::from_value(prepared["request"].clone()).map_err(|e| e.to_string())?;
        let source_player = request
            .input
            .race
            .competitors
            .iter()
            .find(|p| p.is_player)
            .ok_or("source player")?
            .clone();
        let source_components = request
            .input
            .competitor_vehicle_components
            .get(&source_player.id)
            .cloned();
        let id = if career || c.actor == "reference" {
            "player"
        } else {
            &c.actor
        };
        let mut p = source_player;
        p.id = id.into();
        p.name = if let Some(person) = self.roster.drivers.get(&c.actor) {
            person.name.clone()
        } else {
            "Player reference".into()
        };
        p.team_id = format!("FIXED_{id}");
        p.is_player = true;
        // Stage1 resolves shared mechanics/thermal with the known reference resource.
        // Stage2 below explicitly applies the named V2 resource before any calculation.
        p.driver_id = Some("balanced_reference".into());
        let offsets = if career {
            [
                fixture["offsets"]["aero"].as_u64().ok_or("offset")? as u16,
                fixture["offsets"]["chassis"].as_u64().ok_or("offset")? as u16,
                fixture["offsets"]["engine"].as_u64().ok_or("offset")? as u16,
                fixture["offsets"]["cooling"].as_u64().ok_or("offset")? as u16,
            ]
        } else if era == 1 {
            [0, 0, 0, 0]
        } else {
            [2, 1, 4, 1]
        };
        let mut points = [0.0; 4];
        for i in 0..4 {
            let v = c.allocation[i]
                .checked_add(offsets[i])
                .ok_or("point overflow")?;
            if v > 20 {
                return Err("fixed field: effective point cap".into());
            }
            points[i] = f64::from(v);
        }
        p.budget_cap = points.iter().sum();
        p.tuning.aero_points = points[0];
        p.tuning.chassis_points = points[1];
        p.tuning.engine_points = points[2];
        p.tuning.cooling_points = points[3];
        p.tuning.gear_ratio_slider = c.gear;
        p.tuning.downforce_slider = c.downforce;
        let locked = if career {
            fixture["sliders"]["downforce"][0] == fixture["sliders"]["downforce"][1]
        } else {
            era == 1
        };
        if locked && c.downforce != 0.5 {
            return Err("fixed field: unavailable downforce control".into());
        }
        if c.strategy.stints.len() < 2
            || c.strategy.stints[0].tire_id == c.strategy.stints[1].tire_id
        {
            return Err("fixed field: require at least1stop and different opening compounds; stint lengths/sum validated by resolver".into());
        }
        p.stint_strategy = Some(c.strategy.clone());
        request
            .input
            .pit_strategy
            .as_mut()
            .ok_or("pit strategy")?
            .player_pit_laps = c.strategy.pit_laps.clone();
        request.input.race.competitors = vec![p];
        request.input.competitor_vehicle_components.clear();
        request.input.competitor_profiles.clear();
        let components = if career {
            source_components
        } else if era == 2 {
            Some(serde_json::from_value::<VehicleComponentSelectionV1>(json!({"schema_version":"pitgun.racing-vehicle-components/v1","engine_id":"v8_1970","aero_id":"basic"})).map_err(|e|e.to_string())?)
        } else {
            None
        };
        if let Some(components) = components {
            request
                .input
                .competitor_vehicle_components
                .insert(id.into(), components);
        }
        request.seed = c.seed;
        let base = resolve_thermal_career_v1(
            &request,
            &self.catalog,
            &self.thermal,
            &self.driver_policy,
            &BTreeMap::new(),
        )?;
        let car = &base.competitors[id];
        let mut input = car.input.clone();
        let driver = if let Some(person) = self.roster.drivers.get(&c.actor) {
            let mut resource = person.resource.clone();
            if self.roster.front_runners.contains(&c.actor) {
                let gain = 0.02 * f64::from(c.elite_level);
                resource.traits.limit_exploitation =
                    (resource.traits.limit_exploitation + gain).min(0.99);
                resource.traits.consistency = (resource.traits.consistency + gain).min(0.99);
                resource.traits.tire_management =
                    (resource.traits.tire_management + gain).min(0.99);
            }
            resolve_thermal_driver_resource_v1(
                &resource,
                person.mode,
                &self.driver_policy,
                &format!("fixed-field:{}", c.actor),
            )?
        } else {
            car.driver.clone()
        };
        apply_thermal_driver_v1(&mut input, &driver)?;
        if let Some(e) = &c.environment {
            if !e.air_c.is_finite()
                || !(0.0..=60.0).contains(&e.air_c)
                || !e.road_c.is_finite()
                || !(0.0..=80.0).contains(&e.road_c)
            {
                return Err("fixed field: environment bounds".into());
            }
            input.vehicle.engine.t_amb = e.air_c;
            input.config.tire_temp_amb = e.air_c;
            input
                .thermal_physics_v2
                .as_mut()
                .unwrap()
                .road_temperature_c = e.road_c;
        }
        let receipt = json!({"roster_digest":self.roster_digest,"catalog":self.catalog.release_identity(),"thermal_policy_digest":base.thermal_policy_digest,"driver_policy_digest":base.driver_policy_digest,"staged_request_digest":base.source_request_digest,"shared_thermal_input_digest":car.input_digest,"engine_id":car.engine_id,"components":car.installed_components,"offsets":offsets,"effective_points":points,"driver":driver,"initial_fuel_kg":input.state.fuel_mass,"initial_tire_temperature_c":input.state.tire_temp,"laps":input.lap_count});
        Ok((input, receipt))
    }
    fn run(&self, c: &Case) -> Result<Value, String> {
        let (input, receipt) = self.resolve(c)?;
        let mut expected = input.state.clone();
        let mut speed = 0.0;
        let mut carcass = input
            .thermal_physics_v2
            .as_ref()
            .unwrap()
            .initial_carcass_temperature_c;
        let physical = canonical_json_digest(&input).map_err(|e| e.to_string())?;
        let req = BoundaryDiagnosticRequestV1 {
            schema_version: BoundaryDiagnosticVersionV1::V1,
            forecast: BoundaryForecastV1::ReachableWearV1,
            recompute_invariant: false,
            input: input.clone(),
        };
        let digest = canonical_json_digest(&req).map_err(|e| e.to_string())?;
        let mut session = ResolvedSimulationSessionV3::new_boundary_diagnostic(req, false)?;
        let mut times = Vec::new();
        let mut jumps = [0.0_f64; 5];
        let mut thermal = ThermalLedgerV2::default();
        let mut min_power = 1.0_f64;
        let mut derated = 0.0;
        let mut peak_engine = expected.engine_temp;
        let mut peak_surface = expected.tire_temp;
        let mut peak_wear = expected.tire_wear;
        let mut lapses = 0;
        let outcome = loop {
            match session.advance() {
                Ok(ResolvedSimulationStepV3::Lap(lap)) => {
                    let s = &lap.solution;
                    let starts = [
                        s.v[0],
                        s.temp[0],
                        s.tire_temp[0],
                        s.tire_carcass_temp[0],
                        s.tire_wear[0],
                    ];
                    let before = [
                        speed,
                        expected.engine_temp,
                        expected.tire_temp,
                        carcass,
                        expected.tire_wear,
                    ];
                    for i in 0..5 {
                        jumps[i] = jumps[i].max((starts[i] - before[i]).abs());
                    }
                    times.push(lap.lap_time_s);
                    thermal.accumulate(&lap.thermal_v2.as_ref().unwrap().ledger);
                    min_power = s
                        .engine_derating_factor
                        .iter()
                        .fold(min_power, |a, &b| a.min(b));
                    derated +=
                        s.t.windows(2)
                            .zip(&s.engine_derating_factor)
                            .filter(|(_, p)| **p < 0.999)
                            .map(|(t, _)| t[1] - t[0])
                            .sum::<f64>();
                    peak_engine = s.temp.iter().fold(peak_engine, |a, &b| a.max(b));
                    peak_surface = s.tire_temp.iter().fold(peak_surface, |a, &b| a.max(b));
                    peak_wear = s.tire_wear.iter().fold(peak_wear, |a, &b| a.max(b));
                    lapses += s.driver_lapses_v1.len();
                    expected = lap.state_after_lap;
                    speed = lap.end_speed_mps.unwrap_or(0.0);
                    carcass = lap.thermal_v2.as_ref().unwrap().carcass_temperature_c;
                }
                Ok(ResolvedSimulationStepV3::Complete(r)) => {
                    break json!({"ok":true,"time_s":r.total_time_s,"result_digest":canonical_json_digest(&r).map_err(|e|e.to_string())?,"final_state":r.final_state,"energy":r.longitudinal_energy_v1});
                }
                Err(e) => {
                    break json!({"ok":false,"error":e,"accepted_laps":times.len(),"last_accepted_state":expected});
                }
            }
        };
        let work = session.boundary_work().unwrap_or_default();
        let fallbacks = work
            .laps
            .iter()
            .filter(|l| {
                l.reachable
                    .as_ref()
                    .is_some_and(|r| r.conservative_recompute)
            })
            .count();
        let updates = work
            .laps
            .iter()
            .filter_map(|l| l.reachable.as_ref().map(|r| r.uncommitted_cap_updates))
            .max()
            .unwrap_or(0);
        Ok(
            json!({"schema":"pitgun.fixed-field-probe/v1","id":c.id,"receipt":receipt,"physical_input_digest":physical,"diagnostic_input_digest":digest,"lap_times_s":times,"outcome":outcome,"maximum_carry_jumps":jumps,"thermal_ledger":thermal,"metrics":{"minimum_power_fraction":min_power,"derated_time_s":derated,"peak_engine_c":peak_engine,"peak_surface_c":peak_surface,"peak_wear":peak_wear,"recoverable_throttle_lapses":lapses,"conservative_recomputes":fallbacks,"maximum_cap_updates":updates},"binary_digest":self.binary_digest}),
        )
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 2 && args.len() != 3 {
        return Err("usage: fixed_field_probe_v2 ROOT CASE [--field]".into());
    }
    let context = Context::load(Path::new(&args[0]))?;
    if args.get(2).is_some_and(|a| a == "--field") {
        let cases: Vec<Case> = serde_json::from_slice(&fs::read(&args[1])?)?;
        let actors = cases
            .iter()
            .map(|c| c.actor.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        if cases.len() != 10
            || actors.len() != 10
            || !actors.contains("player")
            || context
                .roster
                .drivers
                .keys()
                .any(|id| !actors.contains(id.as_str()))
        {
            return Err("fixed field: batch requires player and all9named opponents".into());
        }
        if cases.iter().any(|c| {
            c.circuit != cases[0].circuit
                || c.seed != cases[0].seed
                || c.environment != cases[0].environment
        }) {
            return Err("fixed field: batch context mismatch".into());
        }
        let era_of = |c: &Case| {
            context.fixtures["fixtures"]
                .as_array()
                .unwrap()
                .iter()
                .find(|f| f["id"] == c.fixture_id)
                .and_then(|f| f["era"].as_u64())
        };
        if cases.iter().any(|c| era_of(c) != era_of(&cases[0])) {
            return Err("fixed field: mixed era".into());
        }
        let ai = cases
            .iter()
            .filter(|c| c.actor != "player")
            .collect::<Vec<_>>();
        if ai
            .iter()
            .any(|c| c.kind != "field" || c.budget != ai[0].budget)
        {
            return Err("fixed field: unequal fixed opponent budgets/kinds".into());
        }
        let mut rows = Vec::new();
        for c in &cases {
            rows.push(json!({"actor":c.actor,"data":context.run(c)?}));
        }
        rows.sort_by(|a, b| {
            let oa = &a["data"]["outcome"];
            let ob = &b["data"]["outcome"];
            let good_a = oa["ok"].as_bool().unwrap();
            let good_b = ob["ok"].as_bool().unwrap();
            let status = good_b.cmp(&good_a);
            let progress = ob["accepted_laps"]
                .as_u64()
                .unwrap_or(0)
                .cmp(&oa["accepted_laps"].as_u64().unwrap_or(0));
            let time = oa["time_s"]
                .as_f64()
                .unwrap_or(f64::INFINITY)
                .total_cmp(&ob["time_s"].as_f64().unwrap_or(f64::INFINITY));
            status
                .then(progress)
                .then(time)
                .then_with(|| a["actor"].as_str().cmp(&b["actor"].as_str()))
        });
        println!(
            "{}",
            json!({"schema":"pitgun.fixed-field-race/v1","independent_cars_no_traffic_model":true,"standings":rows})
        );
    } else {
        let case: Case = serde_json::from_slice(&fs::read(&args[1])?)?;
        println!("{}", context.run(&case)?);
    }
    Ok(())
}
