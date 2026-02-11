//! Pitgun UDP Telemetry Source
//!
//! This crate provides UDP telemetry sources for the Pitgun framework.
//!
//! # Source Implementations
//!
//! - **AsyncUdpSource**: Async source implementing [`TelemetrySource`](pitgun_contract::TelemetrySource)
//!   with multi-codec support (ECUBridge, F1, PitgunV1)
//! - **UdpSource**: Legacy synchronous source implementing the `Source` trait
//!
//! # Usage
//!
//! ## Async Source (Recommended)
//!
//! ```rust,ignore
//! use pitgun_source_udp::{AsyncUdpSource, UdpSourceConfig, UdpCodecType};
//! use pitgun_contract::TelemetrySource;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = UdpSourceConfig::parse("0.0.0.0:20777")?
//!         .with_codec(UdpCodecType::F1);
//!
//!     let mut source = AsyncUdpSource::new(config);
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
//! use pitgun_source_udp::{UdpSource, UdpWireFormat};
//! use pitgun_core::Source;
//!
//! let source = UdpSource::new(
//!     "0.0.0.0:9999".parse()?,
//!     None, // no multicast
//!     "0.0.0.0".parse()?,
//!     100,  // batch size
//!     1_000_000, // batch timeout ns
//!     UdpWireFormat::PitgunV1,
//! )?;
//! ```

// Async source (implements TelemetrySource)
mod async_source;
pub use async_source::{AsyncUdpSource, UdpCodecType, UdpSourceConfig};

// Re-export contract types for convenience
pub use pitgun_contract::{
    SourceConfig, SourceError, SourceMetadata, SourceState, SourceStats, SourceType,
    TelemetrySource,
};

// Legacy synchronous source
use pitgun_codec_udp::{UdpDecoded, UdpDecoder};
use pitgun_core::{Event, EventBatch, Source};
use std::collections::VecDeque;
use std::io;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::time::Instant;

pub struct UdpSource<D> {
    socket: UdpSocket,
    buf: Vec<u8>,
    pending_events: Vec<Event>,
    pending_batches: VecDeque<EventBatch>,
    batch_max_len: usize,
    batch_max_ns: u64,
    last_flush: Instant,
    decoder: D,
}

impl<D> UdpSource<D>
where
    D: UdpDecoder,
{
    pub fn new(
        bind: SocketAddr,
        mcast: Option<Ipv4Addr>,
        iface: Ipv4Addr,
        batch_max_len: usize,
        batch_max_ns: u64,
        decoder: D,
    ) -> io::Result<Self> {
        let socket = UdpSocket::bind(bind)?;
        socket.set_nonblocking(false)?;
        if let Some(group) = mcast {
            socket.join_multicast_v4(&group, &iface)?;
        }
        Ok(Self {
            socket,
            buf: vec![0u8; 64 * 1024],
            pending_events: Vec::with_capacity(256),
            pending_batches: VecDeque::new(),
            batch_max_len,
            batch_max_ns,
            last_flush: Instant::now(),
            decoder,
        })
    }

    fn should_flush(&self) -> bool {
        let len_due = self.batch_max_len > 0 && self.pending_events.len() >= self.batch_max_len;
        let time_due = self.batch_max_ns > 0
            && self.last_flush.elapsed().as_nanos() as u64 >= self.batch_max_ns;
        len_due || time_due
    }

    fn flush_events(&mut self) -> Option<EventBatch> {
        if self.pending_events.is_empty() {
            return None;
        }
        self.last_flush = Instant::now();
        let events = std::mem::take(&mut self.pending_events);
        Some(EventBatch {
            events,
            aggregates: Vec::new(),
            end_of_stream: false,
        })
    }
}

impl<D> Source for UdpSource<D>
where
    D: UdpDecoder,
{
    fn next_batch(&mut self) -> Option<EventBatch> {
        if let Some(batch) = self.pending_batches.pop_front() {
            return Some(batch);
        }

        loop {
            match self.socket.recv(&mut self.buf) {
                Ok(n) => {
                    if n < self.decoder.min_datagram_len() {
                        continue;
                    }

                    match self.decoder.decode(&self.buf[..n]) {
                        Ok(UdpDecoded::Events(mut events)) => {
                            self.pending_events.append(&mut events);
                        }
                        Ok(UdpDecoded::Batches(batches)) => {
                            self.pending_batches.extend(batches);
                        }
                        Err(err) => {
                            eprintln!("pitgun-core: failed to decode frame: {err}");
                            continue;
                        }
                    }

                    if let Some(batch) = self.pending_batches.pop_front() {
                        return Some(batch);
                    }

                    if self.should_flush()
                        && let Some(batch) = self.flush_events()
                    {
                        return Some(batch);
                    }
                }
                Err(err) => {
                    eprintln!("pitgun-core: UDP receive error: {err}");
                    return None;
                }
            }
        }
    }
}
