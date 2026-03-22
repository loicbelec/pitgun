use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TelemetryFrame {
    pub time_s: f64,
    pub s_m: f64,
    pub x_m: f64,
    pub y_m: f64,
    pub heading_rad: f64,
    pub speed_kph: f64,
    pub rpm: f64,
    pub gear: u8,
    pub throttle_pct: f64,
    pub brake_pct: f64,
    pub g_lat: f64,
    pub g_long: f64,
    pub g_vert: f64,
    pub engine_temp_c: f64,
    pub engine_power_w: f64,
    pub tire_temp_c: f64,
    pub tire_wear_pct: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tire_mu: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub n_lap: Option<u16>,
}
