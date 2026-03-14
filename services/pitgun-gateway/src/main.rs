mod insight_ingress;
mod insight_requests;
mod insight_responses;
mod insight_stats_plan;
mod llm_core;
mod model;
mod storage;

use std::{
    collections::HashSet,
    net::SocketAddr,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

use axum::{
    Router,
    extract::{
        ConnectInfo, Query, State,
        ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade, close_code},
    },
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::get,
};
use insight_ingress::extract_sim_metric_points;
use insight_requests::build_insight_request;
use insight_stats_plan::resolve_insight_stats_plan;
use llm_core::{LlmCoreClient, LlmCoreConfig, LlmProvider};
use model::{EventPayload, parse_event_envelope};
use serde::Deserialize;
use storage::{
    EventInsertOutcome, IngestMetadata, InsightRequestInsertOutcome, InsightResponseInsertOutcome,
    QueueMessage, SqliteEventStore,
};
use tokio::{
    net::TcpListener,
    signal,
    sync::mpsc::{self, error::TrySendError},
};
use tracing::{debug, error, info, warn};

const DEFAULT_BIND_ADDR: &str = "127.0.0.1:8080";
const DEFAULT_DB_PATH: &str = "./telemetry/events.db";
const DEFAULT_SCHEMA_VERSION: &str = "pitgun-envelope-v1";
const DEFAULT_MAX_MESSAGE_BYTES: usize = 512 * 1024;
const DEFAULT_MAX_MESSAGES_PER_SEC: u32 = 120;
const DEFAULT_INGEST_QUEUE_SIZE: usize = 4096;
const DEFAULT_LLM_MODEL: &str = "llama3.2:3b";
const DEFAULT_LLM_TIMEOUT_MS: u64 = 8_000;
const DEFAULT_LLM_NUM_CTX: u32 = 1_024;
const DEFAULT_LLM_NUM_PREDICT: u32 = 180;
const DEFAULT_LLM_TEMPERATURE: f32 = 0.0;

#[derive(Clone)]
struct GatewayConfig {
    bind_addr: SocketAddr,
    allow_non_loopback: bool,
    db_path: String,
    schema_version: String,
    max_message_bytes: usize,
    max_messages_per_sec: u32,
    ingest_queue_size: usize,
    api_keys: HashSet<String>,
    insight_manifest_path: Option<String>,
    llm_provider: LlmProvider,
    llm_core_url: Option<String>,
    llm_api_key: Option<String>,
    llm_model: String,
    llm_timeout_ms: u64,
    llm_num_ctx: u32,
    llm_num_predict: u32,
    llm_temperature: f32,
}

#[derive(Clone)]
struct AppState {
    tx: mpsc::Sender<QueueMessage>,
    store: Arc<SqliteEventStore>,
    config: Arc<GatewayConfig>,
}

#[derive(Debug, Default, Deserialize)]
struct WsAuthQuery {
    token: Option<String>,
    api_key: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let log_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(log_filter).init();

    let config = Arc::new(GatewayConfig::from_env()?);
    let insight_stats_plan = Arc::new(resolve_insight_stats_plan(
        config.insight_manifest_path.as_deref(),
    )?);

    validate_bind_addr(config.bind_addr, config.allow_non_loopback)?;

    if config.api_keys.is_empty() {
        warn!("PITGUN_GATEWAY_API_KEY/PITGUN_GATEWAY_API_KEYS not set; websocket auth is disabled");
    }

    let store = Arc::new(SqliteEventStore::new(&config.db_path).await?);
    let llm_client = build_llm_client(&config)?;
    let (tx, rx) = mpsc::channel(config.ingest_queue_size);
    let queue_task = tokio::spawn(process_queue(
        store.clone(),
        insight_stats_plan,
        llm_client,
        rx,
    ));

    let app_state = AppState { tx, store, config };

    let app = Router::new()
        .route("/health", get(health))
        .route("/ws", get(ws_handler))
        .with_state(app_state.clone());

    let listener = TcpListener::bind(app_state.config.bind_addr).await?;
    info!(bind = %app_state.config.bind_addr, "pitgun-gateway listening");

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;

