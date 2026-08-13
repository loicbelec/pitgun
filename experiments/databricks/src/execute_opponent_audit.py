# Databricks notebook source
"""Execute the immutable Racing V2 opponent audit into governed Delta tables."""

# COMMAND ----------

from collections import Counter
from datetime import datetime, timezone
import importlib.metadata
import json
import re
import time

from delta.tables import DeltaTable
import mlflow
from pyspark.sql import functions as F

from pitgun_databricks_adapter import (
    execute_packaged_racing_catalog_scenario,
    extract_opponent_audit_evidence,
    inspect_packaged_runner,
    load_opponent_audit_campaign,
    materialize_opponent_audit_plan,
    summarize_opponent_audit,
)


dbutils.widgets.text("catalog_name", "workspace")
dbutils.widgets.text("calibration_schema", "pitgun_calibration")
dbutils.widgets.text("experiment_id", "")

catalog_name = dbutils.widgets.get("catalog_name")
calibration_schema = dbutils.widgets.get("calibration_schema")
experiment_id = dbutils.widgets.get("experiment_id")


def validated_identifier(label: str, value: str) -> str:
    if not re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", value):
        raise ValueError(f"{label} is not a portable SQL identifier: {value!r}")
    return value


catalog_name = validated_identifier("catalog_name", catalog_name)
calibration_schema = validated_identifier("calibration_schema", calibration_schema)
if calibration_schema.lower() in {"default", "information_schema"}:
    raise ValueError(
        f"protected schema cannot be a campaign target: {calibration_schema!r}"
    )
if not experiment_id:
    raise ValueError("experiment_id is required")

calibration = f"`{catalog_name}`.`{calibration_schema}`"
campaigns_table = f"{calibration}.campaigns"
runs_table = f"{calibration}.runs"
metrics_table = f"{calibration}.metrics"

campaign_row_schema = """
  campaign_id STRING, manifest_digest STRING, question STRING,
  parameter_space_version STRING, scenario_id STRING, scenario_version STRING,
  scenario_digest STRING, model_id STRING, model_version STRING,
  model_digest STRING, data_pack_id STRING, data_pack_version STRING,
  data_pack_digest STRING, runner_version STRING, source_git_revision STRING,
  status STRING, planned_run_count BIGINT, created_at TIMESTAMP,
  updated_at TIMESTAMP, mlflow_run_id STRING, completed_at TIMESTAMP
"""
run_row_schema = """
  campaign_id STRING, execution_key STRING, configuration_id STRING,
  configuration_family STRING, seed STRING, run_id STRING,
  scenario_id STRING, scenario_version STRING, scenario_digest STRING,
  model_id STRING, model_version STRING, model_digest STRING,
  data_pack_id STRING, data_pack_version STRING, data_pack_digest STRING,
  runner_version STRING, adapter_version STRING, runner_artifact_digest STRING,
  canonical_result_digest STRING, source_git_revision STRING,
  circuit_id STRING, era INT, progression STRING, strategy_profile STRING,
  player_reference STRING, source_contract_digest STRING,
  setup_json STRING, strategy_json STRING, execution_status STRING,
  failure_phase STRING, failure_code STRING, failure_message STRING,
  duration_ms BIGINT, result_json STRING, started_at TIMESTAMP,
  completed_at TIMESTAMP, ingested_at TIMESTAMP
"""
metric_row_schema = """
  campaign_id STRING, run_id STRING, configuration_id STRING, seed STRING,
  metric_id STRING, metric_value DOUBLE, metric_unit STRING,
  sample_count BIGINT, statistic STRING, recorded_at TIMESTAMP
"""

manifest, manifest_digest = load_opponent_audit_campaign()
plan = materialize_opponent_audit_plan(manifest)
campaign_id = manifest["campaign_id"]
adapter_version = importlib.metadata.version("pitgun-databricks-adapter")
source_git_revision = (
    adapter_version.split("+g", maxsplit=1)[1]
    if "+g" in adapter_version
    else adapter_version
)
runner_identity = inspect_packaged_runner()

# COMMAND ----------

required_tables = {"campaigns", "runs", "metrics"}
actual_tables = {row[1] for row in spark.sql(f"SHOW TABLES IN {calibration}").collect()}
missing_tables = required_tables - actual_tables
if missing_tables:
    raise RuntimeError(
        "run bootstrap before the audit; missing tables: "
        + ", ".join(sorted(missing_tables))
    )

