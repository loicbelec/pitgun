//! F1 UDP Telemetry Codec
//!
//! This module implements a decoder for the F1 UDP telemetry format.
//! Real F1 telemetry is sent via UDP packets on port 20777.
//!
//! # Packet Types
//!
//! The F1 game uses multiple packet types:
//! - **Motion** (0): Car physics (position, velocity, g-forces)
//! - **Session** (1): Track, weather, session info
//! - **Lap Data** (2): Lap times, sector times, positions
//! - **Event** (3): Race events (penalties, fastest lap, etc.)
//! - **Participants** (4): Driver names, teams
//! - **Car Setup** (5): Car setup parameters
//! - **Car Telemetry** (6): Detailed car telemetry
//! - **Car Status** (7): Car status (damage, ERS, tyres)
//! - **Final Classification** (8): End-of-session results
//! - **Lobby Info** (9): Multiplayer lobby
//! - **Car Damage** (10): Car damage status
//! - **Session History** (11): Lap/sector history
//! - **Tyre Sets** (12): Available tyre sets
//! - **Motion Ex** (13): Extended motion data
//!
//! # Packet Header
//!
//! All packets share a common 29-byte header:
//! ```text
//! - Packet Format (u16): Year (e.g., 2024)
//! - Game Year (u8): Year - 2000 (e.g., 24)
//! - Game Major Version (u8)
//! - Game Minor Version (u8)
//! - Packet Version (u8)
//! - Packet ID (u8): Type of packet
//! - Session UID (u64): Unique session identifier
//! - Session Time (f32): Session timestamp
//! - Frame Identifier (u32)
//! - Overall Frame Identifier (u32)
//! - Player Car Index (u8)
//! - Secondary Player Car Index (u8)
//! ```

use pitgun_contract::{
    CodecCapabilities, CodecContext, CodecError, CodecResult, DecodeOutput, Event, EventSeverity,
    ParameterId, ProtocolInfo, Sample, SampleValue, TelemetryCodec, TelemetryFrameBuilder,
};
use std::io::{Cursor, Read};

/// F1 UDP header size
pub const F1_HEADER_SIZE: usize = 29;

/// Minimum packet size
pub const F1_MIN_PACKET_SIZE: usize = F1_HEADER_SIZE;

/// Parameter ID ranges for F1 telemetry
pub mod param_ids {
    //! Parameter ID assignments for F1 telemetry
    //! These IDs are part of this codec's current wire mapping. A future
    //! observed-data registry may attach canonical names and units to them.

    // Motion (1-50)
    pub const POSITION_X: u16 = 120;
    pub const POSITION_Y: u16 = 121;
    pub const POSITION_Z: u16 = 122;
    pub const VELOCITY_X: u16 = 41;
    pub const VELOCITY_Y: u16 = 42;
    pub const VELOCITY_Z: u16 = 43;
    pub const G_FORCE_LATERAL: u16 = 50;
    pub const G_FORCE_LONGITUDINAL: u16 = 51;
    pub const G_FORCE_VERTICAL: u16 = 52;
    pub const YAW: u16 = 130;
    pub const PITCH: u16 = 131;
    pub const ROLL: u16 = 132;

    // Car Telemetry (100-150)
    pub const SPEED: u16 = 40;
    pub const THROTTLE: u16 = 10;
    pub const BRAKE: u16 = 11;
    pub const STEERING: u16 = 20;
    pub const GEAR: u16 = 30;
    pub const ENGINE_RPM: u16 = 1;
    pub const DRS: u16 = 100;
    pub const ENGINE_TEMP: u16 = 2;

    // Tyres (200-250)
    pub const TYRE_PRESSURE_FL: u16 = 80;
    pub const TYRE_PRESSURE_FR: u16 = 81;
    pub const TYRE_PRESSURE_RL: u16 = 82;
    pub const TYRE_PRESSURE_RR: u16 = 83;
    pub const TYRE_TEMP_FL: u16 = 70;
    pub const TYRE_TEMP_FR: u16 = 71;
    pub const TYRE_TEMP_RL: u16 = 72;
    pub const TYRE_TEMP_RR: u16 = 73;

    // Brakes (260-280)
    pub const BRAKE_TEMP_FL: u16 = 150;
    pub const BRAKE_TEMP_FR: u16 = 151;
    pub const BRAKE_TEMP_RL: u16 = 152;
    pub const BRAKE_TEMP_RR: u16 = 153;