    drop(app_state);
    let _ = queue_task.await;

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut sig) = signal::unix::signal(signal::unix::SignalKind::terminate()) {
            sig.recv().await;
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {}
        _ = terminate => {}
    }
}

async fn process_queue(
    store: Arc<SqliteEventStore>,
    insight_stats_plan: Arc<insight_stats_plan::InsightStatsPlan>,
    llm_client: Option<Arc<LlmCoreClient>>,
    mut rx: mpsc::Receiver<QueueMessage>,
) {
    while let Some(msg) = rx.recv().await {
        if let EventPayload::TelemetrySampleBatch(payload) = &msg.envelope.payload {
            let extraction = extract_sim_metric_points(payload);
            debug!(
                event_id = %msg.envelope.event_id,
                session_id = %msg.envelope.session_id,
                frame_count = payload.frames.len(),
                sim_points = extraction.points.len(),
                dropped_non_sim = extraction.dropped_non_sim,
                dropped_non_numeric = extraction.dropped_non_numeric,
                dropped_bad_quality = extraction.dropped_bad_quality,
                unknown_parameter_ids = ?extraction.unknown_parameter_ids,
                "telemetry batch evaluated for sim-only insight ingress"
            );

            if let Some(request) =
                build_insight_request(&msg.envelope, payload, &extraction, &insight_stats_plan)
            {
                match store.insert_insight_request(&request).await {
                    Ok(InsightRequestInsertOutcome::Inserted) => {
                        debug!(
                            event_id = %msg.envelope.event_id,
                            trace_id = %request.trace_id,
                            metric_count = request.metrics.len(),
                            "insight request stored"
                        );

                        if let Some(client) = &llm_client {
                            let llm_result = client.generate_insights(&request).await;
                            match store
                                .insert_insight_response(
                                    &llm_result.response,
                                    llm_result.raw_model_response.as_deref(),
                                )
                                .await
                            {
                                Ok(InsightResponseInsertOutcome::Inserted) => {
                                    debug!(
                                        trace_id = %request.trace_id,
                                        status = %llm_result.response.status.as_str(),
                                        done_reason = ?llm_result.done_reason,
                                        "insight response stored"
                                    );
                                }
                                Ok(InsightResponseInsertOutcome::Duplicate) => {
                                    debug!(
                                        trace_id = %request.trace_id,
                                        "duplicate insight response dropped"
                                    );
                                }
                                Err(err) => {
                                    warn!(
                                        ?err,
                                        trace_id = %request.trace_id,
                                        "failed to persist insight response"
                                    );
                                }
                            }
                        }
                    }
                    Ok(InsightRequestInsertOutcome::Duplicate) => {
                        debug!(
                            event_id = %msg.envelope.event_id,
                            trace_id = %request.trace_id,
                            "duplicate insight request dropped"
                        );
                    }
                    Err(err) => {
                        warn!(
                            ?err,
                            event_id = %msg.envelope.event_id,
                            "failed to persist insight request"
                        );
                    }
                }
            }
        }

        match store.insert_event(msg).await {
            Ok(EventInsertOutcome::Inserted) => {}
            Ok(EventInsertOutcome::Duplicate) => {
                debug!("duplicate event dropped (event_id already exists)");
            }
            Err(err) => {
                error!(?err, "failed to persist event");
            }
        }
    }

    info!("ingestion queue closed");
}

async fn health(State(state): State<AppState>) -> impl IntoResponse {
    match state.store.health_check().await {
        Ok(()) => StatusCode::OK,
        Err(err) => {
            error!(?err, "health check failed");
            StatusCode::SERVICE_UNAVAILABLE
        }
    }
}

async fn ws_handler(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Query(query): Query<WsAuthQuery>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    let query_token = query.token.as_deref().or(query.api_key.as_deref());

    if !is_authorized(&headers, query_token, &state.config.api_keys) {
        warn!(peer = %peer, "websocket auth failed");
        return (StatusCode::UNAUTHORIZED, "missing or invalid API key").into_response();
    }

    let meta = build_metadata(&headers, peer);
    let max_message_bytes = state.config.max_message_bytes;

    ws.max_message_size(max_message_bytes)
        .max_frame_size(max_message_bytes)
        .on_upgrade(move |socket| handle_socket(socket, state, meta))
        .into_response()
}

