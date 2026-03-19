use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};

use crate::{
    insight_ingress::{InsightExtraction, InsightMetricPoint},
    insight_stats_plan::{InsightStatsPlan, StatMetric},
    model::{EventEnvelope, TelemetrySampleBatchPayload},
};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct InsightRequestPayload {
    pub schema_version: String,
    pub run_id: String,
    pub session_id: String,
    pub trace_id: String,
    pub emitted_at_ms: i64,
    pub context: InsightContext,
    pub metrics: Vec<InsightMetric>,
    pub constraints: InsightConstraints,
    pub policy_version: String,
    pub prompt_version: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct InsightContext {
    pub circuit_id: String,
    pub era: u32,
    pub lap: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weather: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub track_status: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct InsightMetric {
    pub key: String,
    pub value: f64,
    pub unit: String,
    pub trend: String,
    pub horizon: String,
    pub confidence: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct InsightConstraints {
    pub max_insights: u8,
    pub max_words_per_insight: u8,
    pub language: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct LapSummaryPayload {
    pub schema_version: String,
    pub summary_id: String,
    pub player_id: String,
    pub run_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weekend_id: Option<String>,
    pub session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_type: Option<String>,
    pub lap_number: u32,
    pub started_at_us: i64,
    pub ended_at_us: i64,
    pub context: InsightContext,
    pub metrics: Vec<InsightMetric>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SessionSummaryPayload {
    pub schema_version: String,
    pub summary_id: String,
    pub player_id: String,
    pub run_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weekend_id: Option<String>,
    pub session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_type: Option<String>,
    pub emitted_at_ms: i64,
    pub ended_at_us: i64,
    pub lap_count: u32,
    pub context: InsightContext,
    pub metrics: Vec<InsightMetric>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct PracticeSummaryPayload {
    pub schema_version: String,
    pub summary_id: String,
    pub player_id: String,
    pub weekend_id: String,
    pub emitted_at_ms: i64,
    pub ended_at_us: i64,
    pub session_count: u32,
    pub source_run_ids: Vec<String>,
    pub source_session_ids: Vec<String>,
    pub source_session_types: Vec<String>,
    pub context: InsightContext,
    pub metrics: Vec<InsightMetric>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RaceSummaryPayload {
    pub schema_version: String,
    pub summary_id: String,
    pub player_id: String,
    pub run_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weekend_id: Option<String>,
    pub session_id: String,
    pub emitted_at_ms: i64,
    pub ended_at_us: i64,
    pub lap_count: u32,
    pub context: InsightContext,
    pub metrics: Vec<InsightMetric>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SummaryMetadata {
    pub player_id: String,
    pub run_id: String,
    pub weekend_id: Option<String>,
    pub session_type: Option<String>,
    pub context: InsightContext,
}

#[derive(Clone, Debug)]
pub(crate) struct MetricAggregate {
    count: u64,
    min: f64,
    max: f64,
    sum: f64,
    mean: f64,
    m2: f64,
    unit: String,
    metric_key: String,
}

impl MetricAggregate {
    fn new(value: f64, unit: String, metric_key: String) -> Self {
        Self {
            count: 1,
            min: value,
            max: value,
            sum: value,
            mean: value,
            m2: 0.0,
            unit,
            metric_key,
        }
    }

    fn update(&mut self, value: f64) {
        self.count = self.count.saturating_add(1);
        self.min = self.min.min(value);
        self.max = self.max.max(value);
        self.sum += value;

        let delta = value - self.mean;
        self.mean += delta / self.count as f64;
        let delta2 = value - self.mean;
        self.m2 += delta * delta2;
    }

    fn value_for_metric(&self, metric: StatMetric) -> f64 {
        match metric {
            StatMetric::Count => self.count as f64,
            StatMetric::Min => self.min,
            StatMetric::Max => self.max,
            StatMetric::Mean => self.mean,
            StatMetric::Sum => self.sum,
            StatMetric::Stddev => {
                if self.count <= 1 {
                    0.0
                } else {
                    (self.m2 / self.count as f64).sqrt()
                }
            }
        }
    }

    fn unit_for_metric(&self, metric: StatMetric) -> String {
        match metric {
            StatMetric::Count => "count".to_string(),
            _ => self.unit.clone(),
        }
    }
}

pub(crate) fn accumulate_metric_point(
    aggregates: &mut BTreeMap<String, MetricAggregate>,
    point: &InsightMetricPoint,
) {
    let channel = point.channel.to_string();
    if let Some(entry) = aggregates.get_mut(&channel) {
        entry.update(point.value);
    } else {
        aggregates.insert(
            channel,
            MetricAggregate::new(
                point.value,
                point.unit.to_string(),
                point.metric_key.to_string(),
            ),
        );
    }
}

pub(crate) fn aggregate_points_by_channel(
    points: &[InsightMetricPoint],
) -> BTreeMap<String, MetricAggregate> {
    let mut aggregates = BTreeMap::new();
    for point in points {
        accumulate_metric_point(&mut aggregates, point);
    }
    aggregates
}

pub(crate) fn build_insight_metrics(
    aggregates: &BTreeMap<String, MetricAggregate>,
    stats_plan: &InsightStatsPlan,
) -> Vec<InsightMetric> {
    let mut metrics = Vec::new();

    for target in &stats_plan.targets {
        let Some(agg) = aggregates.get(&target.channel) else {
            continue;
        };

        for stat_metric in &target.metrics {
            let key = format!("{}.{}", agg.metric_key, stat_metric.suffix());
            metrics.push(InsightMetric {
                key: key.clone(),
                value: agg.value_for_metric(*stat_metric),
                unit: agg.unit_for_metric(*stat_metric),
                trend: "unknown".to_string(),
                horizon: infer_horizon(&key).to_string(),
                confidence: 0.9,
            });
        }
    }

    metrics
}

pub fn resolve_summary_metadata(
    player_id: &str,
    metadata: &HashMap<String, String>,
    session_id: &str,
    fallback_run_id: &str,
    lap: u32,
) -> SummaryMetadata {
    let run_id = metadata
        .get("run_id")
        .map(|value| trim_to_max(value, 64))
        .filter(|value| !value.is_empty())
        .or_else(|| {
            let trimmed = trim_to_max(fallback_run_id, 64);
            (!trimmed.is_empty()).then_some(trimmed)
        })
        .unwrap_or_else(|| trim_to_max(session_id, 64));

    let weekend_id = metadata
        .get("weekend_id")
        .map(|value| trim_to_max(value, 64))
        .filter(|value| !value.is_empty());

    let session_type = metadata
        .get("session_type")
        .map(|value| trim_to_max(&value.trim().to_ascii_uppercase(), 16))
        .filter(|value| !value.is_empty());

    let circuit_id = metadata
        .get("track_id")
        .map(|value| trim_to_max(&value.trim().to_ascii_uppercase(), 32))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "UNKNOWN".to_string());

    let era = metadata
        .get("era")
        .and_then(|value| value.trim().parse::<u32>().ok())
        .filter(|value| *value >= 1)
        .unwrap_or(1);

    let position = metadata
        .get("position")
        .and_then(|value| value.trim().parse::<u32>().ok())
        .filter(|value| *value >= 1);

    let weather = metadata
        .get("weather")
        .map(|value| trim_to_max(value, 32))
        .filter(|value| !value.is_empty());

    let track_status = metadata
        .get("track_status")
        .map(|value| trim_to_max(value, 32))
        .filter(|value| !value.is_empty());

    SummaryMetadata {
        player_id: trim_to_max(player_id, 64),
        run_id,
        weekend_id,
        session_type,
        context: InsightContext {
            circuit_id,
            era,
            lap,
            position,
            weather,
            track_status,
        },
    }
}

pub fn build_lap_summary(
    summary_id: String,
    player_id: &str,
    session_id: String,
    lap_number: u32,
    started_at_us: i64,
    ended_at_us: i64,
    metadata: &HashMap<String, String>,
    fallback_run_id: &str,
    aggregates: &BTreeMap<String, MetricAggregate>,
    stats_plan: &InsightStatsPlan,
) -> Option<LapSummaryPayload> {
    let metrics = build_insight_metrics(aggregates, stats_plan);
    if metrics.is_empty() {
        return None;
    }

    let summary_metadata = resolve_summary_metadata(
        player_id,
        metadata,
        &session_id,
        fallback_run_id,
        lap_number,
    );

    Some(LapSummaryPayload {
        schema_version: "pitgun-lap-summary-v1".to_string(),
        summary_id: trim_to_max(&summary_id, 128),
        player_id: summary_metadata.player_id,
        run_id: summary_metadata.run_id,
        weekend_id: summary_metadata.weekend_id,
        session_id: trim_to_max(&session_id, 64),
        session_type: summary_metadata.session_type,
        lap_number,
        started_at_us,
        ended_at_us,
        context: summary_metadata.context,
        metrics,
    })
}

pub fn build_insight_request_from_lap_summary(
    summary: &LapSummaryPayload,
    trace_id: String,
) -> InsightRequestPayload {
    build_insight_request_payload(
        summary.run_id.clone(),
        summary.session_id.clone(),
        trace_id,
        (summary.ended_at_us / 1_000).max(0),
        summary.context.clone(),
        summary.metrics.clone(),
    )
}

pub fn build_session_summary(
    summary_id: String,
    player_id: &str,
    session_id: String,
    emitted_at_ms: i64,
    lap: u32,
    metadata: &HashMap<String, String>,
    fallback_run_id: &str,
    aggregates: &BTreeMap<String, MetricAggregate>,
    stats_plan: &InsightStatsPlan,
) -> Option<SessionSummaryPayload> {
    let metrics = build_insight_metrics(aggregates, stats_plan);
    if metrics.is_empty() {
        return None;
    }

    let summary_metadata =
        resolve_summary_metadata(player_id, metadata, &session_id, fallback_run_id, lap);

    Some(SessionSummaryPayload {
        schema_version: "pitgun-session-summary-v1".to_string(),
        summary_id: trim_to_max(&summary_id, 128),
        player_id: summary_metadata.player_id,
        run_id: summary_metadata.run_id,
        weekend_id: summary_metadata.weekend_id,
        session_id: trim_to_max(&session_id, 64),
        session_type: summary_metadata.session_type,
        emitted_at_ms: emitted_at_ms.max(0),
        ended_at_us: emitted_at_ms.saturating_mul(1_000),
        lap_count: lap,
        context: summary_metadata.context,
        metrics,
    })
}

pub fn build_insight_request_from_session_summary(
    summary: &SessionSummaryPayload,
    trace_id: String,
) -> InsightRequestPayload {
    build_insight_request_payload(
        summary.run_id.clone(),
        summary.session_id.clone(),
        trace_id,
        summary.emitted_at_ms,
        summary.context.clone(),
        summary.metrics.clone(),
    )
}

pub fn build_race_summary_from_session(
    summary: &SessionSummaryPayload,
) -> Option<RaceSummaryPayload> {
    if !summary
        .session_type
        .as_deref()
        .is_some_and(|value| value.eq_ignore_ascii_case("RACE"))
    {
        return None;
    }

    Some(RaceSummaryPayload {
        schema_version: "pitgun-race-summary-v1".to_string(),
        summary_id: format!("{}:race", summary.session_id),
        player_id: summary.player_id.clone(),
        run_id: summary.run_id.clone(),
        weekend_id: summary.weekend_id.clone(),
        session_id: summary.session_id.clone(),
        emitted_at_ms: summary.emitted_at_ms,
        ended_at_us: summary.ended_at_us,
        lap_count: summary.lap_count,
        context: summary.context.clone(),
        metrics: summary.metrics.clone(),
    })
}

pub fn build_insight_request(
    envelope: &EventEnvelope,
    payload: &TelemetrySampleBatchPayload,
    extraction: &InsightExtraction,
    stats_plan: &InsightStatsPlan,
) -> Option<InsightRequestPayload> {
    if extraction.points.is_empty() {
        return None;
    }

    let empty_metadata = HashMap::new();
    let latest_metadata = payload
        .frames
        .iter()
        .rev()
        .find(|frame| !frame.metadata.is_empty())
        .map(|frame| &frame.metadata)
        .or_else(|| payload.frames.last().map(|frame| &frame.metadata))
        .unwrap_or(&empty_metadata);

    let aggregates = aggregate_points_by_channel(&extraction.points);
    if aggregates.is_empty() {
        return None;
    }

    let metrics = build_insight_metrics(&aggregates, stats_plan);
    if metrics.is_empty() {
        return None;
    }

    let latest_lap = payload
        .frames
        .iter()
        .rev()
        .find_map(|frame| frame.lap_number)
        .unwrap_or(0) as u32;
    let metadata = resolve_summary_metadata(
        &envelope.player_id,
        latest_metadata,
        &envelope.session_id,
        &envelope.event_id.to_string(),
        latest_lap,
    );

    Some(build_insight_request_payload(
        metadata.run_id,
        trim_to_max(&envelope.session_id, 64),
        envelope.event_id.to_string(),
        (envelope.ts.unix_timestamp_nanos() / 1_000_000).max(0) as i64,
        metadata.context,
        metrics,
    ))
}

fn build_insight_request_payload(
    run_id: String,
    session_id: String,
    trace_id: String,
    emitted_at_ms: i64,
    context: InsightContext,
    metrics: Vec<InsightMetric>,
) -> InsightRequestPayload {
    InsightRequestPayload {
        schema_version: "pitgun-insight-request-v1".to_string(),
        run_id,
        session_id: trim_to_max(&session_id, 64),
        trace_id: trim_to_max(&trace_id, 128),
        emitted_at_ms,
        context,
        metrics,
        constraints: default_constraints(),
        policy_version: "policy.v1".to_string(),
        prompt_version: "chief-race.v1".to_string(),
    }
}

fn default_constraints() -> InsightConstraints {
    InsightConstraints {
        max_insights: 3,
        max_words_per_insight: 32,
        language: "en".to_string(),
    }
}

fn infer_horizon(metric_key: &str) -> &'static str {
    if metric_key.starts_with("tires.") {
        return "stint";
    }
    if metric_key.starts_with("sim.") {
        return "instant";
    }
    "lap"
}

fn trim_to_max(input: &str, max_len: usize) -> String {
    if input.len() <= max_len {
        return input.trim().to_string();
    }
    input.trim().chars().take(max_len).collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use pitgun_contract::{Sample, SampleValue, SignalQuality, TelemetryFrame};
    use time::OffsetDateTime;
    use uuid::Uuid;

    use crate::{
        insight_ingress::extract_sim_metric_points,
        insight_stats_plan::InsightStatsPlan,
        model::{EventEnvelope, EventPayload, TelemetrySampleBatchPayload},
    };

    use super::{
        InsightContext, InsightMetric, SessionSummaryPayload, aggregate_points_by_channel,
        build_insight_request, build_insight_request_from_lap_summary,
        build_insight_request_from_session_summary, build_lap_summary,
        build_race_summary_from_session, build_session_summary,
    };

    #[test]
    fn builds_insight_request_from_sim_points_only() {
        let mut metadata = HashMap::new();
        metadata.insert("track_id".to_string(), "monaco".to_string());
        metadata.insert("run_id".to_string(), "run_insight_001".to_string());
        metadata.insert("session_type".to_string(), "RACE".to_string());

        let payload = TelemetrySampleBatchPayload {
            frames: vec![TelemetryFrame {
                session_id: 4242,
                sequence: 1,
                timestamp_us: 1_770_000_000_000_000,
                received_at_us: 1_770_000_000_000_010,
                source_id: "pitgun-solver".to_string(),
                samples: vec![
                    Sample::new(5005, SampleValue::F64(201.4), SignalQuality::Good),
                    Sample::new(5016, SampleValue::F64(42.3), SignalQuality::Good),
                    Sample::new(65000, SampleValue::F64(1.0), SignalQuality::Good),
                ],
                events: Vec::new(),
                lap_number: Some(18),
                sector: Some(2),
                lap_distance_m: Some(3222.0),
                metadata,
            }],
        };

        let envelope = EventEnvelope {
            schema_version: "pitgun-envelope-v1".to_string(),
            event_id: Uuid::parse_str("95d81f5b-edbc-440c-bdf8-1fdbf6496a8d").expect("valid UUID"),
            ts: OffsetDateTime::from_unix_timestamp(1_773_417_600).expect("valid timestamp"),
            player_id: "player-123".to_string(),
            weekend_id: None,
            session_id: "session-abc".to_string(),
            event_type: "telemetry.sample_batch".to_string(),
            payload: EventPayload::TelemetrySampleBatch(payload.clone()),
        };

        let extraction = extract_sim_metric_points(&payload);
        let request = build_insight_request(
            &envelope,
            &payload,
            &extraction,
            &InsightStatsPlan::default_sim_plan(),
        )
        .expect("insight request should be built");

        assert_eq!(request.schema_version, "pitgun-insight-request-v1");
        assert_eq!(request.run_id, "run_insight_001");
        assert_eq!(request.session_id, "session-abc");
        assert_eq!(request.trace_id, "95d81f5b-edbc-440c-bdf8-1fdbf6496a8d");
        assert_eq!(request.context.circuit_id, "MONACO");
        assert_eq!(request.context.lap, 18);
        assert!(
            request
                .metrics
                .iter()
                .any(|m| m.key == "pace.speed_kph.mean")
        );
        assert!(
            request
                .metrics
                .iter()
                .any(|m| m.key == "tires.avg_wear_pct.stddev")
        );
        assert!(!request.metrics.iter().any(|m| m.key.contains("65000")));
    }

    #[test]
    fn builds_lap_summary_and_compact_request() {
        let mut metadata = HashMap::new();
        metadata.insert("track_id".to_string(), "spa".to_string());
        metadata.insert("weekend_id".to_string(), "weekend-spa".to_string());
        metadata.insert("run_id".to_string(), "run_lap_001".to_string());

        let payload = TelemetrySampleBatchPayload {
            frames: vec![
                TelemetryFrame {
                    session_id: 4242,
                    sequence: 1,
                    timestamp_us: 1_770_000_000_000_000,
                    received_at_us: 1_770_000_000_000_010,
                    source_id: "pitgun-solver".to_string(),
                    samples: vec![Sample::new(
                        5005,
                        SampleValue::F64(201.4),
                        SignalQuality::Good,
                    )],
                    events: Vec::new(),
                    lap_number: Some(7),
                    sector: Some(1),
                    lap_distance_m: Some(120.0),
                    metadata: metadata.clone(),
                },
                TelemetryFrame {
                    session_id: 4242,
                    sequence: 2,
                    timestamp_us: 1_770_000_001_000_000,
                    received_at_us: 1_770_000_001_000_010,
                    source_id: "pitgun-solver".to_string(),
                    samples: vec![Sample::new(
                        5005,
                        SampleValue::F64(208.2),
                        SignalQuality::Good,
                    )],
                    events: Vec::new(),
                    lap_number: Some(7),
                    sector: Some(3),
                    lap_distance_m: Some(7020.0),
                    metadata,
                },
            ],
        };

        let extraction = extract_sim_metric_points(&payload);
        let aggregates = aggregate_points_by_channel(&extraction.points);
        let summary = build_lap_summary(
            "session-abc:lap:7".to_string(),
            "player-123",
            "session-abc".to_string(),
            7,
            1_770_000_000_000_000,
            1_770_000_001_000_000,
            &payload.frames[1].metadata,
            "fallback-run",
            &aggregates,
            &InsightStatsPlan::default_sim_plan(),
        )
        .expect("lap summary should be built");

        assert_eq!(summary.schema_version, "pitgun-lap-summary-v1");
        assert_eq!(summary.player_id, "player-123");
        assert_eq!(summary.run_id, "run_lap_001");
        assert_eq!(summary.weekend_id.as_deref(), Some("weekend-spa"));
        assert_eq!(summary.context.circuit_id, "SPA");
        assert_eq!(summary.context.lap, 7);
        assert!(
            summary
                .metrics
                .iter()
                .any(|metric| metric.key == "pace.speed_kph.max")
        );

        let request = build_insight_request_from_lap_summary(
            &summary,
            "session-abc:lap:7:insight".to_string(),
        );
        assert_eq!(request.trace_id, "session-abc:lap:7:insight");
        assert_eq!(request.session_id, "session-abc");
        assert_eq!(request.context.lap, 7);
        assert_eq!(request.metrics, summary.metrics);
    }

    #[test]
    fn builds_session_summary_and_compact_request() {
        let mut metadata = HashMap::new();
        metadata.insert("track_id".to_string(), "monza".to_string());
        metadata.insert("weekend_id".to_string(), "weekend-monza".to_string());
        metadata.insert("run_id".to_string(), "run_session_001".to_string());

        let payload = TelemetrySampleBatchPayload {
            frames: vec![TelemetryFrame {
                session_id: 4242,
                sequence: 1,
                timestamp_us: 1_770_000_000_000_000,
                received_at_us: 1_770_000_000_000_010,
                source_id: "pitgun-solver".to_string(),
                samples: vec![
                    Sample::new(5005, SampleValue::F64(211.4), SignalQuality::Good),
                    Sample::new(5016, SampleValue::F64(33.1), SignalQuality::Good),
                ],
                events: Vec::new(),
                lap_number: Some(12),
                sector: Some(3),
                lap_distance_m: Some(5710.0),
                metadata,
            }],
        };

        let extraction = extract_sim_metric_points(&payload);
        let aggregates = aggregate_points_by_channel(&extraction.points);
        let summary = build_session_summary(
            "session-abc:session".to_string(),
            "player-123",
            "session-abc".to_string(),
            1_770_000_001_234,
            12,
            &payload.frames[0].metadata,
            "fallback-run",
            &aggregates,
            &InsightStatsPlan::default_sim_plan(),
        )
        .expect("session summary should be built");

        assert_eq!(summary.schema_version, "pitgun-session-summary-v1");
        assert_eq!(summary.player_id, "player-123");
        assert_eq!(summary.run_id, "run_session_001");
        assert_eq!(summary.weekend_id.as_deref(), Some("weekend-monza"));
        assert_eq!(summary.lap_count, 12);
        assert_eq!(summary.context.circuit_id, "MONZA");

        let request = build_insight_request_from_session_summary(
            &summary,
            "session-abc:session:insight".to_string(),
        );
        assert_eq!(request.trace_id, "session-abc:session:insight");
        assert_eq!(request.session_id, "session-abc");
        assert_eq!(request.context.lap, 12);
        assert_eq!(request.metrics, summary.metrics);
    }

    #[test]
    fn builds_race_summary_from_race_session_summary() {
        let summary = SessionSummaryPayload {
            schema_version: "pitgun-session-summary-v1".to_string(),
            summary_id: "session-race:session".to_string(),
            player_id: "player-123".to_string(),
            run_id: "run-race-001".to_string(),
            weekend_id: Some("weekend-singapore".to_string()),
            session_id: "session-race".to_string(),
            session_type: Some("RACE".to_string()),
            emitted_at_ms: 1_770_000_002_345,
            ended_at_us: 1_770_000_002_345_000,
            lap_count: 57,
            context: InsightContext {
                circuit_id: "SINGAPORE".to_string(),
                era: 2026,
                lap: 57,
                position: Some(4),
                weather: Some("dry".to_string()),
                track_status: Some("green".to_string()),
            },
            metrics: vec![InsightMetric {
                key: "pace.speed_kph.mean".to_string(),
                value: 181.2,
                unit: "kph".to_string(),
                trend: "mixed".to_string(),
                horizon: "lap".to_string(),
                confidence: 0.91,
            }],
        };

        let race_summary =
            build_race_summary_from_session(&summary).expect("race summary should be built");

        assert_eq!(race_summary.schema_version, "pitgun-race-summary-v1");
        assert_eq!(race_summary.summary_id, "session-race:race");
        assert_eq!(race_summary.player_id, "player-123");
        assert_eq!(race_summary.run_id, "run-race-001");
        assert_eq!(
            race_summary.weekend_id.as_deref(),
            Some("weekend-singapore")
        );
        assert_eq!(race_summary.session_id, "session-race");
        assert_eq!(race_summary.lap_count, 57);
        assert_eq!(race_summary.metrics.len(), 1);
    }
}
