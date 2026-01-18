use pitgun_codec_json::{EventBatchDto, SESSION_ENVELOPE_SCHEMA_VERSION};
use pitgun_contract::game::v1::GamePlayerTuningV1;
use serde::{Deserialize, Serialize};

use crate::game::batching::telemetry_to_event_batches;
use crate::game::competitors::{generate_competitors, CompetitorArchetype};
use crate::game::load_track;
use crate::game::summary::compute_summary_metrics;
use crate::{simulate_stint, TrackPoint};

const DEFAULT_SESSION_HZ: f32 = 10.0;
const DEFAULT_COMPETITOR_COUNT: u32 = 9;
const DEFAULT_BATCH_EVENTS: usize = 260;
const OVERHEAT_TEMP_C: f32 = 110.0;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct GameSessionRequestV1 {
    pub track_id: String,
    pub seed: u64,
    #[serde(default = "default_session_hz")]
    pub hz: f32,
    pub laps: u32,
    pub player_tuning: GamePlayerTuningV1,
    #[serde(default = "default_competitor_count")]
    pub competitor_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_budget_t: Option<i32>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SessionMetadataV1 {
    pub session_id: String,
    pub track_id: String,
    pub seed: u64,
    pub hz: f32,
    pub laps: u32,
    pub total_budget_t: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct PlayerSessionSummaryV1 {
    pub total_time_s: f32,
    pub avg_lap_s: f32,
    pub vmax_kph: f32,
    pub rpm_max: f32,
    pub temp_max_c: f32,
    pub overheat_count: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CompetitorSummaryV1 {
    pub competitor_id: u32,
    pub display_name: String,
    pub archetype: CompetitorArchetype,
    pub total_time_s: f32,
    pub avg_lap_s: f32,
    pub vmax_kph: f32,
    pub rpm_max: f32,
    pub temp_max_c: f32,
    pub overheat_count: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SessionParticipantKindV1 {
    Player,
    Competitor,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SessionStandingEntryV1 {
    pub kind: SessionParticipantKindV1,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub competitor_id: Option<u32>,
    pub display_name: String,
    pub total_time_s: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionEnvelopeV1 {
    pub schema_version: u32,
    pub session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sent_at_ms: Option<i64>,
    pub batch: EventBatchDto,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GameSessionResultV1 {
    pub metadata: SessionMetadataV1,
    pub player_summary: PlayerSessionSummaryV1,
    pub competitor_summaries: Vec<CompetitorSummaryV1>,
    pub standings: Vec<SessionStandingEntryV1>,
    pub player_batches: Vec<SessionEnvelopeV1>,
}

#[derive(Debug)]
pub enum GameSessionError {
    InvalidRequest(String),
    TrackNotFound(String),
    TrackLoad(String),
    Physics(String),
}

impl std::fmt::Display for GameSessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GameSessionError::InvalidRequest(msg) => write!(f, "invalid request: {msg}"),
            GameSessionError::TrackNotFound(track) => write!(f, "unknown track_id '{track}'"),
            GameSessionError::TrackLoad(msg) => write!(f, "track load failed: {msg}"),
            GameSessionError::Physics(msg) => write!(f, "simulation failed: {msg}"),
        }
    }
}

impl std::error::Error for GameSessionError {}

pub fn simulate_session(
    input: &GameSessionRequestV1,
) -> Result<GameSessionResultV1, GameSessionError> {
    let track_id = input.track_id.trim();
    if track_id.is_empty() {
        return Err(GameSessionError::InvalidRequest(
            "track_id must be non-empty".to_string(),
        ));
    }

    if !input.hz.is_finite() || input.hz < 1.0 || input.hz > 60.0 {
        return Err(GameSessionError::InvalidRequest(
            "hz must be within 1..60".to_string(),
        ));
    }

    if input.laps == 0 {
        return Err(GameSessionError::InvalidRequest(
            "laps must be >= 1".to_string(),
        ));
    }

    let tuning_budget = tuning_budget(&input.player_tuning);
    if tuning_budget < 0 {
        return Err(GameSessionError::InvalidRequest(
            "tuning points must be non-negative".to_string(),
        ));
    }

    let total_budget_t = input.total_budget_t.unwrap_or(tuning_budget);
    if total_budget_t != tuning_budget {
        return Err(GameSessionError::InvalidRequest(format!(
            "tuning budget must equal T={total_budget_t}"
        )));
    }

    let track = load_track(track_id).map_err(|err| match err {
        crate::game::GameSimulationError::TrackNotFound(track) => {
            GameSessionError::TrackNotFound(track)
        }
        crate::game::GameSimulationError::TrackLoad(msg) => GameSessionError::TrackLoad(msg),
        other => GameSessionError::Physics(other.to_string()),
    })?;

    let session_id = build_session_id(track_id, input.seed, input.laps);

    let player_telemetry =
        run_session_simulation(&track, &input.player_tuning, input.hz, input.laps)?;
    let player_summary = build_player_summary(&player_telemetry, input.laps);

    let batch_summary = telemetry_to_event_batches(&player_telemetry, DEFAULT_BATCH_EVENTS);
    let player_batches: Vec<SessionEnvelopeV1> = batch_summary
        .batches
        .iter()
        .map(|batch| SessionEnvelopeV1 {
            schema_version: SESSION_ENVELOPE_SCHEMA_VERSION,
            session_id: session_id.clone(),
            sent_at_ms: None,
            batch: EventBatchDto::from(batch),
        })
        .collect();

    let competitors =
        generate_competitors(input.seed, track_id, total_budget_t, input.competitor_count);

    let mut competitor_summaries = Vec::with_capacity(competitors.len());
    let mut standings = Vec::with_capacity(competitors.len() + 1);

    standings.push(SessionStandingEntryV1 {
        kind: SessionParticipantKindV1::Player,
        competitor_id: None,
        display_name: "Player".to_string(),
        total_time_s: player_summary.total_time_s,
    });

    for competitor in competitors {
        let telemetry = run_session_simulation(&track, &competitor.tuning, input.hz, input.laps)?;
        let summary = build_competitor_summary(&telemetry, input.laps, &competitor);

        standings.push(SessionStandingEntryV1 {
            kind: SessionParticipantKindV1::Competitor,
            competitor_id: Some(competitor.competitor_id),
            display_name: competitor.display_name.clone(),
            total_time_s: summary.total_time_s,
        });

        competitor_summaries.push(summary);
    }

    standings.sort_by(|a, b| {
        a.total_time_s
            .partial_cmp(&b.total_time_s)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.display_name.cmp(&b.display_name))
    });

    Ok(GameSessionResultV1 {
        metadata: SessionMetadataV1 {
            session_id,
            track_id: track_id.to_string(),
            seed: input.seed,
            hz: input.hz,
            laps: input.laps,
            total_budget_t,
        },
        player_summary,
        competitor_summaries,
        standings,
        player_batches,
    })
}

fn run_session_simulation(
    track: &[TrackPoint],
    tuning: &GamePlayerTuningV1,
    hz: f32,
    laps: u32,
) -> Result<Vec<pitgun_contract::game::v1::GameTelemetryPointV1>, GameSessionError> {
    simulate_stint(track, *tuning, hz, laps)
        .map_err(|err| GameSessionError::Physics(err.to_string()))
}

fn build_player_summary(
    telemetry: &[pitgun_contract::game::v1::GameTelemetryPointV1],
    laps: u32,
) -> PlayerSessionSummaryV1 {
    let summary = compute_summary_metrics(telemetry);
    let total_time_s = summary.lap_time_s;
    let avg_lap_s = avg_lap_time(total_time_s, laps);
    let overheat_count = count_overheat_points(telemetry);

    PlayerSessionSummaryV1 {
        total_time_s,
        avg_lap_s,
        vmax_kph: summary.vmax_kph,
        rpm_max: summary.rpm_max,
        temp_max_c: summary.temp_max_c,
        overheat_count,
    }
}

fn build_competitor_summary(
    telemetry: &[pitgun_contract::game::v1::GameTelemetryPointV1],
    laps: u32,
    competitor: &crate::game::competitors::CompetitorProfile,
) -> CompetitorSummaryV1 {
    let summary = compute_summary_metrics(telemetry);
    let total_time_s = summary.lap_time_s;
    let avg_lap_s = avg_lap_time(total_time_s, laps);
    let overheat_count = count_overheat_points(telemetry);

    CompetitorSummaryV1 {
        competitor_id: competitor.competitor_id,
        display_name: competitor.display_name.clone(),
        archetype: competitor.archetype,
        total_time_s,
        avg_lap_s,
        vmax_kph: summary.vmax_kph,
        rpm_max: summary.rpm_max,
        temp_max_c: summary.temp_max_c,
        overheat_count,
    }
}

fn avg_lap_time(total_time_s: f32, laps: u32) -> f32 {
    // TODO: Derive best_lap_s from lap boundary detection once lap splits are tracked.
    if laps == 0 {
        0.0
    } else {
        total_time_s / laps as f32
    }
}

fn count_overheat_points(telemetry: &[pitgun_contract::game::v1::GameTelemetryPointV1]) -> u32 {
    telemetry
        .iter()
        .filter(|point| point.engine_temp_c >= OVERHEAT_TEMP_C)
        .count() as u32
}

fn tuning_budget(tuning: &GamePlayerTuningV1) -> i32 {
    tuning.aero_points + tuning.chassis_points + tuning.engine_points + tuning.cooling_points
}

fn default_session_hz() -> f32 {
    DEFAULT_SESSION_HZ
}

fn default_competitor_count() -> u32 {
    DEFAULT_COMPETITOR_COUNT
}

fn build_session_id(track_id: &str, seed: u64, laps: u32) -> String {
    format!("{track_id}-{seed}-{laps}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_tuning() -> GamePlayerTuningV1 {
        GamePlayerTuningV1 {
            aero_points: 10,
            chassis_points: 10,
            engine_points: 10,
            cooling_points: 10,
            downforce_slider: 0.5,
            gear_ratio_slider: 0.5,
        }
    }

    #[test]
    fn session_is_deterministic() {
        let request = GameSessionRequestV1 {
            track_id: "demo-oval".to_string(),
            seed: 123,
            hz: 10.0,
            laps: 2,
            player_tuning: sample_tuning(),
            competitor_count: 3,
            total_budget_t: Some(40),
        };

        let a = simulate_session(&request).expect("session");
        let b = simulate_session(&request).expect("session");

        assert_eq!(a.competitor_summaries, b.competitor_summaries);
        assert_eq!(a.standings, b.standings);
    }

    #[test]
    fn session_returns_player_batches_and_competitors() {
        let request = GameSessionRequestV1 {
            track_id: "demo-oval".to_string(),
            seed: 7,
            hz: 10.0,
            laps: 1,
            player_tuning: sample_tuning(),
            competitor_count: 2,
            total_budget_t: Some(40),
        };

        let result = simulate_session(&request).expect("session");
        assert_eq!(result.competitor_summaries.len(), 2);
        assert_eq!(result.standings.len(), 3);
        assert!(!result.player_batches.is_empty());
        assert!(!result.player_batches[0].batch.events.is_empty());
    }
}
