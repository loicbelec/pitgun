use std::time::Instant;

use reqwest::{
    StatusCode,
    header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue},
};
use serde::{Deserialize, Serialize};

use crate::{
    insight_requests::InsightRequestPayload,
    insight_responses::{InsightResponsePayload, InsightStatus},
};

const DEFAULT_TIMEOUT_MS: u64 = 8_000;
const DEFAULT_NUM_CTX: u32 = 1_024;
const DEFAULT_NUM_PREDICT: u32 = 180;
const DEFAULT_TEMPERATURE: f32 = 0.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LlmProvider {
    Ollama,
    OpenAiCompatible,
}

impl LlmProvider {
    pub fn from_env_value(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "ollama" => Some(Self::Ollama),
            "openai_compatible" => Some(Self::OpenAiCompatible),
            "openai-compatible" => Some(Self::OpenAiCompatible),
            "gemini_openai" => Some(Self::OpenAiCompatible),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ollama => "ollama",
            Self::OpenAiCompatible => "openai_compatible",
        }
    }
}

#[derive(Clone, Debug)]
pub struct LlmCoreConfig {
    pub provider: LlmProvider,
    pub url: String,
    pub model: String,
    pub api_key: Option<String>,
    pub timeout_ms: u64,
    pub num_ctx: u32,
    pub num_predict: u32,
    pub temperature: f32,
}

impl LlmCoreConfig {
    pub fn with_defaults(url: String, model: String) -> Self {
        Self {
            provider: LlmProvider::Ollama,
            url,
            model,
            api_key: None,
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

#[derive(Debug, Serialize)]
struct OpenAiChatCompletionsRequest {
    model: String,
    temperature: f32,
    max_tokens: u32,
    messages: Vec<OpenAiMessage>,
}

#[derive(Debug, Serialize)]
struct OpenAiMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct OpenAiChatCompletionsResponse {
    #[serde(default)]
    model: String,
    #[serde(default)]
    choices: Vec<OpenAiChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChoice {
    #[serde(default)]
    finish_reason: Option<String>,
    message: OpenAiAssistantMessage,
}

#[derive(Debug, Deserialize)]
struct OpenAiAssistantMessage {
    #[serde(default)]
    content: String,
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
                return self.error_result(
                    request,
                    started,
                    "request_json_error",
                    format!("failed to serialize request: {err}"),
                    None,
                    None,
                );
            }
        };

        match self.config.provider {
            LlmProvider::Ollama => {
                self.generate_with_ollama(request, &request_json, started)
                    .await
            }
            LlmProvider::OpenAiCompatible => {
                self.generate_with_openai_compatible(request, &request_json, started)
                    .await
            }
        }
    }

    async fn generate_with_ollama(
        &self,
        request: &InsightRequestPayload,
        request_json: &str,
        started: Instant,
    ) -> LlmCoreResult {
        let payload = OllamaGenerateRequest {
            model: self.config.model.clone(),
            prompt: build_prompt(request_json),
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
                return self.error_result(
                    request,
                    started,
                    "llm_http_error",
                    format!("failed to call llm provider: {err}"),
                    None,
                    None,
                );
            }
        };

        if response.status() != StatusCode::OK {
            let status = response.status();
            let body = response.text().await.unwrap_or_else(|_| String::new());
            let body_preview = truncate(&body, 220);
            return self.error_result(
                request,
                started,
                "llm_http_status",
                format!("llm provider returned HTTP {status}: {body_preview}"),
                Some(body_preview),
                None,
            );
        }

        let api_response: OllamaGenerateResponse = match response.json().await {
            Ok(value) => value,
            Err(err) => {
                return self.error_result(
                    request,
                    started,
                    "llm_invalid_api_json",
                    format!("failed to decode ollama response: {err}"),
                    None,
                    None,
                );
            }
        };

        let raw_model_response = api_response.response.trim().to_string();
        let source_model = if api_response.model.trim().is_empty() {
            self.config.model.clone()
        } else {
            api_response.model.trim().to_string()
        };

        self.normalize_model_response(
            request,
            started,
            &source_model,
            &raw_model_response,
            Some(truncate(&raw_model_response, 8_000)),
            api_response.done_reason,
        )
    }

