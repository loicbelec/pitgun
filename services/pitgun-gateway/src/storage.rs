use anyhow::Context;
use sqlx::{PgPool, postgres::PgPoolOptions};
use time::OffsetDateTime;

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
}

#[cfg(test)]
mod tests {
    use super::{EventInsertOutcome, IngestMetadata, PgEventStore, QueueMessage};
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
}
