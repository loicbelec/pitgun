//! Bounded diagnostic of the actual qualified engine/transmission, not a fit.
use pitgun_racing_simulator::{RacingCatalogSnapshot, RunRaceRequest, qualified_career};
use pitgun_racing_solver::{ResolvedSimulationSessionV3, ResolvedSimulationStepV3};
use serde_json::{Value, json};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let catalog = RacingCatalogSnapshot::embedded_qualified_career()?;
    let refs: Value = serde_json::from_str(include_str!(
        "../../../experiments/qualified_career_v1/native-reference-v1.json"
    ))?;
    let mut request: RunRaceRequest =
        serde_json::from_value(refs["vectors"][0]["request"].clone())?;
    request.input.race.laps = 1;
    request.input.pit_strategy = None;
    request.input.race.competitors[0].stint_strategy = Some(serde_json::from_value(
        json!({"stints":[{"tire_id":"medium","laps":1}],"pit_laps":[]}),
    )?);
    let request = qualified_career::prepare_request(request, &catalog)?;
    let mut inputs = qualified_career::resolve_inputs(&request, &catalog)?;
    let input = inputs.remove("player").unwrap();
    let engine = input.vehicle.engine.clone();
    let mechanical = input.mechanical;
    let radius = input.vehicle.chassis.r_wheel;
    let mut session = ResolvedSimulationSessionV3::new(input)?;
    let result = loop {
        if let ResolvedSimulationStepV3::Complete(result) = session.advance()? {
            break result;
        }
    };
    let max_rpm = result
        .solution
        .v
        .iter()
        .zip(&result.solution.gear)
        .map(|(v, g)| {
            v * engine.gear_ratios[usize::from(*g) - 1] / radius * 60. / std::f64::consts::TAU
        })
        .fold(0., f64::max);
    println!(
        "{}",
        serde_json::to_string_pretty(
            &json!({"scope":"One actual Monza I player lap; diagnostic only", "max_gear":result.solution.gear.iter().max(),"max_rpm":max_rpm,"upshift_rpm":mechanical.upshift_rpm,"redline_rpm":engine.n_max,"gear_ratios":engine.gear_ratios,"max_speed_kph":result.solution.v.iter().copied().fold(0.,f64::max)*3.6,"time_s":result.total_time_s})
        )?
    );
    Ok(())
}