required_run_columns = {
    "execution_key",
    "progression",
    "strategy_profile",
    "player_reference",
    "source_contract_digest",
}
actual_run_columns = {field.name for field in spark.table(runs_table).schema.fields}
missing_run_columns = required_run_columns - actual_run_columns
if missing_run_columns:
    raise RuntimeError(
        "bootstrap did not evolve runs table: " + ", ".join(sorted(missing_run_columns))
    )

existing_campaign_rows = (
    spark.table(campaigns_table).where(F.col("campaign_id") == campaign_id).collect()
)
if len(existing_campaign_rows) > 1:
    raise RuntimeError(f"campaign ledger contains duplicate key: {campaign_id}")
existing_campaign = (
    existing_campaign_rows[0].asDict() if existing_campaign_rows else None
)
if existing_campaign:
    immutable_expectations = {
        "manifest_digest": manifest_digest,
        "question": manifest["question"],
        "parameter_space_version": manifest["schema_version"],
        "model_digest": manifest["catalog"]["model_digest"],
        "data_pack_digest": manifest["catalog"]["simulation_pack_digest"],
        "planned_run_count": manifest["planned_run_count"],
    }
    mismatches = {
        field: {"stored": existing_campaign.get(field), "expected": expected}
        for field, expected in immutable_expectations.items()
        if existing_campaign.get(field) != expected
    }
    if mismatches:
        raise RuntimeError(
            "campaign manifest cannot change after execution begins: "
            + json.dumps(mismatches, sort_keys=True)
        )

existing_mlflow_run_id = (
    existing_campaign.get("mlflow_run_id") if existing_campaign else None
)
is_new_tracking_run = not existing_mlflow_run_id
tracking_context = (
    mlflow.start_run(run_id=existing_mlflow_run_id)
    if existing_mlflow_run_id
    else mlflow.start_run(
        experiment_id=experiment_id,
        run_name=campaign_id,
        tags={
            "pitgun.campaign_id": campaign_id,
            "pitgun.manifest_digest": manifest_digest,
            "pitgun.domain": "racing-opponent-audit",
            "pitgun.automatic_promotion": "false",
        },
    )
)

# COMMAND ----------

