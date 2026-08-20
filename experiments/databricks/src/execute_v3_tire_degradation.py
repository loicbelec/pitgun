# Databricks notebook source
"""Execute the immutable Racing V3 tire-degradation physics campaign."""

# COMMAND ----------

from collections import Counter, defaultdict
from datetime import datetime, timezone
import importlib.metadata
import json
import re
import statistics
import time

from delta.tables import DeltaTable
import mlflow
from pyspark.sql import functions as F

from pitgun_databricks_adapter import (
    execute_packaged_v3_tire_degradation,
    inspect_packaged_v3_validation_probe,
    load_tire_degradation_campaign,
    materialize_tire_degradation_plan,
)


dbutils.widgets.text("catalog_name", "workspace")
dbutils.widgets.text("calibration_schema", "pitgun_calibration")
dbutils.widgets.text("experiment_id", "")
dbutils.widgets.text("campaign_name", "racing-v3-tire-degradation-v1")

catalog_name = dbutils.widgets.get("catalog_name")
calibration_schema = dbutils.widgets.get("calibration_schema")
experiment_id = dbutils.widgets.get("experiment_id")
campaign_name = dbutils.widgets.get("campaign_name")


def validated_identifier(label: str, value: str) -> str:
    if not re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", value):
        raise ValueError(f"{label} is not a portable SQL identifier: {value!r}")
    return value


catalog_name = validated_identifier("catalog_name", catalog_name)
calibration_schema = validated_identifier("calibration_schema", calibration_schema)
if calibration_schema.lower() in {"default", "information_schema"}:
    raise ValueError("protected schema cannot be an experimental campaign target")
if not experiment_id:
    raise ValueError("experiment_id is required")

calibration = f"`{catalog_name}`.`{calibration_schema}`"
campaigns_table = f"{calibration}.campaigns"
runs_table = f"{calibration}.experimental_runs"
metrics_table = f"{calibration}.experimental_metrics"

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
  campaign_id STRING, experimental_configuration_id STRING,
  response_id STRING, response_digest STRING, seed STRING,
  experimental_execution_id STRING, scenario_digest STRING,
  adapter_version STRING, probe_artifact_digest STRING,
  canonical_result_digest STRING, source_git_revision STRING,
  circuit_id STRING, era INT, setup_json STRING, strategy_json STRING,
  execution_status STRING, failure_phase STRING, failure_code STRING,
  failure_message STRING, duration_ms BIGINT, result_json STRING,
  started_at TIMESTAMP, completed_at TIMESTAMP, ingested_at TIMESTAMP
"""
metric_row_schema = """
  campaign_id STRING, experimental_execution_id STRING,
  experimental_configuration_id STRING, response_id STRING, seed STRING,
  metric_id STRING, metric_value DOUBLE, metric_unit STRING,
  statistic STRING, recorded_at TIMESTAMP