    async fn generate_with_openai_compatible(
        &self,
        request: &InsightRequestPayload,
        request_json: &str,
        started: Instant,
    ) -> LlmCoreResult {
        let payload = OpenAiChatCompletionsRequest {
            model: self.config.model.clone(),
            temperature: self.config.temperature.max(0.0),
            max_tokens: self.config.num_predict.max(32),
            messages: vec![
                OpenAiMessage {
                    role: "system".to_string(),
                    content: "You are the Pitgun Chief Race Engineer.".to_string(),
                },
                OpenAiMessage {
                    role: "user".to_string(),
                    content: build_prompt(request_json),
                },
            ],
        };

        let headers = match build_openai_headers(self.config.api_key.as_deref()) {
            Ok(value) => value,
            Err(err) => {
                return self.error_result(
                    request,
                    started,
                    "llm_config_error",
                    format!("invalid API key header: {err}"),
                    None,
                    None,
                );
            }
        };

        let response = match self
            .http
            .post(&self.config.url)
            .headers(headers)
            .json(&payload)
            .send()
            .await
        {
            Ok(value) => value,
            Err(err) => {
                return self.error_result(
                    request,
                    started,
                    "llm_http_error",
                    format!("failed to call llm provider: {err}"),
                    None,
                    None,
                );
            }
        };

        if response.status() != StatusCode::OK {
            let status = response.status();
            let body = response.text().await.unwrap_or_else(|_| String::new());
            let body_preview = truncate(&body, 300);
            return self.error_result(
                request,
                started,
                "llm_http_status",
                format!("llm provider returned HTTP {status}: {body_preview}"),
                Some(body_preview),
                None,
            );
        }

        let api_response: OpenAiChatCompletionsResponse = match response.json().await {
            Ok(value) => value,
            Err(err) => {
                return self.error_result(
                    request,
                    started,
                    "llm_invalid_api_json",
                    format!("failed to decode openai-compatible response: {err}"),
                    None,
                    None,
                );
            }
        };

        let Some(first_choice) = api_response.choices.first() else {
            return self.error_result(
                request,
                started,
                "llm_invalid_api_json",
                "openai-compatible response had no choices".to_string(),
                None,
                None,
            );
        };

        let raw_model_response = first_choice.message.content.trim().to_string();
        if raw_model_response.is_empty() {
            return self.error_result(
                request,
                started,
                "llm_invalid_api_json",
                "openai-compatible response had empty message content".to_string(),
                None,
                first_choice.finish_reason.clone(),
            );
        }

        let source_model = if api_response.model.trim().is_empty() {
            self.config.model.clone()
        } else {
            api_response.model.trim().to_string()
        };

        self.normalize_model_response(
            request,
            started,
            &source_model,
            &raw_model_response,
            Some(truncate(&raw_model_response, 8_000)),
            first_choice.finish_reason.clone(),
        )
    }

    fn normalize_model_response(
        &self,
        request: &InsightRequestPayload,
        started: Instant,
        source_model: &str,
        raw_model_response: &str,
        raw_preview: Option<String>,
        done_reason: Option<String>,
    ) -> LlmCoreResult {
        let parsed_model_payload: InsightResponsePayload =
            match serde_json::from_str(raw_model_response) {
                Ok(value) => value,
                Err(err) => {
                    return self.error_result(
                        request,
                        started,
                        "llm_invalid_contract_json",
                        format!("failed to decode model JSON: {err}"),
                        Some(truncate(raw_model_response, 1_500)),
                        done_reason,
                    );
                }
            };

        let mut normalized = InsightResponsePayload::normalize_from_model(
            parsed_model_payload,
            request,
            source_model,
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
            raw_model_response: raw_preview,
            done_reason,
        }
    }

    fn error_result(
        &self,
        request: &InsightRequestPayload,
        started: Instant,
        code: &str,
        message: String,
        raw_model_response: Option<String>,
        done_reason: Option<String>,
    ) -> LlmCoreResult {
        LlmCoreResult {
            response: InsightResponsePayload::error_from_request(
                request,
                &self.config.model,
                elapsed_ms(started),
                code,
                message,
            ),
            raw_model_response,
            done_reason,
        }
    }
}

fn build_openai_headers(api_key: Option<&str>) -> anyhow::Result<HeaderMap> {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

    if let Some(value) = api_key {
        let token = value.trim();
        if !token.is_empty() {
            let auth_value = format!("Bearer {token}");
            headers.insert(AUTHORIZATION, HeaderValue::from_str(&auth_value)?);
        }
    }

    Ok(headers)
}

fn build_prompt(request_json: &str) -> String {
    format!(
        "Convert the request JSON into one strict JSON object matching schema_version \"pitgun-insight-response-v1\".\n\
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
    use super::{LlmProvider, build_prompt};

    #[test]
    fn prompt_contains_contract_targets() {
        let prompt = build_prompt("{\"schema_version\":\"pitgun-insight-request-v1\"}");
        assert!(prompt.contains("pitgun-insight-response-v1"));
        assert!(prompt.contains("status must be one of"));
        assert!(prompt.contains("Request JSON:"));
    }

    #[test]
    fn parses_provider_aliases() {
        assert_eq!(
            LlmProvider::from_env_value("ollama"),
            Some(LlmProvider::Ollama)
        );
        assert_eq!(
            LlmProvider::from_env_value("openai_compatible"),
            Some(LlmProvider::OpenAiCompatible)
        );
        assert_eq!(
            LlmProvider::from_env_value("gemini_openai"),
            Some(LlmProvider::OpenAiCompatible)
        );
        assert_eq!(LlmProvider::from_env_value("unknown"), None);
    }
}
