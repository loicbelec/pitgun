use pitgun_core::{
    Event, EventBatch, Processor, SegmentAggregateProcessor, SegmentMetric, SegmentTarget,
};

fn batch(events: Vec<Event>, end_of_stream: bool) -> EventBatch {
    EventBatch {
        events,
        aggregates: Vec::new(),
        end_of_stream,
    }
}

fn processor() -> SegmentAggregateProcessor {
    SegmentAggregateProcessor::new(
        "segment_id".into(),
        vec![SegmentTarget {
            channel: "value".into(),
            metrics: vec![
                SegmentMetric::Count,
                SegmentMetric::Min,
                SegmentMetric::Max,
                SegmentMetric::Mean,
                SegmentMetric::Sum,
            ],
        }],
        true,
        true,
    )
}

#[test]
fn aggregates_across_batches() {
    let mut proc = processor();
    let mut aggregates = Vec::new();

    let mut b1 = batch(
        vec![
            Event {
                channel: "segment_id".into(),
                ts_ns: 0,
                value: 1.0,
            },
            Event {
                channel: "value".into(),
                ts_ns: 1,
                value: 1000.0,
            },
            Event {
                channel: "segment_id".into(),
                ts_ns: 2,
                value: 1.0,
            },
            Event {
                channel: "value".into(),
                ts_ns: 3,
                value: 2000.0,
            },
        ],
        false,
    );
    proc.process(&mut b1);
    aggregates.extend(b1.aggregates);

    let mut b2 = batch(
        vec![
            Event {
                channel: "value".into(),
                ts_ns: 4,
                value: 3000.0,
            },
            Event {
                channel: "segment_id".into(),
                ts_ns: 5,
                value: 2.0,
            },
            Event {
                channel: "value".into(),
                ts_ns: 6,
                value: 4000.0,
            },
            Event {
                channel: "segment_id".into(),
                ts_ns: 7,
                value: 2.0,
            },
            Event {
                channel: "value".into(),
                ts_ns: 8,
                value: 5000.0,
            },
        ],
        false,
    );
    proc.process(&mut b2);
    aggregates.extend(b2.aggregates);

    let mut b3 = batch(
        vec![
            Event {
                channel: "segment_id".into(),
                ts_ns: 9,
                value: 3.0,
            },
            Event {
                channel: "value".into(),
                ts_ns: 10,
                value: 6000.0,
            },
        ],
        true,
    );
    proc.process(&mut b3);
    aggregates.extend(b3.aggregates);

    assert_eq!(aggregates.len(), 3);
    assert_eq!(aggregates[0].segment_value, 1.0);
    assert_eq!(aggregates[1].segment_value, 2.0);
    assert_eq!(aggregates[2].segment_value, 3.0);

    let first = &aggregates[0].targets[0];
    assert_eq!(first.count, Some(3));
    assert_eq!(first.min, Some(1000.0));
    assert_eq!(first.max, Some(3000.0));
    assert_eq!(first.sum, Some(6000.0));
    assert!((first.mean.unwrap() - 2000.0).abs() < 1e-9);
}