    // Wheels (300-350)
    pub const WHEEL_SPEED_FL: u16 = 60;
    pub const WHEEL_SPEED_FR: u16 = 61;
    pub const WHEEL_SPEED_RL: u16 = 62;
    pub const WHEEL_SPEED_RR: u16 = 63;

    // Suspension (350-400)
    pub const SUSPENSION_FL: u16 = 140;
    pub const SUSPENSION_FR: u16 = 141;
    pub const SUSPENSION_RL: u16 = 142;
    pub const SUSPENSION_RR: u16 = 143;

    // Lap/Timing (400-450)
    pub const LAP_TIME: u16 = 110;
    pub const LAP_NUMBER: u16 = 113;
    pub const LAP_DISTANCE: u16 = 114;
    pub const SECTOR: u16 = 115;
}

/// F1 packet types
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum F1PacketType {
    Motion = 0,
    Session = 1,
    LapData = 2,
    Event = 3,
    Participants = 4,
    CarSetups = 5,
    CarTelemetry = 6,
    CarStatus = 7,
    FinalClassification = 8,
    LobbyInfo = 9,
    CarDamage = 10,
    SessionHistory = 11,
    TyreSets = 12,
    MotionEx = 13,
}

impl F1PacketType {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Motion),
            1 => Some(Self::Session),
            2 => Some(Self::LapData),
            3 => Some(Self::Event),
            4 => Some(Self::Participants),
            5 => Some(Self::CarSetups),
            6 => Some(Self::CarTelemetry),
            7 => Some(Self::CarStatus),
            8 => Some(Self::FinalClassification),
            9 => Some(Self::LobbyInfo),
            10 => Some(Self::CarDamage),
            11 => Some(Self::SessionHistory),
            12 => Some(Self::TyreSets),
            13 => Some(Self::MotionEx),
            _ => None,
        }
    }

    /// Returns true if this packet type contains real-time telemetry
    pub fn is_telemetry(&self) -> bool {
        matches!(
            self,
            Self::Motion | Self::CarTelemetry | Self::CarStatus | Self::MotionEx
        )
    }
}

/// F1 packet header
#[derive(Clone, Debug)]
pub struct F1Header {
    pub packet_format: u16,
    pub game_year: u8,
    pub game_major_version: u8,
    pub game_minor_version: u8,
    pub packet_version: u8,
    pub packet_id: F1PacketType,
    pub session_uid: u64,
    pub session_time: f32,
    pub frame_identifier: u32,
    pub overall_frame_identifier: u32,
    pub player_car_index: u8,
    pub secondary_player_car_index: u8,
}

/// F1 UDP Codec
#[derive(Clone, Debug, Default)]
pub struct F1UdpCodec {
    /// Which car index to extract telemetry for (default: player car)
    pub car_index: Option<u8>,
    /// Whether to only process telemetry packets
    pub telemetry_only: bool,
}

impl F1UdpCodec {
    /// Creates a new codec for the player car
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a codec for a specific car index
    pub fn for_car(car_index: u8) -> Self {
        Self {
            car_index: Some(car_index),
            telemetry_only: false,
        }
    }

    /// Creates a codec that only processes telemetry packets
    pub fn telemetry_only() -> Self {
        Self {
            car_index: None,
            telemetry_only: true,
        }
    }

    /// Decodes the packet header
    fn decode_header(&self, data: &[u8]) -> CodecResult<F1Header> {
        if data.len() < F1_HEADER_SIZE {
            return Err(CodecError::InsufficientData {
                expected: F1_HEADER_SIZE,
                actual: data.len(),
            });
        }

        let packet_format = u16::from_le_bytes([data[0], data[1]]);
        let game_year = data[2];
        let game_major_version = data[3];
        let game_minor_version = data[4];
        let packet_version = data[5];
        let packet_id_raw = data[6];
        let packet_id = F1PacketType::from_u8(packet_id_raw)
            .ok_or(CodecError::InvalidPacketType(packet_id_raw))?;
        let session_uid = u64::from_le_bytes([
            data[7], data[8], data[9], data[10], data[11], data[12], data[13], data[14],
        ]);
        let session_time = f32::from_le_bytes([data[15], data[16], data[17], data[18]]);
        let frame_identifier = u32::from_le_bytes([data[19], data[20], data[21], data[22]]);
        let overall_frame_identifier = u32::from_le_bytes([data[23], data[24], data[25], data[26]]);
        let player_car_index = data[27];
        let secondary_player_car_index = data[28];

        Ok(F1Header {
            packet_format,
            game_year,
            game_major_version,
            game_minor_version,
            packet_version,
            packet_id,
            session_uid,
            session_time,
            frame_identifier,
            overall_frame_identifier,
            player_car_index,
            secondary_player_car_index,
        })
    }

