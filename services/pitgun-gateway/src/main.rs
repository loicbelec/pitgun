mod insight_ingress;
mod insight_requests;
mod insight_responses;
mod insight_stats_plan;
mod llm_core;
mod model;
mod questdb;
mod run_registry;
mod storage;

use std::{
    collections::{BTreeMap, HashMap, HashSet},
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
use insight_ingress::{extract_sim_metric_points, extract_sim_metric_points_from_frame};
use insight_requests::{
    InsightRequestPayload, LapSummaryPayload, MetricAggregate, SessionSummaryPayload,
    accumulate_metric_point, build_insight_request, build_insight_request_from_lap_summary,
    build_insight_request_from_session_summary, build_lap_summary, build_race_summary_from_session,
    build_session_summary,
};
use insight_stats_plan::resolve_insight_stats_plan;
use llm_core::{LlmCoreClient, LlmCoreConfig, LlmProvider};
use model::{EventPayload, parse_event_envelope};
use pitgun_contract::TelemetryFrame;
use questdb::{QuestDbStore, TelemetryPointRow};
use run_registry::{RunRegistryClient, RunRegistryUpsertRequest};
use serde::Deserialize;
use storage::{
    EventInsertOutcome, IngestMetadata, InsightRequestInsertOutcome, InsightResponseInsertOutcome,
    LapSummaryInsertOutcome, QueueMessage, SqliteEventStore, SqliteJournalMode,
};
use time::OffsetDateTime;
use tokio::{
    net::TcpListener,
    signal,
    sync::mpsc::{self, error::TrySendError},
    time::sleep,
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
const DEFAULT_LLM_DISPATCH_MODE: &str = "lap_end";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LlmDispatchMode {
    PerRequest,
    LapEnd,
    SessionEndSummary,
}

impl LlmDispatchMode {
    fn from_env_value(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "per_request" => Some(Self::PerRequest),
            "lap_end" => Some(Self::LapEnd),
            "session_end_summary" => Some(Self::SessionEndSummary),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::PerRequest => "per_request",
            Self::LapEnd => "lap_end",
            Self::SessionEndSummary => "session_end_summary",
        }
    }
}

#[derive(Clone)]
struct GatewayConfig {
    bind_addr: SocketAddr,
    allow_non_loopback: bool,
    db_path: String,
    sqlite_journal_mode: SqliteJournalMode,
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
    llm_dispatch_mode: LlmDispatchMode,
    questdb_url: Option<String>,
    run_registry_url: Option<String>,
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

    let store = Arc::new(SqliteEventStore::new(&config.db_path, config.sqlite_journal_mode).await?);
    let questdb_store = build_questdb_store(&config).await?;
    backfill_practice_summaries(questdb_store.as_deref()).await?;
    backfill_race_summaries(questdb_store.as_deref()).await?;
    let run_registry_client = build_run_registry_client(&config)?;
    let llm_client = build_llm_client(&config)?;
    let (tx, rx) = mpsc::channel(config.ingest_queue_size);
    let queue_task = tokio::spawn(process_queue(
        store.clone(),
        questdb_store,
        run_registry_client,
        insight_stats_plan,
        llm_client,
        config.llm_dispatch_mode,
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
    questdb_store: Option<Arc<QuestDbStore>>,
    run_registry_client: Option<Arc<RunRegistryClient>>,
    insight_stats_plan: Arc<insight_stats_plan::InsightStatsPlan>,
    llm_client: Option<Arc<LlmCoreClient>>,
    llm_dispatch_mode: LlmDispatchMode,
    mut rx: mpsc::Receiver<QueueMessage>,
) {
    let mut session_states: HashMap<String, SessionAggregationState> = HashMap::new();

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

            let session_state = session_states
                .entry(msg.envelope.session_id.clone())
                .or_insert_with(|| {
                    SessionAggregationState::new(
                        msg.envelope.player_id.clone(),
                        msg.envelope.session_id.clone(),
                        msg.envelope.event_id.to_string(),
                    )
                });

            let mut telemetry_points = Vec::new();
            for frame in &payload.frames {
                let frame_extraction = extract_sim_metric_points_from_frame(frame);
                telemetry_points.extend(build_telemetry_points(
                    &msg.envelope,
                    frame,
                    &frame_extraction,
                ));
                let completed_laps =
                    session_state.ingest_frame(frame, &frame_extraction, &insight_stats_plan);
                for summary in completed_laps {
                    persist_lap_summary_and_dispatch(
                        &store,
                        questdb_store.as_ref(),
                        llm_client.as_ref(),
                        llm_dispatch_mode,
                        summary,
                    )
                    .await;
                }
            }

            persist_telemetry_points(questdb_store.as_ref(), &telemetry_points).await;

            if matches!(llm_dispatch_mode, LlmDispatchMode::PerRequest) {
                if let Some(request) =
                    build_insight_request(&msg.envelope, payload, &extraction, &insight_stats_plan)
                {
                    persist_insight_request_and_dispatch(
                        &store,
                        llm_client.as_ref(),
                        request,
                        "telemetry.sample_batch",
                    )
                    .await;
                }
            }
        }

        if let EventPayload::PitWallSessionConfigured(payload) = &msg.envelope.payload {
            persist_run_configuration(
                run_registry_client.as_ref(),
                &msg.envelope.player_id,
                &msg.envelope.session_id,
                payload,
            )
            .await;
        }

        if matches!(msg.envelope.payload, EventPayload::SessionEnd(_)) {
            let session_id = msg.envelope.session_id.clone();
            if let Some(mut state) = session_states.remove(&session_id) {
                if let Some(summary) = state.finalize_open_lap(&insight_stats_plan) {
                    persist_lap_summary_and_dispatch(
                        &store,
                        questdb_store.as_ref(),
                        llm_client.as_ref(),
                        llm_dispatch_mode,
                        summary,
                    )
                    .await;
                }

                let emitted_at_ms =
                    (msg.envelope.ts.unix_timestamp_nanos() / 1_000_000).max(0) as i64;
                if let Some(summary) =
                    state.build_session_summary_payload(emitted_at_ms, &insight_stats_plan)
                {
                    persist_session_summary(questdb_store.as_ref(), summary.clone()).await;

                    persist_practice_summary(
                        questdb_store.as_ref(),
                        summary.weekend_id.as_deref(),
                        emitted_at_ms,
                    )
                    .await;
                    persist_race_summary(questdb_store.as_ref(), &summary).await;

                    if matches!(llm_dispatch_mode, LlmDispatchMode::SessionEndSummary) {
                        let trace_id = format!("{}-session-summary", msg.envelope.event_id);
                        let request =
                            build_insight_request_from_session_summary(&summary, trace_id);
                        persist_insight_request_and_dispatch(
                            &store,
                            llm_client.as_ref(),
                            request,
                            "session.end",
                        )
                        .await;
                    }
                } else {
                    debug!(
                        event_id = %msg.envelope.event_id,
                        session_id = %msg.envelope.session_id,
                        "no compact session summary produced for session.end"
                    );
                }
            } else {
                debug!(
                    event_id = %msg.envelope.event_id,
                    session_id = %msg.envelope.session_id,
                    "no telemetry aggregation state found for session.end"
                );
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

async fn persist_lap_summary_and_dispatch(
    store: &Arc<SqliteEventStore>,
    questdb_store: Option<&Arc<QuestDbStore>>,
    llm_client: Option<&Arc<LlmCoreClient>>,
    llm_dispatch_mode: LlmDispatchMode,
    summary: LapSummaryPayload,
) {
    match store.insert_lap_summary(&summary).await {
        Ok(LapSummaryInsertOutcome::Inserted) => {
            debug!(
                summary_id = %summary.summary_id,
                lap = summary.lap_number,
                metric_count = summary.metrics.len(),
                "lap summary stored"
            );

            if let Some(questdb_store) = questdb_store {
                match questdb_store.insert_lap_summary(&summary).await {
                    Ok(()) => {
                        debug!(
                            summary_id = %summary.summary_id,
                            lap = summary.lap_number,
                            "lap summary mirrored to QuestDB"
                        );
                    }
                    Err(err) => {
                        warn!(
                            ?err,
                            summary_id = %summary.summary_id,
                            "failed to mirror lap summary to QuestDB"
                        );
                    }
                }
            }

            if matches!(llm_dispatch_mode, LlmDispatchMode::LapEnd) {
                let trace_id = format!("{}:insight", summary.summary_id);
                let request = build_insight_request_from_lap_summary(&summary, trace_id);
                persist_insight_request_and_dispatch(store, llm_client, request, "lap.end").await;
            }
        }
        Ok(LapSummaryInsertOutcome::Duplicate) => {
            debug!(
                summary_id = %summary.summary_id,
                "duplicate lap summary dropped"
            );
        }
        Err(err) => {
            warn!(
                ?err,
                summary_id = %summary.summary_id,
                "failed to persist lap summary"
            );
        }
    }
}

async fn persist_telemetry_points(
    questdb_store: Option<&Arc<QuestDbStore>>,
    points: &[TelemetryPointRow],
) {
    let Some(questdb_store) = questdb_store else {
        return;
    };

    if points.is_empty() {
        return;
    }

    match questdb_store.insert_telemetry_points(points).await {
        Ok(()) => {
            debug!(
                point_count = points.len(),
                "telemetry points mirrored to QuestDB"
            );
        }
        Err(err) => {
            warn!(
                ?err,
                point_count = points.len(),
                "failed to mirror telemetry points to QuestDB"
            );
        }
    }
}

async fn persist_run_configuration(
    run_registry_client: Option<&Arc<RunRegistryClient>>,
    player_id: &str,
    session_id: &str,
    payload: &model::PitWallSessionConfiguredPayload,
) {
    let Some(run_registry_client) = run_registry_client else {
        return;
    };

    let request = RunRegistryUpsertRequest::from_configured_event(player_id, session_id, payload);
    match run_registry_client.upsert_run(&request).await {
        Ok(()) => {
            debug!(run_id = %payload.run_id, "pitwall run mirrored to run registry");
        }
        Err(err) => {
            warn!(?err, run_id = %payload.run_id, "failed to mirror pitwall run to run registry");
        }
    }
}

async fn persist_session_summary(
    questdb_store: Option<&Arc<QuestDbStore>>,
    summary: SessionSummaryPayload,
) {
    let Some(questdb_store) = questdb_store else {
        return;
    };

    match questdb_store.insert_session_summary(&summary).await {
        Ok(()) => {
            debug!(
                summary_id = %summary.summary_id,
                lap_count = summary.lap_count,
                metric_count = summary.metrics.len(),
                "session summary mirrored to QuestDB"
            );
        }
        Err(err) => {
            warn!(
                ?err,
                summary_id = %summary.summary_id,
                "failed to mirror session summary to QuestDB"
            );
        }
    }
}

async fn persist_practice_summary(
    questdb_store: Option<&Arc<QuestDbStore>>,
    weekend_id: Option<&str>,
    emitted_at_ms: i64,
) {
    let Some(questdb_store) = questdb_store else {
        return;
    };
    let Some(weekend_id) = weekend_id.filter(|value| !value.is_empty()) else {
        return;
    };

    match questdb_store.has_practice_summary(weekend_id).await {
        Ok(true) => {
            debug!(weekend_id = %weekend_id, "practice summary already present in QuestDB; skipping");
            return;
        }
        Ok(false) => {}
        Err(err) => {
            warn!(
                ?err,
                weekend_id = %weekend_id,
                "failed to check existing practice summary before rebuild"
            );
        }
    }

    for attempt in 1..=5 {
        match questdb_store
            .rebuild_practice_summary(weekend_id, emitted_at_ms)
            .await
        {
            Ok(Some(summary)) => {
                debug!(
                    summary_id = %summary.summary_id,
                    weekend_id = %summary.weekend_id,
                    session_count = summary.session_count,
                    metric_count = summary.metrics.len(),
                    attempt,
                    "practice summary mirrored to QuestDB"
                );
                return;
            }
            Ok(None) if attempt < 5 => {
                debug!(
                    weekend_id = %weekend_id,
                    attempt,
                    "practice summary not visible yet in QuestDB; retrying"
                );
                sleep(Duration::from_millis(250)).await;
            }
            Ok(None) => {
                warn!(
                    weekend_id = %weekend_id,
                    attempt,
                    "practice summary not emitted after retries; waiting for enough practice sessions or fresh QuestDB visibility"
                );
                return;
            }
            Err(err) => {
                warn!(
                    ?err,
                    weekend_id = %weekend_id,
                    attempt,
                    "failed to rebuild practice summary in QuestDB"
                );
                return;
            }
        }
    }
}

async fn persist_race_summary(
    questdb_store: Option<&Arc<QuestDbStore>>,
    summary: &SessionSummaryPayload,
) {
    let Some(questdb_store) = questdb_store else {
        return;
    };
    let Some(race_summary) = build_race_summary_from_session(summary) else {
        return;
    };

    match questdb_store
        .has_race_summary(&race_summary.session_id)
        .await
    {
        Ok(true) => {
            debug!(
                session_id = %race_summary.session_id,
                "race summary already present in QuestDB; skipping"
            );
            return;
        }
        Ok(false) => {}
        Err(err) => {
            warn!(
                ?err,
                session_id = %race_summary.session_id,
                "failed to check existing race summary before insert"
            );
        }
    }

    match questdb_store.insert_race_summary(&race_summary).await {
        Ok(()) => {
            debug!(
                summary_id = %race_summary.summary_id,
                session_id = %race_summary.session_id,
                metric_count = race_summary.metrics.len(),
                "race summary mirrored to QuestDB"
            );
        }
        Err(err) => {
            warn!(
                ?err,
                summary_id = %race_summary.summary_id,
                "failed to mirror race summary to QuestDB"
            );
        }
    }
}

async fn backfill_practice_summaries(questdb_store: Option<&QuestDbStore>) -> anyhow::Result<()> {
    let Some(questdb_store) = questdb_store else {
        return Ok(());
    };

    let candidate_weekend_ids = questdb_store
        .list_backfillable_practice_weekend_ids()
        .await?;
    if candidate_weekend_ids.is_empty() {
        return Ok(());
    }

    let emitted_at_ms = (OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000) as i64;
    for weekend_id in candidate_weekend_ids {
        if questdb_store.has_practice_summary(&weekend_id).await? {
            continue;
        }

        match questdb_store
            .rebuild_practice_summary(&weekend_id, emitted_at_ms)
            .await
        {
            Ok(Some(summary)) => {
                info!(
                    summary_id = %summary.summary_id,
                    weekend_id = %summary.weekend_id,
                    session_count = summary.session_count,
                    metric_count = summary.metrics.len(),
                    "backfilled practice summary from existing practice sessions"
                );
            }
            Ok(None) => {
                warn!(
                    weekend_id,
                    "skipped practice summary backfill because practice sessions were still incomplete"
                );
            }
            Err(err) => {
                warn!(weekend_id, %err, "failed to backfill practice summary");
            }
        }
    }

    Ok(())
}

async fn backfill_race_summaries(questdb_store: Option<&QuestDbStore>) -> anyhow::Result<()> {
    let Some(questdb_store) = questdb_store else {
        return Ok(());
    };

    let candidate_session_ids = questdb_store.list_backfillable_race_session_ids().await?;
    if candidate_session_ids.is_empty() {
        return Ok(());
    }

    for session_id in candidate_session_ids {
        if questdb_store.has_race_summary(&session_id).await? {
            continue;
        }

        match questdb_store.rebuild_race_summary(&session_id).await {
            Ok(Some(summary)) => {
                info!(
                    summary_id = %summary.summary_id,
                    session_id = %summary.session_id,
                    metric_count = summary.metrics.len(),
                    "backfilled race summary from existing race session"
                );
            }
            Ok(None) => {
                warn!(
                    session_id,
                    "skipped race summary backfill because race session data was incomplete"
                );
            }
            Err(err) => {
                warn!(session_id, %err, "failed to backfill race summary");
            }
        }
    }

    Ok(())
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
        let sqlite_journal_mode = std::env::var("PITGUN_GATEWAY_SQLITE_JOURNAL_MODE")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .map(|value| {
                SqliteJournalMode::from_env_value(&value).ok_or_else(|| {
                    anyhow::anyhow!(
                        "invalid PITGUN_GATEWAY_SQLITE_JOURNAL_MODE: {value} (expected wal, delete, truncate, persist, memory or off)"
                    )
                })
            })
            .transpose()?
            .unwrap_or(SqliteJournalMode::Wal);

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
        let llm_dispatch_mode = std::env::var("PITGUN_GATEWAY_LLM_DISPATCH_MODE")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .map(|value| {
                LlmDispatchMode::from_env_value(&value).ok_or_else(|| {
                    anyhow::anyhow!(
                        "invalid PITGUN_GATEWAY_LLM_DISPATCH_MODE: {value} (expected per_request, lap_end or session_end_summary)"
                    )
                })
            })
            .transpose()?
            .unwrap_or_else(|| {
                LlmDispatchMode::from_env_value(DEFAULT_LLM_DISPATCH_MODE)
                    .unwrap_or(LlmDispatchMode::PerRequest)
            });
        let questdb_url = std::env::var("PITGUN_GATEWAY_QUESTDB_URL")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let run_registry_url = std::env::var("PITGUN_GATEWAY_RUN_REGISTRY_URL")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        Ok(Self {
            bind_addr,
            allow_non_loopback,
            db_path,
            sqlite_journal_mode,
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
            llm_dispatch_mode,
            questdb_url,
            run_registry_url,
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
        llm_dispatch_mode = %config.llm_dispatch_mode.as_str(),
        "llm-core integration enabled"
    );

    Ok(Some(Arc::new(client)))
}

async fn build_questdb_store(config: &GatewayConfig) -> anyhow::Result<Option<Arc<QuestDbStore>>> {
    let Some(url) = config.questdb_url.as_ref() else {
        info!("QuestDB integration disabled (PITGUN_GATEWAY_QUESTDB_URL not set)");
        return Ok(None);
    };

    let store = QuestDbStore::new(url).await?;
    info!(questdb_url = %url, "QuestDB integration enabled");
    Ok(Some(Arc::new(store)))
}

fn build_run_registry_client(
    config: &GatewayConfig,
) -> anyhow::Result<Option<Arc<RunRegistryClient>>> {
    let Some(url) = config.run_registry_url.as_ref() else {
        info!("run registry integration disabled (PITGUN_GATEWAY_RUN_REGISTRY_URL not set)");
        return Ok(None);
    };

    let client = RunRegistryClient::new(url.clone())?;
    info!(run_registry_url = %url, "run registry integration enabled");
    Ok(Some(Arc::new(client)))
}

fn build_telemetry_points(
    envelope: &model::EventEnvelope,
    frame: &TelemetryFrame,
    extraction: &insight_ingress::InsightExtraction,
) -> Vec<TelemetryPointRow> {
    let run_id = frame.metadata.get("run_id").cloned();
    let session_type = frame.metadata.get("session_type").cloned();
    let track_id = frame.metadata.get("track_id").cloned();
    let weekend_id = envelope
        .weekend_id
        .clone()
        .or_else(|| frame.metadata.get("weekend_id").cloned());

    extraction
        .points
        .iter()
        .map(|point| TelemetryPointRow {
            player_id: envelope.player_id.clone(),
            weekend_id: weekend_id.clone(),
            session_id: envelope.session_id.clone(),
            run_id: run_id.clone(),
            session_type: session_type.clone(),
            track_id: track_id.clone(),
            source_id: point.source_id.clone(),
            frame_session_id: frame.session_id,
            frame_sequence: frame.sequence,
            timestamp_us: point.timestamp_us,
            received_at_us: frame.received_at_us,
            lap_number: frame.lap_number,
            sector: frame.sector,
            lap_distance_m: frame.lap_distance_m,
            parameter_id: point.parameter_id,
            channel: point.channel.to_string(),
            metric_key: point.metric_key.to_string(),
            unit: point.unit.to_string(),
            value: point.value,
        })
        .collect()
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

#[derive(Clone, Debug)]
struct SessionAggregationState {
    player_id: String,
    session_id: String,
    fallback_run_id: String,
    latest_metadata: HashMap<String, String>,
    max_lap: u32,
    current_lap: Option<OpenLapState>,
    session_aggregates: BTreeMap<String, MetricAggregate>,
}

impl SessionAggregationState {
    fn new(player_id: String, session_id: String, fallback_run_id: String) -> Self {
        Self {
            player_id,
            session_id,
            fallback_run_id,
            latest_metadata: HashMap::new(),
            max_lap: 0,
            current_lap: None,
            session_aggregates: BTreeMap::new(),
        }
    }

    fn ingest_frame(
        &mut self,
        frame: &TelemetryFrame,
        extraction: &insight_ingress::InsightExtraction,
        stats_plan: &insight_stats_plan::InsightStatsPlan,
    ) -> Vec<LapSummaryPayload> {
        if !frame.metadata.is_empty() {
            self.latest_metadata = frame.metadata.clone();
        }

        if extraction.points.is_empty() {
            return Vec::new();
        }

        let Some(lap_number) = frame
            .lap_number
            .map(|value| value as u32)
            .filter(|value| *value >= 1)
        else {
            return Vec::new();
        };

        self.max_lap = self.max_lap.max(lap_number);

        let mut completed = Vec::new();
        if self
            .current_lap
            .as_ref()
            .is_some_and(|open_lap| open_lap.lap_number != lap_number)
        {
            if let Some(summary) = self.finalize_open_lap(stats_plan) {
                completed.push(summary);
            }
        }

        let open_lap = self
            .current_lap
            .get_or_insert_with(|| OpenLapState::new(lap_number, frame.timestamp_us));
        open_lap.ended_at_us = frame.timestamp_us;
        if !frame.metadata.is_empty() {
            open_lap.latest_metadata = frame.metadata.clone();
        }

        for point in &extraction.points {
            accumulate_metric_point(&mut open_lap.aggregates, point);
            accumulate_metric_point(&mut self.session_aggregates, point);
        }

        completed
    }

    fn finalize_open_lap(
        &mut self,
        stats_plan: &insight_stats_plan::InsightStatsPlan,
    ) -> Option<LapSummaryPayload> {
        let open_lap = self.current_lap.take()?;
        let summary_id = format!("{}:lap:{}", self.session_id, open_lap.lap_number);
        let metadata = if open_lap.latest_metadata.is_empty() {
            &self.latest_metadata
        } else {
            &open_lap.latest_metadata
        };

        build_lap_summary(
            summary_id,
            &self.player_id,
            self.session_id.clone(),
            open_lap.lap_number,
            open_lap.started_at_us,
            open_lap.ended_at_us,
            metadata,
            &self.fallback_run_id,
            &open_lap.aggregates,
            stats_plan,
        )
    }

    fn build_session_summary_payload(
        &self,
        emitted_at_ms: i64,
        stats_plan: &insight_stats_plan::InsightStatsPlan,
    ) -> Option<SessionSummaryPayload> {
        build_session_summary(
            format!("{}:session", self.session_id),
            &self.player_id,
            self.session_id.clone(),
            emitted_at_ms,
            self.max_lap,
            &self.latest_metadata,
            &self.fallback_run_id,
            &self.session_aggregates,
            stats_plan,
        )
    }
}

#[derive(Clone, Debug)]
struct OpenLapState {
    lap_number: u32,
    started_at_us: i64,
    ended_at_us: i64,
    latest_metadata: HashMap<String, String>,
    aggregates: BTreeMap<String, MetricAggregate>,
}

impl OpenLapState {
    fn new(lap_number: u32, started_at_us: i64) -> Self {
        Self {
            lap_number,
            started_at_us,
            ended_at_us: started_at_us,
            latest_metadata: HashMap::new(),
            aggregates: BTreeMap::new(),
        }
    }
}

async fn persist_insight_request_and_dispatch(
    store: &Arc<SqliteEventStore>,
    llm_client: Option<&Arc<LlmCoreClient>>,
    request: InsightRequestPayload,
    source: &str,
) {
    match store.insert_insight_request(&request).await {
        Ok(InsightRequestInsertOutcome::Inserted) => {
            debug!(
                source = %source,
                trace_id = %request.trace_id,
                metric_count = request.metrics.len(),
                "insight request stored"
            );

            if let Some(client) = llm_client {
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
                            source = %source,
                            trace_id = %request.trace_id,
                            status = %llm_result.response.status.as_str(),
                            done_reason = ?llm_result.done_reason,
                            "insight response stored"
                        );
                    }
                    Ok(InsightResponseInsertOutcome::Duplicate) => {
                        debug!(
                            source = %source,
                            trace_id = %request.trace_id,
                            "duplicate insight response dropped"
                        );
                    }
                    Err(err) => {
                        warn!(
                            ?err,
                            source = %source,
                            trace_id = %request.trace_id,
                            "failed to persist insight response"
                        );
                    }
                }
            }
        }
        Ok(InsightRequestInsertOutcome::Duplicate) => {
            debug!(
                source = %source,
                trace_id = %request.trace_id,
                "duplicate insight request dropped"
            );
        }
        Err(err) => {
            warn!(
                ?err,
                source = %source,
                trace_id = %request.trace_id,
                "failed to persist insight request"
            );
        }
    }
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
