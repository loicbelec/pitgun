use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};

use crate::{
    insight_ingress::InsightExtraction,
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

#[derive(Clone, Debug)]
struct MetricAggregate {
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

    let mut aggregates: BTreeMap<String, MetricAggregate> = BTreeMap::new();
    for point in &extraction.points {
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

    if aggregates.is_empty() {
        return None;
    }

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

    if metrics.is_empty() {
        return None;
    }

    let event_trace_id = envelope.event_id.to_string();
    let run_id = latest_metadata
        .get("run_id")
        .map(|value| trim_to_max(value, 64))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| event_trace_id.clone());

    let emitted_at_ms = (envelope.ts.unix_timestamp_nanos() / 1_000_000).max(0) as i64;
    let latest_lap = payload
        .frames
        .iter()
        .rev()
        .find_map(|frame| frame.lap_number)
        .unwrap_or(0) as u32;

    let circuit_id = latest_metadata
        .get("track_id")
        .map(|value| trim_to_max(&value.trim().to_ascii_uppercase(), 32))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "UNKNOWN".to_string());

    let era = latest_metadata
        .get("era")
        .and_then(|value| value.trim().parse::<u32>().ok())
        .filter(|value| *value >= 1)
        .unwrap_or(1);

    let position = latest_metadata
        .get("position")
        .and_then(|value| value.trim().parse::<u32>().ok())
        .filter(|value| *value >= 1);

    let weather = latest_metadata
        .get("weather")
        .map(|value| trim_to_max(value, 32))
        .filter(|value| !value.is_empty());

    let track_status = latest_metadata
        .get("track_status")
        .map(|value| trim_to_max(value, 32))
        .filter(|value| !value.is_empty());

    Some(InsightRequestPayload {
        schema_version: "pitgun-insight-request-v1".to_string(),
        run_id,
        session_id: trim_to_max(&envelope.session_id, 64),
        trace_id: event_trace_id,
        emitted_at_ms,
        context: InsightContext {
            circuit_id,
            era,
            lap: latest_lap,
            position,
            weather,
            track_status,
        },
        metrics,
        constraints: InsightConstraints {
            max_insights: 3,
            max_words_per_insight: 32,
            language: "en".to_string(),
        },
        policy_version: "policy.v1".to_string(),
        prompt_version: "chief-race.v1".to_string(),
    })
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

    use super::build_insight_request;

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
}
