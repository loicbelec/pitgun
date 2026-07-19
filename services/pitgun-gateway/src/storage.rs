use anyhow::Context;
use sqlx::{PgPool, postgres::PgPoolOptions};
use time::OffsetDateTime;

use crate::insight_requests::LapSummaryPayload;
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
pub struct PgEventStore {
    pool: PgPool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventInsertOutcome {
    Inserted,
    Duplicate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LapSummaryInsertOutcome {
    Inserted,
    Duplicate,
}

impl PgEventStore {
    pub async fn new(database_url: &str) -> anyhow::Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect(database_url)
            .await
            .with_context(|| format!("failed to connect to PostgreSQL at {database_url}"))?;

        Self::init_schema(&pool).await?;
        Ok(Self { pool })
    }

    async fn init_schema(pool: &PgPool) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS events (
                seq_id BIGSERIAL PRIMARY KEY,
                event_id TEXT NOT NULL UNIQUE,
                schema_version TEXT NOT NULL,
                ts TIMESTAMPTZ NOT NULL,
                player_id TEXT NOT NULL,
                weekend_id TEXT,
                session_id TEXT NOT NULL,
                event_type TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                envelope_json TEXT NOT NULL,
                received_at TIMESTAMPTZ NOT NULL,
                remote_ip TEXT,
                user_agent TEXT
            );
            "#,
        )
        .execute(pool)
        .await
        .context("failed to create events table")?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_events_session_ts ON events(session_id, ts);")
            .execute(pool)
            .await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_events_player_ts ON events(player_id, ts);")
            .execute(pool)
            .await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_events_weekend_ts ON events(weekend_id, ts);")
            .execute(pool)
            .await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_events_type_ts ON events(event_type, ts);")
            .execute(pool)
            .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS lap_summaries (
                seq_id BIGSERIAL PRIMARY KEY,
                summary_id TEXT NOT NULL UNIQUE,
                run_id TEXT NOT NULL,
                weekend_id TEXT,
                session_id TEXT NOT NULL,
                lap_number BIGINT NOT NULL,
                started_at_us BIGINT NOT NULL,
                ended_at_us BIGINT NOT NULL,
                payload_json TEXT NOT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );
            "#,
        )
        .execute(pool)
        .await
        .context("failed to create lap_summaries table")?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_lap_summaries_run_lap ON lap_summaries(run_id, lap_number);",
        )
        .execute(pool)
        .await?;
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_lap_summaries_session_lap ON lap_summaries(session_id, lap_number);",
        )
        .execute(pool)
        .await?;
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_lap_summaries_weekend_lap ON lap_summaries(weekend_id, lap_number);",
        )
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn insert_event(&self, msg: QueueMessage) -> anyhow::Result<EventInsertOutcome> {
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
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            ON CONFLICT(event_id) DO NOTHING;
            "#,
        )
        .bind(msg.envelope.event_id.to_string())
        .bind(msg.envelope.schema_version)
        .bind(msg.envelope.ts)
        .bind(msg.envelope.player_id)
        .bind(msg.envelope.weekend_id)
        .bind(msg.envelope.session_id)
        .bind(msg.envelope.event_type)
        .bind(payload_json)
        .bind(msg.raw_json)
        .bind(msg.received_at)
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
        let _: i32 = sqlx::query_scalar("SELECT 1").fetch_one(&self.pool).await?;
        Ok(())
    }

    pub async fn insert_lap_summary(
        &self,
        summary: &LapSummaryPayload,
    ) -> anyhow::Result<LapSummaryInsertOutcome> {
        let payload_json = serde_json::to_string(summary)?;

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
                payload_json
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
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
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            Ok(LapSummaryInsertOutcome::Duplicate)
        } else {
            Ok(LapSummaryInsertOutcome::Inserted)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        EventInsertOutcome, IngestMetadata, LapSummaryInsertOutcome, PgEventStore, QueueMessage,
    };
    use crate::insight_requests::{InsightContext, InsightMetric, LapSummaryPayload};
    use crate::model::parse_event_envelope;

    // These tests require a live PostgreSQL instance.
    // Set DATABASE_URL before running: e.g. DATABASE_URL=postgresql://user:pass@localhost/pitgun_test
    // Run with: cargo test -- --ignored

    #[ignore]
    #[tokio::test]
    async fn deduplicates_by_event_id() {
        let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let store = PgEventStore::new(&db_url)
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

    #[ignore]
    #[tokio::test]
    async fn deduplicates_lap_summary_by_summary_id() {
        let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let store = PgEventStore::new(&db_url)
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
