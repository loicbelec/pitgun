//! Pitgun WebSocket Telemetry Source
//!
//! This crate provides WebSocket telemetry sources for the Pitgun framework.
//!
//! # Source Implementations
//!
//! - **AsyncWsSource**: Async source implementing [`TelemetrySource`](pitgun_contract::TelemetrySource)
//!   with auto-reconnect and JSON decoding
//! - **WsSource**: Legacy synchronous source implementing the `Source` trait
//!
//! # Usage
//!
//! ## Async Source (Recommended)
//!
//! ```rust,ignore
//! use pitgun_source_ws::{AsyncWsSource, WsSourceConfig};
//! use pitgun_contract::TelemetrySource;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = WsSourceConfig::new("ws://localhost:8080/telemetry")
//!         .with_reconnect(true);
//!
//!     let mut source = AsyncWsSource::new(config);
//!     source.start().await?;
//!
//!     let mut rx = source.subscribe();
//!     while let Some(frame) = rx.recv().await {
//!         println!("Frame: {} samples", frame.sample_count());
//!     }
//!     Ok(())
//! }
//! ```
//!
//! ## Legacy Sync Source
//!
//! ```rust,ignore
//! use pitgun_source_ws::WsSource;
//! use pitgun_core::Source;
//!
//! let mut source = WsSource::connect("ws://localhost:8080/telemetry")?;
//! while let Some(batch) = source.next_batch() {
//!     println!("Batch: {} events", batch.events.len());
//! }
//! ```

// Async source (implements TelemetrySource)
mod async_source;
pub use async_source::{AsyncWsSource, WsSourceConfig};

// Re-export contract types for convenience
pub use pitgun_contract::{
    SourceConfig, SourceError, SourceMetadata, SourceState, SourceStats, SourceType,
    TelemetrySource,
};

// Legacy synchronous source
use pitgun_codec_json::deserialize_session_envelope;
use pitgun_core::{EventBatch, Source};
use std::collections::VecDeque;
use std::io;
use std::net::TcpStream;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket, connect};
use url::Url;

pub struct WsSource {
    socket: WebSocket<MaybeTlsStream<TcpStream>>,
    pending: VecDeque<EventBatch>,
}

impl WsSource {
    pub fn connect(url: &str) -> io::Result<Self> {
        let url =
            Url::parse(url).map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
        let (socket, _response) = connect(url).map_err(io::Error::other)?;
        Ok(Self {
            socket,
            pending: VecDeque::new(),
        })
    }
}

impl Source for WsSource {
    fn next_batch(&mut self) -> Option<EventBatch> {
        if let Some(batch) = self.pending.pop_front() {
            return Some(batch);
        }

        loop {
            match self.socket.read() {
                Ok(Message::Text(text)) => match deserialize_session_envelope(text.as_bytes()) {
                    Ok(envelope) => {
                        self.pending.push_back(envelope.batch);
                    }
                    Err(err) => {
                        eprintln!("pitgun-source-ws: invalid JSON payload over websocket: {err}");
                    }
                },
                Ok(Message::Binary(_)) => {
                    eprintln!("pitgun-source-ws: unexpected binary websocket frame");
                }
                Ok(Message::Ping(payload)) => {
                    if self.socket.send(Message::Pong(payload)).is_err() {
                        return None;
                    }
                }
                Ok(Message::Pong(_)) => {}
                Ok(Message::Frame(_)) => {}
                Ok(Message::Close(_)) => return None,
                Err(err) => {
                    eprintln!("pitgun-source-ws: websocket error: {err}");
                    return None;
                }
            }

            if let Some(batch) = self.pending.pop_front() {
                return Some(batch);
            }
        }
    }
}
