//! ECUBridge-style UDP codec for high-throughput telemetry.
//!
//! This module implements a binary codec inspired by McLaren ECUBridge's
//! 8193-byte UDP packet format. It's designed for:
//! - High sample density (hundreds of parameters per packet)
//! - Low latency decoding
//! - Multicast distribution
//!
//! # Packet Format
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │ Header (24 bytes)                                           │
//! ├─────────────────────────────────────────────────────────────┤
//! │ Magic: 0x45435542 "ECUB" (4 bytes)                          │
//! │ Version: u16 (2 bytes)                                      │
//! │ Flags: u16 (2 bytes)                                        │
//! │ Session ID: u64 (8 bytes)                                   │
//! │ Timestamp: i64 microseconds (8 bytes)                       │
//! ├─────────────────────────────────────────────────────────────┤
//! │ Sample Header (4 bytes)                                     │
//! ├─────────────────────────────────────────────────────────────┤
//! │ Sample Count: u16 (2 bytes)                                 │
//! │ Event Count: u16 (2 bytes)                                  │
//! ├─────────────────────────────────────────────────────────────┤
//! │ Samples (variable)                                          │
//! ├─────────────────────────────────────────────────────────────┤
//! │ Parameter ID: u16 (2 bytes)                                 │
//! │ Quality + Type: u8 (2 bits quality, 6 bits type)            │
//! │ Value: 1-8 bytes depending on type                          │
//! ├─────────────────────────────────────────────────────────────┤
//! │ Events (variable)                                           │
//! ├─────────────────────────────────────────────────────────────┤
//! │ Event ID: u16 (2 bytes)                                     │
//! │ Severity: u8 (1 byte)                                       │
//! │ Data Length: u8 (1 byte)                                    │
//! │ Data: 0-255 bytes                                           │
//! ├─────────────────────────────────────────────────────────────┤
//! │ Padding (to 8193 bytes)                                     │
//! │ CRC32: u32 (last 4 bytes)                                   │
//! └─────────────────────────────────────────────────────────────┘
//! ```

use pitgun_contract::{
    CodecCapabilities, CodecContext, CodecError, CodecResult, DecodeOutput, Event, EventSeverity,
    ParameterId, ProtocolInfo, Sample, SampleValue, SessionId, SignalQuality, TelemetryCodec,
    TelemetryFrame, TelemetryFrameBuilder,
};
use std::io::{Cursor, Read};

/// Magic bytes for ECUBridge packets: "ECUB" = 0x45435542
pub const ECUBRIDGE_MAGIC: u32 = 0x45435542;

/// Standard ECUBridge packet size
pub const ECUBRIDGE_PACKET_SIZE: usize = 8193;

/// Minimum valid packet size (header + sample header + CRC)
pub const ECUBRIDGE_MIN_SIZE: usize = 24 + 4 + 4;

/// Current protocol version
pub const ECUBRIDGE_VERSION: u16 = 1;

/// Header size in bytes
pub const HEADER_SIZE: usize = 24;

/// Sample header size
pub const SAMPLE_HEADER_SIZE: usize = 4;

/// Data type codes
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataTypeCode {
    Bool = 0,
    U8 = 1,
    U16 = 2,
    U32 = 3,
    U64 = 4,
    I8 = 5,
    I16 = 6,
    I32 = 7,
    I64 = 8,
    F32 = 9,
    F64 = 10,
    Bytes4 = 11,  // Fixed 4 bytes
    Bytes8 = 12,  // Fixed 8 bytes
    Bytes16 = 13, // Fixed 16 bytes
}

impl DataTypeCode {
    /// Returns the size in bytes for this type
    pub fn size_bytes(&self) -> usize {
        match self {
            Self::Bool | Self::U8 | Self::I8 => 1,
            Self::U16 | Self::I16 => 2,
            Self::U32 | Self::I32 | Self::F32 | Self::Bytes4 => 4,
            Self::U64 | Self::I64 | Self::F64 | Self::Bytes8 => 8,
            Self::Bytes16 => 16,
        }
    }

