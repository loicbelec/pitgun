use crate::insight_ingress::sim_metric_dictionary;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
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
}
