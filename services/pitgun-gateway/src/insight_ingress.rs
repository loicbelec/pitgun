use std::collections::BTreeSet;

use pitgun_contract::TelemetryFrame;

use crate::model::TelemetrySampleBatchPayload;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SimMetricDef {
    pub parameter_id: u16,
    pub channel: &'static str,
    pub metric_key: &'static str,
    pub unit: &'static str,
}

const SIM_METRIC_DEFS: [SimMetricDef; 17] = [
    SimMetricDef {
        parameter_id: 5000,
        channel: "sim.time_s",
        metric_key: "sim.time_s",
        unit: "s",
    },
    SimMetricDef {
        parameter_id: 5001,
        channel: "sim.s_m",
        metric_key: "sim.distance_m",
        unit: "m",
    },
    SimMetricDef {
        parameter_id: 5002,
        channel: "sim.x_m",
        metric_key: "sim.position_x_m",
        unit: "m",
    },
    SimMetricDef {
        parameter_id: 5003,
        channel: "sim.y_m",
        metric_key: "sim.position_y_m",
        unit: "m",
    },
    SimMetricDef {
        parameter_id: 5004,
        channel: "sim.heading_rad",
        metric_key: "sim.heading_rad",
        unit: "rad",
    },
    SimMetricDef {
        parameter_id: 5005,
        channel: "sim.speed_kph",
        metric_key: "pace.speed_kph",
        unit: "kph",
    },
    SimMetricDef {
        parameter_id: 5006,
        channel: "sim.rpm",
        metric_key: "powertrain.rpm",
        unit: "rpm",
    },
    SimMetricDef {
        parameter_id: 5007,
        channel: "sim.gear",
        metric_key: "powertrain.gear",
        unit: "gear",
    },
    SimMetricDef {
        parameter_id: 5008,
        channel: "sim.throttle_pct",
        metric_key: "driver.throttle_pct",
        unit: "pct",
    },
    SimMetricDef {
        parameter_id: 5009,
        channel: "sim.brake_pct",
        metric_key: "driver.brake_pct",
        unit: "pct",
    },
    SimMetricDef {
        parameter_id: 5010,
        channel: "sim.g_lat",
        metric_key: "dynamics.g_lat",
        unit: "g",
    },
    SimMetricDef {
        parameter_id: 5011,
        channel: "sim.g_long",
        metric_key: "dynamics.g_long",
        unit: "g",
    },
    SimMetricDef {
        parameter_id: 5012,
        channel: "sim.g_vert",
        metric_key: "dynamics.g_vert",
        unit: "g",
    },
    SimMetricDef {
        parameter_id: 5013,
        channel: "sim.engine_temp_c",
        metric_key: "powertrain.engine_temp_c",
        unit: "c",
    },
    SimMetricDef {
        parameter_id: 5014,
        channel: "sim.engine_power_w",
        metric_key: "powertrain.engine_power_w",
        unit: "w",
    },
    SimMetricDef {
        parameter_id: 5015,
        channel: "sim.tire_temp_c",
        metric_key: "tires.avg_temp_c",
        unit: "c",
    },
    SimMetricDef {
        parameter_id: 5016,
        channel: "sim.tire_wear_pct",
        metric_key: "tires.avg_wear_pct",
        unit: "pct",
    },
];

#[derive(Clone, Debug, PartialEq)]
pub struct InsightMetricPoint {
    pub parameter_id: u16,
    pub channel: &'static str,
    pub metric_key: &'static str,
    pub unit: &'static str,
    pub value: f64,
    pub timestamp_us: i64,
    pub session_id: u64,
    pub source_id: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct InsightExtraction {
    pub points: Vec<InsightMetricPoint>,
    pub dropped_non_sim: usize,
    pub dropped_non_numeric: usize,
    pub dropped_bad_quality: usize,
    pub unknown_parameter_ids: BTreeSet<u16>,
}

pub fn sim_metric_dictionary() -> &'static [SimMetricDef] {
    &SIM_METRIC_DEFS
}

pub fn extract_sim_metric_points(payload: &TelemetrySampleBatchPayload) -> InsightExtraction {
    let mut extraction = InsightExtraction::default();

    for frame in &payload.frames {
        merge_extraction(&mut extraction, extract_sim_metric_points_from_frame(frame));
    }

    extraction
}

pub fn extract_sim_metric_points_from_frame(frame: &TelemetryFrame) -> InsightExtraction {
    let mut extraction = InsightExtraction::default();

    for sample in &frame.samples {
        let Some(metric) = sim_metric_by_parameter_id(sample.parameter_id) else {
            extraction.dropped_non_sim += 1;
            extraction.unknown_parameter_ids.insert(sample.parameter_id);
            continue;
        };

        if !sample.quality.is_usable() {
            extraction.dropped_bad_quality += 1;
            continue;
        }

        let Some(value) = sample.as_f64() else {
            extraction.dropped_non_numeric += 1;
            continue;
        };

        let timestamp_us = frame
            .timestamp_us
            .saturating_add(sample.timestamp_offset_us.unwrap_or(0) as i64);

        extraction.points.push(InsightMetricPoint {
            parameter_id: sample.parameter_id,
            channel: metric.channel,
            metric_key: metric.metric_key,
            unit: metric.unit,
            value,
            timestamp_us,
            session_id: frame.session_id,
            source_id: frame.source_id.clone(),
        });
    }

    extraction
}

