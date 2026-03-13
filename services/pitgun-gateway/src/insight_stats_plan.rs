use std::{collections::BTreeSet, path::Path};

use anyhow::Context;
use serde::Deserialize;

use crate::insight_ingress::sim_metric_dictionary;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatMetric {
    Count,
    Min,
    Max,
    Mean,
    Sum,
    Stddev,
}

impl StatMetric {
    pub fn suffix(self) -> &'static str {
        match self {
            Self::Count => "count",
            Self::Min => "min",
            Self::Max => "max",
            Self::Mean => "mean",
            Self::Sum => "sum",
            Self::Stddev => "stddev",
        }
    }

    fn from_manifest_value(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InsightStatsTarget {
    pub channel: String,
    pub metrics: Vec<StatMetric>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InsightStatsPlan {
    pub targets: Vec<InsightStatsTarget>,
}

impl InsightStatsPlan {
    pub fn default_sim_plan() -> Self {
        let targets = sim_metric_dictionary()
            .iter()
            .map(|entry| InsightStatsTarget {
                channel: entry.channel.to_string(),
                metrics: vec![
                    StatMetric::Count,
                    StatMetric::Min,
                    StatMetric::Max,
                    StatMetric::Mean,
                    StatMetric::Stddev,
                ],
            })
            .collect();
        Self { targets }
    }

    pub fn from_pipeline_manifest_path(path: &str) -> anyhow::Result<Self> {
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read insight manifest at {path}"))?;
        Self::from_pipeline_manifest_yaml(&raw)
            .with_context(|| format!("invalid insight manifest at {path}"))
    }

    pub fn from_pipeline_manifest_yaml(raw: &str) -> anyhow::Result<Self> {
        let parsed: PipelineManifest =
            serde_yaml::from_str(raw).context("manifest YAML parsing failed")?;

        let valid_channels: BTreeSet<String> = sim_metric_dictionary()
            .iter()
            .map(|entry| entry.channel.to_string())
            .collect();

        let mut allowlist = BTreeSet::new();
        for proc in &parsed.processors {
            if proc.r#type == "channel_filter" {
                for channel in proc.channels.clone().unwrap_or_default() {
                    if valid_channels.contains(&channel) {
                        allowlist.insert(channel);
                    }
                }
            }
        }
        let has_allowlist = !allowlist.is_empty();

        let segment = parsed
            .processors
            .iter()
            .find(|proc| proc.r#type == "segment_aggregate")
            .context("segment_aggregate processor is required for insight stats plan")?;

        let raw_targets = segment
            .targets
            .as_ref()
            .context("segment_aggregate.targets is required")?;

        let mut targets = Vec::new();
        for target in raw_targets {
            if !valid_channels.contains(&target.channel) {
                continue;
            }
            if has_allowlist && !allowlist.contains(&target.channel) {
                continue;
            }

            let mut metrics = target
                .metrics
                .clone()
                .unwrap_or_else(|| vec!["mean".to_string()])
                .into_iter()
                .filter_map(|name| StatMetric::from_manifest_value(&name))
                .collect::<Vec<_>>();

            if metrics.is_empty() {
                metrics.push(StatMetric::Mean);
            }

            targets.push(InsightStatsTarget {
                channel: target.channel.clone(),
                metrics,
            });
        }

        if targets.is_empty() {
            anyhow::bail!("insight stats plan has no valid sim.* targets");
        }

        Ok(Self { targets })
    }
}

pub fn resolve_insight_stats_plan(manifest_path: Option<&str>) -> anyhow::Result<InsightStatsPlan> {
    let Some(path) = manifest_path
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(InsightStatsPlan::default_sim_plan());
    };

    if !Path::new(path).exists() {
        anyhow::bail!("insight manifest path does not exist: {path}");
    }

    InsightStatsPlan::from_pipeline_manifest_path(path)
}

#[derive(Debug, Deserialize)]
struct PipelineManifest {
    processors: Vec<ProcessorConfig>,
}

#[derive(Clone, Debug, Deserialize)]
struct ProcessorConfig {
    #[serde(rename = "type")]
    r#type: String,
    channels: Option<Vec<String>>,
    targets: Option<Vec<SegmentTargetConfig>>,
}

#[derive(Clone, Debug, Deserialize)]
struct SegmentTargetConfig {
    channel: String,
    metrics: Option<Vec<String>>,
}

#[cfg(test)]
mod tests {
    use super::{InsightStatsPlan, StatMetric};

    #[test]
    fn parses_segment_aggregate_manifest_into_plan() {
        let raw = r#"
source:
  type: websocket
processors:
  - type: channel_filter
    channels: ["sim.speed_kph", "sim.rpm"]
  - type: segment_aggregate
    segment_key: "sim.time_s"
    targets:
      - channel: "sim.speed_kph"
        metrics: ["min", "max", "mean"]
      - channel: "sim.rpm"
        metrics: ["mean", "stddev"]
sink:
  type: console
"#;

        let plan =
            InsightStatsPlan::from_pipeline_manifest_yaml(raw).expect("manifest should parse");
        assert_eq!(plan.targets.len(), 2);
        assert_eq!(plan.targets[0].channel, "sim.speed_kph");
        assert_eq!(
            plan.targets[0].metrics,
            vec![StatMetric::Min, StatMetric::Max, StatMetric::Mean]
        );
        assert_eq!(plan.targets[1].channel, "sim.rpm");
        assert_eq!(
            plan.targets[1].metrics,
            vec![StatMetric::Mean, StatMetric::Stddev]
        );
    }

    #[test]
    fn defaults_to_mean_if_metrics_missing_or_invalid() {
        let raw = r#"
processors:
  - type: segment_aggregate
    targets:
      - channel: "sim.speed_kph"
        metrics: ["nope"]
"#;

        let plan =
            InsightStatsPlan::from_pipeline_manifest_yaml(raw).expect("manifest should parse");
        assert_eq!(plan.targets.len(), 1);
        assert_eq!(plan.targets[0].metrics, vec![StatMetric::Mean]);
    }

    #[test]
    fn parses_repository_example_manifest() {
        let raw = include_str!("../../../examples/manifests/pipeline/sim_insight_requests.yaml");
        let plan =
            InsightStatsPlan::from_pipeline_manifest_yaml(raw).expect("manifest should parse");
        assert!(!plan.targets.is_empty());
        assert!(
            plan.targets
                .iter()
                .any(|target| target.channel == "sim.speed_kph")
        );
    }
}
