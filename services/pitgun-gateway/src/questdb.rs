use std::collections::BTreeMap;

use anyhow::Context;
use sqlx::{PgPool, Row, postgres::PgPoolOptions};

use crate::insight_requests::{
    InsightContext, InsightMetric, LapSummaryPayload, SessionSummaryPayload, WeekendSummaryPayload,
};

#[derive(Clone)]
pub struct QuestDbStore {
    pool: PgPool,
}

#[derive(Clone, Debug)]
pub struct TelemetryPointRow {
    pub player_id: String,
    pub weekend_id: Option<String>,
    pub session_id: String,
    pub run_id: Option<String>,
    pub session_type: Option<String>,
    pub track_id: Option<String>,
    pub source_id: String,
    pub frame_session_id: u64,
    pub frame_sequence: u64,
    pub timestamp_us: i64,
    pub received_at_us: i64,
    pub lap_number: Option<u16>,
    pub sector: Option<u8>,
    pub lap_distance_m: Option<f32>,
    pub parameter_id: u16,
    pub channel: String,
    pub metric_key: String,
    pub unit: String,
    pub value: f64,
}

impl QuestDbStore {
    pub async fn new(database_url: &str) -> anyhow::Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await
            .with_context(|| format!("failed to connect to QuestDB at {database_url}"))?;

