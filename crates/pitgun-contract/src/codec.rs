//! Telemetry Codec trait for encoding/decoding raw data.
//!
//! This module defines the [`TelemetryCodec`] trait that all protocol-specific
//! decoders must implement. Codecs transform raw bytes into [`TelemetryFrame`]
//! instances and vice versa.
//!
//! # Architecture
//!
//! ```text
//! Raw Bytes (UDP, WebSocket, etc.)
//!        │
//!        ▼
//! ┌──────────────┐
//! │ TelemetryCodec │  ← Protocol-specific implementation
//! └──────┬───────┘
//!        │
//!        ▼
//! ┌──────────────┐
//! │ TelemetryFrame │  ← Canonical format
//! └──────────────┘
//! ```
//!
//! # Implementations
//!
//! Each protocol has its own codec implementation:
//! - `F1UdpCodec` - EA F1 game UDP format
//! - `AccUdpCodec` - Assetto Corsa Competizione
//! - `IracingCodec` - iRacing telemetry
//! - `JsonCodec` - Generic JSON format
//! - `MsgPackCodec` - MessagePack binary format
//!
//! # Example
//!
//! ```rust,ignore
//! use pitgun_contract::codec::{TelemetryCodec, CodecContext};
//!
//! struct MyCodec;
//!
//! impl TelemetryCodec for MyCodec {
//!     fn name(&self) -> &str { "my-codec" }
//!     
//!     fn decode(&self, data: &[u8], ctx: &CodecContext) -> CodecResult<DecodeOutput> {
//!         // Parse bytes into TelemetryFrame
//!     }
//! }
//! ```

use serde::{Deserialize, Serialize};
use std::fmt;

use crate::frame::TelemetryFrame;
use crate::registry::ParameterRegistry;

/// Result type for codec operations.
pub type CodecResult<T> = Result<T, CodecError>;

/// Errors that can occur during encoding/decoding.
#[derive(Clone, Debug)]
pub enum CodecError {
    /// Data is too short to decode.
    InsufficientData { expected: usize, actual: usize },
    /// Invalid magic number or header.
    InvalidHeader { expected: Vec<u8>, actual: Vec<u8> },
    /// Unsupported protocol version.
    UnsupportedVersion { version: u32, supported: Vec<u32> },
    /// Invalid packet type or ID.
    InvalidPacketType(u8),
    /// Checksum or CRC mismatch.
    ChecksumMismatch { expected: u32, actual: u32 },
    /// Data corruption or invalid format.
    MalformedData(String),
    /// Required field is missing.
    MissingField(String),
    /// Field value is out of valid range.
    ValueOutOfRange {
        field: String,
        value: String,
        range: String,
    },
    /// Unknown parameter ID.
    UnknownParameter(u16),
    /// Encoding not supported by this codec.
    EncodeNotSupported,
    /// Decoding not supported by this codec.
    DecodeNotSupported,
    /// Buffer too small for encoding.
    BufferTooSmall { required: usize, available: usize },
    /// I/O error during encoding/decoding.
    IoError(String),
    /// JSON/serialization error.
    SerializationError(String),
    /// Internal codec error.
    Internal(String),
}

impl fmt::Display for CodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InsufficientData { expected, actual } => {
                write!(
                    f,
                    "insufficient data: expected {expected} bytes, got {actual}"
                )
            }
            Self::InvalidHeader { expected, actual } => {
                write!(
                    f,
                    "invalid header: expected {:?}, got {:?}",
                    expected, actual
                )
            }
            Self::UnsupportedVersion { version, supported } => {
                write!(f, "unsupported version {version}, supported: {supported:?}")
            }
            Self::InvalidPacketType(t) => write!(f, "invalid packet type: {t}"),
            Self::ChecksumMismatch { expected, actual } => {
                write!(
                    f,
                    "checksum mismatch: expected {expected:#x}, got {actual:#x}"
                )
            }
            Self::MalformedData(msg) => write!(f, "malformed data: {msg}"),
            Self::MissingField(field) => write!(f, "missing required field: {field}"),
            Self::ValueOutOfRange {
                field,
                value,
                range,
            } => {
                write!(f, "value out of range: {field}={value}, expected {range}")
            }
            Self::UnknownParameter(id) => write!(f, "unknown parameter ID: {id}"),
            Self::EncodeNotSupported => write!(f, "encoding not supported by this codec"),
            Self::DecodeNotSupported => write!(f, "decoding not supported by this codec"),
            Self::BufferTooSmall {
                required,
                available,
            } => {
                write!(
                    f,
                    "buffer too small: need {required} bytes, have {available}"
                )
            }
            Self::IoError(msg) => write!(f, "I/O error: {msg}"),
            Self::SerializationError(msg) => write!(f, "serialization error: {msg}"),
            Self::Internal(msg) => write!(f, "internal error: {msg}"),
        }
    }
}