"""

manifest, manifest_digest = load_tire_degradation_campaign(campaign_name)
plan = materialize_tire_degradation_plan(manifest)
campaign_id = manifest["campaign_id"]
adapter_version = importlib.metadata.version("pitgun-databricks-adapter")
source_git_revision = adapter_version.split("+g", 1)[-1]
probe_identity = inspect_packaged_v3_validation_probe()

# COMMAND ----------

required_tables = {"campaigns", "experimental_runs", "experimental_metrics"}
actual_tables = {row[1] for row in spark.sql(f"SHOW TABLES IN {calibration}").collect()}
missing_tables = required_tables - actual_tables
if missing_tables:
    raise RuntimeError(
        "run bootstrap first; missing tables: " + ", ".join(sorted(missing_tables))
    )

existing_rows = (
    spark.table(campaigns_table).where(F.col("campaign_id") == campaign_id).collect()
)
if len(existing_rows) > 1:
    raise RuntimeError("campaign ledger contains duplicate keys")
existing_campaign = existing_rows[0].asDict() if existing_rows else None
if existing_campaign:
    immutable = {
        "manifest_digest": manifest_digest,
        "question": manifest["question"],
        "parameter_space_version": manifest["parameter_space_version"],
        "model_digest": manifest["model"]["digest"],
        "data_pack_digest": manifest["data_pack"]["digest"],
        "planned_run_count": manifest["planned_run_count"],
    }
    mismatches = {
        key: [existing_campaign.get(key), value]
        for key, value in immutable.items()
        if existing_campaign.get(key) != value
    }
    if mismatches:
        raise RuntimeError(
            "V3 physics campaign changed after execution began: "
            + json.dumps(mismatches, sort_keys=True)
        )

existing_mlflow_run_id = (
    existing_campaign.get("mlflow_run_id") if existing_campaign else None
)
tracking_context = (
    mlflow.start_run(run_id=existing_mlflow_run_id)
    if existing_mlflow_run_id
    else mlflow.start_run(
        experiment_id=experiment_id,
        run_name=campaign_id,
        tags={
            "pitgun.campaign_id": campaign_id,
            "pitgun.manifest_digest": manifest_digest,
            "pitgun.execution_class": "experimental-v3-physics",
            "pitgun.promotion_policy": "human-review-required",
        },
    )
)

# COMMAND ----------

with tracking_context as tracking_run:
    now = datetime.now(timezone.utc)
    campaign_row = {
        "campaign_id": campaign_id,
        "manifest_digest": manifest_digest,
        "question": manifest["question"],
        "parameter_space_version": manifest["parameter_space_version"],
        "scenario_id": manifest["scenario"]["id"],
        "scenario_version": manifest["scenario"]["version"],
        "scenario_digest": None,
        "model_id": manifest["model"]["id"],
        "model_version": manifest["model"]["version"],
        "model_digest": manifest["model"]["digest"],
        "data_pack_id": manifest["data_pack"]["id"],
        "data_pack_version": manifest["data_pack"]["version"],
        "data_pack_digest": manifest["data_pack"]["digest"],
        "runner_version": "v3-validation-probe/v1",
        "source_git_revision": source_git_revision,
        "status": "RUNNING",
        "planned_run_count": manifest["planned_run_count"],
        "created_at": existing_campaign.get("created_at", now)
        if existing_campaign
        else now,
        "updated_at": now,
        "mlflow_run_id": tracking_run.info.run_id,
        "completed_at": None,
    }
    campaign_source = spark.createDataFrame([campaign_row], campaign_row_schema)
    (
        DeltaTable.forName(spark, campaigns_table)
        .alias("target")
        .merge(
            campaign_source.alias("source"),
            "target.campaign_id = source.campaign_id",
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

    if not existing_mlflow_run_id:
        mlflow.log_params(
            {
                "campaign_id": campaign_id,
                "manifest_digest": manifest_digest,
                "planned_execution_count": manifest["planned_run_count"],
                "circuit_count": len(manifest["circuits"]),
                "vehicle_count": len(manifest["vehicles"]),
                "probe_artifact_digest": probe_identity["digest"],
                "adapter_version": adapter_version,
            }
        )
        mlflow.log_dict(manifest, "inputs/v3-tire-degradation-manifest.json")

    existing = (
        spark.table(runs_table)
        .where(F.col("campaign_id") == campaign_id)
        .select("experimental_configuration_id", "seed", "execution_status")
        .collect()
    )
    natural_keys = {
        (row["experimental_configuration_id"], row["seed"]) for row in existing
    }
    if len(existing) != len(natural_keys):
        raise RuntimeError("experimental ledger contains duplicate natural keys")
    planned_keys = {
        (row["expected_experimental_configuration_id"], row["seed"])
        for row in plan
    }
    if natural_keys - planned_keys:
        raise RuntimeError("experimental ledger contains unplanned rows")
    accepted = {
        (row["experimental_configuration_id"], row["seed"])
        for row in existing
        if row["execution_status"] == "SUCCESS"
    }

    run_rows = []
    metric_rows = []
    for entry in plan:
        natural_key = (entry["expected_experimental_configuration_id"], entry["seed"])
        if natural_key in accepted:
            continue
        started_at = datetime.now(timezone.utc)
        invocation_started = time.perf_counter_ns()
        status, phase, code, message = "FAILED", None, None, None
        execution_id, result_json, adapter_result = None, None, None
        try:
            adapter_result = execute_packaged_v3_tire_degradation(
                entry["id"], campaign_name
            )
            result = adapter_result["result"]
            expected = {
                "experimental_execution_id": entry[
                    "expected_experimental_execution_id"
                ],
                "scenario_digest": entry["expected_scenario_digest"],
                "profile_digest": entry["expected_profile_digest"],
                "seed": entry["seed"],
                "model": manifest["model"],
            }
            mismatches = {
                key: result.get(key)
                for key, value in expected.items()
                if result.get(key) != value
            }
            if mismatches:
                raise ValueError(
                    "V3 probe identity differs from immutable plan: "
                    + json.dumps(mismatches, sort_keys=True)
                )
            diagnostics = result.get("tire_degradation_diagnostics")
            if not isinstance(diagnostics, dict):
                raise ValueError("V3 result contains no tire-degradation lineage")
            execution_id = result["experimental_execution_id"]
            status = "SUCCESS"
            result_json = json.dumps(result, sort_keys=True, separators=(",", ":"))
            recorded_at = datetime.now(timezone.utc)
            metrics = (
                ("racing.total-time", result["total_time_ms"], "ms", "total"),
                (
                    "racing.maximum-speed",
                    result["observed_maximum_speed_kph"],
                    "km/h",
                    "maximum",
                ),
                (
                    "racing.final-tire-wear",
                    result["final_tire_wear_pct"],
                    "percent",
                    "final",
                ),
                (
                    "racing.requested-baseline-wear",
                    diagnostics["requested_baseline_wear_fraction"],
                    "fraction",
                    "total",
                ),
                (
                    "racing.requested-workload-wear",
                    diagnostics["requested_workload_wear_fraction"],
                    "fraction",
                    "total",
                ),
                (
                    "racing.maximum-thermal-wear-multiplier",
                    diagnostics["maximum_thermal_wear_multiplier"],
                    "ratio",
                    "maximum",
                ),
            )
            for metric_id, value, unit, statistic in metrics:
                metric_rows.append(
                    {
                        "campaign_id": campaign_id,
                        "experimental_execution_id": execution_id,
                        "experimental_configuration_id": entry[
                            "expected_experimental_configuration_id"
                        ],
                        "response_id": entry["response_id"],
                        "seed": entry["seed"],
                        "metric_id": metric_id,
                        "metric_value": float(value),
                        "metric_unit": unit,
                        "statistic": statistic,
                        "recorded_at": recorded_at,
                    }
                )
        except ValueError as error:
            status, phase, code, message = (
                "INVALID",
                "IDENTITY",
                "immutable_identity_mismatch",
                str(error)[:2000],
            )
        except Exception as error:
            status, phase, code, message = (
                "FAILED",
                "EXECUTION",
                type(error).__name__,
                str(error)[:2000],
            )

        completed_at = datetime.now(timezone.utc)
        strategy = entry["scenario"]["request"]["competitors"][0].get(
            "stint_strategy", {}
        )
        run_rows.append(
            {
                "campaign_id": campaign_id,
                "experimental_configuration_id": entry[
                    "expected_experimental_configuration_id"
                ],
                "response_id": entry["response_id"],
                "response_digest": entry["expected_profile_digest"],
                "seed": entry["seed"],
                "experimental_execution_id": execution_id,
                "scenario_digest": entry["expected_scenario_digest"],
                "adapter_version": adapter_version,
                "probe_artifact_digest": probe_identity["digest"],
                "canonical_result_digest": adapter_result["canonical_result_digest"]
                if adapter_result
                else None,
                "source_git_revision": source_git_revision,
                "circuit_id": entry["circuit_id"],
                "era": entry["era"],
                "setup_json": json.dumps(
                    entry["metadata"], sort_keys=True, separators=(",", ":")
                ),
                "strategy_json": json.dumps(
                    strategy, sort_keys=True, separators=(",", ":")
                ),
                "execution_status": status,
                "failure_phase": phase,
                "failure_code": code,
                "failure_message": message,
                "duration_ms": (time.perf_counter_ns() - invocation_started)
                // 1_000_000,
                "result_json": result_json,
                "started_at": started_at,
                "completed_at": completed_at,
                "ingested_at": completed_at,
            }
        )

    if run_rows:
        source = spark.createDataFrame(run_rows, run_row_schema)
        (
            DeltaTable.forName(spark, runs_table)
            .alias("target")
            .merge(
                source.alias("source"),
                "target.campaign_id = source.campaign_id AND target.experimental_configuration_id = source.experimental_configuration_id AND target.seed = source.seed",
            )
            .whenMatchedUpdateAll(condition="target.execution_status <> 'SUCCESS'")
            .whenNotMatchedInsertAll()
            .execute()
        )
    if metric_rows:
        source = spark.createDataFrame(metric_rows, metric_row_schema)
        (
            DeltaTable.forName(spark, metrics_table)
            .alias("target")
            .merge(
                source.alias("source"),
                "target.experimental_execution_id = source.experimental_execution_id AND target.metric_id = source.metric_id",
            )
            .whenNotMatchedInsertAll()
            .execute()
        )

    persisted = (
        spark.table(runs_table)
        .where(F.col("campaign_id") == campaign_id)
        .select("execution_status", "setup_json", "result_json")
        .collect()
    )
    counts = Counter(row["execution_status"] for row in persisted)
    if len(persisted) != manifest["planned_run_count"]:
        raise RuntimeError("V3 experimental ledger does not reconcile with the plan")

    family_durations = defaultdict(list)
    final_wear_by_compound = defaultdict(list)
    for row in persisted:
        if row["execution_status"] != "SUCCESS":
            continue
        metadata = json.loads(row["setup_json"])
        result = json.loads(row["result_json"])
        family_durations[metadata["family"]].append(float(result["total_time_ms"]))
        if metadata.get("family") == "compound.longrun":
            final_wear_by_compound[metadata["compound"]].append(
                float(result["final_tire_wear_pct"])
            )

    completed = counts["SUCCESS"] == manifest["planned_run_count"]
    report = {
        "schema_version": "pitgun.racing-v3-tire-degradation-databricks-report/v1",
        "campaign_id": campaign_id,
        "manifest_digest": manifest_digest,
        "status": "COMPLETED" if completed else "FAILED",
        "planned_execution_count": manifest["planned_run_count"],
        "terminal_counts": dict(counts),
        "family_mean_total_time_ms": {
            family: statistics.fmean(values)
            for family, values in sorted(family_durations.items())
        },
        "compound_mean_final_wear_pct": {
            compound: statistics.fmean(values)
            for compound, values in sorted(final_wear_by_compound.items())
        },
        "decision": "REVIEW_REQUIRED" if completed else "REJECTED",
        "automatic_catalog_promotion": False,
    }
    mlflow.log_metrics(
        {
            "successful_execution_count": counts["SUCCESS"],
            "invalid_execution_count": counts["INVALID"],
            "failed_execution_count": counts["FAILED"],
        }
    )
    mlflow.log_dict(report, "reports/v3-tire-degradation.json")

    completed_at = datetime.now(timezone.utc)
    completion_source = spark.createDataFrame(
        [
            {
                "campaign_id": campaign_id,
                "status": report["status"],
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
            completion_source.alias("source"),
            "target.campaign_id = source.campaign_id",
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
    raise RuntimeError(f"V3 tire-degradation campaign did not complete: {report_json}")
dbutils.notebook.exit(report_json)
