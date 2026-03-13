use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::insight_requests::InsightRequestPayload;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct InsightResponsePayload {
    pub schema_version: String,
    pub run_id: String,
    pub session_id: String,
    pub trace_id: String,
    pub generated_at_ms: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_model: Option<String>,
    pub status: InsightStatus,
    pub insights: Vec<InsightItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warnings: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<InsightError>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InsightStatus {
    Ok,
    Degraded,
    InsufficientData,
    Error,
}

impl InsightStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Degraded => "degraded",
            Self::InsufficientData => "insufficient_data",
            Self::Error => "error",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct InsightItem {
    pub id: String,
    pub severity: InsightSeverity,
    pub confidence: f64,
    pub title: String,
    pub rationale: String,
    pub recommendation: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metric_keys: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl_ms: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InsightSeverity {
    Info,
    Advisory,
    Warning,
    Critical,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct InsightError {
    pub code: String,
    pub message: String,
}

impl InsightResponsePayload {
    pub fn normalize_from_model(
        mut model_response: Self,
        request: &InsightRequestPayload,
        source_model: &str,
        latency_ms: i64,
    ) -> Self {
        model_response.schema_version = "pitgun-insight-response-v1".to_string();
        model_response.run_id = request.run_id.clone();
        model_response.session_id = request.session_id.clone();
        model_response.trace_id = request.trace_id.clone();
        model_response.generated_at_ms = current_unix_ms();
        model_response.latency_ms = Some(latency_ms.max(0));
        model_response.source_model =
            Some(trim_non_empty_or_default(source_model, "unknown-model"));

        let max_insights = request.constraints.max_insights as usize;
        if model_response.insights.len() > max_insights {
            model_response.insights.truncate(max_insights);
            append_warning(
                &mut model_response.warnings,
                "insights truncated to request constraints".to_string(),
            );
        }

        model_response
    }

    pub fn error_from_request(
        request: &InsightRequestPayload,
        source_model: &str,
        latency_ms: i64,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            schema_version: "pitgun-insight-response-v1".to_string(),
            run_id: request.run_id.clone(),
            session_id: request.session_id.clone(),
            trace_id: request.trace_id.clone(),
            generated_at_ms: current_unix_ms(),
            latency_ms: Some(latency_ms.max(0)),
            source_model: Some(trim_non_empty_or_default(source_model, "unknown-model")),
            status: InsightStatus::Error,
            insights: Vec::new(),
            warnings: None,
            error: Some(InsightError {
                code: code.into(),
                message: message.into(),
            }),
        }
    }
}

fn append_warning(warnings: &mut Option<Vec<String>>, warning: String) {
    if let Some(values) = warnings.as_mut() {
        values.push(warning);
    } else {
        *warnings = Some(vec![warning]);
    }
}

fn trim_non_empty_or_default(value: &str, default: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        default.to_string()
    } else {
        trimmed.to_string()
    }
}

fn current_unix_ms() -> i64 {
    (OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000).max(0) as i64
}

#[cfg(test)]
mod tests {
    use crate::insight_requests::{InsightConstraints, InsightContext, InsightMetric};

    use super::{InsightResponsePayload, InsightSeverity, InsightStatus};

    fn sample_request() -> crate::insight_requests::InsightRequestPayload {
        crate::insight_requests::InsightRequestPayload {
            schema_version: "pitgun-insight-request-v1".to_string(),
            run_id: "run_001".to_string(),
            session_id: "session_001".to_string(),
            trace_id: "trace_001".to_string(),
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
                key: "pace.speed_kph.mean".to_string(),
                value: 205.0,
                unit: "kph".to_string(),
                trend: "flat".to_string(),
                horizon: "lap".to_string(),
                confidence: 0.9,
            }],
            constraints: InsightConstraints {
                max_insights: 1,
                max_words_per_insight: 32,
                language: "en".to_string(),
            },
            policy_version: "policy.v1".to_string(),
            prompt_version: "chief-race.v1".to_string(),
        }
    }

    #[test]
    fn normalize_overrides_ids_and_truncates() {
        let request = sample_request();
        let response = InsightResponsePayload {
            schema_version: "wrong".to_string(),
            run_id: "other".to_string(),
            session_id: "other".to_string(),
            trace_id: "other".to_string(),
            generated_at_ms: 1,
            latency_ms: None,
            source_model: None,
            status: InsightStatus::Ok,
            insights: vec![
                super::InsightItem {
                    id: "a".to_string(),
                    severity: InsightSeverity::Advisory,
                    confidence: 0.7,
                    title: "One".to_string(),
                    rationale: "One".to_string(),
                    recommendation: "One".to_string(),
                    metric_keys: None,
                    ttl_ms: None,
                    tags: None,
                },
                super::InsightItem {
                    id: "b".to_string(),
                    severity: InsightSeverity::Warning,
                    confidence: 0.7,
                    title: "Two".to_string(),
                    rationale: "Two".to_string(),
                    recommendation: "Two".to_string(),
                    metric_keys: None,
                    ttl_ms: None,
                    tags: None,
                },
            ],
            warnings: None,
            error: None,
        };

        let normalized =
            InsightResponsePayload::normalize_from_model(response, &request, "llama3.2:3b", 55);

        assert_eq!(normalized.schema_version, "pitgun-insight-response-v1");
        assert_eq!(normalized.run_id, "run_001");
        assert_eq!(normalized.session_id, "session_001");
        assert_eq!(normalized.trace_id, "trace_001");
        assert_eq!(normalized.latency_ms, Some(55));
        assert_eq!(normalized.insights.len(), 1);
        assert!(normalized.warnings.is_some());
    }

    #[test]
    fn creates_error_payload() {
        let request = sample_request();
        let response = InsightResponsePayload::error_from_request(
            &request,
            "llama3.2:3b",
            40,
            "llm_http_error",
            "timed out",
        );

        assert_eq!(response.status, InsightStatus::Error);
        assert_eq!(response.trace_id, "trace_001");
        assert!(response.error.is_some());
        assert!(response.insights.is_empty());
    }
}