impl std::error::Error for CodecError {}

impl From<std::io::Error> for CodecError {
    fn from(e: std::io::Error) -> Self {
        Self::IoError(e.to_string())
    }
}

impl From<serde_json::Error> for CodecError {
    fn from(e: serde_json::Error) -> Self {
        Self::SerializationError(e.to_string())
    }
}

/// Output from a decode operation.
#[derive(Clone, Debug)]
pub enum DecodeOutput {
    /// A complete telemetry frame was decoded.
    Frame(TelemetryFrame),
    /// Multiple frames were decoded from the data.
    Frames(Vec<TelemetryFrame>),
    /// Partial data - need more bytes to complete decoding.
    NeedMoreData(usize),
    /// Data was consumed but no frame produced (e.g., heartbeat packet).
    NoOutput,
    /// Unknown packet type - data was skipped.
    Skipped(usize),
}

impl DecodeOutput {
    /// Returns the frames if this output contains any.
    pub fn into_frames(self) -> Vec<TelemetryFrame> {
        match self {
            Self::Frame(f) => vec![f],
            Self::Frames(fs) => fs,
            _ => Vec::new(),
        }
    }

    /// Returns true if this output contains at least one frame.
    pub fn has_frames(&self) -> bool {
        matches!(self, Self::Frame(_) | Self::Frames(_))
    }

    /// Returns the number of frames in this output.
    pub fn frame_count(&self) -> usize {
        match self {
            Self::Frame(_) => 1,
            Self::Frames(fs) => fs.len(),
            _ => 0,
        }
    }
}

/// Context provided to codec during encoding/decoding.
#[derive(Clone, Debug, Default)]
pub struct CodecContext {
    /// Session ID for frames being decoded.
    pub session_id: u64,
    /// Source ID for frames being decoded.
    pub source_id: String,
    /// Parameter registry for ID lookups and conversions.
    pub registry: Option<ParameterRegistry>,
    /// Whether to apply parameter conversions.
    pub apply_conversions: bool,
    /// Whether to validate values against ranges.
    pub validate_ranges: bool,
    /// Whether to include unknown parameters.
    pub include_unknown: bool,
    /// Sequence number for tracking.
    pub sequence: u64,
    /// Custom options as key-value pairs.
    pub options: std::collections::HashMap<String, String>,
}

impl CodecContext {
    /// Creates a new context with session and source IDs.
    pub fn new(session_id: u64, source_id: impl Into<String>) -> Self {
        Self {
            session_id,
            source_id: source_id.into(),
            ..Default::default()
        }
    }

    /// Builder method to set the registry.
    pub fn with_registry(mut self, registry: ParameterRegistry) -> Self {
        self.registry = Some(registry);
        self
    }

    /// Builder method to enable conversions.
    pub fn with_conversions(mut self, enable: bool) -> Self {
        self.apply_conversions = enable;
        self
    }

    /// Builder method to enable validation.
    pub fn with_validation(mut self, enable: bool) -> Self {
        self.validate_ranges = enable;
        self
    }

    /// Builder method to include unknown parameters.
    pub fn with_unknown(mut self, include: bool) -> Self {
        self.include_unknown = include;
        self
    }

