use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub mod batching;
pub mod events;
pub mod json;
pub mod summary;
pub mod udp;

use pitgun_contract::game::v1::{
    GameSimulationRequestV1, GameSimulationResultV1, GameTelemetryPointV1, GameTelemetrySummaryV1,
    DEFAULT_HZ,
};

use crate::{load_track_from_csv_bytes, load_track_from_csv_path, run_simulation, TrackPoint};

pub const DEFAULT_TRACK_ID: &str = "demo-oval";
const DEFAULT_TRACK_CSV: &str = include_str!("../../assets/tracks/demo-oval.csv");

#[derive(Debug)]
pub enum GameSimulationError {
    InvalidRequest(String),
    TrackNotFound(String),
    TrackLoad(String),
    Physics(String),
}

impl std::fmt::Display for GameSimulationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GameSimulationError::InvalidRequest(msg) => write!(f, "invalid request: {msg}"),
            GameSimulationError::TrackNotFound(track) => {
                write!(f, "unknown track_id '{track}'")
            }
            GameSimulationError::TrackLoad(msg) => write!(f, "track load failed: {msg}"),
            GameSimulationError::Physics(msg) => write!(f, "simulation failed: {msg}"),
        }
    }
}

impl std::error::Error for GameSimulationError {}

#[derive(Clone, Debug)]
pub struct TrackRegistry {
    tracks: HashMap<String, TrackSource>,
}

#[derive(Clone, Debug)]
enum TrackSource {
    Embedded(&'static str),
    Path(PathBuf),
}

impl Default for TrackRegistry {
    fn default() -> Self {
        let mut registry = Self {
            tracks: HashMap::new(),
        };
        registry.insert_embedded(DEFAULT_TRACK_ID, DEFAULT_TRACK_CSV);
        registry
    }
}

impl TrackRegistry {
    pub fn insert_embedded(&mut self, track_id: impl Into<String>, csv: &'static str) {
        self.tracks
            .insert(track_id.into(), TrackSource::Embedded(csv));
    }

    pub fn insert_path(&mut self, track_id: impl Into<String>, path: impl Into<PathBuf>) {
        self.tracks
            .insert(track_id.into(), TrackSource::Path(path.into()));
    }

    pub fn load(&self, track_id: &str) -> Result<Vec<TrackPoint>, GameSimulationError> {
        let Some(source) = self.tracks.get(track_id) else {
            return Err(GameSimulationError::TrackNotFound(track_id.to_string()));
        };

        match source {
            TrackSource::Embedded(csv) => load_track_from_csv_bytes(csv.as_bytes())
                .map_err(|err| GameSimulationError::TrackLoad(err.to_string())),
            TrackSource::Path(path) => load_track_from_csv_path(path)
                .map_err(|err| GameSimulationError::TrackLoad(err.to_string())),
        }
    }
}

pub fn simulate_request(
    req: &GameSimulationRequestV1,
) -> Result<GameSimulationResultV1, GameSimulationError> {
    let registry = TrackRegistry::default();
    simulate_request_with_registry(req, &registry)
}

pub fn simulate_request_with_registry(
    req: &GameSimulationRequestV1,
    registry: &TrackRegistry,
) -> Result<GameSimulationResultV1, GameSimulationError> {
    let track_id = req.track_id.trim();
    if track_id.is_empty() {
        return Err(GameSimulationError::InvalidRequest(
            "track_id must be non-empty".to_string(),
        ));
    }

    let hz = if req.hz == 0.0 {
        DEFAULT_HZ
    } else if req.hz.is_finite() && req.hz > 0.0 {
        req.hz
    } else {
        return Err(GameSimulationError::InvalidRequest(
            "hz must be finite and > 0".to_string(),
        ));
    };

    let track = registry.load(track_id)?;
    let telemetry = run_simulation(&track, req.tuning, hz)
        .map_err(|err| GameSimulationError::Physics(err.to_string()))?;

    let summary = summarize_telemetry(&telemetry);
    let lap_time_s = telemetry.last().map(|point| point.time_s).unwrap_or(0.0);

    Ok(GameSimulationResultV1 {
        lap_time_s,
        summary,
        telemetry: Some(telemetry),
        telemetry_ref: None,
    })
}

pub fn summarize_telemetry(telemetry: &[GameTelemetryPointV1]) -> GameTelemetrySummaryV1 {
    if telemetry.is_empty() {
        return GameTelemetrySummaryV1 {
            lap_time_s: 0.0,
            max_speed_kph: 0.0,
            avg_speed_kph: 0.0,
            max_rpm: 0.0,
            max_g_lat: 0.0,
            max_g_long: 0.0,
            max_engine_temp_c: 0.0,
        };
    }

    let mut max_speed: f32 = 0.0;
    let mut max_rpm: f32 = 0.0;
    let mut max_g_lat: f32 = 0.0;
    let mut max_g_long: f32 = 0.0;
    let mut max_temp: f32 = 0.0;
    let mut sum_speed: f32 = 0.0;

    for point in telemetry {
        max_speed = max_speed.max(point.speed_kph);
        max_rpm = max_rpm.max(point.rpm);
        max_g_lat = max_g_lat.max(point.g_lat);
        max_g_long = max_g_long.max(point.g_long);
        max_temp = max_temp.max(point.engine_temp_c);
        sum_speed += point.speed_kph;
    }

    let lap_time_s = telemetry.last().map(|point| point.time_s).unwrap_or(0.0);
    let avg_speed_kph = sum_speed / telemetry.len() as f32;

    GameTelemetrySummaryV1 {
        lap_time_s,
        max_speed_kph: max_speed,
        avg_speed_kph,
        max_rpm,
        max_g_lat,
        max_g_long,
        max_engine_temp_c: max_temp,
    }
}

pub fn load_track_from_path(
    path: impl AsRef<Path>,
) -> Result<Vec<TrackPoint>, GameSimulationError> {
    load_track_from_csv_path(path).map_err(|err| GameSimulationError::TrackLoad(err.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pitgun_contract::game::v1::GamePlayerTuningV1;

    #[test]
    fn deterministic_simulation_result() {
        let req = GameSimulationRequestV1 {
            tuning: GamePlayerTuningV1 {
                aero_points: 10,
                chassis_points: 10,
                engine_points: 10,
                cooling_points: 10,
                downforce_slider: 0.5,
                gear_ratio_slider: 0.5,
            },
            track_id: DEFAULT_TRACK_ID.to_string(),
            hz: 60.0,
            seed: Some(1),
            engine_version: Some("0.1.0".to_string()),
        };

        let first = simulate_request(&req).expect("simulate");
        let second = simulate_request(&req).expect("simulate again");

        assert_eq!(first.lap_time_s, second.lap_time_s);
        assert_eq!(first.summary, second.summary);
        assert_eq!(
            first.telemetry.as_ref().unwrap().len(),
            second.telemetry.as_ref().unwrap().len()
        );
    }
}
