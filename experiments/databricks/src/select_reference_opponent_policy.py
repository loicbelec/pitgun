# Databricks notebook source
"""Select a reproducible Racing opponent-policy proposal from pinned Delta versions."""

# COMMAND ----------

from datetime import datetime, timezone
import json
import re

from delta.tables import DeltaTable
import mlflow
from pyspark.sql import functions as F

from pitgun_databricks_adapter import (
    canonical_json,
    digest_json,
    load_reference_campaign,
    select_reference_policy,
)


dbutils.widgets.text("catalog_name", "workspace")
dbutils.widgets.text("calibration_schema", "pitgun_calibration")
dbutils.widgets.text("policies_schema", "pitgun_policies")
dbutils.widgets.text("campaigns_table_version", "3")
dbutils.widgets.text("runs_table_version", "2")
dbutils.widgets.text("metrics_table_version", "1")

catalog_name = dbutils.widgets.get("catalog_name")
calibration_schema = dbutils.widgets.get("calibration_schema")
policies_schema = dbutils.widgets.get("policies_schema")
campaigns_table_version = int(dbutils.widgets.get("campaigns_table_version"))
runs_table_version = int(dbutils.widgets.get("runs_table_version"))
metrics_table_version = int(dbutils.widgets.get("metrics_table_version"))


def validated_identifier(label: str, value: str) -> str:
    if not re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", value):
        raise ValueError(f"{label} is not a portable SQL identifier: {value!r}")
    return value


catalog_name = validated_identifier("catalog_name", catalog_name)
calibration_schema = validated_identifier("calibration_schema", calibration_schema)
policies_schema = validated_identifier("policies_schema", policies_schema)
for label, version in {
    "campaigns": campaigns_table_version,
    "runs": runs_table_version,
    "metrics": metrics_table_version,
}.items():
    if version < 0:
        raise ValueError(f"{label} Delta version must be pinned explicitly")

calibration_name = f"{catalog_name}.{calibration_schema}"
policies_name = f"{catalog_name}.{policies_schema}"
campaigns_table_name = f"{calibration_name}.campaigns"
runs_table_name = f"{calibration_name}.runs"
metrics_table_name = f"{calibration_name}.metrics"
candidates_table_name = f"{calibration_name}.candidates"
releases_table_name = f"{policies_name}.releases"

manifest, manifest_digest = load_reference_campaign()
campaign_id = manifest["campaign_id"]

# COMMAND ----------

campaign_rows = (
    spark.read.option("versionAsOf", campaigns_table_version)
    .table(campaigns_table_name)
    .where(F.col("campaign_id") == campaign_id)
    .collect()
)
if len(campaign_rows) != 1:
    raise RuntimeError(f"pinned campaign snapshot contains {len(campaign_rows)} matching rows")
campaign = campaign_rows[0].asDict()
if campaign["status"] != "COMPLETED":
    raise RuntimeError("pinned campaign is not complete")
if campaign["manifest_digest"] != manifest_digest:
    raise RuntimeError("pinned campaign manifest differs from the packaged immutable input")

run_rows = (
    spark.read.option("versionAsOf", runs_table_version)
    .table(runs_table_name)
    .where(F.col("campaign_id") == campaign_id)
    .collect()
)
if len(run_rows) != manifest["planned_run_count"]:
    raise RuntimeError("pinned runs snapshot does not contain the complete planned campaign")
if any(row["execution_status"] != "SUCCESS" for row in run_rows):
    raise RuntimeError("pinned runs snapshot contains a non-successful execution")

metric_rows = (
    spark.read.option("versionAsOf", metrics_table_version)
    .table(metrics_table_name)
    .where(F.col("campaign_id") == campaign_id)
    .where(
        F.col("metric_id").isin(
            "racing.lap-time",
            "racing.observed-maximum-speed",
        )
    )
    .collect()
)
metrics_by_run = {}
for row in metric_rows:
    values = metrics_by_run.setdefault(row["run_id"], {})
    if row["metric_id"] in values:
        raise RuntimeError(f"duplicate metric {row['metric_id']} for run {row['run_id']}")
    values[row["metric_id"]] = float(row["metric_value"])

families = {}
for row in run_rows:
    required_metrics = metrics_by_run.get(row["run_id"], {})
    if set(required_metrics) != {
        "racing.lap-time",
        "racing.observed-maximum-speed",
    }:
        raise RuntimeError(f"run {row['run_id']} lacks required selection metrics")
    family = families.setdefault(
        row["configuration_family"],
        {
            "configuration_family": row["configuration_family"],
            "configuration_id": row["configuration_id"],
            "setup": json.loads(row["setup_json"]),
            "strategy": json.loads(row["strategy_json"]),
            "seeds": set(),
            "lap_times": [],
            "maximum_speeds": [],
        },
    )
    if family["configuration_id"] != row["configuration_id"]:
        raise RuntimeError(f"family {row['configuration_family']} has multiple configurations")
    family["seeds"].add(row["seed"])
    family["lap_times"].append(required_metrics["racing.lap-time"])
    family["maximum_speeds"].append(
        required_metrics["racing.observed-maximum-speed"]
    )

