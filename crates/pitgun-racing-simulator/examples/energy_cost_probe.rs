//! Native performance audit only; observations are not a game wire contract.
use pitgun_contract::{Digest, canonical_json_digest};
use pitgun_racing_simulator::{
    RacingCatalogSnapshot, RunRaceRequest, resolve_career_physics_inputs_for_audit,
};
use pitgun_racing_solver::longitudinal_energy::LongitudinalEnergyModelV1;
use pitgun_racing_solver::{SimulationSolution, run_resolved_simulation_v3};
use serde::Serialize;
use serde_json::json;
use std::{collections::BTreeMap, env, fs, time::Instant};

#[derive(Debug, Serialize)]
struct Observation {
    time_s: f64,
    native_time_s: f64,
    distance_m: f64,
    speed_mps: f64,
    engine_temperature_c: f64,
    tire_temperature_c: f64,
    tire_wear_fraction: f64,
    gear: u8,
    lap: u16,
}
// Causal observation of an existing solution, including its final point. Does
// not reconstruct pedals, integrate physics or infer events from sampled rows.
fn observe(s: &SimulationSolution, hz: f64) -> Result<Vec<Observation>, String> {
    let n = s.t.len();
    if !hz.is_finite()
        || !(1.0..=10.0).contains(&hz)
        || n < 2
        || [
            s.s.len(),
            s.v.len(),
            s.temp.len(),
            s.tire_temp.len(),
            s.tire_wear.len(),
            s.gear.len(),
            s.lap_index.len(),
        ]
        .iter()
        .any(|l| *l != n)
        || s.t[0] != 0.0
        || s.t.iter().any(|t| !t.is_finite())
        || s.t.windows(2).any(|w| w[1] <= w[0])
    {
        return Err(
            "bounded observations require consistent increasing native arrays and1..10Hz".into(),
        );
    }
    let end = *s.t.last().unwrap();
    let count = (end * hz).floor();
    if count > 1_000_000.0 {
        return Err("observation limit exceeded".into());
    }
    let mut out = Vec::with_capacity(count as usize + 2);
    let mut index = 0;
    for tick in 0..=count as usize + 1 {
        let time = (tick as f64 / hz).min(end);
        while index + 1 < n && s.t[index + 1] <= time {
            index += 1;
        }
        out.push(Observation {
            time_s: time,
            native_time_s: s.t[index],
            distance_m: s.s[index],
            speed_mps: s.v[index],
            engine_temperature_c: s.temp[index],
            tire_temperature_c: s.tire_temp[index],
            tire_wear_fraction: s.tire_wear[index],
            gear: s.gear[index],
            lap: s.lap_index[index],
        });
        if time == end {
            break;
        }
    }
    Ok(out)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 3 {
        return Err("usage: energy_cost_probe REQUEST CATALOG OBSERVATION_HZ".into());
    }
    let hz = args[2].parse::<f64>()?;
    if !hz.is_finite() || !(1.0..=10.0).contains(&hz) {
        return Err("observation Hz must be in1..10".into());
    }
    let request: RunRaceRequest = serde_json::from_slice(&fs::read(&args[0])?)?;
    if request.input.race.laps > 100 || request.input.race.competitors.len() > 10 {
        return Err("bounded100laps/10cars".into());
    }
    let begin = Instant::now();
    let catalog = RacingCatalogSnapshot::from_release_dir(&args[1])?;
    let mut inputs = resolve_career_physics_inputs_for_audit(request.clone(), &catalog)?;
    let resolve_ms = begin.elapsed().as_secs_f64() * 1000.;
    let mut cars = BTreeMap::new();
    for (id, input) in &mut inputs {
        input.longitudinal_energy = Some(LongitudinalEnergyModelV1::V1);
        let t = Instant::now();
        let r = run_resolved_simulation_v3(input)?;
        let solve_ms = t.elapsed().as_secs_f64() * 1000.;
        let t = Instant::now();
        let result_digest = canonical_json_digest(&r)?;
        let native_digest_ms = t.elapsed().as_secs_f64() * 1000.;
        let t = Instant::now();
        let rows = observe(&r.solution, hz)?;
        let observation_ms = t.elapsed().as_secs_f64() * 1000.;
        let t = Instant::now();
        let bytes = serde_json::to_vec(&rows)?;
        let observation_json_ms = t.elapsed().as_secs_f64() * 1000.;
        let t = Instant::now();
        let observation_digest = Digest::from_bytes(&bytes);
        let observation_digest_ms = t.elapsed().as_secs_f64() * 1000.;
        cars.insert(id,json!({"result_digest":result_digest,"observation_digest":observation_digest,"native_nodes":r.solution.t.len(),"observation_rows":rows.len(),"observation_json_bytes":bytes.len(),"last_observed_time_s":rows.last().unwrap().time_s,"total_time_s":r.total_time_s,"final_state":r.final_state,"energy":r.longitudinal_energy_v1,"timings_ms":{"solve":solve_ms,"native_digest":native_digest_ms,"observation":observation_ms,"observation_json":observation_json_ms,"observation_digest":observation_digest_ms}}));
    }
    println!(
        "{}",
        json!({"schema":"pitgun.energy-cost-probe/v1","request_digest":canonical_json_digest(&request)?,"catalog":catalog.release_identity(),"binary_digest":Digest::from_bytes(&fs::read(env::current_exe()?)?),"sources":{"solver":Digest::from_bytes(include_bytes!("../../pitgun-racing-solver/src/lib.rs")),"longitudinal":Digest::from_bytes(include_bytes!("../../pitgun-racing-solver/src/longitudinal_energy.rs")),"simulator":Digest::from_bytes(include_bytes!("../src/lib.rs")),"probe":Digest::from_bytes(include_bytes!("energy_cost_probe.rs"))},"observation_hz":hz,"resolve_ms":resolve_ms,"cars":cars,"scope":"native audit JSON only; causal observations, no changed integration grid/equations or game admission; rendering/network/gateway packing not measured"})
    );
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn solution() -> SimulationSolution {
        SimulationSolution {
            t: vec![0., 0.75, 1.25],
            s: vec![0., 10., 20.],
            v: vec![0., 10., 20.],
            temp: vec![90.; 3],
            tire_temp: vec![90.; 3],
            tire_wear: vec![0.; 3],
            gear: vec![1; 3],
            lap_index: vec![1; 3],
            ..Default::default()
        }
    }
    #[test]
    fn observations_are_causal_include_final_and_leave_native_bytes_unchanged() {
        let s = solution();
        let before = canonical_json_digest(&s).unwrap();
        let slow = observe(&s, 1.).unwrap();
        let fast = observe(&s, 5.).unwrap();
        assert_eq!(
            slow.iter().map(|r| r.time_s).collect::<Vec<_>>(),
            vec![0., 1., 1.25]
        );
        assert_eq!(slow[1].distance_m, 10.);
        assert_eq!(slow[1].native_time_s, 0.75);
        assert!(fast.len() > slow.len());
        assert_eq!(fast.last().unwrap().distance_m, 20.);
        assert!(fast.iter().all(|r| r.native_time_s <= r.time_s));
        assert_eq!(before, canonical_json_digest(&s).unwrap());
    }
    #[test]
    fn observations_refuse_oversized_or_inconsistent_requests() {
        let s = solution();
        for hz in [0., f64::NAN, 11.] {
            assert!(observe(&s, hz).is_err());
        }
        let mut bad = s;
        bad.v.pop();
        assert!(observe(&bad, 5.).is_err());
        let mut bad = solution();
        bad.t[1] = 0.;
        assert!(observe(&bad, 5.).is_err());
        let mut bad = solution();
        bad.t[2] = 1_000_001.;
        assert!(observe(&bad, 1.).is_err());
    }
}
