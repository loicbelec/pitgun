//! Versioned canonical derived-metric artifacts.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::Identifier;

const MAX_SAFE_JSON_INTEGER: u64 = 9_007_199_254_740_991;

/// Wire version of `metrics.json`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum DerivedMetricsVersion {
    #[serde(rename = "pitgun.derived-metrics/v1")]
    V1,
}

/// Exact reusable processor semantics used to calculate a metric.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum DerivedMetricProcessorV1 {
    #[serde(rename = "pitgun.telemetry-aggregate/v1")]
    TelemetryAggregateV1,
}

/// Aggregate selected by the versioned processor configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum DerivedMetricStatisticV1 {
    #[serde(rename = "maximum")]
    Maximum,
}

/// Configuration and result for one deterministic telemetry metric.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DerivedMetricV1 {
    pub id: Identifier,
    pub processor: DerivedMetricProcessorV1,
    pub parameter_id: u16,
    pub statistic: DerivedMetricStatisticV1,
    pub unit: String,
    pub sample_count: u64,
    pub value: f64,
}

/// Canonical collection stored as `metrics.json`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DerivedMetricsV1 {
    pub schema_version: DerivedMetricsVersion,
    pub metrics: Vec<DerivedMetricV1>,
}

/// Structural or numeric failure in a derived-metric artifact.
#[derive(Clone, Debug, PartialEq)]
pub enum DerivedMetricsError {
    Empty,
    InvalidOrder,
    InvalidUnit { id: Identifier, unit: String },
    InvalidSampleCount { id: Identifier, count: u64 },
    NonFiniteValue { id: Identifier },
}

impl fmt::Display for DerivedMetricsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("derived metrics must not be empty"),
            Self::InvalidOrder => {
                formatter.write_str("derived metric ids must be unique and strictly sorted")
            }
            Self::InvalidUnit { id, unit } => {
                write!(formatter, "metric {id} has invalid unit {unit:?}")
            }
            Self::InvalidSampleCount { id, count } => write!(
                formatter,
                "metric {id} has invalid exact JSON sample count {count}"
            ),
            Self::NonFiniteValue { id } => {
                write!(formatter, "metric {id} has a non-finite value")
            }
        }
    }
}

impl std::error::Error for DerivedMetricsError {}

impl DerivedMetricsV1 {
    /// Sorts metrics by stable id and validates the complete artifact.
    pub fn new(mut metrics: Vec<DerivedMetricV1>) -> Result<Self, DerivedMetricsError> {
        metrics.sort_by(|left, right| left.id.cmp(&right.id));
        let artifact = Self {
            schema_version: DerivedMetricsVersion::V1,
            metrics,
        };
        artifact.validate()?;
        Ok(artifact)
    }

    /// Validates deterministic ordering and portable scalar values.
    pub fn validate(&self) -> Result<(), DerivedMetricsError> {
        if self.metrics.is_empty() {
            return Err(DerivedMetricsError::Empty);
        }
        if self.metrics.windows(2).any(|pair| pair[0].id >= pair[1].id) {
            return Err(DerivedMetricsError::InvalidOrder);
        }
        for metric in &self.metrics {
            if metric.unit.is_empty()
                || metric.unit.len() > 32
                || !metric.unit.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'%' | b'.' | b'/' | b'_' | b'-')
                })
            {
                return Err(DerivedMetricsError::InvalidUnit {
                    id: metric.id.clone(),
                    unit: metric.unit.clone(),
                });
            }
            if metric.sample_count == 0 || metric.sample_count > MAX_SAFE_JSON_INTEGER {
                return Err(DerivedMetricsError::InvalidSampleCount {
                    id: metric.id.clone(),
                    count: metric.sample_count,
                });
            }
            if !metric.value.is_finite() {
                return Err(DerivedMetricsError::NonFiniteValue {
                    id: metric.id.clone(),
                });
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metric(id: &str, value: f64) -> DerivedMetricV1 {
        DerivedMetricV1 {
            id: Identifier::new(id).expect("metric id"),
            processor: DerivedMetricProcessorV1::TelemetryAggregateV1,
            parameter_id: 5005,
            statistic: DerivedMetricStatisticV1::Maximum,
            unit: "km/h".to_owned(),
            sample_count: 427,
            value,
        }
    }

    #[test]
    fn constructor_sorts_metrics_for_canonical_output() {
        let artifact =
            DerivedMetricsV1::new(vec![metric("metric.z", 2.0), metric("metric.a", 1.0)])
                .expect("valid metrics");
        assert_eq!(artifact.metrics[0].id.as_str(), "metric.a");
        assert_eq!(artifact.metrics[1].id.as_str(), "metric.z");
    }

    #[test]
    fn validation_rejects_duplicates_and_invalid_scalars() {
        assert_eq!(
            DerivedMetricsV1::new(vec![metric("metric.a", 1.0), metric("metric.a", 2.0)]),
            Err(DerivedMetricsError::InvalidOrder)
        );
        assert_eq!(
            DerivedMetricsV1::new(vec![metric("metric.a", f64::NAN)]),
            Err(DerivedMetricsError::NonFiniteValue {
                id: Identifier::new("metric.a").expect("metric id")
            })
        );
    }
}