summaries = []
for family in sorted(families.values(), key=lambda item: item["configuration_family"]):
    lap_times = family.pop("lap_times")
    maximum_speeds = family.pop("maximum_speeds")
    seeds = family.pop("seeds")
    mean_lap_time = sum(lap_times) / len(lap_times)
    variance = sum((value - mean_lap_time) ** 2 for value in lap_times) / len(lap_times)
    summaries.append(
        {
            **family,
            "successful_seed_count": len(seeds),
            "mean_lap_time_ms": mean_lap_time,
            "lap_time_stddev_ms": variance**0.5,
            "mean_maximum_speed_kmh": sum(maximum_speeds) / len(maximum_speeds),
        }
    )

lineage = {
    "campaign_id": campaign_id,
    "campaign_manifest_digest": manifest_digest,
    "runs_table_name": runs_table_name,
    "runs_table_version": runs_table_version,
    "metrics_table_name": metrics_table_name,
    "metrics_table_version": metrics_table_version,
    "mlflow_run_id": campaign["mlflow_run_id"],
    "source_git_revision": campaign["source_git_revision"],
    "model_digest": campaign["model_digest"],
    "data_pack_digest": campaign["data_pack_digest"],
    "circuit_model_id": manifest["circuit_id"],
    "era": manifest["era"],
}
policy = select_reference_policy(summaries=summaries, lineage=lineage)
artifact_digest = digest_json(policy)
candidate_set_digest = policy["calibration"]["candidate_set_digest"]

# COMMAND ----------

now = datetime.now(timezone.utc)
candidate_rows = [
    {
        "campaign_id": campaign_id,
        "candidate_id": profile["id"],
        "configuration_id": profile["source_configuration_id"],
        "circuit_id": policy["scope"]["circuit_model_id"],
        "era": policy["scope"]["era"],
        "difficulty_band": policy["scope"]["difficulty_band"],
        "selection_score": profile["selection_score"],
        "constraint_results_json": canonical_json(
            {
                "valid": True,
                "fair": True,
                "distinct_role": profile["role"],
                "successful_seed_count": profile["evidence"]["successful_seed_count"],
            }
        ),
        "candidate_rank": index + 1,
        "decision_state": "PROPOSED",
        "decision_reason": f"selected for deterministic {profile['role']} composition",
        "reviewed_at": None,
    }
    for index, profile in enumerate(policy["profiles"])
]
candidate_schema = """
  campaign_id STRING, candidate_id STRING, configuration_id STRING,
  circuit_id STRING, era INT, difficulty_band STRING, selection_score DOUBLE,
  constraint_results_json STRING, candidate_rank INT, decision_state STRING,
  decision_reason STRING, reviewed_at TIMESTAMP
"""
candidate_source = spark.createDataFrame(candidate_rows, candidate_schema)
(
    DeltaTable.forName(spark, candidates_table_name)
    .alias("target")
    .merge(
        candidate_source.alias("source"),
        "target.campaign_id = source.campaign_id "
        "AND target.candidate_id = source.candidate_id "
        "AND target.difficulty_band = source.difficulty_band",
    )
    .whenMatchedUpdateAll(condition="target.decision_state = 'PROPOSED'")
    .whenNotMatchedInsertAll()
    .execute()
)

artifact_uri = f"runs:/{campaign['mlflow_run_id']}/policies/racing-opponents-reference-v1.json"
release_row = {
    "policy_id": policy["policy"]["id"],
    "policy_version": policy["policy"]["version"],
    "artifact_digest": artifact_digest,
    "artifact_uri": artifact_uri,
    "source_campaign_id": campaign_id,
    "source_candidate_set_digest": candidate_set_digest,
    "target_catalog_id": "pitgun.racing",
    "target_catalog_version": "1.1.0",
    "release_state": "PROPOSED",
    "approved_by": None,
    "approved_at": None,
    "published_at": None,
}
release_schema = """
  policy_id STRING, policy_version STRING, artifact_digest STRING,
  artifact_uri STRING, source_campaign_id STRING,
  source_candidate_set_digest STRING, target_catalog_id STRING,
  target_catalog_version STRING, release_state STRING, approved_by STRING,
  approved_at TIMESTAMP, published_at TIMESTAMP
"""
release_source = spark.createDataFrame([release_row], release_schema)
(
    DeltaTable.forName(spark, releases_table_name)
    .alias("target")
    .merge(
        release_source.alias("source"),
        "target.policy_id = source.policy_id "
        "AND target.policy_version = source.policy_version",
    )
    .whenMatchedUpdateAll(condition="target.release_state = 'PROPOSED'")
    .whenNotMatchedInsertAll()
    .execute()
)

with mlflow.start_run(run_id=campaign["mlflow_run_id"]):
    mlflow.log_dict(policy, "policies/racing-opponents-reference-v1.json")
    mlflow.set_tags(
        {
            "pitgun.opponent_policy_id": policy["policy"]["id"],
            "pitgun.opponent_policy_version": policy["policy"]["version"],
            "pitgun.opponent_policy_digest": artifact_digest,
        }
    )

result = {
    "schema_version": "pitgun.opponent-policy-proposal-result/v1",
    "artifact_digest": artifact_digest,
    "candidate_set_digest": candidate_set_digest,
    "release_state": "PROPOSED",
    "delta_versions": {
        "campaigns": campaigns_table_version,
        "runs": runs_table_version,
        "metrics": metrics_table_version,
    },
    "policy": policy,
}
dbutils.notebook.exit(canonical_json(result))
