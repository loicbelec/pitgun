use pitgun_contract::game::v1::GameTelemetryPointV1;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TelemetrySummaryMetrics {
    pub lap_time_s: f32,
    pub vmax_kph: f32,
    pub rpm_max: f32,
    pub temp_max_c: f32,
}

pub fn compute_summary_metrics(telemetry: &[GameTelemetryPointV1]) -> TelemetrySummaryMetrics {
    if telemetry.is_empty() {
        return TelemetrySummaryMetrics {
            lap_time_s: 0.0,
            vmax_kph: 0.0,
            rpm_max: 0.0,
            temp_max_c: 0.0,
        };
    }

    let mut vmax_kph: f32 = 0.0;
    let mut rpm_max: f32 = 0.0;
    let mut temp_max_c: f32 = 0.0;

    for point in telemetry {
        vmax_kph = vmax_kph.max(point.speed_kph);
        rpm_max = rpm_max.max(point.rpm);
        temp_max_c = temp_max_c.max(point.engine_temp_c);
    }

    let lap_time_s = telemetry.last().map(|point| point.time_s).unwrap_or(0.0);

    TelemetrySummaryMetrics {
        lap_time_s,
        vmax_kph,
        rpm_max,
        temp_max_c,
    }
}
