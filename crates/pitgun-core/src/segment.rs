use std::collections::{HashMap, HashSet};

use crate::{EventBatch, Processor, SegmentAggregateRecord, SegmentTargetMetrics};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SegmentMetric {
    Count,
    Min,
    Max,
    Mean,
    Sum,
    Stddev,
}

impl SegmentMetric {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "count" => Some(Self::Count),
            "min" => Some(Self::Min),
            "max" => Some(Self::Max),
            "mean" => Some(Self::Mean),
            "sum" => Some(Self::Sum),
            "stddev" => Some(Self::Stddev),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct SegmentTarget {
    pub channel: String,
    pub metrics: Vec<SegmentMetric>,
}

#[derive(Default, Clone, Debug)]
struct RunningStats {
    count: u64,
    mean: f64,
    m2: f64,
    min: Option<f64>,
    max: Option<f64>,
    sum: f64,
}

impl RunningStats {
    fn update(&mut self, value: f64) {
        let count = self.count + 1;
        let delta = value - self.mean;
        self.mean += delta / count as f64;
        let delta2 = value - self.mean;
        self.m2 += delta * delta2;
        self.count = count;
        self.sum += value;
        self.min = Some(self.min.map_or(value, |v| v.min(value)));
        self.max = Some(self.max.map_or(value, |v| v.max(value)));
    }

    fn stddev_population(&self) -> Option<f64> {
        if self.count == 0 {
            None
        } else {
            Some((self.m2 / self.count as f64).sqrt())
        }
    }
}

#[derive(Default, Clone, Debug)]
struct SegmentState {
    key_value: f64,
    start_ts: Option<u64>,
    end_ts: Option<u64>,
    per_channel: HashMap<String, RunningStats>,
}

impl SegmentState {
    fn new(key_value: f64, ts_ns: u64) -> Self {
        Self {
            key_value,
            start_ts: Some(ts_ns),
            end_ts: Some(ts_ns),
            per_channel: HashMap::new(),
        }
    }

    fn touch_ts(&mut self, ts_ns: u64) {
        self.start_ts = Some(self.start_ts.map_or(ts_ns, |v| v.min(ts_ns)));
        self.end_ts = Some(self.end_ts.map_or(ts_ns, |v| v.max(ts_ns)));
    }
}

pub struct SegmentAggregateProcessor {
    segment_key: String,
    targets: Vec<SegmentTarget>,
    target_lookup: HashMap<String, HashSet<SegmentMetric>>,
    emit_on_change: bool,
    emit_last_segment_on_eof: bool,
    current: Option<SegmentState>,
    warned_missing_key: bool,
    warned_non_numeric: HashSet<String>,
}

impl SegmentAggregateProcessor {
    pub fn new(
        segment_key: String,
        targets: Vec<SegmentTarget>,
        emit_on_change: bool,
        emit_last_segment_on_eof: bool,
    ) -> Self {
        let mut lookup = HashMap::new();
        for t in &targets {
            lookup.insert(t.channel.clone(), t.metrics.iter().copied().collect());
        }
        Self {
            segment_key,
            targets,
            target_lookup: lookup,
            emit_on_change,
            emit_last_segment_on_eof,
            current: None,
            warned_missing_key: false,
            warned_non_numeric: HashSet::new(),
        }
    }

    fn start_new_segment(&mut self, value: f64, ts: u64, batch: &mut EventBatch) {
        if self.emit_on_change {
            self.flush_current(batch);
        } else {
            self.current.take();
        }
        self.current = Some(SegmentState::new(value, ts));
    }

    fn handle_segment_key(&mut self, value: f64, ts: u64, batch: &mut EventBatch) {
        if !value.is_finite() {
            if self
                .warned_non_numeric
                .insert(format!("segment_key:{}", self.segment_key))
            {
                eprintln!(
                    "pitgun-core: segment_aggregate skipping non-numeric segment key '{:.3}'",
                    value
                );
            }
            return;
        }

        if let Some(state) = &self.current
            && value != state.key_value
        {
            self.start_new_segment(value, ts, batch);
            return;
        }

        match self.current.as_mut() {
            Some(state) => state.touch_ts(ts),
            None => self.current = Some(SegmentState::new(value, ts)),
        }
    }

    fn handle_value(&mut self, channel: &str, value: f64, ts: u64) {
        let Some(state) = self.current.as_mut() else {
            if !self.warned_missing_key {
                eprintln!(
                    "pitgun-core: segment_aggregate received data for '{}' before any segment key; dropping until first key",
                    channel
                );
                self.warned_missing_key = true;
            }
            return;
        };

        state.touch_ts(ts);

        let Some(_metrics) = self.target_lookup.get(channel) else {
            return;
        };

        if !value.is_finite() {
            if self.warned_non_numeric.insert(channel.to_string()) {
                eprintln!(
                    "pitgun-core: segment_aggregate skipping non-numeric value on '{}'",
                    channel
                );
            }
            return;
        }

        let stats = state.per_channel.entry(channel.to_string()).or_default();
        stats.update(value);
    }

