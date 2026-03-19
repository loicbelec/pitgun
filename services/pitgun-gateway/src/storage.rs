use std::{path::Path, str::FromStr};

use anyhow::Context;
use sqlx::{
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use time::{OffsetDateTime, UtcOffset, format_description::well_known::Rfc3339};

use crate::insight_requests::{InsightRequestPayload, LapSummaryPayload};
use crate::insight_responses::InsightResponsePayload;
use crate::model::EventEnvelope;

#[derive(Clone, Debug)]
pub struct IngestMetadata {
    pub remote_ip: Option<String>,
    pub user_agent: Option<String>,
}

#[derive(Clone, Debug)]
pub struct QueueMessage {
    pub envelope: EventEnvelope,
    pub raw_json: String,
    pub meta: IngestMetadata,
    pub received_at: OffsetDateTime,
}

impl QueueMessage {
    pub fn new(envelope: EventEnvelope, raw_json: String, meta: IngestMetadata) -> Self {
        Self {
            envelope,
            raw_json,
            meta,
            received_at: OffsetDateTime::now_utc(),
        }
    }
}

#[derive(Clone)]
pub struct SqliteEventStore {
    pool: SqlitePool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SqliteJournalMode {
    Wal,
    Delete,
    Truncate,
    Persist,
    Memory,
    Off,
}

impl SqliteJournalMode {
    pub fn from_env_value(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "wal" => Some(Self::Wal),
            "delete" => Some(Self::Delete),
            "truncate" => Some(Self::Truncate),
            "persist" => Some(Self::Persist),
            "memory" => Some(Self::Memory),
            "off" => Some(Self::Off),
            _ => None,
        }
    }

    fn pragma_value(self) -> &'static str {
        match self {
            Self::Wal => "WAL",
            Self::Delete => "DELETE",
            Self::Truncate => "TRUNCATE",
            Self::Persist => "PERSIST",
            Self::Memory => "MEMORY",
            Self::Off => "OFF",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventInsertOutcome {
    Inserted,
    Duplicate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InsightRequestInsertOutcome {
    Inserted,
    Duplicate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InsightResponseInsertOutcome {
    Inserted,
    Duplicate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LapSummaryInsertOutcome {
    Inserted,
    Duplicate,
}

impl SqliteEventStore {
    pub async fn new(db_path: &str, journal_mode: SqliteJournalMode) -> anyhow::Result<Self> {
        ensure_parent_dir_exists(db_path).await?;

        let db_url = sqlite_url(db_path);
        let connect_options = SqliteConnectOptions::from_str(&db_url)
            .with_context(|| format!("invalid sqlite database url: {db_url}"))?
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(10)
            .connect_with(connect_options)
            .await
            .with_context(|| format!("failed to connect to sqlite database at {db_path}"))?;

        let journal_mode_sql = format!("PRAGMA journal_mode = {};", journal_mode.pragma_value());
        sqlx::query(&journal_mode_sql).execute(&pool).await?;
        sqlx::query("PRAGMA synchronous = NORMAL;")
            .execute(&pool)
            .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS events (
                seq_id INTEGER PRIMARY KEY AUTOINCREMENT,
                event_id TEXT NOT NULL UNIQUE,
                schema_version TEXT NOT NULL,
                ts TEXT NOT NULL,
                player_id TEXT NOT NULL,
                weekend_id TEXT,
                session_id TEXT NOT NULL,
                event_type TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                envelope_json TEXT NOT NULL,
                received_at TEXT NOT NULL,
                remote_ip TEXT,
                user_agent TEXT
            );
            "#,
        )
        .execute(&pool)
        .await?;

        ensure_nullable_text_column(&pool, "events", "weekend_id").await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_events_session_ts ON events(session_id, ts);")
            .execute(&pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_events_player_ts ON events(player_id, ts);")
            .execute(&pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_events_weekend_ts ON events(weekend_id, ts);")
            .execute(&pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_events_type_ts ON events(event_type, ts);")
            .execute(&pool)
            .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS insight_requests (
                seq_id INTEGER PRIMARY KEY AUTOINCREMENT,
                trace_id TEXT NOT NULL UNIQUE,
                event_id TEXT NOT NULL,
                run_id TEXT NOT NULL,
                session_id TEXT NOT NULL,
                emitted_at_ms INTEGER NOT NULL,
                payload_json TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
            "#,
        )
        .execute(&pool)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_insight_requests_session_ts ON insight_requests(session_id, created_at);",
        )
        .execute(&pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS insight_responses (
                seq_id INTEGER PRIMARY KEY AUTOINCREMENT,
                trace_id TEXT NOT NULL UNIQUE,
                run_id TEXT NOT NULL,
                session_id TEXT NOT NULL,
                status TEXT NOT NULL,
                generated_at_ms INTEGER NOT NULL,
                latency_ms INTEGER,
                source_model TEXT,
                payload_json TEXT NOT NULL,
                raw_model_response TEXT,
                created_at TEXT NOT NULL
            );
            "#,
        )
        .execute(&pool)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_insight_responses_session_ts ON insight_responses(session_id, created_at);",
        )
        .execute(&pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS lap_summaries (
                seq_id INTEGER PRIMARY KEY AUTOINCREMENT,
                summary_id TEXT NOT NULL UNIQUE,
                run_id TEXT NOT NULL,
                weekend_id TEXT,
                session_id TEXT NOT NULL,
                lap_number INTEGER NOT NULL,
                started_at_us INTEGER NOT NULL,
                ended_at_us INTEGER NOT NULL,
                payload_json TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
            "#,
        )
        .execute(&pool)
        .await?;

        ensure_nullable_text_column(&pool, "lap_summaries", "weekend_id").await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_lap_summaries_run_lap ON lap_summaries(run_id, lap_number);",
        )
        .execute(&pool)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_lap_summaries_session_lap ON lap_summaries(session_id, lap_number);",
        )
        .execute(&pool)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_lap_summaries_weekend_lap ON lap_summaries(weekend_id, lap_number);",
        )
        .execute(&pool)
        .await?;

        Ok(Self { pool })
    }

    pub async fn insert_event(&self, msg: QueueMessage) -> anyhow::Result<EventInsertOutcome> {
        let event_ts = msg
            .envelope
            .ts
            .to_offset(UtcOffset::UTC)
            .format(&Rfc3339)
            .map_err(|err| anyhow::anyhow!("failed to format event ts: {err}"))?;

        let received_at = msg
            .received_at
            .format(&Rfc3339)
            .map_err(|err| anyhow::anyhow!("failed to format received_at: {err}"))?;

        let payload_json = serde_json::to_string(&msg.envelope.payload_json()?)?;

        let result = sqlx::query(
            r#"
            INSERT INTO events (
                event_id,
                schema_version,
                ts,
                player_id,
                weekend_id,
                session_id,
                event_type,
                payload_json,
                envelope_json,
                received_at,
                remote_ip,
                user_agent
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(event_id) DO NOTHING;
            "#,
        )
        .bind(msg.envelope.event_id.to_string())
        .bind(msg.envelope.schema_version)
        .bind(event_ts)
        .bind(msg.envelope.player_id)
        .bind(msg.envelope.weekend_id)
        .bind(msg.envelope.session_id)
        .bind(msg.envelope.event_type)
        .bind(payload_json)
        .bind(msg.raw_json)
        .bind(received_at)
        .bind(msg.meta.remote_ip)
        .bind(msg.meta.user_agent)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            Ok(EventInsertOutcome::Duplicate)
        } else {
            Ok(EventInsertOutcome::Inserted)
        }
    }

    pub async fn health_check(&self) -> anyhow::Result<()> {
        let _: i64 = sqlx::query_scalar("SELECT 1;")
            .fetch_one(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn insert_insight_request(
        &self,
        request: &InsightRequestPayload,
    ) -> anyhow::Result<InsightRequestInsertOutcome> {
        let payload_json = serde_json::to_string(request)?;
        let created_at = OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .map_err(|err| anyhow::anyhow!("failed to format insight request created_at: {err}"))?;

        let result = sqlx::query(
            r#"
            INSERT INTO insight_requests (
                trace_id,
                event_id,
                run_id,
                session_id,
                emitted_at_ms,
                payload_json,
                created_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(trace_id) DO NOTHING;
            "#,
        )
        .bind(&request.trace_id)
        .bind(&request.trace_id)
        .bind(&request.run_id)
        .bind(&request.session_id)
        .bind(request.emitted_at_ms)
        .bind(payload_json)
        .bind(created_at)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            Ok(InsightRequestInsertOutcome::Duplicate)
        } else {
            Ok(InsightRequestInsertOutcome::Inserted)
        }
    }

    pub async fn insert_insight_response(
        &self,
        response: &InsightResponsePayload,
        raw_model_response: Option<&str>,
    ) -> anyhow::Result<InsightResponseInsertOutcome> {
        let payload_json = serde_json::to_string(response)?;
        let created_at = OffsetDateTime::now_utc().format(&Rfc3339).map_err(|err| {
            anyhow::anyhow!("failed to format insight response created_at: {err}")
        })?;

        let result = sqlx::query(
            r#"
            INSERT INTO insight_responses (
                trace_id,
                run_id,
                session_id,
                status,
                generated_at_ms,
                latency_ms,
                source_model,
                payload_json,
                raw_model_response,
                created_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(trace_id) DO NOTHING;
            "#,
        )
        .bind(&response.trace_id)
        .bind(&response.run_id)
        .bind(&response.session_id)
        .bind(response.status.as_str())
        .bind(response.generated_at_ms)
        .bind(response.latency_ms)
        .bind(response.source_model.as_deref())
        .bind(payload_json)
        .bind(raw_model_response)
        .bind(created_at)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            Ok(InsightResponseInsertOutcome::Duplicate)
        } else {
            Ok(InsightResponseInsertOutcome::Inserted)
        }
    }

    pub async fn insert_lap_summary(
        &self,
        summary: &LapSummaryPayload,
    ) -> anyhow::Result<LapSummaryInsertOutcome> {
        let payload_json = serde_json::to_string(summary)?;
        let created_at = OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .map_err(|err| anyhow::anyhow!("failed to format lap summary created_at: {err}"))?;

        let result = sqlx::query(
            r#"
            INSERT INTO lap_summaries (
                summary_id,
                run_id,
                weekend_id,
                session_id,
                lap_number,
                started_at_us,
                ended_at_us,
                payload_json,
                created_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(summary_id) DO NOTHING;
            "#,
        )
        .bind(&summary.summary_id)
        .bind(&summary.run_id)
        .bind(summary.weekend_id.as_deref())
        .bind(&summary.session_id)
        .bind(summary.lap_number as i64)
        .bind(summary.started_at_us)
        .bind(summary.ended_at_us)
        .bind(payload_json)
        .bind(created_at)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            Ok(LapSummaryInsertOutcome::Duplicate)
        } else {
            Ok(LapSummaryInsertOutcome::Inserted)
        }
    }
}

fn sqlite_url(path: &str) -> String {
    if path.starts_with("sqlite:") {
        path.to_string()
    } else {
        format!("sqlite://{path}")
    }
}

async fn ensure_parent_dir_exists(path: &str) -> anyhow::Result<()> {
    if path.starts_with("sqlite::memory:") {
        return Ok(());
    }

    if path.starts_with("sqlite:") {
        return Ok(());
    }

    let db_path = Path::new(path);
    if let Some(parent) = db_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .with_context(|| format!("failed to create db directory {}", parent.display()))?;
    }

    Ok(())
}

async fn ensure_nullable_text_column(
    pool: &SqlitePool,
    table_name: &str,
    column_name: &str,
) -> anyhow::Result<()> {
    let exists_sql =
        format!("SELECT COUNT(*) FROM pragma_table_info('{table_name}') WHERE name = ?;");
    let exists: i64 = sqlx::query_scalar(&exists_sql)
        .bind(column_name)
        .fetch_one(pool)
        .await?;

    if exists == 0 {
        let alter_sql = format!("ALTER TABLE {table_name} ADD COLUMN {column_name} TEXT;");
        sqlx::query(&alter_sql).execute(pool).await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        EventInsertOutcome, IngestMetadata, InsightRequestInsertOutcome,
        InsightResponseInsertOutcome, LapSummaryInsertOutcome, QueueMessage, SqliteEventStore,
        SqliteJournalMode,
    };
    use crate::insight_requests::{
        InsightConstraints, InsightContext, InsightMetric, InsightRequestPayload, LapSummaryPayload,
    };
    use crate::insight_responses::{
        InsightError, InsightItem, InsightResponsePayload, InsightSeverity, InsightStatus,
    };
    use crate::model::parse_event_envelope;

    #[tokio::test]
    async fn deduplicates_by_event_id() {
        let store = SqliteEventStore::new("sqlite::memory:", SqliteJournalMode::Memory)
            .await
            .expect("store should be created");

        let raw = r#"{
            "schema_version": "pitgun-envelope-v1",
            "event_id": "c9c23d6f-7ca5-4064-a7df-f2bf9e95f5fd",
            "ts": "2026-02-11T10:00:00Z",
            "player_id": "player-a",
            "session_id": "session-a",
            "event_type": "session.start",
            "payload": {
                "game_version": "1.0.0"
            }
        }"#;

        let envelope =
            parse_event_envelope(raw, "pitgun-envelope-v1").expect("envelope should parse");

        let msg = QueueMessage::new(
            envelope.clone(),
            raw.to_string(),
            IngestMetadata {
                remote_ip: None,
                user_agent: None,
            },
        );

        let first = store
            .insert_event(msg)
            .await
            .expect("first insert should work");
        assert_eq!(first, EventInsertOutcome::Inserted);

        let msg = QueueMessage::new(
            envelope,
            raw.to_string(),
            IngestMetadata {
                remote_ip: None,
                user_agent: None,
            },
        );

        let second = store
            .insert_event(msg)
            .await
            .expect("second insert should work");
        assert_eq!(second, EventInsertOutcome::Duplicate);
    }

    #[tokio::test]
    async fn deduplicates_insight_request_by_trace_id() {
        let store = SqliteEventStore::new("sqlite::memory:", SqliteJournalMode::Memory)
            .await
            .expect("store should be created");

        let request = InsightRequestPayload {
            schema_version: "pitgun-insight-request-v1".to_string(),
            run_id: "run_123".to_string(),
            session_id: "session_123".to_string(),
            trace_id: "trace_123".to_string(),
            emitted_at_ms: 1_773_401_234_567,
            context: InsightContext {
                circuit_id: "MONACO".to_string(),
                era: 2026,
                lap: 12,
                position: Some(4),
                weather: Some("clear".to_string()),
                track_status: Some("green".to_string()),
            },
            metrics: vec![InsightMetric {
                key: "pace.speed_kph".to_string(),
                value: 212.4,
                unit: "kph".to_string(),
                trend: "up".to_string(),
                horizon: "lap".to_string(),
                confidence: 0.91,
            }],
            constraints: InsightConstraints {
                max_insights: 3,
                max_words_per_insight: 32,
                language: "en".to_string(),
            },
            policy_version: "policy.v1".to_string(),
            prompt_version: "chief-race.v1".to_string(),
        };

        let first = store
            .insert_insight_request(&request)
            .await
            .expect("first insert should work");
        assert_eq!(first, InsightRequestInsertOutcome::Inserted);

        let second = store
            .insert_insight_request(&request)
            .await
            .expect("second insert should work");
        assert_eq!(second, InsightRequestInsertOutcome::Duplicate);
    }

    #[tokio::test]
    async fn deduplicates_insight_response_by_trace_id() {
        let store = SqliteEventStore::new("sqlite::memory:", SqliteJournalMode::Memory)
            .await
            .expect("store should be created");

        let response = InsightResponsePayload {
            schema_version: "pitgun-insight-response-v1".to_string(),
            run_id: "run_123".to_string(),
            session_id: "session_123".to_string(),
            trace_id: "trace_123".to_string(),
            generated_at_ms: 1_773_401_234_622,
            latency_ms: Some(155),
            source_model: Some("llama3.2:3b".to_string()),
            status: InsightStatus::Error,
            insights: vec![InsightItem {
                id: "pit_window".to_string(),
                severity: InsightSeverity::Advisory,
                confidence: 0.8,
                title: "Open pit window".to_string(),
                rationale: "Wear is increasing".to_string(),
                recommendation: "Prepare stop in 2 laps".to_string(),
                metric_keys: Some(vec!["tires.avg_wear_pct.max".to_string()]),
                ttl_ms: Some(90_000),
                tags: Some(vec!["strategy".to_string()]),
            }],
            warnings: None,
            error: Some(InsightError {
                code: "llm_http_error".to_string(),
                message: "timeout".to_string(),
            }),
        };

        let first = store
            .insert_insight_response(&response, Some("{\"raw\":true}"))
            .await
            .expect("first insert should work");
        assert_eq!(first, InsightResponseInsertOutcome::Inserted);

        let second = store
            .insert_insight_response(&response, Some("{\"raw\":true}"))
            .await
            .expect("second insert should work");
        assert_eq!(second, InsightResponseInsertOutcome::Duplicate);
    }

    #[tokio::test]
    async fn deduplicates_lap_summary_by_summary_id() {
        let store = SqliteEventStore::new("sqlite::memory:", SqliteJournalMode::Memory)
            .await
            .expect("store should be created");

        let summary = LapSummaryPayload {
            schema_version: "pitgun-lap-summary-v1".to_string(),
            summary_id: "session-001:lap:12".to_string(),
            player_id: "player_001".to_string(),
            run_id: "run_001".to_string(),
            weekend_id: Some("weekend_001".to_string()),
            session_id: "session_001".to_string(),
            session_type: Some("FP1".to_string()),
            lap_number: 12,
            started_at_us: 1_773_401_000_000,
            ended_at_us: 1_773_491_000_000,
            context: InsightContext {
                circuit_id: "MONZA".to_string(),
                era: 2026,
                lap: 12,
                position: Some(3),
                weather: Some("clear".to_string()),
                track_status: Some("green".to_string()),
            },
            metrics: vec![InsightMetric {
                key: "pace.speed_kph.mean".to_string(),
                value: 210.2,
                unit: "kph".to_string(),
                trend: "unknown".to_string(),
                horizon: "lap".to_string(),
                confidence: 0.9,
            }],
        };

        let first = store
            .insert_lap_summary(&summary)
            .await
            .expect("first insert should work");
        assert_eq!(first, LapSummaryInsertOutcome::Inserted);

        let second = store
            .insert_lap_summary(&summary)
            .await
            .expect("second insert should work");
        assert_eq!(second, LapSummaryInsertOutcome::Duplicate);
    }
}