async fn handle_socket(mut socket: WebSocket, state: AppState, meta: IngestMetadata) {
    let mut limiter = ConnectionRateLimiter::new(state.config.max_messages_per_sec);

    while let Some(message) = socket.recv().await {
        match message {
            Ok(Message::Text(text)) => {
                if text.len() > state.config.max_message_bytes {
                    close_with_reason(
                        &mut socket,
                        1009,
                        "message exceeds PITGUN_GATEWAY_MAX_MESSAGE_BYTES",
                    )
                    .await;
                    break;
                }

                if !limiter.allow() {
                    close_with_reason(&mut socket, close_code::POLICY, "rate limit exceeded").await;
                    break;
                }

                match parse_event_envelope(&text, &state.config.schema_version) {
                    Ok(envelope) => {
                        let msg = QueueMessage::new(envelope, text.to_string(), meta.clone());

                        match state.tx.try_send(msg) {
                            Ok(()) => {}
                            Err(TrySendError::Full(_)) => {
                                close_with_reason(
                                    &mut socket,
                                    1013,
                                    "ingestion queue is full; retry later",
                                )
                                .await;
                                break;
                            }
                            Err(TrySendError::Closed(_)) => {
                                close_with_reason(
                                    &mut socket,
                                    close_code::ERROR,
                                    "ingestion queue is unavailable",
                                )
                                .await;
                                break;
                            }
                        }
                    }
                    Err(err) => {
                        warn!(?err, "invalid websocket payload");
                        close_with_reason(&mut socket, close_code::POLICY, "invalid payload").await;
                        break;
                    }
                }
            }
            Ok(Message::Binary(_)) => {
                close_with_reason(
                    &mut socket,
                    close_code::UNSUPPORTED,
                    "binary payload is not supported; send JSON text",
                )
                .await;
                break;
            }
            Ok(Message::Ping(payload)) => {
                if socket.send(Message::Pong(payload)).await.is_err() {
                    break;
                }
            }
            Ok(Message::Pong(_)) => {}
            Ok(Message::Close(_)) => break,
            Err(err) => {
                warn!(?err, "websocket transport error");
                break;
            }
        }
    }

    let _ = socket.close().await;
    info!("websocket connection closed");
}

async fn close_with_reason(socket: &mut WebSocket, code: u16, reason: &'static str) {
    let _ = socket
        .send(Message::Close(Some(CloseFrame {
            code,
            reason: reason.into(),
        })))
        .await;
}

fn build_metadata(headers: &HeaderMap, peer: SocketAddr) -> IngestMetadata {
    let user_agent = headers
        .get(axum::http::header::USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_string());

    let forwarded = headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(|raw| raw.trim().to_string());

    let remote_ip = forwarded
        .filter(|value| !value.is_empty())
        .or_else(|| {
            headers
                .get("x-real-ip")
                .and_then(|value| value.to_str().ok())
                .map(|value| value.to_string())
        })
        .or_else(|| Some(peer.ip().to_string()));

    IngestMetadata {
        remote_ip,
        user_agent,
    }
}

fn is_authorized(
    headers: &HeaderMap,
    query_token: Option<&str>,
    api_keys: &HashSet<String>,
) -> bool {
    if api_keys.is_empty() {
        return true;
    }

    extract_api_key(headers, query_token)
        .as_ref()
        .is_some_and(|provided| api_keys.contains(provided))
}

fn extract_api_key(headers: &HeaderMap, query_token: Option<&str>) -> Option<String> {
    if let Some(value) = headers
        .get("x-api-key")
        .and_then(|value| value.to_str().ok())
    {
        let token = value.trim();
        if !token.is_empty() {
            return Some(token.to_string());
        }
    }

    headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(parse_bearer_token)
        .map(|value| value.to_string())
        .or_else(|| parse_query_token(query_token))
}