    /// Converts from raw byte
    pub fn from_u8(v: u8) -> Option<Self> {
        match v & 0x3F {
            // Lower 6 bits
            0 => Some(Self::Bool),
            1 => Some(Self::U8),
            2 => Some(Self::U16),
            3 => Some(Self::U32),
            4 => Some(Self::U64),
            5 => Some(Self::I8),
            6 => Some(Self::I16),
            7 => Some(Self::I32),
            8 => Some(Self::I64),
            9 => Some(Self::F32),
            10 => Some(Self::F64),
            11 => Some(Self::Bytes4),
            12 => Some(Self::Bytes8),
            13 => Some(Self::Bytes16),
            _ => None,
        }
    }
}

/// Packet flags
#[derive(Clone, Copy, Debug, Default)]
pub struct PacketFlags {
    /// Packet contains compressed data
    pub compressed: bool,
    /// Packet is a heartbeat (no samples)
    pub heartbeat: bool,
    /// Packet contains events
    pub has_events: bool,
    /// Packet is encrypted
    pub encrypted: bool,
}

impl PacketFlags {
    /// Decodes flags from u16
    pub fn from_u16(v: u16) -> Self {
        Self {
            compressed: (v & 0x0001) != 0,
            heartbeat: (v & 0x0002) != 0,
            has_events: (v & 0x0004) != 0,
            encrypted: (v & 0x0008) != 0,
        }
    }

    /// Encodes flags to u16
    pub fn to_u16(&self) -> u16 {
        let mut v = 0u16;
        if self.compressed {
            v |= 0x0001;
        }
        if self.heartbeat {
            v |= 0x0002;
        }
        if self.has_events {
            v |= 0x0004;
        }
        if self.encrypted {
            v |= 0x0008;
        }
        v
    }
}

/// Decoded packet header
#[derive(Clone, Debug)]
pub struct PacketHeader {
    pub version: u16,
    pub flags: PacketFlags,
    pub session_id: SessionId,
    pub timestamp_us: i64,
    pub sample_count: u16,
    pub event_count: u16,
}

/// ECUBridge codec implementation
#[derive(Clone, Debug, Default)]
pub struct EcuBridgeCodec {
    /// Whether to validate CRC
    pub verify_crc: bool,
    /// Whether to use standard 8193-byte packets for encoding
    pub use_standard_size: bool,
}

impl EcuBridgeCodec {
    /// Creates a new codec with default settings
    pub fn new() -> Self {
        Self {
            verify_crc: true,
            use_standard_size: true,
        }
    }

    /// Creates a codec that skips CRC verification
    pub fn without_crc() -> Self {
        Self {
            verify_crc: false,
            use_standard_size: true,
        }
    }

    /// Decodes the packet header
    fn decode_header(&self, data: &[u8]) -> CodecResult<PacketHeader> {
        if data.len() < HEADER_SIZE + SAMPLE_HEADER_SIZE {
            return Err(CodecError::InsufficientData {
                expected: HEADER_SIZE + SAMPLE_HEADER_SIZE,
                actual: data.len(),
            });
        }

        // Check magic
        let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        if magic != ECUBRIDGE_MAGIC {
            return Err(CodecError::InvalidHeader {
                expected: ECUBRIDGE_MAGIC.to_le_bytes().to_vec(),
                actual: data[0..4].to_vec(),
            });
        }

        let version = u16::from_le_bytes([data[4], data[5]]);
        let flags = PacketFlags::from_u16(u16::from_le_bytes([data[6], data[7]]));
        let session_id = u64::from_le_bytes([
            data[8], data[9], data[10], data[11], data[12], data[13], data[14], data[15],
        ]);
        let timestamp_us = i64::from_le_bytes([
            data[16], data[17], data[18], data[19], data[20], data[21], data[22], data[23],
        ]);

        // Sample header
        let sample_count = u16::from_le_bytes([data[24], data[25]]);
        let event_count = u16::from_le_bytes([data[26], data[27]]);

        Ok(PacketHeader {
            version,
            flags,
            session_id,
            timestamp_us,
            sample_count,
            event_count,
        })
    }

