mod json;
mod processor;
mod proto;

use std::{net::SocketAddr, sync::Arc};

use axum::{
    Router,
    body::Bytes,
    extract::{
        ConnectInfo, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
};
use pitgun_core::EventBatch;
use processor::{DefaultProcessor, IngestMessage, IngestMetadata, TelemetryProcessor};
use tokio::{
    net::TcpListener,
    signal,
    sync::mpsc::{self, error::TrySendError},
};
use tracing::{error, info, warn};

const DEFAULT_BIND_ADDR: &str = "127.0.0.1:8080";
const DEFAULT_DATA_DIR: &str = "/opt/pitgun/telemetry/data";
const INGEST_QUEUE_SIZE: usize = 1024;

#[derive(Clone)]
struct AppState {
    tx: mpsc::Sender<IngestMessage>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let log_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    tracing_subscriber::fmt().with_env_filter(log_filter).init();

    let bind_addr =
        std::env::var("PITGUN_TELEMETRY_BIND").unwrap_or_else(|_| DEFAULT_BIND_ADDR.to_string());
    let data_dir =
        std::env::var("PITGUN_TELEMETRY_DATA_DIR").unwrap_or_else(|_| DEFAULT_DATA_DIR.to_string());

    let addr: SocketAddr = bind_addr
        .parse()
        .map_err(|err| anyhow::anyhow!("invalid PITGUN_TELEMETRY_BIND: {err}"))?;

    if !addr.ip().is_loopback() {
        anyhow::bail!("pitgun-telemetryd must bind to a loopback address; got {addr}");
    }

    let processor = Arc::new(DefaultProcessor::new(&data_dir).await?);
    let (tx, rx) = mpsc::channel(INGEST_QUEUE_SIZE);

    let processor_task = tokio::spawn(process_queue(processor.clone(), rx));

    let app_state = AppState { tx };

    let app = Router::new()
        .route("/health", get(health))
        .route("/beacon", post(beacon))
        .route("/ws", get(ws_handler))
        .with_state(app_state);

    let listener = TcpListener::bind(addr).await?;
    info!("pitgun-telemetryd listening on {addr}");

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;

    processor_task.await?;

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

async fn process_queue<P: TelemetryProcessor + 'static>(
    processor: Arc<P>,
    mut rx: mpsc::Receiver<IngestMessage>,
) {
    while let Some(msg) = rx.recv().await {
        if let Err(err) = processor.process(msg).await {
            error!(?err, "failed to process telemetry batch");
        }
    }

    info!("ingestion queue closed");
}

async fn health() -> StatusCode {
    StatusCode::OK
}

async fn beacon(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    match json::deserialize_session_envelope(&body) {
        Ok(envelope) => {
            let meta = build_metadata(&headers, peer);
            enqueue_envelope(&state, envelope, meta);
            StatusCode::ACCEPTED.into_response()
        }
        Err(err) => {
            warn!(?err, "failed to decode beacon payload");
            (StatusCode::BAD_REQUEST, err.to_string()).into_response()
        }
    }
}

async fn ws_handler(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    let meta = build_metadata(&headers, peer);
    ws.on_upgrade(move |socket| handle_socket(socket, state, meta))
}

async fn handle_socket(socket: WebSocket, state: AppState, meta: IngestMetadata) {
    let mut socket = socket;

    while let Some(message) = socket.recv().await {
        match message {
            Ok(Message::Text(text)) => match json::deserialize_session_envelope(text.as_bytes()) {
                Ok(envelope) => enqueue_envelope(&state, envelope, meta.clone()),
                Err(err) => warn!(?err, "invalid JSON payload over websocket"),
            },
            Ok(Message::Binary(bytes)) => match proto::decode_event_batch(&bytes) {
                Ok(batch) => enqueue_protobuf_batch(&state, batch, meta.clone()),
                Err(err) => warn!(?err, "invalid protobuf payload over websocket"),
            },
            Ok(Message::Ping(payload)) => {
                if socket.send(Message::Pong(payload)).await.is_err() {
                    break;
                }
            }
            Ok(Message::Pong(_)) => {}
            Ok(Message::Close(_)) => break,
            Err(err) => {
                warn!(?err, "websocket error");
                break;
            }
        }
    }

    let _ = socket.close().await;
    info!("websocket connection closed");
}

fn enqueue_envelope(state: &AppState, envelope: json::SessionEnvelopeIn, meta: IngestMetadata) {
    debug_assert_eq!(
        envelope.schema_version,
        json::SESSION_ENVELOPE_SCHEMA_VERSION
    );

    let msg = IngestMessage {
        session_id: Some(envelope.session_id),
        sent_at_ms: envelope.sent_at_ms,
        batch: envelope.batch,
        meta,
    };

    enqueue_message(state, msg);
}

fn enqueue_protobuf_batch(state: &AppState, batch: EventBatch, meta: IngestMetadata) {
    let msg = IngestMessage {
        session_id: None,
        sent_at_ms: None,
        batch,
        meta,
    };

    enqueue_message(state, msg);
}

fn enqueue_message(state: &AppState, msg: IngestMessage) {
    match state.tx.try_send(msg) {
        Ok(_) => {}
        Err(TrySendError::Full(_)) => {
            warn!("ingestion queue full; dropping batch");
        }
        Err(TrySendError::Closed(_)) => {
            error!("ingestion queue closed; dropping batch");
        }
    }
}

fn build_metadata(headers: &HeaderMap, peer: SocketAddr) -> IngestMetadata {
    let user_agent = headers
        .get(axum::http::header::USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .map(|ua| ua.to_string());

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
                .map(|s| s.to_string())
        })
        .or_else(|| Some(peer.ip().to_string()));

    IngestMetadata {
        remote_ip,
        user_agent,
    }
}