    fn flush_current(&mut self, batch: &mut EventBatch) {
        let Some(state) = self.current.take() else {
            return;
        };

        let mut targets_out = Vec::new();
        for target in &self.targets {
            let requested = self
                .target_lookup
                .get(&target.channel)
                .cloned()
                .unwrap_or_default();
            let stats = state
                .per_channel
                .get(&target.channel)
                .cloned()
                .unwrap_or_default();
            let mut out = SegmentTargetMetrics {
                channel: target.channel.clone(),
                ..Default::default()
            };
            if requested.contains(&SegmentMetric::Count) {
                out.count = Some(stats.count);
            }
            if requested.contains(&SegmentMetric::Min) {
                out.min = stats.min;
            }
            if requested.contains(&SegmentMetric::Max) {
                out.max = stats.max;
            }
            if requested.contains(&SegmentMetric::Sum) {
                out.sum = Some(stats.sum);
            }
            if requested.contains(&SegmentMetric::Mean) {
                out.mean = (stats.count > 0).then_some(stats.mean);
            }
            if requested.contains(&SegmentMetric::Stddev) {
                out.stddev = stats.stddev_population();
            }
            targets_out.push(out);
        }

        batch.aggregates.push(SegmentAggregateRecord {
            segment_key_channel: self.segment_key.clone(),
            segment_value: state.key_value,
            start_ts_ns: state.start_ts,
            end_ts_ns: state.end_ts,
            targets: targets_out,
        });
    }
}

impl Processor for SegmentAggregateProcessor {
    fn process(&mut self, batch: &mut EventBatch) {
        if batch.events.is_empty() && !batch.end_of_stream {
            return;
        }

        for idx in 0..batch.events.len() {
            let (channel, ts_ns, value) = {
                let event = &batch.events[idx];
                (event.channel.clone(), event.ts_ns, event.value)
            };
            if channel == self.segment_key {
                self.handle_segment_key(value, ts_ns, batch);
            } else {
                self.handle_value(&channel, value, ts_ns);
            }
        }

        if batch.end_of_stream && self.emit_last_segment_on_eof {
            self.flush_current(batch);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Event;

    fn make_batch(events: Vec<Event>, end_of_stream: bool) -> EventBatch {
        EventBatch {
            events,
            aggregates: Vec::new(),
            end_of_stream,
        }
    }

    #[test]
    fn aggregates_per_segment() {
        let mut processor = SegmentAggregateProcessor::new(
            "segment_id".into(),
            vec![SegmentTarget {
                channel: "value".into(),
                metrics: vec![
                    SegmentMetric::Count,
                    SegmentMetric::Min,
                    SegmentMetric::Max,
                    SegmentMetric::Mean,
                    SegmentMetric::Sum,
                    SegmentMetric::Stddev,
                ],
            }],
            true,
            true,
        );

        let events = vec![
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
                channel: "value".into(),
                ts_ns: 2,
                value: 2000.0,
            },
            Event {
                channel: "value".into(),
                ts_ns: 3,
                value: 3000.0,
            },
            Event {
                channel: "segment_id".into(),
                ts_ns: 4,
                value: 2.0,
            },
            Event {
                channel: "value".into(),
                ts_ns: 5,
                value: 4000.0,
            },
            Event {
                channel: "value".into(),
                ts_ns: 6,
                value: 5000.0,
            },
            Event {
                channel: "segment_id".into(),
                ts_ns: 7,
                value: 3.0,
            },
            Event {
                channel: "value".into(),
                ts_ns: 8,
                value: 6000.0,
            },
        ];

        let mut batch = make_batch(events, true);
        processor.process(&mut batch);

        assert_eq!(batch.aggregates.len(), 3);

        let seg1 = &batch.aggregates[0];
        assert_eq!(seg1.segment_value, 1.0);
        assert_eq!(seg1.start_ts_ns, Some(0));
        assert_eq!(seg1.end_ts_ns, Some(3));
        let m1 = &seg1.targets[0];
        assert_eq!(m1.count, Some(3));
        assert_eq!(m1.min, Some(1000.0));
        assert_eq!(m1.max, Some(3000.0));
        assert_eq!(m1.sum, Some(6000.0));
        assert!((m1.mean.unwrap() - 2000.0).abs() < 1e-9);
        assert!((m1.stddev.unwrap() - 816.496580927726).abs() < 1e-6);

        let seg2 = &batch.aggregates[1];
        assert_eq!(seg2.segment_value, 2.0);
        let m2 = &seg2.targets[0];
        assert_eq!(m2.count, Some(2));
        assert_eq!(m2.min, Some(4000.0));
        assert_eq!(m2.max, Some(5000.0));

        let seg3 = &batch.aggregates[2];
        assert_eq!(seg3.segment_value, 3.0);
        let m3 = &seg3.targets[0];
        assert_eq!(m3.count, Some(1));
        assert_eq!(m3.min, Some(6000.0));
        assert_eq!(m3.max, Some(6000.0));
        assert_eq!(m3.stddev, Some(0.0));
    }

    #[test]
    fn skips_non_finite_values() {
        let mut processor = SegmentAggregateProcessor::new(
            "segment_id".into(),
            vec![SegmentTarget {
                channel: "value".into(),
                metrics: vec![
                    SegmentMetric::Count,
                    SegmentMetric::Mean,
                    SegmentMetric::Min,
                ],
            }],
            true,
            true,
        );

        let events = vec![
            Event {
                channel: "segment_id".into(),
                ts_ns: 10,
                value: 1.0,
            },
            Event {
                channel: "value".into(),
                ts_ns: 11,
                value: f64::NAN,
            },
            Event {
                channel: "value".into(),
                ts_ns: 12,
                value: 10.0,
            },
        ];

        let mut batch = make_batch(events, true);
        processor.process(&mut batch);
        assert_eq!(batch.aggregates.len(), 1);
        let metrics = &batch.aggregates[0].targets[0];
        assert_eq!(metrics.count, Some(1));
        assert_eq!(metrics.min, Some(10.0));
        assert_eq!(metrics.mean, Some(10.0));
    }
}