    /// Decodes a single sample from the buffer
    fn decode_sample(&self, cursor: &mut Cursor<&[u8]>) -> CodecResult<Sample> {
        let mut buf = [0u8; 2];
        cursor
            .read_exact(&mut buf)
            .map_err(|_| CodecError::MalformedData("failed to read parameter ID".into()))?;
        let parameter_id: ParameterId = u16::from_le_bytes(buf);

        let mut type_byte = [0u8; 1];
        cursor
            .read_exact(&mut type_byte)
            .map_err(|_| CodecError::MalformedData("failed to read type byte".into()))?;

        // Upper 2 bits = quality, lower 6 bits = type
        let quality_bits = (type_byte[0] >> 6) & 0x03;
        let quality = match quality_bits {
            0 => SignalQuality::Good,
            1 => SignalQuality::Degraded,
            2 => SignalQuality::Bad,
            3 => SignalQuality::NoSignal,
            _ => SignalQuality::Unknown,
        };

        let data_type =
            DataTypeCode::from_u8(type_byte[0]).ok_or(CodecError::MalformedData(format!(
                "invalid data type: {}",
                type_byte[0] & 0x3F
            )))?;

        let value = self.read_value(cursor, data_type)?;

        Ok(Sample::new(parameter_id, value, quality))
    }

    /// Reads a value of the given type from the cursor
    fn read_value(&self, cursor: &mut Cursor<&[u8]>, data_type: DataTypeCode) -> CodecResult<SampleValue> {
        let mut buf1 = [0u8; 1];
        let mut buf2 = [0u8; 2];
        let mut buf4 = [0u8; 4];
        let mut buf8 = [0u8; 8];
        let mut buf16 = [0u8; 16];

        let read_err = |_| CodecError::MalformedData("failed to read value".into());

        match data_type {
            DataTypeCode::Bool => {
                cursor.read_exact(&mut buf1).map_err(read_err)?;
                Ok(SampleValue::Bool(buf1[0] != 0))
            }
            DataTypeCode::U8 => {
                cursor.read_exact(&mut buf1).map_err(read_err)?;
                Ok(SampleValue::U8(buf1[0]))
            }
            DataTypeCode::U16 => {
                cursor.read_exact(&mut buf2).map_err(read_err)?;
                Ok(SampleValue::U16(u16::from_le_bytes(buf2)))
            }
            DataTypeCode::U32 => {
                cursor.read_exact(&mut buf4).map_err(read_err)?;
                Ok(SampleValue::U32(u32::from_le_bytes(buf4)))
            }
            DataTypeCode::U64 => {
                cursor.read_exact(&mut buf8).map_err(read_err)?;
                Ok(SampleValue::U64(u64::from_le_bytes(buf8)))
            }
            DataTypeCode::I8 => {
                cursor.read_exact(&mut buf1).map_err(read_err)?;
                Ok(SampleValue::I8(buf1[0] as i8))
            }
            DataTypeCode::I16 => {
                cursor.read_exact(&mut buf2).map_err(read_err)?;
                Ok(SampleValue::I16(i16::from_le_bytes(buf2)))
            }
            DataTypeCode::I32 => {
                cursor.read_exact(&mut buf4).map_err(read_err)?;
                Ok(SampleValue::I32(i32::from_le_bytes(buf4)))
            }
            DataTypeCode::I64 => {
                cursor.read_exact(&mut buf8).map_err(read_err)?;
                Ok(SampleValue::I64(i64::from_le_bytes(buf8)))
            }
            DataTypeCode::F32 => {
                cursor.read_exact(&mut buf4).map_err(read_err)?;
                Ok(SampleValue::F32(f32::from_le_bytes(buf4)))
            }
            DataTypeCode::F64 => {
                cursor.read_exact(&mut buf8).map_err(read_err)?;
                Ok(SampleValue::F64(f64::from_le_bytes(buf8)))
            }
            DataTypeCode::Bytes4 => {
                cursor.read_exact(&mut buf4).map_err(read_err)?;
                Ok(SampleValue::Bytes(buf4.to_vec()))
            }
            DataTypeCode::Bytes8 => {
                cursor.read_exact(&mut buf8).map_err(read_err)?;
                Ok(SampleValue::Bytes(buf8.to_vec()))
            }
            DataTypeCode::Bytes16 => {
                cursor.read_exact(&mut buf16).map_err(read_err)?;
                Ok(SampleValue::Bytes(buf16.to_vec()))
            }
        }
    }