fn parse_bearer_token(value: &str) -> Option<&str> {
    let mut parts = value.splitn(2, ' ');
    let scheme = parts.next()?.trim();
    let token = parts.next()?.trim();

    if !scheme.eq_ignore_ascii_case("bearer") || token.is_empty() {
        return None;
    }

    Some(token)
}

fn parse_query_token(value: Option<&str>) -> Option<String> {
    let token = value?.trim();
    if token.is_empty() {
        return None;
    }
    Some(token.to_string())
}

fn allow_non_loopback_enabled(raw: Option<String>) -> bool {
    raw.map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn validate_bind_addr(addr: SocketAddr, allow_non_loopback: bool) -> anyhow::Result<()> {
    if !addr.ip().is_loopback() {
        if allow_non_loopback {
            warn!(
                bind = %addr,
                "allowing non-loopback bind because PITGUN_GATEWAY_ALLOW_NON_LOOPBACK is set"
            );
        } else {
            anyhow::bail!(
                "pitgun-gateway must bind to a loopback address unless PITGUN_GATEWAY_ALLOW_NON_LOOPBACK=1 is set; got {addr}"
            );
        }
    }

    Ok(())
}

impl GatewayConfig {
    fn from_env() -> anyhow::Result<Self> {
        let bind_addr_raw =
            std::env::var("PITGUN_GATEWAY_BIND").unwrap_or_else(|_| DEFAULT_BIND_ADDR.to_string());

        let bind_addr = bind_addr_raw
            .parse()
            .map_err(|err| anyhow::anyhow!("invalid PITGUN_GATEWAY_BIND: {err}"))?;

        let allow_non_loopback =
            allow_non_loopback_enabled(std::env::var("PITGUN_GATEWAY_ALLOW_NON_LOOPBACK").ok());

        let db_path = read_db_path();

        let schema_version = std::env::var("PITGUN_GATEWAY_SCHEMA_VERSION")
            .unwrap_or_else(|_| DEFAULT_SCHEMA_VERSION.to_string());

        let max_message_bytes = read_env_usize(
            "PITGUN_GATEWAY_MAX_MESSAGE_BYTES",
            DEFAULT_MAX_MESSAGE_BYTES,
        )?;

        let max_messages_per_sec = read_env_u32(
            "PITGUN_GATEWAY_MAX_MESSAGES_PER_SEC",
            DEFAULT_MAX_MESSAGES_PER_SEC,
        )?;

        let ingest_queue_size = read_env_usize(
            "PITGUN_GATEWAY_INGEST_QUEUE_SIZE",
            DEFAULT_INGEST_QUEUE_SIZE,
        )?;

        let api_keys = read_api_keys();
        let insight_manifest_path = std::env::var("PITGUN_GATEWAY_INSIGHT_MANIFEST")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        let llm_provider = std::env::var("PITGUN_GATEWAY_LLM_PROVIDER")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .map(|value| {
                LlmProvider::from_env_value(&value).ok_or_else(|| {
                    anyhow::anyhow!(
                        "invalid PITGUN_GATEWAY_LLM_PROVIDER: {value} (expected ollama or openai_compatible)"
                    )
                })
            })
            .transpose()?
            .unwrap_or(LlmProvider::Ollama);

        let llm_core_url = std::env::var("PITGUN_GATEWAY_LLM_URL")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .or_else(|| {
                std::env::var("PITGUN_GATEWAY_LLM_CORE_URL")
                    .ok()
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty())
            });

        let llm_api_key = std::env::var("PITGUN_GATEWAY_LLM_API_KEY")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        let llm_model = std::env::var("PITGUN_GATEWAY_LLM_MODEL")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .or_else(|| {
                std::env::var("OLLAMA_MODEL")
                    .ok()
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty())
            })
            .unwrap_or_else(|| DEFAULT_LLM_MODEL.to_string());

        let llm_timeout_ms = read_env_u64("PITGUN_GATEWAY_LLM_TIMEOUT_MS", DEFAULT_LLM_TIMEOUT_MS)?;
        let llm_num_ctx = read_env_u32("PITGUN_GATEWAY_LLM_NUM_CTX", DEFAULT_LLM_NUM_CTX)?;
        let llm_num_predict =
            read_env_u32("PITGUN_GATEWAY_LLM_NUM_PREDICT", DEFAULT_LLM_NUM_PREDICT)?;
        let llm_temperature =
            read_env_f32("PITGUN_GATEWAY_LLM_TEMPERATURE", DEFAULT_LLM_TEMPERATURE)?;

        Ok(Self {
            bind_addr,
            allow_non_loopback,
            db_path,
            schema_version,
            max_message_bytes,
            max_messages_per_sec,
            ingest_queue_size,
            api_keys,
            insight_manifest_path,
            llm_provider,
            llm_core_url,
            llm_api_key,
            llm_model,
            llm_timeout_ms,
            llm_num_ctx,
            llm_num_predict,
            llm_temperature,
        })
    }
}

