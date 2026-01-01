use std::path::{Path, PathBuf};

use anyhow::Context;
use async_trait::async_trait;
use pitgun_core::EventBatch;
use serde::Serialize;
use tokio::{
    fs,
    fs::OpenOptions,
    io::{AsyncWriteExt, BufWriter},
    sync::Mutex,
};
use tracing::debug;

use crate::json::EventBatchDto;

#[derive(Clone, Debug)]
pub struct IngestMetadata {
    pub remote_ip: Option<String>,
    pub user_agent: Option<String>,
}

#[derive(Debug)]
pub struct IngestMessage {
    pub session_id: Option<String>,
    pub sent_at_ms: Option<i64>,
    pub batch: EventBatch,
    pub meta: IngestMetadata,
}

#[async_trait]
pub trait TelemetryProcessor: Send + Sync {
    async fn process(&self, msg: IngestMessage) -> anyhow::Result<()>;
}

pub struct DefaultProcessor {
    writer: NdjsonWriter,
}

impl DefaultProcessor {
    pub async fn new<P: AsRef<Path>>(data_dir: P) -> anyhow::Result<Self> {
        let writer = NdjsonWriter::new(data_dir).await?;
        Ok(Self { writer })
    }
}

#[async_trait]
impl TelemetryProcessor for DefaultProcessor {
    async fn process(&self, msg: IngestMessage) -> anyhow::Result<()> {
        self.writer.append(msg).await?;

        // TODO: forward to Pitgun pipeline entrypoint when available.

        Ok(())
    }
}

struct NdjsonWriter {
    data_dir: PathBuf,
    inner: Mutex<WriterState>,
}

struct WriterState {
    date: time::Date,
    writer: BufWriter<tokio::fs::File>,
}

#[derive(Serialize)]
struct Envelope {
    received_at_ms: u64,
    remote_ip: Option<String>,
    user_agent: Option<String>,
    session_id: Option<String>,
    sent_at_ms: Option<i64>,
    batch: EventBatchDto,
}

impl NdjsonWriter {
    async fn new<P: AsRef<Path>>(data_dir: P) -> anyhow::Result<Self> {
        let data_dir = data_dir.as_ref().to_path_buf();
        fs::create_dir_all(&data_dir)
            .await
            .with_context(|| format!("failed to create {}", data_dir.display()))?;

        let today = time::OffsetDateTime::now_utc().date();
        let writer = WriterState::open(&data_dir, today).await?;

        Ok(Self {
            data_dir,
            inner: Mutex::new(writer),
        })
    }

    async fn append(&self, msg: IngestMessage) -> anyhow::Result<()> {
        let mut guard = self.inner.lock().await;
        let now = time::OffsetDateTime::now_utc();

        if now.date() != guard.date {
            let next = WriterState::open(&self.data_dir, now.date()).await?;
            debug!("rotated NDJSON sink for new day");
            *guard = next;
        }

        let timestamp_ms = (now.unix_timestamp_nanos() / 1_000_000).max(0_i128) as u64;
        let envelope = Envelope {
            received_at_ms: timestamp_ms,
            remote_ip: msg.meta.remote_ip,
            user_agent: msg.meta.user_agent,
            session_id: msg.session_id,
            sent_at_ms: msg.sent_at_ms,
            batch: EventBatchDto::from(&msg.batch),
        };

        let line = serde_json::to_vec(&envelope)?;
        guard.writer.write_all(&line).await?;
        guard.writer.write_all(b"\n").await?;
        guard.writer.flush().await?;
        Ok(())
    }
}

impl WriterState {
    async fn open(dir: &Path, date: time::Date) -> anyhow::Result<Self> {
        let filename = format!(
            "{}.ndjson",
            date.format(&DATE_FORMAT)
                .map_err(|err| anyhow::anyhow!("failed to format date: {err}"))?
        );

        let path = dir.join(filename);
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await
            .with_context(|| format!("failed to open {}", path.display()))?;

        Ok(Self {
            date,
            writer: BufWriter::new(file),
        })
    }
}

const DATE_FORMAT: &[time::format_description::FormatItem<'static>] =
    time::macros::format_description!("[year]-[month]-[day]");