    /// Decodes Motion packet (packet ID 0)
    fn decode_motion(
        &self,
        data: &[u8],
        header: &F1Header,
        samples: &mut Vec<Sample>,
    ) -> CodecResult<()> {
        // Motion packet: 22 cars × 60 bytes each = 1320 bytes + header
        let car_motion_size = 60;
        let car_index = self.car_index.unwrap_or(header.player_car_index) as usize;
        let offset = F1_HEADER_SIZE + car_index * car_motion_size;

        if data.len() < offset + car_motion_size {
            return Ok(()); // Not enough data for this car
        }

        let mut cursor = Cursor::new(&data[offset..offset + car_motion_size]);

        // Position (3 × f32)
        samples.push(self.read_f32_sample(&mut cursor, param_ids::POSITION_X)?);
        samples.push(self.read_f32_sample(&mut cursor, param_ids::POSITION_Y)?);
        samples.push(self.read_f32_sample(&mut cursor, param_ids::POSITION_Z)?);

        // Velocity (3 × f32)
        samples.push(self.read_f32_sample(&mut cursor, param_ids::VELOCITY_X)?);
        samples.push(self.read_f32_sample(&mut cursor, param_ids::VELOCITY_Y)?);
        samples.push(self.read_f32_sample(&mut cursor, param_ids::VELOCITY_Z)?);

        // Forward direction (skip 2 × i16)
        let mut skip = [0u8; 4];
        let _ = cursor.read_exact(&mut skip);

        // Right direction (skip 2 × i16)
        let _ = cursor.read_exact(&mut skip);

        // G-force (3 × f32)
        samples.push(self.read_f32_sample(&mut cursor, param_ids::G_FORCE_LATERAL)?);
        samples.push(self.read_f32_sample(&mut cursor, param_ids::G_FORCE_LONGITUDINAL)?);
        samples.push(self.read_f32_sample(&mut cursor, param_ids::G_FORCE_VERTICAL)?);

        // Yaw, Pitch, Roll (3 × f32)
        samples.push(self.read_f32_sample(&mut cursor, param_ids::YAW)?);
        samples.push(self.read_f32_sample(&mut cursor, param_ids::PITCH)?);
        samples.push(self.read_f32_sample(&mut cursor, param_ids::ROLL)?);

        Ok(())
    }

