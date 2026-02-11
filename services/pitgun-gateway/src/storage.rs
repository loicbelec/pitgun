use std::path::Path;

use anyhow::Context;
use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};
use time::{OffsetDateTime, UtcOffset, format_description::well_known::Rfc3339};

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
pub enum EventInsertOutcome {
    Inserted,
    Duplicate,
}

impl SqliteEventStore {
    pub async fn new(db_path: &str) -> anyhow::Result<Self> {
        ensure_parent_dir_exists(db_path).await?;

        let db_url = sqlite_url(db_path);
        let pool = SqlitePoolOptions::new()
            .max_connections(10)
            .connect(&db_url)
            .await
            .with_context(|| format!("failed to connect to sqlite database at {db_path}"))?;

        sqlx::query("PRAGMA journal_mode = WAL;")
            .execute(&pool)
            .await?;
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

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_events_session_ts ON events(session_id, ts);")
            .execute(&pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_events_player_ts ON events(player_id, ts);")
            .execute(&pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_events_type_ts ON events(event_type, ts);")
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
                session_id,
                event_type,
                payload_json,
                envelope_json,
                received_at,
                remote_ip,
                user_agent
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(event_id) DO NOTHING;
            "#,
        )
        .bind(msg.envelope.event_id.to_string())
        .bind(msg.envelope.schema_version)
        .bind(event_ts)
        .bind(msg.envelope.player_id)
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

#[cfg(test)]
mod tests {
    use super::{EventInsertOutcome, IngestMetadata, QueueMessage, SqliteEventStore};
    use crate::model::parse_event_envelope;

    #[tokio::test]
    async fn deduplicates_by_event_id() {
        let store = SqliteEventStore::new("sqlite::memory:")
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
