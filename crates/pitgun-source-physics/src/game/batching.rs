use pitgun_contract::game::v1::GameTelemetryPointV1;
use pitgun_core::EventBatch;

use crate::game::events::telemetry_point_to_events;

#[derive(Clone, Debug)]
pub struct TelemetryBatchSummary {
    pub batches: Vec<EventBatch>,
    pub total_events: usize,
    pub first_ts_ns: Option<u64>,
    pub last_ts_ns: Option<u64>,
}

pub fn telemetry_to_event_batches(
    telemetry: &[GameTelemetryPointV1],
    batch_events: usize,
) -> TelemetryBatchSummary {
    if batch_events == 0 {
        return TelemetryBatchSummary {
            batches: Vec::new(),
            total_events: 0,
            first_ts_ns: None,
            last_ts_ns: None,
        };
    }

    let mut batches = Vec::new();
    let mut current = Vec::with_capacity(batch_events);
    let mut total_events = 0;
    let mut first_ts_ns = None;
    let mut last_ts_ns = None;

    for point in telemetry {
        let ts_ns = (point.time_s * 1_000_000_000.0) as u64;
        if first_ts_ns.is_none() {
            first_ts_ns = Some(ts_ns);
        }
        last_ts_ns = Some(ts_ns);

        let point_events = telemetry_point_to_events(point, ts_ns);
        total_events += point_events.len();

        for event in point_events {
            current.push(event);
            if current.len() >= batch_events {
                batches.push(EventBatch {
                    events: std::mem::take(&mut current),
                    aggregates: Vec::new(),
                    end_of_stream: false,
                });
            }
        }
    }

    if !current.is_empty() {
        batches.push(EventBatch {
            events: current,
            aggregates: Vec::new(),
            end_of_stream: false,
        });
    }

    if let Some(last) = batches.last_mut() {
        last.end_of_stream = true;
    }

    TelemetryBatchSummary {
        batches,
        total_events,
        first_ts_ns,
        last_ts_ns,
    }
}
