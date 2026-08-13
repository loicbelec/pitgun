# Databricks notebook source
"""Diagnose exact opponent-audit snapshots without selecting a policy."""

# COMMAND ----------

import json
import re

import mlflow
from pyspark.sql import functions as F

from pitgun_databricks_adapter import (
    diagnose_opponent_audit,
    extract_opponent_audit_evidence,
    load_opponent_audit_campaign,
)


dbutils.widgets.text("catalog_name", "workspace")
dbutils.widgets.text("calibration_schema", "pitgun_calibration")
dbutils.widgets.text("campaigns_table_version", "")
dbutils.widgets.text("runs_table_version", "")
dbutils.widgets.text("metrics_table_version", "")

catalog_name = dbutils.widgets.get("catalog_name")
calibration_schema = dbutils.widgets.get("calibration_schema")
campaigns_version_text = dbutils.widgets.get("campaigns_table_version")
runs_version_text = dbutils.widgets.get("runs_table_version")
metrics_version_text = dbutils.widgets.get("metrics_table_version")


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
campaigns_version = validated_version("campaigns_table_version", campaigns_version_text)
runs_version = validated_version("runs_table_version", runs_version_text)
metrics_version = validated_version("metrics_table_version", metrics_version_text)
calibration = f"`{catalog_name}`.`{calibration_schema}`"
campaigns_table = f"{calibration}.campaigns"
runs_table = f"{calibration}.runs"
metrics_table = f"{calibration}.metrics"

manifest, manifest_digest = load_opponent_audit_campaign()
campaign_id = manifest["campaign_id"]
entry_by_key = {entry["run_key"]: entry for entry in manifest["runs"]}

# COMMAND ----------

run_rows = (
    spark.read.option("versionAsOf", runs_version)
    .table(runs_table)
    .where(F.col("campaign_id") == campaign_id)
    .select("execution_key", "execution_status", "run_id", "result_json")
    .collect()
)
if len(run_rows) != manifest["planned_run_count"]:
    raise RuntimeError("pinned runs snapshot does not contain the complete campaign")
if any(row["execution_status"] != "SUCCESS" for row in run_rows):
    raise RuntimeError("pinned runs snapshot contains a non-successful audit run")
if set(row["execution_key"] for row in run_rows) != set(entry_by_key):
    raise RuntimeError("pinned runs snapshot differs from the packaged manifest")

successful_run_ids = {row["run_id"] for row in run_rows}
metric_rows = (
    spark.read.option("versionAsOf", metrics_version)
    .table(metrics_table)
    .where(F.col("campaign_id") == campaign_id)
    .select("run_id", "metric_id")
    .collect()
)
metric_run_ids = {row["run_id"] for row in metric_rows}
if metric_run_ids != successful_run_ids:
    raise RuntimeError("pinned metrics snapshot does not cover the exact successful runs")
metrics_by_run = {}
for row in metric_rows:
    metrics_by_run.setdefault(row["run_id"], set()).add(row["metric_id"])
if any(len(metric_ids) != 10 for metric_ids in metrics_by_run.values()):
    raise RuntimeError("pinned metrics snapshot is incomplete or contains unexpected metrics")

campaign_rows = (
    spark.read.option("versionAsOf", campaigns_version)
    .table(campaigns_table)
    .where(F.col("campaign_id") == campaign_id)
    .collect()
)
if len(campaign_rows) != 1:
    raise RuntimeError("campaign ledger must contain exactly one audit campaign")
campaign = campaign_rows[0].asDict()
if campaign["manifest_digest"] != manifest_digest:
    raise RuntimeError("campaign ledger and packaged manifest digests differ")
if not campaign.get("mlflow_run_id"):
    raise RuntimeError("campaign has no MLflow run to receive the diagnosis")

evidence = [
    extract_opponent_audit_evidence(
        entry_by_key[row["execution_key"]],
        json.loads(row["result_json"]),
        manifest,
    )
    for row in run_rows
]
lineage = {
    "campaign_manifest_digest": manifest_digest,
    "source_git_revision": campaign["source_git_revision"],
    "mlflow_run_id": campaign["mlflow_run_id"],
    "delta_tables": {
        "campaigns": {"name": campaigns_table, "version": campaigns_version},
        "runs": {"name": runs_table, "version": runs_version},
        "metrics": {"name": metrics_table, "version": metrics_version},
    },
}
report = diagnose_opponent_audit(manifest, evidence, lineage)

# COMMAND ----------


def markdown_report(value):
    lines = [
        "# Racing V2 opponent diagnosis",
        "",
        f"- Successful runs: {value['sample']['successful_run_count']}",
        f"- Exact setup pairs: {value['sample']['setup_pair_count']}",
        f"- Strategy pairs: {value['sample']['strategy_pair_count']}",
        "- Policy selected: no",
        "",
    ]
    for title, key in (
        ("Evidence", "evidence"),
        ("Inference", "inference"),
        ("Unresolved", "unresolved"),
    ):
        lines.extend([f"## {title}", ""])
        lines.extend(f"- {statement}" for statement in value["claims"][key])
        lines.append("")
    return "\n".join(lines)


with mlflow.start_run(run_id=campaign["mlflow_run_id"]):
    mlflow.set_tags(
        {
            "pitgun.opponent_diagnosis": "racing-v2-v1",
            "pitgun.opponent_policy_selected": "false",
            "pitgun.automatic_promotion": "false",
        }
    )
    mlflow.log_metrics(
        {
            "diagnosis.setup_pair_count": report["sample"]["setup_pair_count"],
            "diagnosis.strategy_pair_count": report["sample"]["strategy_pair_count"],
            "diagnosis.setup_seed_stability_rate": report["setup_alignment"][
                "seed_direction_stability"
            ]["stable_group_rate"],
        }
    )
    mlflow.log_dict(report, "diagnosis/opponent-diagnosis-v1.json")
    mlflow.log_text(markdown_report(report), "diagnosis/opponent-diagnosis-v1.md")

dbutils.notebook.exit(json.dumps(report, sort_keys=True, separators=(",", ":")))