    /// Decodes an event from the buffer
    fn decode_event(&self, cursor: &mut Cursor<&[u8]>) -> CodecResult<Event> {
        let mut buf2 = [0u8; 2];
        let mut buf1 = [0u8; 1];

        cursor
            .read_exact(&mut buf2)
            .map_err(|_| CodecError::MalformedData("failed to read event ID".into()))?;
        let event_id = u16::from_le_bytes(buf2);

        cursor
            .read_exact(&mut buf1)
            .map_err(|_| CodecError::MalformedData("failed to read severity".into()))?;
        let severity = match buf1[0] {
            0 => EventSeverity::Trace,
            1 => EventSeverity::Debug,
            2 => EventSeverity::Info,
            3 => EventSeverity::Warning,
            4 => EventSeverity::Error,
            5 => EventSeverity::Critical,
            _ => EventSeverity::Info,
        };

        cursor
            .read_exact(&mut buf1)
            .map_err(|_| CodecError::MalformedData("failed to read data length".into()))?;
        let data_len = buf1[0] as usize;

        // Skip event data for now (we could parse it based on event type)
        let mut _data = vec![0u8; data_len];
        if data_len > 0 {
            cursor
                .read_exact(&mut _data)
                .map_err(|_| CodecError::MalformedData("failed to read event data".into()))?;
        }

        Ok(Event::new(event_id, format!("event_{}", event_id), severity))
    }

    /// Calculates CRC32 of the packet data
    fn calculate_crc(&self, data: &[u8]) -> u32 {
        // Simple CRC32 implementation
        let mut crc = 0xFFFFFFFFu32;
        for byte in data {
            crc ^= *byte as u32;
            for _ in 0..8 {
                if crc & 1 != 0 {
                    crc = (crc >> 1) ^ 0xEDB88320;
                } else {
                    crc >>= 1;
                }
            }
        }
        !crc
    }

    /// Encodes a sample to bytes
    fn encode_sample(&self, sample: &Sample, buf: &mut Vec<u8>) {
        // Parameter ID
        buf.extend_from_slice(&(sample.parameter_id).to_le_bytes());

        // Quality (2 bits) + Type (6 bits)
        let quality_bits = match sample.quality {
            SignalQuality::Good => 0,
            SignalQuality::Degraded => 1,
            SignalQuality::Bad => 2,
            SignalQuality::NoSignal => 3,
            SignalQuality::Unknown => 0,
        };

        let (type_code, value_bytes) = match &sample.value {
            SampleValue::Bool(v) => (DataTypeCode::Bool as u8, vec![if *v { 1 } else { 0 }]),
            SampleValue::U8(v) => (DataTypeCode::U8 as u8, vec![*v]),
            SampleValue::U16(v) => (DataTypeCode::U16 as u8, v.to_le_bytes().to_vec()),
            SampleValue::U32(v) => (DataTypeCode::U32 as u8, v.to_le_bytes().to_vec()),
            SampleValue::U64(v) => (DataTypeCode::U64 as u8, v.to_le_bytes().to_vec()),
            SampleValue::I8(v) => (DataTypeCode::I8 as u8, vec![*v as u8]),
            SampleValue::I16(v) => (DataTypeCode::I16 as u8, v.to_le_bytes().to_vec()),
            SampleValue::I32(v) => (DataTypeCode::I32 as u8, v.to_le_bytes().to_vec()),
            SampleValue::I64(v) => (DataTypeCode::I64 as u8, v.to_le_bytes().to_vec()),
            SampleValue::F32(v) => (DataTypeCode::F32 as u8, v.to_le_bytes().to_vec()),
            SampleValue::F64(v) => (DataTypeCode::F64 as u8, v.to_le_bytes().to_vec()),
            SampleValue::Bytes(v) if v.len() <= 4 => {
                let mut bytes = v.clone();
                bytes.resize(4, 0);
                (DataTypeCode::Bytes4 as u8, bytes)
            }
            SampleValue::Bytes(v) if v.len() <= 8 => {
                let mut bytes = v.clone();
                bytes.resize(8, 0);
                (DataTypeCode::Bytes8 as u8, bytes)
            }
            SampleValue::Bytes(v) => {
                let mut bytes = v.clone();
                bytes.resize(16, 0);
                (DataTypeCode::Bytes16 as u8, bytes)
            }
            SampleValue::String(s) => {
                let mut bytes = s.as_bytes().to_vec();
                bytes.resize(16, 0);
                (DataTypeCode::Bytes16 as u8, bytes)
            }
        };

        buf.push((quality_bits << 6) | type_code);
        buf.extend_from_slice(&value_bytes);
    }
}

