use std::time::Instant;

use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

use crate::{
    insight_requests::InsightRequestPayload,
    insight_responses::{InsightResponsePayload, InsightStatus},
};

const DEFAULT_TIMEOUT_MS: u64 = 8_000;
const DEFAULT_NUM_CTX: u32 = 1_024;
const DEFAULT_NUM_PREDICT: u32 = 180;
const DEFAULT_TEMPERATURE: f32 = 0.0;

#[derive(Clone, Debug)]
pub struct LlmCoreConfig {
    pub url: String,
    pub model: String,
    pub timeout_ms: u64,
    pub num_ctx: u32,
    pub num_predict: u32,
    pub temperature: f32,
}

impl LlmCoreConfig {
    pub fn with_defaults(url: String, model: String) -> Self {
        Self {
            url,
            model,
            timeout_ms: DEFAULT_TIMEOUT_MS,
            num_ctx: DEFAULT_NUM_CTX,
            num_predict: DEFAULT_NUM_PREDICT,
            temperature: DEFAULT_TEMPERATURE,
        }
    }
}

#[derive(Clone)]
pub struct LlmCoreClient {
    config: LlmCoreConfig,
    http: reqwest::Client,
}

#[derive(Clone, Debug)]
pub struct LlmCoreResult {
    pub response: InsightResponsePayload,
    pub raw_model_response: Option<String>,
    pub done_reason: Option<String>,
}

#[derive(Debug, Serialize)]
struct OllamaGenerateRequest {
    model: String,
    prompt: String,
    stream: bool,
    format: String,
    options: OllamaGenerateOptions,
}

#[derive(Debug, Serialize)]
struct OllamaGenerateOptions {
    num_ctx: u32,
    num_predict: u32,
    temperature: f32,
}

#[derive(Debug, Deserialize)]
struct OllamaGenerateResponse {
    #[serde(default)]
    model: String,
    #[serde(default)]
    response: String,
    #[serde(default)]
    done_reason: Option<String>,
}

impl LlmCoreClient {
    pub fn new(config: LlmCoreConfig) -> anyhow::Result<Self> {
        let timeout_ms = config.timeout_ms.max(250);
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(timeout_ms))
            .build()?;
        Ok(Self { config, http })
    }

    pub async fn generate_insights(&self, request: &InsightRequestPayload) -> LlmCoreResult {
        let started = Instant::now();
        let request_json = match serde_json::to_string(request) {
            Ok(value) => value,
            Err(err) => {
                return LlmCoreResult {
                    response: InsightResponsePayload::error_from_request(
                        request,
                        &self.config.model,
                        elapsed_ms(started),
                        "request_json_error",
                        format!("failed to serialize request: {err}"),
                    ),
                    raw_model_response: None,
                    done_reason: None,
                };
            }
        };

        let payload = OllamaGenerateRequest {
            model: self.config.model.clone(),
            prompt: build_prompt(&request_json),
            stream: false,
            format: "json".to_string(),
            options: OllamaGenerateOptions {
                num_ctx: self.config.num_ctx.max(128),
                num_predict: self.config.num_predict.max(32),
                temperature: self.config.temperature.max(0.0),
            },
        };

        let response = match self.http.post(&self.config.url).json(&payload).send().await {
            Ok(value) => value,
            Err(err) => {
                return LlmCoreResult {
                    response: InsightResponsePayload::error_from_request(
                        request,
                        &self.config.model,
                        elapsed_ms(started),
                        "llm_http_error",
                        format!("failed to call llm-core: {err}"),
                    ),
                    raw_model_response: None,
                    done_reason: None,
                };
            }
        };

        if response.status() != StatusCode::OK {
            let status = response.status();
            let body = response.text().await.unwrap_or_else(|_| String::new());
            let body_preview = truncate(&body, 220);
            return LlmCoreResult {
                response: InsightResponsePayload::error_from_request(
                    request,
                    &self.config.model,
                    elapsed_ms(started),
                    "llm_http_status",
                    format!("llm-core returned HTTP {status}: {body_preview}"),
                ),
                raw_model_response: Some(body_preview),
                done_reason: None,
            };
        }

        let api_response: OllamaGenerateResponse = match response.json().await {
            Ok(value) => value,
            Err(err) => {
                return LlmCoreResult {
                    response: InsightResponsePayload::error_from_request(
                        request,
                        &self.config.model,
                        elapsed_ms(started),
                        "llm_invalid_api_json",
                        format!("failed to decode ollama response: {err}"),
                    ),
                    raw_model_response: None,
                    done_reason: None,
                };
            }
        };

        let raw_model_response = api_response.response.trim().to_string();
        let source_model = if api_response.model.trim().is_empty() {
            self.config.model.clone()
        } else {
            api_response.model.trim().to_string()
        };

        let parsed_model_payload: InsightResponsePayload =
            match serde_json::from_str(&raw_model_response) {
                Ok(value) => value,
                Err(err) => {
                    return LlmCoreResult {
                        response: InsightResponsePayload::error_from_request(
                            request,
                            &source_model,
                            elapsed_ms(started),
                            "llm_invalid_contract_json",
                            format!("failed to decode model JSON: {err}"),
                        ),
                        raw_model_response: Some(truncate(&raw_model_response, 1_500)),
                        done_reason: api_response.done_reason,
                    };
                }
            };

        let mut normalized = InsightResponsePayload::normalize_from_model(
            parsed_model_payload,
            request,
            &source_model,
            elapsed_ms(started),
        );

        if normalized.status == InsightStatus::Error && normalized.error.is_none() {
            normalized.error = Some(crate::insight_responses::InsightError {
                code: "llm_reported_error".to_string(),
                message: "model returned status=error without error object".to_string(),
            });
        }

        LlmCoreResult {
            response: normalized,
            raw_model_response: Some(truncate(&raw_model_response, 8_000)),
            done_reason: api_response.done_reason,
        }
    }
}

fn build_prompt(request_json: &str) -> String {
    format!(
        "You are the Pitgun Chief Race Engineer.\n\
Convert the request JSON into one strict JSON object matching schema_version \"pitgun-insight-response-v1\".\n\
\n\
Hard rules:\n\
- Return valid JSON only (no markdown, no prose, no code fences).\n\
- Preserve run_id, session_id and trace_id from the request.\n\
- status must be one of: ok, degraded, insufficient_data, error.\n\
- Set generated_at_ms to current unix epoch in milliseconds.\n\
- Keep insights length <= constraints.max_insights.\n\
- Each insight must include: id, severity, confidence, title, rationale, recommendation.\n\
- severity must be one of: info, advisory, warning, critical.\n\
- confidence must be a number between 0 and 1.\n\
- Keep title/rationale/recommendation concise and actionable.\n\
- Use metric_keys only if they reference keys present in request.metrics[].key.\n\
\n\
Request JSON:\n\
{request_json}"
    )
}

fn elapsed_ms(started: Instant) -> i64 {
    started.elapsed().as_millis() as i64
}

fn truncate(value: &str, max_len: usize) -> String {
    if value.len() <= max_len {
        return value.to_string();
    }
    value.chars().take(max_len).collect()
}

#[cfg(test)]
mod tests {
    use super::build_prompt;

    #[test]
    fn prompt_contains_contract_targets() {
        let prompt = build_prompt("{\"schema_version\":\"pitgun-insight-request-v1\"}");
        assert!(prompt.contains("pitgun-insight-response-v1"));
        assert!(prompt.contains("status must be one of"));
        assert!(prompt.contains("Request JSON:"));
    }
}