    /// Increments and returns the next sequence number.
    pub fn next_sequence(&mut self) -> u64 {
        let seq = self.sequence;
        self.sequence += 1;
        seq
    }
}

/// Capabilities of a codec implementation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodecCapabilities {
    /// Whether the codec supports decoding.
    pub can_decode: bool,
    /// Whether the codec supports encoding.
    pub can_encode: bool,
    /// Whether the codec supports streaming (partial data).
    pub supports_streaming: bool,
    /// Whether the codec preserves all original fields.
    pub lossless: bool,
    /// Whether the codec handles multiple packet types.
    pub multi_packet: bool,
}

impl CodecCapabilities {
    /// Creates capabilities for a decode-only codec.
    pub fn decode_only() -> Self {
        Self {
            can_decode: true,
            can_encode: false,
            ..Default::default()
        }
    }

    /// Creates capabilities for an encode-only codec.
    pub fn encode_only() -> Self {
        Self {
            can_decode: false,
            can_encode: true,
            ..Default::default()
        }
    }

    /// Creates capabilities for a bidirectional codec.
    pub fn bidirectional() -> Self {
        Self {
            can_decode: true,
            can_encode: true,
            ..Default::default()
        }
    }

    /// Builder method to enable streaming.
    pub fn with_streaming(mut self) -> Self {
        self.supports_streaming = true;
        self
    }

    /// Builder method to mark as lossless.
    pub fn with_lossless(mut self) -> Self {
        self.lossless = true;
        self
    }

    /// Builder method to enable multi-packet support.
    pub fn with_multi_packet(mut self) -> Self {
        self.multi_packet = true;
        self
    }
}

/// Protocol information for a codec.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProtocolInfo {
    /// Protocol name.
    pub name: String,
    /// Protocol version(s) supported.
    pub versions: Vec<String>,
    /// Brief description.
    pub description: Option<String>,
    /// Expected packet sizes (if fixed).
    pub packet_sizes: Vec<usize>,
    /// Magic bytes at start of packets (if any).
    pub magic_bytes: Option<Vec<u8>>,
}

impl ProtocolInfo {
    /// Creates a new protocol info.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            versions: Vec::new(),
            description: None,
            packet_sizes: Vec::new(),
            magic_bytes: None,
        }
    }

    /// Builder method to add a version.
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.versions.push(version.into());
        self
    }

    /// Builder method to set description.
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }
}

/// Trait for encoding/decoding telemetry data.
///
/// This is the core abstraction for protocol-specific codecs. Each game
/// or data source format implements this trait to convert between raw
/// bytes and canonical [`TelemetryFrame`] structures.
///
/// # Implementation Guidelines
///
/// 1. **Decode should be lenient**: Handle minor format variations gracefully
/// 2. **Encode should be strict**: Produce valid protocol-compliant output
/// 3. **Report capabilities honestly**: Set `CodecCapabilities` accurately
/// 4. **Use context wisely**: Apply conversions/validation based on context
///
/// # Thread Safety
///
/// Codecs should be `Send + Sync` for use across async tasks. If internal
/// state is needed, use appropriate synchronization.
pub trait TelemetryCodec: Send + Sync {
    /// Returns the human-readable name of this codec.
    fn name(&self) -> &str;

    /// Returns protocol information for this codec.
    fn protocol(&self) -> ProtocolInfo;

    /// Returns the capabilities of this codec.
    fn capabilities(&self) -> CodecCapabilities;

    /// Returns the minimum data size needed for decoding.
    ///
    /// Data smaller than this will always fail to decode.
    fn min_decode_size(&self) -> usize {
        1
    }

    /// Checks if the data looks like it could be decoded by this codec.
    ///
    /// This is a quick check (e.g., magic bytes, size) without full parsing.
    /// Returns true if the data might be valid for this codec.
    fn can_decode(&self, data: &[u8]) -> bool {
        data.len() >= self.min_decode_size()
    }