    /// Decodes Car Telemetry packet (packet ID 6)
    fn decode_car_telemetry(
        &self,
        data: &[u8],
        header: &F1Header,
        samples: &mut Vec<Sample>,
    ) -> CodecResult<()> {
        // Car telemetry: 22 cars × 60 bytes each
        let car_telemetry_size = 60;
        let car_index = self.car_index.unwrap_or(header.player_car_index) as usize;
        let offset = F1_HEADER_SIZE + car_index * car_telemetry_size;

        if data.len() < offset + car_telemetry_size {
            return Ok(());
        }

        let car_data = &data[offset..offset + car_telemetry_size];

        // Speed (u16 km/h)
        let speed = u16::from_le_bytes([car_data[0], car_data[1]]);
        samples.push(Sample::good(param_ids::SPEED, SampleValue::U16(speed)));

        // Throttle (f32 0-1)
        let throttle = f32::from_le_bytes([car_data[2], car_data[3], car_data[4], car_data[5]]);
        samples.push(Sample::good(
            param_ids::THROTTLE,
            SampleValue::F32(throttle * 100.0),
        ));

        // Steer (f32 -1 to 1)
        let steer = f32::from_le_bytes([car_data[6], car_data[7], car_data[8], car_data[9]]);
        samples.push(Sample::good(
            param_ids::STEERING,
            SampleValue::F32(steer * 360.0),
        ));

        // Brake (f32 0-1)
        let brake = f32::from_le_bytes([car_data[10], car_data[11], car_data[12], car_data[13]]);
        samples.push(Sample::good(
            param_ids::BRAKE,
            SampleValue::F32(brake * 100.0),
        ));

        // Clutch (skip u8)
        // Gear (i8)
        let gear = car_data[15] as i8;
        samples.push(Sample::good(param_ids::GEAR, SampleValue::I8(gear)));

        // Engine RPM (u16)
        let rpm = u16::from_le_bytes([car_data[16], car_data[17]]);
        samples.push(Sample::good(param_ids::ENGINE_RPM, SampleValue::U16(rpm)));

        // DRS (u8)
        let drs = car_data[18];
        samples.push(Sample::good(param_ids::DRS, SampleValue::Bool(drs != 0)));

        // Rev lights percent (skip u8)

        // Brakes temperature (4 × u16)
        let brake_temp_fl = u16::from_le_bytes([car_data[22], car_data[23]]);
        let brake_temp_fr = u16::from_le_bytes([car_data[24], car_data[25]]);
        let brake_temp_rl = u16::from_le_bytes([car_data[26], car_data[27]]);
        let brake_temp_rr = u16::from_le_bytes([car_data[28], car_data[29]]);
        samples.push(Sample::good(
            param_ids::BRAKE_TEMP_FL,
            SampleValue::U16(brake_temp_fl),
        ));
        samples.push(Sample::good(
            param_ids::BRAKE_TEMP_FR,
            SampleValue::U16(brake_temp_fr),
        ));
        samples.push(Sample::good(
            param_ids::BRAKE_TEMP_RL,
            SampleValue::U16(brake_temp_rl),
        ));
        samples.push(Sample::good(
            param_ids::BRAKE_TEMP_RR,
            SampleValue::U16(brake_temp_rr),
        ));

        // Tyres surface temperature (4 × u8)
        samples.push(Sample::good(
            param_ids::TYRE_TEMP_FL,
            SampleValue::U8(car_data[30]),
        ));
        samples.push(Sample::good(
            param_ids::TYRE_TEMP_FR,
            SampleValue::U8(car_data[31]),
        ));
        samples.push(Sample::good(
            param_ids::TYRE_TEMP_RL,
            SampleValue::U8(car_data[32]),
        ));
        samples.push(Sample::good(
            param_ids::TYRE_TEMP_RR,
            SampleValue::U8(car_data[33]),
        ));

        // Tyres pressure (4 × f32) - offset 42
        if car_data.len() >= 58 {
            let tp_fl =
                f32::from_le_bytes([car_data[42], car_data[43], car_data[44], car_data[45]]);
            let tp_fr =
                f32::from_le_bytes([car_data[46], car_data[47], car_data[48], car_data[49]]);
            let tp_rl =
                f32::from_le_bytes([car_data[50], car_data[51], car_data[52], car_data[53]]);
            let tp_rr =
                f32::from_le_bytes([car_data[54], car_data[55], car_data[56], car_data[57]]);
            samples.push(Sample::good(
                param_ids::TYRE_PRESSURE_FL,
                SampleValue::F32(tp_fl),
            ));
            samples.push(Sample::good(
                param_ids::TYRE_PRESSURE_FR,
                SampleValue::F32(tp_fr),
            ));
            samples.push(Sample::good(
                param_ids::TYRE_PRESSURE_RL,
                SampleValue::F32(tp_rl),
            ));
            samples.push(Sample::good(
                param_ids::TYRE_PRESSURE_RR,
                SampleValue::F32(tp_rr),
            ));
        }

        Ok(())
    }

    /// Decodes Lap Data packet (packet ID 2)
    fn decode_lap_data(
        &self,
        data: &[u8],
        header: &F1Header,
        samples: &mut Vec<Sample>,
    ) -> CodecResult<()> {
        // Lap data: 22 cars × 57 bytes each
        let car_lap_size = 57;
        let car_index = self.car_index.unwrap_or(header.player_car_index) as usize;
        let offset = F1_HEADER_SIZE + car_index * car_lap_size;

        if data.len() < offset + car_lap_size {
            return Ok(());
        }

        let car_data = &data[offset..offset + car_lap_size];

        // Last lap time (u32 ms)
        let last_lap = u32::from_le_bytes([car_data[0], car_data[1], car_data[2], car_data[3]]);
        samples.push(Sample::good(
            param_ids::LAP_TIME,
            SampleValue::U32(last_lap),
        ));

        // Current lap time (u32 ms) - offset 4
        // Sector 1 time (u16 ms) - offset 8
        // etc...

        // Lap distance (f32) - offset 12
        let lap_distance =
            f32::from_le_bytes([car_data[12], car_data[13], car_data[14], car_data[15]]);
        samples.push(Sample::good(
            param_ids::LAP_DISTANCE,
            SampleValue::F32(lap_distance),
        ));

        // Current lap num (u8) - offset 24
        let lap_num = car_data[24];
        samples.push(Sample::good(
            param_ids::LAP_NUMBER,
            SampleValue::U8(lap_num),
        ));

        // Sector (u8) - offset 29
        let sector = car_data[29];
        samples.push(Sample::good(param_ids::SECTOR, SampleValue::U8(sector)));

        Ok(())
    }

