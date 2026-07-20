//! Aggregate an observed telemetry stream by a domain-defined segment key.
//!
//! This example deliberately sits outside Pitgun's deterministic simulation
//! loop. It demonstrates a reusable data-engineering primitive that can later
//! summarize observed data with the same boundaries as simulated telemetry.

use pitgun_core::{
    Event, EventBatch, Processor, SegmentAggregateProcessor, SegmentMetric, SegmentTarget,
};

fn event(channel: &str, ts_ns: u64, value: f64) -> Event {
    Event {
        channel: channel.into(),
        ts_ns,
        value,
    }
}

fn main() -> Result<(), serde_json::Error> {
    let mut processor = SegmentAggregateProcessor::new(
        "lap_id".into(),
        vec![SegmentTarget {
            channel: "engine_rpm".into(),
            metrics: vec![
                SegmentMetric::Count,
                SegmentMetric::Min,
                SegmentMetric::Max,
                SegmentMetric::Mean,
            ],
        }],
        true,
        true,
    );

    let mut batches = [
        EventBatch {
            events: vec![
                event("lap_id", 0, 1.0),
                event("engine_rpm", 1, 10_000.0),
                event("engine_rpm", 2, 12_000.0),
                event("lap_id", 3, 2.0),
            ],
            ..EventBatch::default()
        },
        EventBatch {
            events: vec![
                event("engine_rpm", 4, 11_000.0),
                event("engine_rpm", 5, 13_000.0),
            ],
            end_of_stream: true,
            ..EventBatch::default()
        },
    ];

    for batch in &mut batches {
        processor.process(batch);
        for aggregate in &batch.aggregates {
            println!("{}", serde_json::to_string(aggregate)?);
        }
    }

    Ok(())
}