    /// Decodes raw bytes into telemetry data.
    ///
    /// # Arguments
    ///
    /// * `data` - Raw bytes to decode
    /// * `ctx` - Context with session info and options
    ///
    /// # Returns
    ///
    /// * `Ok(DecodeOutput)` - Decoded frame(s) or status
    /// * `Err(CodecError)` - If decoding fails
    fn decode(&self, data: &[u8], ctx: &CodecContext) -> CodecResult<DecodeOutput>;

    /// Encodes a telemetry frame into raw bytes.
    ///
    /// # Arguments
    ///
    /// * `frame` - Frame to encode
    /// * `ctx` - Context with options
    ///
    /// # Returns
    ///
    /// * `Ok(Vec<u8>)` - Encoded bytes
    /// * `Err(CodecError)` - If encoding fails or not supported
    fn encode(&self, frame: &TelemetryFrame, ctx: &CodecContext) -> CodecResult<Vec<u8>> {
        let _ = (frame, ctx);
        Err(CodecError::EncodeNotSupported)
    }

    /// Encodes a frame into an existing buffer.
    ///
    /// Returns the number of bytes written.
    fn encode_into(
        &self,
        frame: &TelemetryFrame,
        buffer: &mut [u8],
        ctx: &CodecContext,
    ) -> CodecResult<usize> {
        let encoded = self.encode(frame, ctx)?;
        if buffer.len() < encoded.len() {
            return Err(CodecError::BufferTooSmall {
                required: encoded.len(),
                available: buffer.len(),
            });
        }
        buffer[..encoded.len()].copy_from_slice(&encoded);
        Ok(encoded.len())
    }

    /// Estimates the encoded size for a frame.
    ///
    /// This is useful for pre-allocating buffers.
    fn estimate_encoded_size(&self, frame: &TelemetryFrame) -> usize {
        // Default estimate based on sample count
        64 + frame.samples.len() * 16 + frame.events.len() * 32
    }

    /// Resets any internal codec state.
    ///
    /// Called when starting a new session or after errors.
    fn reset(&mut self) {
        // Default: no state to reset
    }
}

/// A simple JSON codec implementation for testing and generic use.
#[derive(Clone, Debug, Default)]
pub struct JsonCodec {
    pretty: bool,
}

impl JsonCodec {
    /// Creates a new JSON codec.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a JSON codec that outputs pretty-printed JSON.
    pub fn pretty() -> Self {
        Self { pretty: true }
    }
}

impl TelemetryCodec for JsonCodec {
    fn name(&self) -> &str {
        "json"
    }

    fn protocol(&self) -> ProtocolInfo {
        ProtocolInfo::new("JSON")
            .with_version("1.0")
            .with_description("Generic JSON telemetry format")
    }

    fn capabilities(&self) -> CodecCapabilities {
        CodecCapabilities::bidirectional().with_lossless()
    }

    fn decode(&self, data: &[u8], _ctx: &CodecContext) -> CodecResult<DecodeOutput> {
        let frame: TelemetryFrame = serde_json::from_slice(data)?;
        Ok(DecodeOutput::Frame(frame))
    }

    fn encode(&self, frame: &TelemetryFrame, _ctx: &CodecContext) -> CodecResult<Vec<u8>> {
        let bytes = if self.pretty {
            serde_json::to_vec_pretty(frame)?
        } else {
            serde_json::to_vec(frame)?
        };
        Ok(bytes)
    }