    /// Decodes Event packet (packet ID 3)
    fn decode_event(
        &self,
        data: &[u8],
        _header: &F1Header,
        events: &mut Vec<Event>,
    ) -> CodecResult<()> {
        if data.len() < F1_HEADER_SIZE + 4 {
            return Ok(());
        }

        // Event code is 4 bytes
        let event_code = &data[F1_HEADER_SIZE..F1_HEADER_SIZE + 4];
        let code_str = std::str::from_utf8(event_code).unwrap_or("????");

        let (event_id, name, severity) = match code_str {
            "SSTA" => (1, "session_started", EventSeverity::Info),
            "SEND" => (2, "session_ended", EventSeverity::Info),
            "FTLP" => (3, "fastest_lap", EventSeverity::Info),
            "RTMT" => (4, "retirement", EventSeverity::Warning),
            "DRSE" => (5, "drs_enabled", EventSeverity::Info),
            "DRSD" => (6, "drs_disabled", EventSeverity::Info),
            "TMPT" => (7, "teammate_in_pits", EventSeverity::Info),
            "CHQF" => (8, "chequered_flag", EventSeverity::Info),
            "RCWN" => (9, "race_winner", EventSeverity::Info),
            "PENA" => (10, "penalty_issued", EventSeverity::Warning),
            "SPTP" => (11, "speed_trap", EventSeverity::Debug),
            "STLG" => (12, "start_lights", EventSeverity::Info),
            "LGOT" => (13, "lights_out", EventSeverity::Info),
            "DTSV" => (14, "drive_through_served", EventSeverity::Info),
            "SGSV" => (15, "stop_go_served", EventSeverity::Info),
            "FLBK" => (16, "flashback", EventSeverity::Debug),
            "BUTN" => (17, "button_status", EventSeverity::Trace),
            "RDFL" => (18, "red_flag", EventSeverity::Critical),
            "OVTK" => (19, "overtake", EventSeverity::Info),
            _ => (0, code_str, EventSeverity::Debug),
        };

        events.push(Event::new(event_id, name, severity));
        Ok(())
    }

    /// Helper to read an f32 sample
    fn read_f32_sample(
        &self,
        cursor: &mut Cursor<&[u8]>,
        param_id: ParameterId,
    ) -> CodecResult<Sample> {
        let mut buf = [0u8; 4];
        cursor
            .read_exact(&mut buf)
            .map_err(|_| CodecError::MalformedData("failed to read f32".into()))?;
        Ok(Sample::good(
            param_id,
            SampleValue::F32(f32::from_le_bytes(buf)),
        ))
    }
}

impl TelemetryCodec for F1UdpCodec {
    fn name(&self) -> &str {
        "f1-udp"
    }

    fn protocol(&self) -> ProtocolInfo {
        ProtocolInfo::new("F1 UDP")
            .with_version("2024")
            .with_version("2023")
            .with_description("F1 UDP telemetry format")
    }

    fn capabilities(&self) -> CodecCapabilities {
        CodecCapabilities::decode_only().with_multi_packet()
    }

    fn min_decode_size(&self) -> usize {
        F1_MIN_PACKET_SIZE
    }

    fn can_decode(&self, data: &[u8]) -> bool {
        if data.len() < F1_HEADER_SIZE {
            return false;
        }
        // Check for reasonable packet format year (2020-2030)
        let format = u16::from_le_bytes([data[0], data[1]]);
        (2020..=2030).contains(&format)
    }

