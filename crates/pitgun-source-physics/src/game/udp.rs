use pitgun_contract::game::v1::GameTelemetryPointV1;
use pitgun_core::Event;

use crate::game::events;

#[deprecated(note = "Use pitgun_source_physics::game::events::telemetry_point_to_events instead")]
pub fn telemetry_point_to_events(point: &GameTelemetryPointV1, ts_ns: u64) -> Vec<Event> {
    events::telemetry_point_to_events(point, ts_ns)
}