impl TelemetryCodec for EcuBridgeCodec {
    fn name(&self) -> &str {
        "ecubridge"
    }

    fn protocol(&self) -> ProtocolInfo {
        ProtocolInfo::new("ECUBridge UDP")
            .with_version("1.0")
            .with_description("McLaren-inspired binary telemetry protocol")
    }

    fn capabilities(&self) -> CodecCapabilities {
        CodecCapabilities::bidirectional()
            .with_streaming()
            .with_multi_packet()
    }

    fn min_decode_size(&self) -> usize {
        ECUBRIDGE_MIN_SIZE
    }

    fn can_decode(&self, data: &[u8]) -> bool {
        if data.len() < 4 {
            return false;
        }
        let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        magic == ECUBRIDGE_MAGIC
    }

    fn decode(&self, data: &[u8], ctx: &CodecContext) -> CodecResult<DecodeOutput> {
        let header = self.decode_header(data)?;

        // Verify CRC if enabled
        if self.verify_crc && data.len() >= 4 {
            let crc_offset = data.len() - 4;
            let stored_crc = u32::from_le_bytes([
                data[crc_offset],
                data[crc_offset + 1],
                data[crc_offset + 2],
                data[crc_offset + 3],
            ]);
            let calculated_crc = self.calculate_crc(&data[..crc_offset]);
            if stored_crc != calculated_crc {
                return Err(CodecError::ChecksumMismatch {
                    expected: stored_crc,
                    actual: calculated_crc,
                });
            }
        }

        // Handle heartbeat packets
        if header.flags.heartbeat {
            return Ok(DecodeOutput::NoOutput);
        }

        // Decode samples
        let mut cursor = Cursor::new(&data[HEADER_SIZE + SAMPLE_HEADER_SIZE..]);
        let mut samples = Vec::with_capacity(header.sample_count as usize);

        for _ in 0..header.sample_count {
            match self.decode_sample(&mut cursor) {
                Ok(sample) => samples.push(sample),
                Err(e) => {
                    // Log error but continue with partial decode
                    eprintln!("ECUBridge: sample decode error: {e}");
                    break;
                }
            }
        }

        // Decode events
        let mut events = Vec::with_capacity(header.event_count as usize);
        if header.flags.has_events {
            for _ in 0..header.event_count {
                match self.decode_event(&mut cursor) {
                    Ok(event) => events.push(event),
                    Err(e) => {
                        eprintln!("ECUBridge: event decode error: {e}");
                        break;
                    }
                }
            }
        }

        let frame = TelemetryFrameBuilder::new()
            .session_id(header.session_id)
            .timestamp_us(header.timestamp_us)
            .source_id(&ctx.source_id)
            .samples(samples)
            .events(events)
            .build();

        Ok(DecodeOutput::Frame(frame))
    }

