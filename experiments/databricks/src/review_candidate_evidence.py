# Databricks notebook source
"""Review an exact Delta snapshot of the aerodynamic candidate campaign."""

# COMMAND ----------

import json
import re

import mlflow
from pyspark.sql import functions as F

from pitgun_databricks_adapter import (
    load_calibration_campaign,
    load_candidate_review_policy,
    review_candidate_evidence,
)


dbutils.widgets.text("catalog_name", "workspace")
dbutils.widgets.text("calibration_schema", "pitgun_calibration")
dbutils.widgets.text("campaign_name", "racing-aero-candidate-validation-v1")
dbutils.widgets.text("review_policy_name", "racing-aero-candidate-review-v1")
dbutils.widgets.text("experimental_runs_table_version", "")
dbutils.widgets.text("experimental_metrics_table_version", "")

catalog_name = dbutils.widgets.get("catalog_name")
calibration_schema = dbutils.widgets.get("calibration_schema")
campaign_name = dbutils.widgets.get("campaign_name")
review_policy_name = dbutils.widgets.get("review_policy_name")
runs_version_text = dbutils.widgets.get("experimental_runs_table_version")
metrics_version_text = dbutils.widgets.get("experimental_metrics_table_version")


def validated_identifier(label: str, value: str) -> str:
    if not re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", value):
        raise ValueError(f"{label} is not a portable SQL identifier: {value!r}")
    return value


def validated_version(label: str, value: str) -> int:
    if not value.isdigit():
        raise ValueError(f"{label} must be one explicit non-negative Delta version")
    return int(value)


catalog_name = validated_identifier("catalog_name", catalog_name)
calibration_schema = validated_identifier("calibration_schema", calibration_schema)
runs_version = validated_version("experimental_runs_table_version", runs_version_text)
metrics_version = validated_version(
    "experimental_metrics_table_version", metrics_version_text
)
calibration = f"`{catalog_name}`.`{calibration_schema}`"
campaigns_table = f"{calibration}.campaigns"
runs_table = f"{calibration}.experimental_runs"
metrics_table = f"{calibration}.experimental_metrics"

manifest, manifest_digest = load_calibration_campaign(campaign_name)
policy, policy_digest = load_candidate_review_policy(review_policy_name)
campaign_id = manifest["campaign_id"]

# COMMAND ----------

runs = (
    spark.read.option("versionAsOf", runs_version)
    .table(runs_table)
    .where(F.col("campaign_id") == campaign_id)
    .select(
        "experimental_configuration_id",
        "seed",
        "execution_status",
        "result_json",
    )
    .collect()
)
metrics = (
    spark.read.option("versionAsOf", metrics_version)
    .table(metrics_table)
    .where(F.col("campaign_id") == campaign_id)
    .select("experimental_execution_id", "metric_id")
)
metric_row_count = metrics.count()
metric_execution_count = metrics.select("experimental_execution_id").distinct().count()
if metric_execution_count != manifest["planned_run_count"]:
    raise RuntimeError("pinned experimental metrics do not cover every execution")

campaign_rows = (
    spark.table(campaigns_table).where(F.col("campaign_id") == campaign_id).collect()
)
if len(campaign_rows) != 1:
    raise RuntimeError("campaign ledger must contain exactly one reviewed campaign")
campaign = campaign_rows[0].asDict()
if campaign["manifest_digest"] != manifest_digest:
    raise RuntimeError("campaign ledger and packaged manifest digests differ")
if not campaign.get("mlflow_run_id"):
    raise RuntimeError("campaign has no MLflow run to receive the review")

evidence_versions = {
    "experimental_runs": runs_version,
    "experimental_metrics": metrics_version,
}
report = review_candidate_evidence(
    manifest,
    manifest_digest,
    policy,
    policy_digest,
    [row.asDict() for row in runs],
    evidence_versions,
)
report["experimental_metric_row_count"] = metric_row_count
report["experimental_metric_execution_count"] = metric_execution_count

# COMMAND ----------

with mlflow.start_run(run_id=campaign["mlflow_run_id"]):
    mlflow.set_tags(
        {
            "pitgun.candidate_review_policy": policy["id"],
            "pitgun.candidate_review_policy_digest": policy_digest,
            "pitgun.candidate_decision": report["decision"],
            "pitgun.automatic_promotion": "false",
        }
    )
    mlflow.log_metrics(
        {
            "review.success_rate": report["success_rate"],
            "review.physically_coherent_circuit_count": report[
                "physically_coherent_circuit_count"
            ],
            "review.refinement_reason_count": len(report["refinement_reasons"]),
            "review.hard_failure_count": len(report["hard_failures"]),
        }
    )
    mlflow.log_dict(policy, "reviews/aerodynamic-candidate-policy.json")
    mlflow.log_dict(report, "reviews/aerodynamic-candidate-evidence.json")

dbutils.notebook.exit(json.dumps(report, sort_keys=True, separators=(",", ":")))
