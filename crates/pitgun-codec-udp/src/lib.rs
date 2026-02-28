//! Pitgun UDP Codecs
//!
//! This crate provides UDP telemetry codecs for various protocols:
//!
//! - **ECUBridge**: McLaren-inspired binary protocol for high-throughput telemetry
//! - **F1 UDP**: F1 telemetry format (real F1 protocol)
//! - **PitgunV1**: Legacy simple telemetry format
//!
//! # Protocol Support
//!
//! All codecs implement the [`TelemetryCodec`](pitgun_contract::TelemetryCodec) trait
//! from `pitgun-contract`, providing a unified interface for encoding and decoding.
//!
//! # Example
//!
//! ```rust,ignore
//! use pitgun_codec_udp::{EcuBridgeCodec, F1UdpCodec};
//! use pitgun_contract::{TelemetryCodec, CodecContext};
//!
//! // Decode ECUBridge packets
//! let ecu_codec = EcuBridgeCodec::new();
//! let ctx = CodecContext::new(1, "ecu-source");
//! let frame = ecu_codec.decode(&packet_data, &ctx)?;
//!
//! // Decode F1 game packets
//! let f1_codec = F1UdpCodec::new();
//! let frame = f1_codec.decode(&packet_data, &ctx)?;
//! ```

// Protocol-specific codecs
mod ecubridge;
mod f1;

// Re-export codec implementations
pub use ecubridge::{
    DataTypeCode, ECUBRIDGE_MAGIC, ECUBRIDGE_MIN_SIZE, ECUBRIDGE_PACKET_SIZE, EcuBridgeCodec,
    EcuBridgePacketBuilder, PacketFlags, PacketHeader,
};
pub use f1::{
    F1_HEADER_SIZE, F1_MIN_PACKET_SIZE, F1Header, F1PacketType, F1UdpCodec, param_ids as f1_params,
};

// Legacy codec support
use pitgun_core::{Event, EventBatch};
use std::io;

const PITGUN_V1_MIN_LEN: usize = 2 + 16 + 8;
pub const UDP_PITGUN_V1_WIRE_ID: &str = "udp/pitgun-v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum UdpWireFormat {
    #[default]
    PitgunV1,
}

#[derive(Clone, Debug)]
pub enum UdpDecoded {
    Events(Vec<Event>),
    Batches(Vec<EventBatch>),
}

pub trait UdpDecoder: Send + Sync {
    fn min_datagram_len(&self) -> usize {
        0
    }

    fn decode(&self, datagram: &[u8]) -> io::Result<UdpDecoded>;
}

impl UdpDecoder for UdpWireFormat {
    fn min_datagram_len(&self) -> usize {
        match self {
            UdpWireFormat::PitgunV1 => PITGUN_V1_MIN_LEN,
        }
    }

    fn decode(&self, datagram: &[u8]) -> io::Result<UdpDecoded> {
        match self {
            UdpWireFormat::PitgunV1 => {
                decode_pitgun_v1(datagram).map(|event| UdpDecoded::Events(vec![event]))
            }
        }
    }
}

pub fn encode_pitgun_v1(channel: &str, ts_ns: u128, value: f64) -> Vec<u8> {
    let name = channel.as_bytes();
    let mut buf = Vec::with_capacity(2 + name.len() + 16 + 8);
    let len = u16::try_from(name.len()).unwrap_or(u16::MAX);
    buf.extend_from_slice(&len.to_le_bytes());
    buf.extend_from_slice(name);
    buf.extend_from_slice(&ts_ns.to_le_bytes());
    buf.extend_from_slice(&value.to_le_bytes());
    buf
}

fn decode_pitgun_v1(mut bytes: &[u8]) -> io::Result<Event> {
    use std::convert::TryInto;

    if bytes.len() < 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "short frame (len)",
        ));
    }
    let len = u16::from_le_bytes(bytes[0..2].try_into().unwrap()) as usize;
    bytes = &bytes[2..];
    if bytes.len() < len {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "short frame (channel)",
        ));
    }
    let channel = std::str::from_utf8(&bytes[..len])
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid channel utf8"))?
        .to_string();
    bytes = &bytes[len..];
    if bytes.len() < 16 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "short frame (ts)",
        ));
    }
    let mut t = [0u8; 16];
    t.copy_from_slice(&bytes[..16]);
    let ts_raw = u128::from_le_bytes(t);
    bytes = &bytes[16..];
    if bytes.len() < 8 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "short frame (value)",
        ));
    }
    let mut v = [0u8; 8];
    v.copy_from_slice(&bytes[..8]);
    let value = f64::from_le_bytes(v);

    let ts_ns = if ts_raw > u128::from(u64::MAX) {
        u64::MAX
    } else {
        ts_raw as u64
    };

    Ok(Event {
        channel,
        ts_ns,
        value,
    })
}