campaign_started = time.perf_counter_ns()
with tracking_context as tracking_run:
    mlflow_run_id = tracking_run.info.run_id
    now = datetime.now(timezone.utc)
    catalog = manifest["catalog"]
    campaign_row = {
        "campaign_id": campaign_id,
        "manifest_digest": manifest_digest,
        "question": manifest["question"],
        "parameter_space_version": manifest["schema_version"],
        "scenario_id": "racing.opponent-audit-campaign",
        "scenario_version": "1.0.0",
        "scenario_digest": None,
        "model_id": catalog["model_id"],
        "model_version": catalog["model_version"],
        "model_digest": catalog["model_digest"],
        "data_pack_id": "pitgun.racing.simulation",
        "data_pack_version": catalog["version"],
        "data_pack_digest": catalog["simulation_pack_digest"],
        "runner_version": runner_identity["version"],
        "source_git_revision": source_git_revision,
        "status": "RUNNING",
        "planned_run_count": manifest["planned_run_count"],
        "created_at": existing_campaign.get("created_at", now)
        if existing_campaign
        else now,
        "updated_at": now,
        "mlflow_run_id": mlflow_run_id,
        "completed_at": None,
    }
    campaign_source = spark.createDataFrame([campaign_row], campaign_row_schema)
    (
        DeltaTable.forName(spark, campaigns_table)
        .alias("target")
        .merge(
            campaign_source.alias("source"), "target.campaign_id = source.campaign_id"
        )
        .whenMatchedUpdate(
            set={
                "status": "source.status",
                "updated_at": "source.updated_at",
                "mlflow_run_id": "source.mlflow_run_id",
            }
        )
        .whenNotMatchedInsertAll()
        .execute()
    )

    if is_new_tracking_run:
        mlflow.log_params(
            {
                "campaign_id": campaign_id,
                "manifest_digest": manifest_digest,
                "planned_run_count": manifest["planned_run_count"],
                "circuit_count": len({entry["circuit_id"] for entry in plan}),
                "progression_count": len({entry["progression"] for entry in plan}),
                "seed_count": len({entry["seed"] for entry in plan}),
                "strategy_profile_count": len(
                    {entry["strategy_profile"] for entry in plan}
                ),
                "player_reference_count": len(
                    {entry["player_reference"] for entry in plan}
                ),
                "runner_version": runner_identity["version"],
                "runner_artifact_digest": runner_identity["digest"],
                "adapter_version": adapter_version,
                "source_git_revision": source_git_revision,
                "automatic_game_or_catalog_promotion": False,
            }
        )
        mlflow.log_dict(manifest, "inputs/opponent-audit-manifest.json")

    existing_runs = (
        spark.table(runs_table)
        .where(F.col("campaign_id") == campaign_id)
        .select("execution_key", "execution_status")
        .collect()
    )
    existing_keys = [row["execution_key"] for row in existing_runs]
    if None in existing_keys or len(existing_keys) != len(set(existing_keys)):
        raise RuntimeError("opponent audit ledger contains invalid execution keys")
    planned_keys = {entry["run_key"] for entry in plan}
    unexpected_keys = set(existing_keys) - planned_keys
    if unexpected_keys:
        raise RuntimeError(
            f"campaign ledger contains unplanned rows: {unexpected_keys}"
        )
    accepted_keys = {
        row["execution_key"]
        for row in existing_runs
        if row["execution_status"] == "SUCCESS"
    }
    skipped_run_count = len(accepted_keys)
    run_rows = []
    metric_rows = []

    for entry in plan:
        if entry["run_key"] in accepted_keys:
            continue

        started_at = datetime.now(timezone.utc)
        invocation_started = time.perf_counter_ns()
        execution_status = "FAILED"
        failure_phase = None
        failure_code = None
        failure_message = None
        result = None
        evidence = None
        adapter_result = None

        try:
            adapter_result = execute_packaged_racing_catalog_scenario(
                int(entry["seed"]), entry["scenario_resource"]
            )
            result = adapter_result["result"]
            evidence = extract_opponent_audit_evidence(entry, result, manifest)
            execution_status = "SUCCESS"
            recorded_at = datetime.now(timezone.utc)
            for metric_id, (metric_value, metric_unit) in evidence["metrics"].items():
                metric_rows.append(
                    {
                        "campaign_id": campaign_id,
                        "run_id": result["run_id"],
                        "configuration_id": result["configuration_id"],
                        "seed": str(entry["seed"]),
                        "metric_id": metric_id,
                        "metric_value": metric_value,
                        "metric_unit": metric_unit,
                        "sample_count": 1,
                        "statistic": "observed",
                        "recorded_at": recorded_at,
                    }
                )
        except ValueError as error:
            execution_status = "INVALID"
            failure_phase = "IDENTITY"
            failure_code = "immutable_identity_mismatch"
            failure_message = str(error)[:2000]
        except Exception as error:
            execution_status = "FAILED"
            failure_phase = "EXECUTION"
            failure_code = type(error).__name__
            failure_message = str(error)[:2000]

        completed_at = datetime.now(timezone.utc)
        run_rows.append(
            {
                "campaign_id": campaign_id,
                "execution_key": entry["run_key"],
                "configuration_id": result["configuration_id"]
                if result
                else "unresolved",
                "configuration_family": entry["player_reference"],
                "seed": str(entry["seed"]),
                "run_id": result["run_id"] if result else None,
                "scenario_id": "racing.opponent-audit-campaign",
                "scenario_version": "1.0.0",
                "scenario_digest": result["scenario_digest"]
                if result
                else entry["scenario_resource_digest"],
                "model_id": catalog["model_id"],
                "model_version": catalog["model_version"],
                "model_digest": catalog["model_digest"],
                "data_pack_id": "pitgun.racing.simulation",
                "data_pack_version": catalog["version"],
                "data_pack_digest": catalog["simulation_pack_digest"],
                "runner_version": runner_identity["version"],
                "adapter_version": adapter_version,
                "runner_artifact_digest": runner_identity["digest"],
                "canonical_result_digest": adapter_result["canonical_result_digest"]
                if adapter_result
                else None,
                "source_git_revision": source_git_revision,
                "circuit_id": entry["circuit_id"],
                "era": int(entry["era"]),
                "progression": entry["progression"],
                "strategy_profile": entry["strategy_profile"],
                "player_reference": entry["player_reference"],
                "source_contract_digest": entry["source_contract_digest"],
                "setup_json": json.dumps(
                    {"controlled_reference": entry["player_reference"]},
                    sort_keys=True,
                    separators=(",", ":"),
                ),
                "strategy_json": json.dumps(
                    {"profile": entry["strategy_profile"]},
                    sort_keys=True,
                    separators=(",", ":"),
                ),
                "execution_status": execution_status,
                "failure_phase": failure_phase,
                "failure_code": failure_code,
                "failure_message": failure_message,
                "duration_ms": (time.perf_counter_ns() - invocation_started)
                // 1_000_000,
                "result_json": json.dumps(result, sort_keys=True, separators=(",", ":"))
                if result
                else None,
                "started_at": started_at,
                "completed_at": completed_at,
                "ingested_at": completed_at,
            }
        )

    if run_rows:
        run_source = spark.createDataFrame(run_rows, run_row_schema)
        (
            DeltaTable.forName(spark, runs_table)
            .alias("target")
            .merge(
                run_source.alias("source"),
                "target.campaign_id = source.campaign_id "
                "AND target.execution_key = source.execution_key",
            )
            .whenMatchedUpdateAll(condition="target.execution_status <> 'SUCCESS'")
            .whenNotMatchedInsertAll()
            .execute()
        )

    if metric_rows:
        metric_source = spark.createDataFrame(metric_rows, metric_row_schema)
        (
            DeltaTable.forName(spark, metrics_table)
            .alias("target")
            .merge(
                metric_source.alias("source"),
                "target.run_id = source.run_id AND target.metric_id = source.metric_id",
            )
            .whenNotMatchedInsertAll()
            .execute()
        )

    persisted_runs = (
        spark.table(runs_table)
        .where(F.col("campaign_id") == campaign_id)
        .select("execution_key", "execution_status", "result_json")
        .collect()
    )
    counts = Counter(row["execution_status"] for row in persisted_runs)
    planned_count = manifest["planned_run_count"]
    if len(persisted_runs) != planned_count:
        raise RuntimeError(
            f"campaign ledger has {len(persisted_runs)} rows; expected {planned_count}"
        )
    terminal_count = counts["SUCCESS"] + counts["INVALID"] + counts["FAILED"]
    if terminal_count != planned_count:
        raise RuntimeError("campaign terminal counts do not reconcile")

    entry_by_key = {entry["run_key"]: entry for entry in plan}
    successful_evidence = [
        extract_opponent_audit_evidence(
            entry_by_key[row["execution_key"]],
            json.loads(row["result_json"]),
            manifest,
        )
        for row in persisted_runs
        if row["execution_status"] == "SUCCESS"
    ]
    analysis = (
        summarize_opponent_audit(successful_evidence)
        if successful_evidence
        else {
            "schema_version": "pitgun.opponent-audit-report/v1",
            "overall": {},
            "references": {},
            "automatic_game_or_catalog_promotion": False,
        }
    )
    campaign_duration_ms = (time.perf_counter_ns() - campaign_started) // 1_000_000
    final_status = "COMPLETED" if counts["SUCCESS"] == planned_count else "PARTIAL"
    report = {
        **analysis,
        "campaign_id": campaign_id,
        "manifest_digest": manifest_digest,
        "mlflow_run_id": mlflow_run_id,
        "status": final_status,
        "planned_run_count": planned_count,
        "successful_run_count": counts["SUCCESS"],
        "invalid_run_count": counts["INVALID"],
        "failed_run_count": counts["FAILED"],
        "skipped_accepted_run_count": skipped_run_count,
        "attempted_run_count": len(run_rows),
        "campaign_duration_ms": campaign_duration_ms,
    }
    mlflow.log_metrics(
        {
            "planned_run_count": planned_count,
            "successful_run_count": counts["SUCCESS"],
            "invalid_run_count": counts["INVALID"],
            "failed_run_count": counts["FAILED"],
            "campaign_duration_ms": campaign_duration_ms,
        }
    )
    for reference, values in analysis.get("overall", {}).items():
        prefix = reference.replace("-", "_")
        for metric in (
            "win_rate",
            "podium_rate",
            "mean_position",
            "median_gap_to_leader_ms",
        ):
            mlflow.log_metric(f"{prefix}.{metric}", values[metric])
    mlflow.log_dict(report, "reports/opponent-audit-report.json")

    completed_at = datetime.now(timezone.utc)
    completion_source = spark.createDataFrame(
        [
            {
                "campaign_id": campaign_id,
                "status": final_status,
                "updated_at": completed_at,
                "completed_at": completed_at,
            }
        ],
        "campaign_id STRING, status STRING, updated_at TIMESTAMP, completed_at TIMESTAMP",
    )
    (
        DeltaTable.forName(spark, campaigns_table)
        .alias("target")
        .merge(
            completion_source.alias("source"), "target.campaign_id = source.campaign_id"
        )
        .whenMatchedUpdate(
            set={
                "status": "source.status",
                "updated_at": "source.updated_at",
                "completed_at": "source.completed_at",
            }
        )
        .execute()
    )

report_json = json.dumps(report, sort_keys=True, separators=(",", ":"))
if report["status"] != "COMPLETED":
    raise RuntimeError(f"opponent audit campaign did not complete: {report_json}")
dbutils.notebook.exit(report_json)
