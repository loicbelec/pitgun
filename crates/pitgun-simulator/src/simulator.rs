use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::drivers::{default_driver_id, deterministic_lap_delta_ms, driver_effects};
use crate::errors::SimulatorError;
use crate::profiles::CompetitorProfile;
use crate::provider::ConfigProvider;
use crate::runtime::{SimulationRunRequest, run_simulation};
use crate::state::SimulatorState;
use crate::telemetry::TelemetryFrame;
use crate::tuning::Tuning;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LapInput {
    pub vehicle_id: String,
    pub track_id: String,
    #[serde(default)]
    pub tuning: Tuning,
    #[serde(default)]
    pub profile_id: Option<String>,
    #[serde(default)]
    pub profile: Option<CompetitorProfile>,
    #[serde(default)]
    pub driver_id: Option<String>,
    #[serde(default)]
    pub tire_id: Option<String>,
    #[serde(default)]
    pub initial_state: Option<SimulatorState>,
    #[serde(default)]
    pub seed: Option<u64>,
    #[serde(default)]
    pub lap_number: Option<u16>,
    #[serde(default = "default_hz")]
    pub hz: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LapOutput {
    pub lap_time_s: f64,
    pub average_speed_kph: f64,
    pub fuel_used_kg: f64,
    pub final_state: SimulatorState,
    pub telemetry: Vec<TelemetryFrame>,
    pub max_engine_temp_c: f64,
    pub max_tire_temp_c: f64,
}

#[derive(Clone)]
pub struct Simulator {
    provider: Arc<dyn ConfigProvider>,
}

impl Simulator {
    pub fn new(provider: Arc<dyn ConfigProvider>) -> Self {
        Self { provider }
    }

    pub fn simulate_lap(&self, input: LapInput) -> Result<LapOutput, SimulatorError> {
        let driver_id = input
            .driver_id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .unwrap_or(default_driver_id());
        let driver = self
            .provider
            .get_driver(driver_id)
            .or_else(|_| self.provider.get_driver(default_driver_id()))?;
        let effects = driver_effects(&driver);
        let lap_number = input.lap_number.unwrap_or(1).max(1);
        let initial_state = input.initial_state.unwrap_or_default();
        let initial_fuel_mass_kg = initial_state.fuel_mass_kg.max(0.0);

        let mut output = run_simulation(
            self.provider.as_ref(),
            &SimulationRunRequest {
                vehicle_id: input.vehicle_id.clone(),
                track_id: input.track_id.clone(),
                tuning: map_tuning(&input.tuning),
                initial_state: Some(map_state_to_solver(&initial_state)),
                lap_count: 1,
                pit_plan: Vec::new(),
                driver_id: Some(driver.id.clone()),
                tire_id: input.tire_id.clone(),
                profile_id: input.profile_id.clone(),
                profile: input.profile.clone(),
                seed: input.seed,
                telemetry_hz: Some(if input.hz > 0.0 {
                    input.hz
                } else {
                    default_hz()
                }),
            },
        )?;

        let mut telemetry = telemetry_frames_from_resampled(
            output.telemetry.take().unwrap_or_default(),
            lap_number,
        );
        let mut lap_time_s = output.simulation.total_time_s;
        let lap_delta_ms = deterministic_lap_delta_ms(&effects, &driver.id, input.seed, lap_number);
        if lap_delta_ms != 0 {
            apply_lap_delta(&mut lap_time_s, &mut telemetry, lap_delta_ms);
        }

        let distance_m = output.simulation.solution.s.last().copied().unwrap_or(0.0);
        let average_speed_kph = if lap_time_s > 0.0 {
            (distance_m / lap_time_s) * 3.6
        } else {
            0.0
        };
        let fuel_used_kg =
            (initial_fuel_mass_kg - output.simulation.final_state.fuel_mass).max(0.0);
        let max_engine_temp_c = telemetry
            .iter()
            .map(|frame| frame.engine_temp_c)
            .reduce(f64::max)
            .unwrap_or(output.simulation.final_state.engine_temp);
        let max_tire_temp_c = telemetry
            .iter()
            .map(|frame| frame.tire_temp_c)
            .reduce(f64::max)
            .unwrap_or(output.simulation.final_state.tire_temp);

        Ok(LapOutput {
            lap_time_s,
            average_speed_kph,
            fuel_used_kg,
            final_state: map_state_from_solver(output.simulation.final_state),
            telemetry,
            max_engine_temp_c,
            max_tire_temp_c,
        })
    }
}

fn default_hz() -> f64 {
    20.0
}

fn map_tuning(value: &Tuning) -> pitgun_solver::Tuning {
    pitgun_solver::Tuning {
        aero_points: (value.aero_points / 2.0).round() as i32,
        chassis_points: (value.chassis_points / 2.0).round() as i32,
        cooling_points: (value.cooling_points / 2.0).round() as i32,
        engine_points: (value.engine_points / 2.0).round() as i32,
        downforce_slider: value.downforce_slider,
        gear_ratio_slider: value.gear_ratio_slider,
    }
}

fn map_state_to_solver(value: &SimulatorState) -> pitgun_solver::VehicleState {
    pitgun_solver::VehicleState {
        fuel_mass: value.fuel_mass_kg,
        tire_wear: value.tire_wear,
        tire_temp: value.tire_temp_c,
        engine_temp: value.engine_temp_c,
        battery_soc: value.battery_soc,
        exit_speed_mps: value.exit_speed_mps,
        exit_gear: value.exit_gear,
    }
}

fn map_state_from_solver(value: pitgun_solver::VehicleState) -> SimulatorState {
    SimulatorState {
        fuel_mass_kg: value.fuel_mass,
        tire_wear: value.tire_wear,
        tire_temp_c: value.tire_temp,
        engine_temp_c: value.engine_temp,
        battery_soc: value.battery_soc,
        exit_speed_mps: value.exit_speed_mps,
        exit_gear: value.exit_gear,
    }
}

fn telemetry_frames_from_resampled(
    telemetry: pitgun_solver::ResampledTelemetry,
    fallback_lap_number: u16,
) -> Vec<TelemetryFrame> {
    let tire_temp = telemetry
        .tire_temp_c
        .clone()
        .unwrap_or_else(|| vec![0.0; telemetry.time_s.len()]);
    let tire_wear = telemetry
        .tire_wear_pct
        .clone()
        .unwrap_or_else(|| vec![0.0; telemetry.time_s.len()]);
    let tire_mu = telemetry
        .tire_mu
        .clone()
        .unwrap_or_else(|| vec![0.0; telemetry.time_s.len()]);
    let lap_numbers = telemetry
        .n_lap
        .clone()
        .unwrap_or_else(|| vec![fallback_lap_number; telemetry.time_s.len()]);

    let mut frames = Vec::with_capacity(telemetry.time_s.len());
    for idx in 0..telemetry.time_s.len() {
        frames.push(TelemetryFrame {
            time_s: telemetry.time_s[idx],
            s_m: telemetry.s_m[idx],
            x_m: telemetry.x_m[idx],
            y_m: telemetry.y_m[idx],
            heading_rad: telemetry.heading_rad[idx],
            speed_kph: telemetry.speed_kph[idx],
            rpm: telemetry.rpm[idx],
            gear: telemetry.gear[idx],
            throttle_pct: telemetry.throttle_pct[idx],
            brake_pct: telemetry.brake_pct[idx],
            g_lat: telemetry.g_lat[idx],
            g_long: telemetry.g_long[idx],
            g_vert: telemetry.g_vert[idx],
            engine_temp_c: telemetry.engine_temp_c[idx],
            engine_power_w: telemetry.engine_power_w[idx],
            tire_temp_c: tire_temp[idx],
            tire_wear_pct: tire_wear[idx],
            tire_mu: Some(tire_mu[idx]),
            n_lap: Some(lap_numbers[idx]),
        });
    }
    frames
}

fn apply_lap_delta(lap_time_s: &mut f64, telemetry: &mut [TelemetryFrame], lap_delta_ms: i32) {
    let adjusted_lap_time_s = (*lap_time_s + lap_delta_ms as f64 / 1000.0).max(0.1);
    let scale = adjusted_lap_time_s / lap_time_s.max(1e-6);
    *lap_time_s = adjusted_lap_time_s;
    for frame in telemetry {
        frame.time_s *= scale;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::default_in_memory_provider;
    use crate::runtime;

    #[test]
    fn lap_simulation_produces_finite_values() {
        let provider = Arc::new(default_in_memory_provider());
        let simulator = Simulator::new(provider);

        let output = simulator
            .simulate_lap(LapInput {
                vehicle_id: "f1_2026".to_string(),
                track_id: "SPA".to_string(),
                tuning: Tuning {
                    engine_points: 12.0,
                    cooling_points: 9.0,
                    aero_points: 12.0,
                    chassis_points: 7.0,
                    downforce_slider: 0.6,
                    gear_ratio_slider: 0.5,
                },
                profile_id: Some("balanced".to_string()),
                profile: None,
                driver_id: None,
                tire_id: None,
                initial_state: None,
                seed: None,
                lap_number: None,
                hz: 20.0,
            })
            .expect("lap simulation");

        assert!(output.lap_time_s > 20.0);
        assert!(output.average_speed_kph > 20.0);
        assert!(output.telemetry.len() > 50);
        assert!(output.telemetry.iter().all(|f| f.speed_kph.is_finite()));
        assert!(output.telemetry.iter().all(|f| f.tire_mu.is_some()));
        assert!(output.telemetry.iter().all(|f| f.n_lap == Some(1)));
        assert!(output.fuel_used_kg > 0.0);
        assert!(output.final_state.fuel_mass_kg < 100.0);
    }

    #[test]
    fn aggressive_profile_is_not_identical_to_conservative() {
        let provider = Arc::new(default_in_memory_provider());
        let simulator = Simulator::new(provider.clone());

        let aggressive = simulator
            .simulate_lap(LapInput {
                vehicle_id: "f1_2026".to_string(),
                track_id: "MONZA".to_string(),
                tuning: Tuning::default(),
                profile_id: Some("aggressive".to_string()),
                profile: None,
                driver_id: None,
                tire_id: None,
                initial_state: None,
                seed: None,
                lap_number: None,
                hz: 20.0,
            })
            .expect("aggressive run");

        let conservative = simulator
            .simulate_lap(LapInput {
                vehicle_id: "f1_2026".to_string(),
                track_id: "MONZA".to_string(),
                tuning: Tuning::default(),
                profile_id: Some("conservative".to_string()),
                profile: None,
                driver_id: None,
                tire_id: None,
                initial_state: None,
                seed: None,
                lap_number: None,
                hz: 20.0,
            })
            .expect("conservative run");

        assert!(
            (aggressive.lap_time_s - conservative.lap_time_s).abs() > 1e-4,
            "profile branches should affect lap time"
        );
    }

    #[test]
    fn carried_exit_speed_affects_next_lap_entry_speed() {
        let provider = Arc::new(default_in_memory_provider());
        let simulator = Simulator::new(provider);

        let fresh = simulator
            .simulate_lap(LapInput {
                vehicle_id: "f1_2026".to_string(),
                track_id: "MONZA".to_string(),
                tuning: Tuning::default(),
                profile_id: Some("balanced".to_string()),
                profile: None,
                driver_id: None,
                tire_id: None,
                initial_state: Some(SimulatorState {
                    exit_speed_mps: 0.0,
                    exit_gear: 1,
                    ..SimulatorState::default()
                }),
                seed: None,
                lap_number: None,
                hz: 20.0,
            })
            .expect("fresh lap");

        let carried = simulator
            .simulate_lap(LapInput {
                vehicle_id: "f1_2026".to_string(),
                track_id: "MONZA".to_string(),
                tuning: Tuning::default(),
                profile_id: Some("balanced".to_string()),
                profile: None,
                driver_id: None,
                tire_id: None,
                initial_state: Some(SimulatorState {
                    exit_speed_mps: 80.0,
                    exit_gear: 6,
                    ..SimulatorState::default()
                }),
                seed: None,
                lap_number: None,
                hz: 20.0,
            })
            .expect("carried lap");

        let fresh_entry_speed = fresh
            .telemetry
            .first()
            .map(|frame| frame.speed_kph)
            .unwrap_or(0.0);
        let carried_entry_speed = carried
            .telemetry
            .first()
            .map(|frame| frame.speed_kph)
            .unwrap_or(0.0);

        assert!(
            carried_entry_speed > fresh_entry_speed + 20.0,
            "expected carried lap to start faster: fresh={fresh_entry_speed:.2} carried={carried_entry_speed:.2}"
        );
        assert!(carried.final_state.exit_speed_mps > 0.0);
        assert!(carried.final_state.exit_gear >= 1);
    }

    #[test]
    fn simulator_facade_matches_runtime_without_noise() {
        let provider = Arc::new(default_in_memory_provider());
        let simulator = Simulator::new(provider.clone());

        let lap = simulator
            .simulate_lap(LapInput {
                vehicle_id: "f1_2026".to_string(),
                track_id: "MONZA".to_string(),
                tuning: Tuning::default(),
                profile_id: Some("balanced".to_string()),
                profile: None,
                driver_id: Some("default".to_string()),
                tire_id: None,
                initial_state: None,
                seed: None,
                lap_number: Some(1),
                hz: 20.0,
            })
            .expect("simulator lap");

        let runtime = runtime::run_simulation(
            provider.as_ref(),
            &runtime::SimulationRunRequest {
                vehicle_id: "f1_2026".to_string(),
                track_id: "MONZA".to_string(),
                tuning: map_tuning(&Tuning::default()),
                initial_state: Some(map_state_to_solver(&SimulatorState::default())),
                lap_count: 1,
                pit_plan: Vec::new(),
                driver_id: Some("default".to_string()),
                tire_id: None,
                profile_id: Some("balanced".to_string()),
                profile: None,
                seed: None,
                telemetry_hz: Some(20.0),
            },
        )
        .expect("runtime lap");

        let runtime_telemetry =
            telemetry_frames_from_resampled(runtime.telemetry.expect("runtime telemetry"), 1);
        let driver = provider.get_driver("default").expect("default driver");
        let effects = driver_effects(&driver);
        let expected_delta_s =
            deterministic_lap_delta_ms(&effects, &driver.id, None, 1) as f64 / 1000.0;

        assert!(
            (lap.lap_time_s - (runtime.simulation.total_time_s + expected_delta_s)).abs() < 1e-9
        );
        assert_eq!(lap.telemetry.len(), runtime_telemetry.len());
        assert!(lap.telemetry[0].time_s <= runtime_telemetry[0].time_s + 1e-9);
    }
}