    fn decode(&self, data: &[u8], ctx: &CodecContext) -> CodecResult<DecodeOutput> {
        let header = self.decode_header(data)?;

        // Skip non-telemetry packets if telemetry_only mode
        if self.telemetry_only && !header.packet_id.is_telemetry() {
            return Ok(DecodeOutput::Skipped(data.len()));
        }

        let timestamp_us = (header.session_time * 1_000_000.0) as i64;
        let mut samples = Vec::new();
        let mut events = Vec::new();

        match header.packet_id {
            F1PacketType::Motion => {
                self.decode_motion(data, &header, &mut samples)?;
            }
            F1PacketType::CarTelemetry => {
                self.decode_car_telemetry(data, &header, &mut samples)?;
            }
            F1PacketType::LapData => {
                self.decode_lap_data(data, &header, &mut samples)?;
            }
            F1PacketType::Event => {
                self.decode_event(data, &header, &mut events)?;
            }
            _ => {
                // Other packet types - skip for now
                return Ok(DecodeOutput::Skipped(data.len()));
            }
        }

        if samples.is_empty() && events.is_empty() {
            return Ok(DecodeOutput::NoOutput);
        }

        let frame = TelemetryFrameBuilder::new()
            .session_id(header.session_uid)
            .sequence(header.frame_identifier as u64)
            .timestamp_us(timestamp_us)
            .source_id(&ctx.source_id)
            .samples(samples)
            .events(events)
            .build();

        Ok(DecodeOutput::Frame(frame))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_f1_header(packet_id: u8) -> Vec<u8> {
        let mut data = vec![0u8; F1_HEADER_SIZE];
        // Packet format: 2024
        data[0] = 0xE8;
        data[1] = 0x07;
        // Game year: 24
        data[2] = 24;
        // Versions
        data[3] = 1;
        data[4] = 0;
        data[5] = 1;
        // Packet ID
        data[6] = packet_id;
        // Session UID
        data[7..15].copy_from_slice(&12345u64.to_le_bytes());
        // Session time (1.5 seconds)
        data[15..19].copy_from_slice(&1.5f32.to_le_bytes());
        // Frame identifier
        data[19..23].copy_from_slice(&100u32.to_le_bytes());
        // Overall frame
        data[23..27].copy_from_slice(&1000u32.to_le_bytes());
        // Player car index
        data[27] = 0;
        data[28] = 255;
        data
    }

    #[test]
    fn decode_header() {
        let codec = F1UdpCodec::new();
        let data = make_f1_header(6); // Car telemetry

        let header = codec.decode_header(&data).unwrap();
        assert_eq!(header.packet_format, 2024);
        assert_eq!(header.game_year, 24);
        assert_eq!(header.packet_id, F1PacketType::CarTelemetry);
        assert_eq!(header.session_uid, 12345);
        assert_eq!(header.player_car_index, 0);
    }

    #[test]
    fn can_decode_check() {
        let codec = F1UdpCodec::new();

        // Valid F1 2024 packet
        let valid = make_f1_header(0);
        assert!(codec.can_decode(&valid));

        // Too short
        let short = vec![0u8; 10];
        assert!(!codec.can_decode(&short));

        // Invalid year
        let mut invalid = make_f1_header(0);
        invalid[0] = 0x00;
        invalid[1] = 0x00;
        assert!(!codec.can_decode(&invalid));
    }

    #[test]
    fn packet_types() {
        assert!(F1PacketType::Motion.is_telemetry());
        assert!(F1PacketType::CarTelemetry.is_telemetry());
        assert!(!F1PacketType::Session.is_telemetry());
        assert!(!F1PacketType::Participants.is_telemetry());
    }

    #[test]
    fn decode_event_packet() {
        let codec = F1UdpCodec::new();
        let ctx = CodecContext::new(1, "test");

        let mut data = make_f1_header(3); // Event packet
        data.extend_from_slice(b"SSTA"); // Session started

        let output = codec.decode(&data, &ctx).unwrap();
        match output {
            DecodeOutput::Frame(f) => {
                assert_eq!(f.event_count(), 1);
            }
            _ => panic!("expected Frame"),
        }
    }

    #[test]
    fn telemetry_only_mode() {
        let codec = F1UdpCodec::telemetry_only();
        let ctx = CodecContext::default();

        // Session packet should be skipped
        let session_data = make_f1_header(1);
        let output = codec.decode(&session_data, &ctx).unwrap();
        assert!(matches!(output, DecodeOutput::Skipped(_)));

        // Motion packet should be processed
        let mut motion_data = make_f1_header(0);
        motion_data.resize(F1_HEADER_SIZE + 22 * 60, 0); // Add car data
        let output = codec.decode(&motion_data, &ctx).unwrap();
        // Should produce frame or at least not skip
        assert!(!matches!(output, DecodeOutput::Skipped(_)));
    }
}
