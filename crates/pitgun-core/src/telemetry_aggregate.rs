//! Domain-neutral scalar aggregation over typed telemetry frames.

use std::fmt;

use pitgun_contract::{ParameterId, TelemetryFrame};

/// Aggregate operation with stable V1 semantics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TelemetryAggregateKind {
    /// Greatest finite usable numeric sample.
    Maximum,
}

/// Versioned-input configuration supplied by an application.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TelemetryAggregateConfig {
    pub parameter_id: ParameterId,
    pub kind: TelemetryAggregateKind,
}

/// Scalar result calculated exclusively from matching telemetry samples.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TelemetryAggregateResult {
    pub sample_count: u64,
    pub value: f64,
}

/// Failures produced by domain-neutral telemetry aggregation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TelemetryAggregateError {
    NoUsableSamples(ParameterId),
    SampleCountOverflow,
}

impl fmt::Display for TelemetryAggregateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoUsableSamples(parameter_id) => write!(
                formatter,
                "no finite usable numeric samples for parameter {parameter_id}"
            ),
            Self::SampleCountOverflow => {
                formatter.write_str("telemetry sample count overflowed u64")
            }
        }
    }
}

impl std::error::Error for TelemetryAggregateError {}

/// Aggregates one parameter across ordered typed telemetry frames.
///
/// V1 considers samples with `good` or `degraded` quality, converts numeric
/// values through [`pitgun_contract::SampleValue::as_f64`], and ignores values
/// that are absent, non-numeric, or non-finite.
pub fn aggregate_telemetry_parameter<'a>(
    frames: impl IntoIterator<Item = &'a TelemetryFrame>,
    config: TelemetryAggregateConfig,
) -> Result<TelemetryAggregateResult, TelemetryAggregateError> {
    let mut sample_count = 0_u64;
    let mut value: Option<f64> = None;

    for sample in frames
        .into_iter()
        .flat_map(|frame| frame.samples.iter())
        .filter(|sample| sample.parameter_id == config.parameter_id)
    {
        if !sample.quality.is_usable() {
            continue;
        }
        let Some(candidate) = sample.as_f64().filter(|candidate| candidate.is_finite()) else {
            continue;
        };
        sample_count = sample_count
            .checked_add(1)
            .ok_or(TelemetryAggregateError::SampleCountOverflow)?;
        value = Some(match (config.kind, value) {
            (TelemetryAggregateKind::Maximum, Some(current)) => current.max(candidate),
            (TelemetryAggregateKind::Maximum, None) => candidate,
        });
    }

    value
        .map(|value| TelemetryAggregateResult {
            sample_count,
            value,
        })
        .ok_or(TelemetryAggregateError::NoUsableSamples(
            config.parameter_id,
        ))
}

#[cfg(test)]
mod tests {
    use pitgun_contract::{Sample, SampleValue, SignalQuality, TelemetryFrame};

    use super::*;

    fn frame(sequence: u64, speed: f64, quality: SignalQuality) -> TelemetryFrame {
        TelemetryFrame::builder()
            .session_id(1)
            .sequence(sequence)
            .timestamp_us(sequence as i64)
            .received_at_us(sequence as i64)
            .source_id("test")
            .sample(Sample::new(5005, SampleValue::F64(speed), quality))
            .build()
    }

    #[test]
    fn maximum_uses_only_finite_usable_matching_samples() {
        let frames = [
            frame(0, 120.0, SignalQuality::Good),
            frame(1, 245.5, SignalQuality::Degraded),
            frame(2, 999.0, SignalQuality::Bad),
            frame(3, f64::NAN, SignalQuality::Good),
        ];

        let result = aggregate_telemetry_parameter(
            &frames,
            TelemetryAggregateConfig {
                parameter_id: 5005,
                kind: TelemetryAggregateKind::Maximum,
            },
        )
        .expect("maximum speed");

        assert_eq!(result.sample_count, 2);
        assert_eq!(result.value, 245.5);
    }

    #[test]
    fn relevant_sample_mutation_changes_the_result() {
        let baseline = [
            frame(0, 120.0, SignalQuality::Good),
            frame(1, 245.5, SignalQuality::Good),
        ];
        let mutated = [
            frame(0, 120.0, SignalQuality::Good),
            frame(1, 310.25, SignalQuality::Good),
        ];
        let config = TelemetryAggregateConfig {
            parameter_id: 5005,
            kind: TelemetryAggregateKind::Maximum,
        };

        let original = aggregate_telemetry_parameter(&baseline, config).expect("baseline");
        let changed = aggregate_telemetry_parameter(&mutated, config).expect("mutation");

        assert_eq!(original.value, 245.5);
        assert_eq!(changed.value, 310.25);
    }
}