fn merge_extraction(target: &mut InsightExtraction, mut source: InsightExtraction) {
    target.points.append(&mut source.points);
    target.dropped_non_sim += source.dropped_non_sim;
    target.dropped_non_numeric += source.dropped_non_numeric;
    target.dropped_bad_quality += source.dropped_bad_quality;
    target
        .unknown_parameter_ids
        .append(&mut source.unknown_parameter_ids);
}

fn sim_metric_by_parameter_id(parameter_id: u16) -> Option<&'static SimMetricDef> {
    SIM_METRIC_DEFS
        .iter()
        .find(|metric| metric.parameter_id == parameter_id)
}

#[cfg(test)]
mod tests {
    use pitgun_contract::{Sample, SampleValue, SignalQuality, TelemetryFrame};

    use super::{
        InsightMetricPoint, SimMetricDef, extract_sim_metric_points,
        extract_sim_metric_points_from_frame, sim_metric_dictionary,
    };
    use crate::model::TelemetrySampleBatchPayload;

    #[test]
    fn dictionary_is_strictly_sim_namespace() {
        let dict = sim_metric_dictionary();
        assert_eq!(dict.len(), 17);
        assert!(dict.iter().all(|entry| entry.channel.starts_with("sim.")));
    }

    #[test]
    fn extracts_only_mapped_sim_metrics() {
        let frame = TelemetryFrame {
            session_id: 4242,
            sequence: 7,
            timestamp_us: 1_770_000_001_000_000,
            received_at_us: 1_770_000_001_000_050,
            source_id: "pitgun-solver".to_string(),
            samples: vec![
                Sample::new(5005, SampleValue::F64(212.4), SignalQuality::Good),
                Sample::new(65000, SampleValue::F64(1.0), SignalQuality::Good),
            ],
            events: Vec::new(),
            cycle_index: Some(12),
            segment_index: Some(2),
            progress_m: Some(3021.4),
            metadata: Default::default(),
        };

        let extraction = extract_sim_metric_points_from_frame(&frame);

        assert_eq!(extraction.points.len(), 1);
        assert_eq!(extraction.dropped_non_sim, 1);
        assert!(extraction.unknown_parameter_ids.contains(&65000));
        assert_eq!(
            extraction.points[0],
            InsightMetricPoint {
                parameter_id: 5005,
                channel: "sim.speed_kph",
                metric_key: "pace.speed_kph",
                unit: "kph",
                value: 212.4,
                timestamp_us: 1_770_000_001_000_000,
                session_id: 4242,
                source_id: "pitgun-solver".to_string(),
            }
        );
    }

    #[test]
    fn drops_bad_quality_and_non_numeric_values() {
        let payload = TelemetrySampleBatchPayload {
            frames: vec![TelemetryFrame {
                session_id: 1,
                sequence: 1,
                timestamp_us: 100,
                received_at_us: 110,
                source_id: "test".to_string(),
                samples: vec![
                    Sample::new(
                        5006,
                        SampleValue::String("N/A".to_string()),
                        SignalQuality::Good,
                    ),
                    Sample::new(5008, SampleValue::F64(88.0), SignalQuality::Bad),
                    Sample::new(5009, SampleValue::F64(21.5), SignalQuality::Degraded),
                ],
                events: Vec::new(),
                cycle_index: None,
                segment_index: None,
                progress_m: None,
                metadata: Default::default(),
            }],
        };

        let extraction = extract_sim_metric_points(&payload);

        assert_eq!(extraction.points.len(), 1);
        assert_eq!(extraction.points[0].parameter_id, 5009);
        assert_eq!(extraction.dropped_non_numeric, 1);
        assert_eq!(extraction.dropped_bad_quality, 1);
    }

    #[test]
    fn dictionary_contains_expected_mapping_for_wear() {
        let dict = sim_metric_dictionary();
        let wear = dict
            .iter()
            .find(|entry| entry.parameter_id == 5016)
            .copied()
            .unwrap_or(SimMetricDef {
                parameter_id: 0,
                channel: "",
                metric_key: "",
                unit: "",
            });
        assert_eq!(wear.channel, "sim.tire_wear_pct");
        assert_eq!(wear.metric_key, "tires.avg_wear_pct");
        assert_eq!(wear.unit, "pct");
    }

    #[test]
    fn batch_extraction_merges_multiple_frames() {
        let payload = TelemetrySampleBatchPayload {
            frames: vec![
                TelemetryFrame {
                    session_id: 4242,
                    sequence: 1,
                    timestamp_us: 100,
                    received_at_us: 110,
                    source_id: "test".to_string(),
                    samples: vec![Sample::new(
                        5005,
                        SampleValue::F64(201.0),
                        SignalQuality::Good,
                    )],
                    events: Vec::new(),
                    cycle_index: Some(1),
                    segment_index: None,
                    progress_m: None,
                    metadata: Default::default(),
                },
                TelemetryFrame {
                    session_id: 4242,
                    sequence: 2,
                    timestamp_us: 200,
                    received_at_us: 210,
                    source_id: "test".to_string(),
                    samples: vec![Sample::new(
                        5006,
                        SampleValue::F64(12000.0),
                        SignalQuality::Good,
                    )],
                    events: Vec::new(),
                    cycle_index: Some(1),
                    segment_index: None,
                    progress_m: None,
                    metadata: Default::default(),
                },
            ],
        };

        let extraction = extract_sim_metric_points(&payload);
        assert_eq!(extraction.points.len(), 2);
        assert!(
            extraction
                .points
                .iter()
                .any(|point| point.parameter_id == 5005)
        );
        assert!(
            extraction
                .points
                .iter()
                .any(|point| point.parameter_id == 5006)
        );
    }
}