fn build_llm_client(config: &GatewayConfig) -> anyhow::Result<Option<Arc<LlmCoreClient>>> {
    let Some(url) = config.llm_core_url.as_ref() else {
        info!("llm-core integration disabled (PITGUN_GATEWAY_LLM_CORE_URL not set)");
        return Ok(None);
    };

    let mut llm_config = LlmCoreConfig::with_defaults(url.clone(), config.llm_model.clone());
    llm_config.provider = config.llm_provider;
    llm_config.api_key = config.llm_api_key.clone();
    llm_config.timeout_ms = config.llm_timeout_ms;
    llm_config.num_ctx = config.llm_num_ctx;
    llm_config.num_predict = config.llm_num_predict;
    llm_config.temperature = config.llm_temperature;

    let client = LlmCoreClient::new(llm_config)?;
    info!(
        llm_url = %url,
        llm_provider = %config.llm_provider.as_str(),
        llm_model = %config.llm_model,
        "llm-core integration enabled"
    );

    Ok(Some(Arc::new(client)))
}

fn read_env_u32(key: &str, default: u32) -> anyhow::Result<u32> {
    match std::env::var(key) {
        Ok(value) => value
            .trim()
            .parse::<u32>()
            .map_err(|err| anyhow::anyhow!("invalid {key}: {err}")),
        Err(_) => Ok(default),
    }
}

fn read_env_u64(key: &str, default: u64) -> anyhow::Result<u64> {
    match std::env::var(key) {
        Ok(value) => value
            .trim()
            .parse::<u64>()
            .map_err(|err| anyhow::anyhow!("invalid {key}: {err}")),
        Err(_) => Ok(default),
    }
}

fn read_env_f32(key: &str, default: f32) -> anyhow::Result<f32> {
    match std::env::var(key) {
        Ok(value) => value
            .trim()
            .parse::<f32>()
            .map_err(|err| anyhow::anyhow!("invalid {key}: {err}")),
        Err(_) => Ok(default),
    }
}

fn read_env_usize(key: &str, default: usize) -> anyhow::Result<usize> {
    match std::env::var(key) {
        Ok(value) => value
            .trim()
            .parse::<usize>()
            .map_err(|err| anyhow::anyhow!("invalid {key}: {err}")),
        Err(_) => Ok(default),
    }
}

fn read_api_keys() -> HashSet<String> {
    let mut keys = HashSet::new();

    if let Ok(value) = std::env::var("PITGUN_GATEWAY_API_KEY") {
        let token = value.trim();
        if !token.is_empty() {
            keys.insert(token.to_string());
        }
    }

    if let Ok(value) = std::env::var("PITGUN_GATEWAY_API_KEYS") {
        for part in value.split(',') {
            let token = part.trim();
            if !token.is_empty() {
                keys.insert(token.to_string());
            }
        }
    }

    keys
}

fn read_db_path() -> String {
    if let Ok(value) = std::env::var("PITGUN_GATEWAY_DB_PATH") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }

    if let Ok(data_dir) = std::env::var("PITGUN_GATEWAY_DATA_DIR") {
        let trimmed = data_dir.trim();
        if !trimmed.is_empty() {
            let mut path = PathBuf::from(trimmed);
            path.push("events.db");
            return path.to_string_lossy().to_string();
        }
    }

    DEFAULT_DB_PATH.to_string()
}