    fn estimate_encoded_size(&self, frame: &TelemetryFrame) -> usize {
        // JSON is verbose
        256 + frame.samples.len() * 64 + frame.events.len() * 128
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::{Sample, SampleValue};

    fn sample_frame() -> TelemetryFrame {
        TelemetryFrame::builder()
            .session_id(12345)
            .timestamp_us(1700000000_000_000)
            .source_id("test-source")
            .sample(Sample::good(1, SampleValue::U16(8500)))
            .sample(Sample::good(2, SampleValue::F32(0.85)))
            .build()
    }

    #[test]
    fn json_codec_roundtrip() {
        let codec = JsonCodec::new();
        let frame = sample_frame();
        let ctx = CodecContext::new(12345, "test");

        let encoded = codec.encode(&frame, &ctx).unwrap();
        let output = codec.decode(&encoded, &ctx).unwrap();

        match output {
            DecodeOutput::Frame(decoded) => {
                assert_eq!(decoded.session_id, frame.session_id);
                assert_eq!(decoded.sample_count(), frame.sample_count());
            }
            _ => panic!("expected Frame output"),
        }
    }

    #[test]
    fn json_codec_pretty() {
        let codec = JsonCodec::pretty();
        let frame = sample_frame();
        let ctx = CodecContext::default();

        let encoded = codec.encode(&frame, &ctx).unwrap();
        let json = String::from_utf8(encoded).unwrap();
        assert!(json.contains('\n')); // Pretty print has newlines
    }

    #[test]
    fn codec_capabilities() {
        let caps = CodecCapabilities::decode_only();
        assert!(caps.can_decode);
        assert!(!caps.can_encode);

        let caps = CodecCapabilities::bidirectional()
            .with_streaming()
            .with_lossless();
        assert!(caps.can_decode);
        assert!(caps.can_encode);
        assert!(caps.supports_streaming);
        assert!(caps.lossless);
    }

    #[test]
    fn decode_output_frames() {
        let frame = sample_frame();

        let output = DecodeOutput::Frame(frame.clone());
        assert!(output.has_frames());
        assert_eq!(output.frame_count(), 1);

        let frames = vec![frame.clone(), frame.clone()];
        let output = DecodeOutput::Frames(frames);
        assert_eq!(output.frame_count(), 2);
        assert_eq!(output.into_frames().len(), 2);

        let output = DecodeOutput::NoOutput;
        assert!(!output.has_frames());
        assert_eq!(output.frame_count(), 0);
    }

    #[test]
    fn codec_context_builder() {
        let registry = ParameterRegistry::new();
        let ctx = CodecContext::new(1, "source-1")
            .with_registry(registry)
            .with_conversions(true)
            .with_validation(true);

        assert_eq!(ctx.session_id, 1);
        assert_eq!(ctx.source_id, "source-1");
        assert!(ctx.registry.is_some());
        assert!(ctx.apply_conversions);
        assert!(ctx.validate_ranges);
    }

    #[test]
    fn codec_context_sequence() {
        let mut ctx = CodecContext::default();
        assert_eq!(ctx.next_sequence(), 0);
        assert_eq!(ctx.next_sequence(), 1);
        assert_eq!(ctx.next_sequence(), 2);
    }

    #[test]
    fn codec_error_display() {
        let err = CodecError::InsufficientData {
            expected: 100,
            actual: 50,
        };
        assert!(err.to_string().contains("100"));
        assert!(err.to_string().contains("50"));

        let err = CodecError::InvalidPacketType(42);
        assert!(err.to_string().contains("42"));
    }

    #[test]
    fn protocol_info_builder() {
        let info = ProtocolInfo::new("F1 UDP")
            .with_version("2024")
            .with_version("2023")
            .with_description("EA F1 game UDP telemetry");

        assert_eq!(info.name, "F1 UDP");
        assert_eq!(info.versions.len(), 2);
        assert!(info.description.is_some());
    }

    #[test]
    fn json_codec_min_size() {
        let codec = JsonCodec::new();
        assert!(codec.min_decode_size() >= 1);
        assert!(codec.can_decode(b"{}"));
        assert!(!codec.can_decode(b""));
    }

    #[test]
    fn encode_into_buffer() {
        let codec = JsonCodec::new();
        let frame = sample_frame();
        let ctx = CodecContext::default();

        let mut buffer = vec![0u8; 4096];
        let written = codec.encode_into(&frame, &mut buffer, &ctx).unwrap();
        assert!(written > 0);
        assert!(written < buffer.len());

        // Too small buffer
        let mut small_buffer = vec![0u8; 10];
        let result = codec.encode_into(&frame, &mut small_buffer, &ctx);
        assert!(matches!(result, Err(CodecError::BufferTooSmall { .. })));
    }
}