    fn encode(&self, frame: &TelemetryFrame, _ctx: &CodecContext) -> CodecResult<Vec<u8>> {
        let mut buf = Vec::with_capacity(ECUBRIDGE_PACKET_SIZE);

        // Header
        buf.extend_from_slice(&ECUBRIDGE_MAGIC.to_le_bytes());
        buf.extend_from_slice(&ECUBRIDGE_VERSION.to_le_bytes());

        let flags = PacketFlags {
            has_events: !frame.events.is_empty(),
            ..Default::default()
        };
        buf.extend_from_slice(&flags.to_u16().to_le_bytes());
        buf.extend_from_slice(&frame.session_id.to_le_bytes());
        buf.extend_from_slice(&frame.timestamp_us.to_le_bytes());

        // Sample header
        let sample_count = frame.samples.len().min(u16::MAX as usize) as u16;
        let event_count = frame.events.len().min(u16::MAX as usize) as u16;
        buf.extend_from_slice(&sample_count.to_le_bytes());
        buf.extend_from_slice(&event_count.to_le_bytes());

        // Samples
        for sample in frame.samples.iter().take(sample_count as usize) {
            self.encode_sample(sample, &mut buf);
        }

        // Events (simplified - just ID + severity + empty data)
        for event in frame.events.iter().take(event_count as usize) {
            buf.extend_from_slice(&event.event_id.to_le_bytes());
            let severity_byte = match event.severity {
                EventSeverity::Trace => 0,
                EventSeverity::Debug => 1,
                EventSeverity::Info => 2,
                EventSeverity::Warning => 3,
                EventSeverity::Error => 4,
                EventSeverity::Critical => 5,
            };
            buf.push(severity_byte);
            buf.push(0); // No data
        }

        // Pad to standard size if enabled
        if self.use_standard_size {
            let target_size = ECUBRIDGE_PACKET_SIZE - 4; // Reserve 4 for CRC
            if buf.len() < target_size {
                buf.resize(target_size, 0);
            }
        }

        // CRC
        let crc = self.calculate_crc(&buf);
        buf.extend_from_slice(&crc.to_le_bytes());

        Ok(buf)
    }

    fn estimate_encoded_size(&self, frame: &TelemetryFrame) -> usize {
        if self.use_standard_size {
            ECUBRIDGE_PACKET_SIZE
        } else {
            // Variable size estimate
            HEADER_SIZE
                + SAMPLE_HEADER_SIZE
                + frame.samples.len() * 12 // avg sample size
                + frame.events.len() * 8  // avg event size
                + 4 // CRC
        }
    }
}

/// Builder for creating ECUBridge packets (for testing/emulation)
#[derive(Clone, Debug, Default)]
pub struct EcuBridgePacketBuilder {
    session_id: SessionId,
    timestamp_us: i64,
    samples: Vec<Sample>,
    events: Vec<Event>,
    flags: PacketFlags,
}