        let store = Self { pool };
        store.ensure_schema().await?;
        Ok(store)
    }

    async fn ensure_schema(&self) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS telemetry_points (
                ts TIMESTAMP,
                player_id SYMBOL,
                weekend_id SYMBOL,
                session_id SYMBOL,
                run_id SYMBOL,
                session_type SYMBOL,
                track_id SYMBOL,
                source_id SYMBOL,
                frame_session_id LONG,
                frame_sequence LONG,
                received_at_us LONG,
                lap_number LONG,
                sector LONG,
                lap_distance_m DOUBLE,
                parameter_id LONG,
                channel SYMBOL,
                metric_key SYMBOL,
                unit SYMBOL,
                value DOUBLE
            ) TIMESTAMP(ts) PARTITION BY DAY WAL;
            "#,
        )
        .execute(&self.pool)
        .await
        .context("failed to ensure QuestDB table telemetry_points")?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS lap_summaries (
                ts_end TIMESTAMP,
                summary_id SYMBOL,
                run_id SYMBOL,
                weekend_id SYMBOL,
                session_id SYMBOL,
                circuit_id SYMBOL,
                lap_number LONG,
                started_at_us LONG,
                ended_at_us LONG,
                metric_count LONG,
                era LONG,
                position LONG,
                weather VARCHAR,
                track_status VARCHAR,
                payload_json VARCHAR
            ) TIMESTAMP(ts_end) PARTITION BY DAY WAL;
            "#,
        )
        .execute(&self.pool)
        .await
        .context("failed to ensure QuestDB table lap_summaries")?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS lap_summary_metrics (
                ts_end TIMESTAMP,
                summary_id SYMBOL,
                run_id SYMBOL,
                weekend_id SYMBOL,
                session_id SYMBOL,
                circuit_id SYMBOL,
                lap_number LONG,
                metric_key SYMBOL,
                unit SYMBOL,
                trend SYMBOL,
                horizon SYMBOL,
                value DOUBLE,
                confidence DOUBLE
            ) TIMESTAMP(ts_end) PARTITION BY DAY WAL;
            "#,
        )
        .execute(&self.pool)
        .await
        .context("failed to ensure QuestDB table lap_summary_metrics")?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS session_summaries (
                ts_end TIMESTAMP,
                summary_id SYMBOL,
                run_id SYMBOL,
                weekend_id SYMBOL,
                session_id SYMBOL,
                circuit_id SYMBOL,
                lap_count LONG,
                emitted_at_ms LONG,
                ended_at_us LONG,
                metric_count LONG,
                era LONG,
                position LONG,
                weather VARCHAR,
                track_status VARCHAR,
                payload_json VARCHAR
            ) TIMESTAMP(ts_end) PARTITION BY DAY WAL;
            "#,
        )
        .execute(&self.pool)
        .await
        .context("failed to ensure QuestDB table session_summaries")?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS session_summary_metrics (
                ts_end TIMESTAMP,
                summary_id SYMBOL,
                run_id SYMBOL,
                weekend_id SYMBOL,
                session_id SYMBOL,
                circuit_id SYMBOL,
                lap_count LONG,
                metric_key SYMBOL,
                unit SYMBOL,
                trend SYMBOL,
                horizon SYMBOL,
                value DOUBLE,
                confidence DOUBLE
            ) TIMESTAMP(ts_end) PARTITION BY DAY WAL;
            "#,
        )
        .execute(&self.pool)
        .await
        .context("failed to ensure QuestDB table session_summary_metrics")?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS weekend_summaries (
                ts_end TIMESTAMP,
                summary_id SYMBOL,
                weekend_id SYMBOL,
                session_count LONG,
                emitted_at_ms LONG,
                ended_at_us LONG,
                circuit_id SYMBOL,
                lap_count LONG,
                metric_count LONG,
                era LONG,
                position LONG,
                weather VARCHAR,
                track_status VARCHAR,
                payload_json VARCHAR
            ) TIMESTAMP(ts_end) PARTITION BY DAY WAL;
            "#,
        )
        .execute(&self.pool)
        .await
        .context("failed to ensure QuestDB table weekend_summaries")?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS weekend_summary_metrics (
                ts_end TIMESTAMP,
                summary_id SYMBOL,
                weekend_id SYMBOL,
                session_count LONG,
                circuit_id SYMBOL,
                lap_count LONG,
                metric_key SYMBOL,
                unit SYMBOL,
                trend SYMBOL,
                horizon SYMBOL,
                value DOUBLE,
                confidence DOUBLE
            ) TIMESTAMP(ts_end) PARTITION BY DAY WAL;
            "#,
        )
        .execute(&self.pool)
        .await
        .context("failed to ensure QuestDB table weekend_summary_metrics")?;

        Ok(())
    }

    pub async fn insert_telemetry_points(
        &self,
        points: &[TelemetryPointRow],
    ) -> anyhow::Result<()> {
        for point in points {
            sqlx::query(
                r#"
                INSERT INTO telemetry_points (
                    ts,
                    player_id,
                    weekend_id,
                    session_id,
                    run_id,
                    session_type,
                    track_id,
                    source_id,
                    frame_session_id,
                    frame_sequence,
                    received_at_us,
                    lap_number,
                    sector,
                    lap_distance_m,
                    parameter_id,
                    channel,
                    metric_key,
                    unit,
                    value
                )
                VALUES (
                    cast($1 as timestamp),
                    $2,
                    $3,
                    $4,
                    $5,
                    $6,
                    $7,
                    $8,
                    $9,
                    $10,
                    $11,
                    $12,
                    $13,
                    $14,
                    $15,
                    $16,
                    $17,
                    $18,
                    $19
                );
                "#,
            )
            .bind(point.timestamp_us)
            .bind(&point.player_id)
            .bind(point.weekend_id.as_deref())
            .bind(&point.session_id)
            .bind(point.run_id.as_deref())
            .bind(point.session_type.as_deref())
            .bind(point.track_id.as_deref())
            .bind(&point.source_id)
            .bind(point.frame_session_id as i64)
            .bind(point.frame_sequence as i64)
            .bind(point.received_at_us)
            .bind(point.lap_number.map(i64::from))
            .bind(point.sector.map(i64::from))
            .bind(point.lap_distance_m.map(f64::from))
            .bind(i64::from(point.parameter_id))
            .bind(&point.channel)
            .bind(&point.metric_key)
            .bind(&point.unit)
            .bind(point.value)
            .execute(&self.pool)
            .await
            .with_context(|| {
                format!(
                    "failed to insert telemetry point {} for session {} into QuestDB",
                    point.metric_key, point.session_id
                )
            })?;
        }

        Ok(())
    }

    pub async fn insert_lap_summary(&self, summary: &LapSummaryPayload) -> anyhow::Result<()> {
        let ts_end = summary.ended_at_us;
        let payload_json =
            serde_json::to_string(summary).context("failed to serialize lap summary payload")?;

        sqlx::query(
            r#"
            INSERT INTO lap_summaries (
                ts_end,
                summary_id,
                run_id,
                weekend_id,
                session_id,
                circuit_id,
                lap_number,
                started_at_us,
                ended_at_us,
                metric_count,
                era,
                position,
                weather,
                track_status,
                payload_json
            )
            VALUES (
                cast($1 as timestamp),
                $2,
                $3,
                $4,
                $5,
                $6,
                $7,
                $8,
                $9,
                $10,
                $11,
                $12,
                $13,
                $14,
                $15
            );
            "#,
        )
        .bind(ts_end)
        .bind(&summary.summary_id)
        .bind(&summary.run_id)
        .bind(summary.weekend_id.as_deref())
        .bind(&summary.session_id)
        .bind(&summary.context.circuit_id)
        .bind(summary.lap_number as i64)
        .bind(summary.started_at_us)
        .bind(summary.ended_at_us)
        .bind(summary.metrics.len() as i64)
        .bind(summary.context.era as i64)
        .bind(summary.context.position.map(i64::from))
        .bind(summary.context.weather.as_deref())
        .bind(summary.context.track_status.as_deref())
        .bind(payload_json)
        .execute(&self.pool)
        .await
        .with_context(|| {
            format!(
                "failed to insert lap summary {} into QuestDB",
                summary.summary_id
            )
        })?;

        for metric in &summary.metrics {
            self.insert_lap_metric(summary, metric).await?;
        }

        Ok(())
    }

    async fn insert_lap_metric(
        &self,
        summary: &LapSummaryPayload,
        metric: &InsightMetric,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            INSERT INTO lap_summary_metrics (
                ts_end,
                summary_id,
                run_id,
                weekend_id,
                session_id,
                circuit_id,
                lap_number,
                metric_key,
                unit,
                trend,
                horizon,
                value,
                confidence
            )
            VALUES (
                cast($1 as timestamp),
                $2,
                $3,
                $4,
                $5,
                $6,
                $7,
                $8,
                $9,
                $10,
                $11,
                $12,
                $13
            );
            "#,
        )
        .bind(summary.ended_at_us)
        .bind(&summary.summary_id)
        .bind(&summary.run_id)
        .bind(summary.weekend_id.as_deref())
        .bind(&summary.session_id)
        .bind(&summary.context.circuit_id)
        .bind(summary.lap_number as i64)
        .bind(&metric.key)
        .bind(&metric.unit)
        .bind(&metric.trend)
        .bind(&metric.horizon)
        .bind(metric.value)
        .bind(metric.confidence)
        .execute(&self.pool)
        .await
        .with_context(|| {
            format!(
                "failed to insert lap summary metric {} for {} into QuestDB",
                metric.key, summary.summary_id
            )
        })?;

        Ok(())
    }

    pub async fn insert_session_summary(
        &self,
        summary: &SessionSummaryPayload,
    ) -> anyhow::Result<()> {
        let payload_json = serde_json::to_string(summary)
            .context("failed to serialize session summary payload")?;

        sqlx::query(
            r#"
            INSERT INTO session_summaries (
                ts_end,
                summary_id,
                run_id,
                weekend_id,
                session_id,
                circuit_id,
                lap_count,
                emitted_at_ms,
                ended_at_us,
                metric_count,
                era,
                position,
                weather,
                track_status,
                payload_json
            )
            VALUES (
                cast($1 as timestamp),
                $2,
                $3,
                $4,
                $5,
                $6,
                $7,
                $8,
                $9,
                $10,
                $11,
                $12,
                $13,
                $14,
                $15
            );
            "#,
        )
        .bind(summary.ended_at_us)
        .bind(&summary.summary_id)
        .bind(&summary.run_id)
        .bind(summary.weekend_id.as_deref())
        .bind(&summary.session_id)
        .bind(&summary.context.circuit_id)
        .bind(summary.lap_count as i64)
        .bind(summary.emitted_at_ms)
        .bind(summary.ended_at_us)
        .bind(summary.metrics.len() as i64)
        .bind(summary.context.era as i64)
        .bind(summary.context.position.map(i64::from))
        .bind(summary.context.weather.as_deref())
        .bind(summary.context.track_status.as_deref())
        .bind(payload_json)
        .execute(&self.pool)
        .await
        .with_context(|| {
            format!(
                "failed to insert session summary {} into QuestDB",
                summary.summary_id
            )
        })?;

        for metric in &summary.metrics {
            self.insert_session_metric(summary, metric).await?;
        }

        Ok(())
    }

    async fn insert_session_metric(
        &self,
        summary: &SessionSummaryPayload,
        metric: &InsightMetric,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            INSERT INTO session_summary_metrics (
                ts_end,
                summary_id,
                run_id,
                weekend_id,
                session_id,
                circuit_id,
                lap_count,
                metric_key,
                unit,
                trend,
                horizon,
                value,
                confidence
            )
            VALUES (
                cast($1 as timestamp),
                $2,
                $3,
                $4,
                $5,
                $6,
                $7,
                $8,
                $9,
                $10,
                $11,
                $12,
                $13
            );
            "#,
        )
        .bind(summary.ended_at_us)
        .bind(&summary.summary_id)
        .bind(&summary.run_id)
        .bind(summary.weekend_id.as_deref())
        .bind(&summary.session_id)
        .bind(&summary.context.circuit_id)
        .bind(summary.lap_count as i64)
        .bind(&metric.key)
        .bind(&metric.unit)
        .bind(&metric.trend)
        .bind(&metric.horizon)
        .bind(metric.value)
        .bind(metric.confidence)
        .execute(&self.pool)
        .await
        .with_context(|| {
            format!(
                "failed to insert session summary metric {} for {} into QuestDB",
                metric.key, summary.summary_id
            )
        })?;

        Ok(())
    }

    pub async fn rebuild_weekend_summary(
        &self,
        weekend_id: &str,
        emitted_at_ms: i64,
    ) -> anyhow::Result<Option<WeekendSummaryPayload>> {
        let summary_rows = sqlx::query(
            r#"
            SELECT summary_id, run_id, session_id, circuit_id, lap_count, era, position, weather, track_status, ended_at_us
            FROM session_summaries
            WHERE weekend_id = $1
            ORDER BY ended_at_us ASC;
            "#,
        )
        .bind(weekend_id)
        .fetch_all(&self.pool)
        .await
        .with_context(|| {
            format!("failed to fetch session summaries for weekend {weekend_id}")
        })?;

        if summary_rows.len() < 3 {
            return Ok(None);
        }

        let latest = summary_rows
            .last()
            .context("expected at least one session summary row")?;

        let mut run_ids = Vec::new();
        let mut session_ids = Vec::new();
        for row in &summary_rows {
            let run_id: String = row.try_get("run_id")?;
            let session_id: String = row.try_get("session_id")?;
            if !run_ids.contains(&run_id) {
                run_ids.push(run_id);
            }
            if !session_ids.contains(&session_id) {
                session_ids.push(session_id);
            }
        }

        let metric_rows = sqlx::query(
            r#"
            SELECT metric_key, unit, horizon, value, confidence
            FROM session_summary_metrics
            WHERE weekend_id = $1;
            "#,
        )
        .bind(weekend_id)
        .fetch_all(&self.pool)
        .await
        .with_context(|| {
            format!("failed to fetch session summary metrics for weekend {weekend_id}")
        })?;

        if metric_rows.is_empty() {
            return Ok(None);
        }

        let mut aggregates: BTreeMap<String, WeekendMetricAggregate> = BTreeMap::new();
        for row in metric_rows {
            let key: String = row.try_get("metric_key")?;
            let unit: String = row.try_get("unit")?;
            let horizon: String = row.try_get("horizon")?;
            let value: f64 = row.try_get("value")?;
            let confidence: f64 = row.try_get("confidence")?;

            let entry = aggregates
                .entry(key.clone())
                .or_insert_with(|| WeekendMetricAggregate::new(unit, horizon));
            entry.update(value, confidence);
        }

        let metrics = aggregates
            .into_iter()
            .map(|(key, agg)| InsightMetric {
                key,
                value: agg.mean(),
                unit: agg.unit.clone(),
                trend: "mixed".to_string(),
                horizon: agg.horizon.clone(),
                confidence: agg.mean_confidence(),
            })
            .collect::<Vec<_>>();

        let lap_count = latest.try_get::<i64, _>("lap_count")?.max(0) as u32;
        let ended_at_us = latest.try_get::<i64, _>("ended_at_us")?;
        let summary = WeekendSummaryPayload {
            schema_version: "pitgun-weekend-summary-v1".to_string(),
            summary_id: format!("{weekend_id}:weekend"),
            weekend_id: weekend_id.to_string(),
            emitted_at_ms: emitted_at_ms.max(0),
            ended_at_us,
            session_count: summary_rows.len() as u32,
            source_run_ids: run_ids,
            source_session_ids: session_ids,
            context: InsightContext {
                circuit_id: latest.try_get::<String, _>("circuit_id")?,
                era: latest.try_get::<i64, _>("era")?.max(0) as u32,
                lap: lap_count,
                position: latest
                    .try_get::<Option<i64>, _>("position")?
                    .map(|value| value.max(0) as u32),
                weather: latest.try_get("weather")?,
                track_status: latest.try_get("track_status")?,
            },
            metrics,
        };

        self.insert_weekend_summary(&summary).await?;
        Ok(Some(summary))
    }

    async fn insert_weekend_summary(&self, summary: &WeekendSummaryPayload) -> anyhow::Result<()> {
        let payload_json = serde_json::to_string(summary)
            .context("failed to serialize weekend summary payload")?;

        sqlx::query(
            r#"
            INSERT INTO weekend_summaries (
                ts_end,
                summary_id,
                weekend_id,
                session_count,
                emitted_at_ms,
                ended_at_us,
                circuit_id,
                lap_count,
                metric_count,
                era,
                position,
                weather,
                track_status,
                payload_json
            )
            VALUES (
                cast($1 as timestamp),
                $2,
                $3,
                $4,
                $5,
                $6,
                $7,
                $8,
                $9,
                $10,
                $11,
                $12,
                $13,
                $14
            );
            "#,
        )
        .bind(summary.ended_at_us)
        .bind(&summary.summary_id)
        .bind(&summary.weekend_id)
        .bind(summary.session_count as i64)
        .bind(summary.emitted_at_ms)
        .bind(summary.ended_at_us)
        .bind(&summary.context.circuit_id)
        .bind(summary.context.lap as i64)
        .bind(summary.metrics.len() as i64)
        .bind(summary.context.era as i64)
        .bind(summary.context.position.map(i64::from))
        .bind(summary.context.weather.as_deref())
        .bind(summary.context.track_status.as_deref())
        .bind(payload_json)
        .execute(&self.pool)
        .await
        .with_context(|| {
            format!(
                "failed to insert weekend summary {} into QuestDB",
                summary.summary_id
            )
        })?;

        for metric in &summary.metrics {
            self.insert_weekend_metric(summary, metric).await?;
        }

        Ok(())
    }

    async fn insert_weekend_metric(
        &self,
        summary: &WeekendSummaryPayload,
        metric: &InsightMetric,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            INSERT INTO weekend_summary_metrics (
                ts_end,
                summary_id,
                weekend_id,
                session_count,
                circuit_id,
                lap_count,
                metric_key,
                unit,
                trend,
                horizon,
                value,
                confidence
            )
            VALUES (
                cast($1 as timestamp),
                $2,
                $3,
                $4,
                $5,
                $6,
                $7,
                $8,
                $9,
                $10,
                $11,
                $12
            );
            "#,
        )
        .bind(summary.ended_at_us)
        .bind(&summary.summary_id)
        .bind(&summary.weekend_id)
        .bind(summary.session_count as i64)
        .bind(&summary.context.circuit_id)
        .bind(summary.context.lap as i64)
        .bind(&metric.key)
        .bind(&metric.unit)
        .bind(&metric.trend)
        .bind(&metric.horizon)
        .bind(metric.value)
        .bind(metric.confidence)
        .execute(&self.pool)
        .await
        .with_context(|| {
            format!(
                "failed to insert weekend summary metric {} for {} into QuestDB",
                metric.key, summary.summary_id
            )
        })?;

        Ok(())
    }
}

#[derive(Clone, Debug)]
struct WeekendMetricAggregate {
    count: u64,
    sum: f64,
    confidence_sum: f64,
    unit: String,
    horizon: String,
}

impl WeekendMetricAggregate {
    fn new(unit: String, horizon: String) -> Self {
        Self {
            count: 0,
            sum: 0.0,
            confidence_sum: 0.0,
            unit,
            horizon,
        }
    }

    fn update(&mut self, value: f64, confidence: f64) {
        self.count = self.count.saturating_add(1);
        self.sum += value;
        self.confidence_sum += confidence;
    }

    fn mean(&self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            self.sum / self.count as f64
        }
    }

    fn mean_confidence(&self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            self.confidence_sum / self.count as f64
        }
    }
}