struct ConnectionRateLimiter {
    max_per_sec: u32,
    window_started_at: Instant,
    count: u32,
}

impl ConnectionRateLimiter {
    fn new(max_per_sec: u32) -> Self {
        Self {
            max_per_sec,
            window_started_at: Instant::now(),
            count: 0,
        }
    }

    fn allow(&mut self) -> bool {
        if self.max_per_sec == 0 {
            return true;
        }

        let now = Instant::now();
        if now.duration_since(self.window_started_at) >= Duration::from_secs(1) {
            self.window_started_at = now;
            self.count = 0;
        }

        self.count = self.count.saturating_add(1);
        self.count <= self.max_per_sec
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ConnectionRateLimiter, allow_non_loopback_enabled, parse_bearer_token, parse_query_token,
        read_api_keys, read_db_path, validate_bind_addr,
    };
    use std::net::SocketAddr;

    #[test]
    fn non_loopback_fails_without_flag() {
        let addr: SocketAddr = "0.0.0.0:8080".parse().unwrap();
        let err = validate_bind_addr(addr, false).unwrap_err();

        assert_eq!(
            err.to_string(),
            "pitgun-gateway must bind to a loopback address unless PITGUN_GATEWAY_ALLOW_NON_LOOPBACK=1 is set; got 0.0.0.0:8080"
        );
    }

    #[test]
    fn non_loopback_allowed_with_flag() {
        let addr: SocketAddr = "0.0.0.0:8080".parse().unwrap();
        assert!(validate_bind_addr(addr, true).is_ok());
    }

    #[test]
    fn parses_allow_non_loopback_values() {
        assert!(allow_non_loopback_enabled(Some("1".to_string())));
        assert!(allow_non_loopback_enabled(Some("TRUE".to_string())));
        assert!(!allow_non_loopback_enabled(Some("yes".to_string())));
        assert!(!allow_non_loopback_enabled(None));
    }

    #[test]
    fn parses_bearer_token() {
        assert_eq!(parse_bearer_token("Bearer abc"), Some("abc"));
        assert_eq!(parse_bearer_token("bearer abc"), Some("abc"));
        assert_eq!(parse_bearer_token("Basic abc"), None);
        assert_eq!(parse_bearer_token("Bearer"), None);
    }

    #[test]
    fn parses_query_token() {
        assert_eq!(parse_query_token(Some("abc")), Some("abc".to_string()));
        assert_eq!(parse_query_token(Some("  abc  ")), Some("abc".to_string()));
        assert_eq!(parse_query_token(Some("   ")), None);
        assert_eq!(parse_query_token(None), None);
    }

    #[test]
    fn rate_limiter_enforces_limit() {
        let mut limiter = ConnectionRateLimiter::new(2);

        assert!(limiter.allow());
        assert!(limiter.allow());
        assert!(!limiter.allow());
    }

    #[test]
    fn read_api_keys_collects_both_env_vars() {
        // Safety: tests can mutate process environment for their own process.
        unsafe {
            std::env::set_var("PITGUN_GATEWAY_API_KEY", "single");
            std::env::set_var("PITGUN_GATEWAY_API_KEYS", "alpha,beta");
        }

        let keys = read_api_keys();

        assert!(keys.contains("single"));
        assert!(keys.contains("alpha"));
        assert!(keys.contains("beta"));

        // Safety: cleanup local process test env keys.
        unsafe {
            std::env::remove_var("PITGUN_GATEWAY_API_KEY");
            std::env::remove_var("PITGUN_GATEWAY_API_KEYS");
        }
    }

    #[test]
    fn read_db_path_uses_data_dir_fallback() {
        // Safety: tests can mutate process environment for their own process.
        unsafe {
            std::env::remove_var("PITGUN_GATEWAY_DB_PATH");
            std::env::set_var("PITGUN_GATEWAY_DATA_DIR", "/tmp/pitgun-data");
        }

        let path = read_db_path();
        assert_eq!(path, "/tmp/pitgun-data/events.db");

        // Safety: cleanup local process test env keys.
        unsafe {
            std::env::remove_var("PITGUN_GATEWAY_DATA_DIR");
        }
    }
}