impl EcuBridgePacketBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn session_id(mut self, id: SessionId) -> Self {
        self.session_id = id;
        self
    }

    pub fn timestamp_us(mut self, ts: i64) -> Self {
        self.timestamp_us = ts;
        self
    }

    pub fn sample(mut self, param_id: ParameterId, value: SampleValue, quality: SignalQuality) -> Self {
        self.samples.push(Sample::new(param_id, value, quality));
        self
    }

    pub fn event(mut self, event: Event) -> Self {
        self.events.push(event);
        self
    }

    pub fn heartbeat(mut self) -> Self {
        self.flags.heartbeat = true;
        self
    }

    pub fn build(self) -> Vec<u8> {
        let codec = EcuBridgeCodec::new();
        let frame = TelemetryFrameBuilder::new()
            .session_id(self.session_id)
            .timestamp_us(self.timestamp_us)
            .source_id("builder")
            .samples(self.samples)
            .events(self.events)
            .build();
        codec.encode(&frame, &CodecContext::default()).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codec_roundtrip() {
        let codec = EcuBridgeCodec::new();
        let ctx = CodecContext::new(12345, "test");

        let frame = TelemetryFrameBuilder::new()
            .session_id(12345)
            .timestamp_us(1700000000_000_000)
            .source_id("test")
            .sample(Sample::good(1, SampleValue::U16(8500)))
            .sample(Sample::good(2, SampleValue::F32(0.85)))
            .sample(Sample::good(3, SampleValue::Bool(true)))
            .build();

        let encoded = codec.encode(&frame, &ctx).unwrap();
        assert_eq!(encoded.len(), ECUBRIDGE_PACKET_SIZE);

        // Verify magic
        let magic = u32::from_le_bytes([encoded[0], encoded[1], encoded[2], encoded[3]]);
        assert_eq!(magic, ECUBRIDGE_MAGIC);

        let decoded = codec.decode(&encoded, &ctx).unwrap();
        match decoded {
            DecodeOutput::Frame(f) => {
                assert_eq!(f.session_id, 12345);
                assert_eq!(f.sample_count(), 3);
            }
            _ => panic!("expected Frame"),
        }
    }

    #[test]
    fn packet_builder() {
        let packet = EcuBridgePacketBuilder::new()
            .session_id(9999)
            .timestamp_us(1234567890)
            .sample(1, SampleValue::U16(1000), SignalQuality::Good)
            .sample(2, SampleValue::F32(3.14), SignalQuality::Good)
            .build();

        assert_eq!(packet.len(), ECUBRIDGE_PACKET_SIZE);

        let codec = EcuBridgeCodec::new();
        let ctx = CodecContext::default();
        let output = codec.decode(&packet, &ctx).unwrap();
        assert!(output.has_frames());
    }

    #[test]
    fn can_decode_check() {
        let codec = EcuBridgeCodec::new();

        // Valid magic
        let mut valid = vec![0u8; 100];
        valid[0..4].copy_from_slice(&ECUBRIDGE_MAGIC.to_le_bytes());
        assert!(codec.can_decode(&valid));

        // Invalid magic
        let invalid = vec![0u8; 100];
        assert!(!codec.can_decode(&invalid));

        // Too short
        let short = vec![0u8; 2];
        assert!(!codec.can_decode(&short));
    }

    #[test]
    fn data_type_sizes() {
        assert_eq!(DataTypeCode::Bool.size_bytes(), 1);
        assert_eq!(DataTypeCode::U16.size_bytes(), 2);
        assert_eq!(DataTypeCode::F32.size_bytes(), 4);
        assert_eq!(DataTypeCode::F64.size_bytes(), 8);
        assert_eq!(DataTypeCode::Bytes16.size_bytes(), 16);
    }

    #[test]
    fn packet_flags() {
        let flags = PacketFlags {
            compressed: true,
            heartbeat: false,
            has_events: true,
            encrypted: false,
        };
        let encoded = flags.to_u16();
        let decoded = PacketFlags::from_u16(encoded);
        assert_eq!(decoded.compressed, flags.compressed);
        assert_eq!(decoded.has_events, flags.has_events);
    }

    #[test]
    fn all_sample_types() {
        let codec = EcuBridgeCodec::new();
        let ctx = CodecContext::default();

        let frame = TelemetryFrameBuilder::new()
            .session_id(1)
            .timestamp_us(1000)
            .source_id("test")
            .sample(Sample::good(1, SampleValue::Bool(true)))
            .sample(Sample::good(2, SampleValue::U8(255)))
            .sample(Sample::good(3, SampleValue::U16(65535)))
            .sample(Sample::good(4, SampleValue::U32(4294967295)))
            .sample(Sample::good(5, SampleValue::I8(-128)))
            .sample(Sample::good(6, SampleValue::I16(-32768)))
            .sample(Sample::good(7, SampleValue::I32(-2147483648)))
            .sample(Sample::good(8, SampleValue::F32(3.14159)))
            .sample(Sample::good(9, SampleValue::F64(2.718281828)))
            .build();

        let encoded = codec.encode(&frame, &ctx).unwrap();
        let output = codec.decode(&encoded, &ctx).unwrap();

        match output {
            DecodeOutput::Frame(f) => {
                assert_eq!(f.sample_count(), 9);
            }
            _ => panic!("expected Frame"),
        }
    }
}
