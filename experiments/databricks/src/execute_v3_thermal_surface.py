# Databricks notebook source
# ruff: noqa: F821
"""Execute the immutable Racing V3 thermal-adequacy campaign."""

# COMMAND ----------

from collections import Counter
import concurrent.futures
from datetime import datetime, timezone
import importlib.metadata
import json
import math
import re
import time

from delta.tables import DeltaTable
import mlflow
from pyspark.sql import functions as F

from pitgun_databricks_adapter import (
    execute_packaged_v3_thermal_surface,
    inspect_packaged_v3_decision_surface_probe,
    load_thermal_surface_campaign,
    materialize_thermal_surface_plan,
)


dbutils.widgets.text("catalog_name", "workspace")
dbutils.widgets.text("calibration_schema", "pitgun_calibration")
dbutils.widgets.text("experiment_id", "")
dbutils.widgets.text("campaign_name", "racing-v3-thermal-adequacy-v1")
dbutils.widgets.text("max_workers", "8")

catalog_name = dbutils.widgets.get("catalog_name")
calibration_schema = dbutils.widgets.get("calibration_schema")
experiment_id = dbutils.widgets.get("experiment_id")
campaign_name = dbutils.widgets.get("campaign_name")
max_workers = int(dbutils.widgets.get("max_workers"))


def validated_identifier(label: str, value: str) -> str:
    if not re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", value):
        raise ValueError(f"{label} is not a portable SQL identifier: {value!r}")
    return value


def thermal_metrics(result: dict) -> dict[str, float]:
    diagnostics = result["mechanical_diagnostics"]
    elapsed_s = float(result["total_time_ms"]) / 1000.0
    derated_time_s = float(diagnostics["engine_derated_time_s"])
    return {
        "total_time_ms": float(result["total_time_ms"]),
        "maximum_speed_kph": float(result["observed_maximum_speed_kph"]),
        "maximum_engine_temperature_c": float(
            diagnostics["maximum_engine_temperature_c"]
        ),
        "engine_derated_time_s": derated_time_s,
        "engine_derated_fraction": derated_time_s / elapsed_s if elapsed_s else 0.0,
        "generated_engine_heat_kj": float(diagnostics["generated_engine_heat_kj"]),
        "removed_engine_heat_kj": float(diagnostics["removed_engine_heat_kj"]),
        "fixed_drag_area_m2": float(diagnostics["fixed_drag_area_m2"]),
    }


def metrics_match(expected: dict, actual: dict) -> dict[str, list[float]]:
    return {
        key: [float(value), float(actual[key])]
        for key, value in expected.items()
        if not math.isclose(
            float(value), float(actual[key]), rel_tol=1e-9, abs_tol=1e-9
        )
    }


catalog_name = validated_identifier("catalog_name", catalog_name)
calibration_schema = validated_identifier("calibration_schema", calibration_schema)
if calibration_schema.lower() in {"default", "information_schema"}:
    raise ValueError("protected schema cannot be an experimental target")
if not experiment_id:
    raise ValueError("experiment_id is required")
if not 1 <= max_workers <= 16:
    raise ValueError("max_workers must be between 1 and 16")

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

manifest, manifest_digest = load_thermal_surface_campaign(campaign_name)
plan = materialize_thermal_surface_plan(manifest)
campaign_id = manifest["campaign_id"]
adapter_version = importlib.metadata.version("pitgun-databricks-adapter")
source_git_revision = adapter_version.split("+g", 1)[-1]
probe_identity = inspect_packaged_v3_decision_surface_probe()

# COMMAND ----------

required_tables = {"campaigns", "experimental_runs", "experimental_metrics"}
actual_tables = {row[1] for row in spark.sql(f"SHOW TABLES IN {calibration}").collect()}
missing_tables = required_tables - actual_tables
if missing_tables:
    raise RuntimeError("run bootstrap first; missing tables: " + ", ".join(sorted(missing_tables)))

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
        "model_digest": manifest["model"]["digest"],
        "data_pack_digest": manifest["source_evidence"]["base_profile_digest"],
        "planned_run_count": manifest["planned_run_count"],
    }
    mismatches = {
        key: [existing_campaign.get(key), value]
        for key, value in immutable.items()
        if existing_campaign.get(key) != value
    }
    if mismatches:
        raise RuntimeError(
            "thermal campaign changed after execution began: "
            + json.dumps(mismatches, sort_keys=True)
        )

existing_mlflow_run_id = existing_campaign.get("mlflow_run_id") if existing_campaign else None
tracking_context = (
    mlflow.start_run(run_id=existing_mlflow_run_id)
    if existing_mlflow_run_id
    else mlflow.start_run(
        experiment_id=experiment_id,
        run_name=campaign_id,
        tags={
            "pitgun.campaign_id": campaign_id,
            "pitgun.manifest_digest": manifest_digest,
            "pitgun.execution_class": manifest["execution_class"],
            "pitgun.promotion_policy": "human-review-required",
        },
    )
)

# COMMAND ----------


def execute_entry(entry: dict) -> dict:
    started_at = datetime.now(timezone.utc)
    started_ns = time.perf_counter_ns()
    try:
        adapter_result = execute_packaged_v3_thermal_surface(
            entry["execution_key"], campaign_name
        )
        result = adapter_result["result"]
        if result.get("model") != manifest["model"]:
            raise ValueError("Rust probe returned a different model identity")
        actual_metrics = thermal_metrics(result)
        expected = entry.get("expected_local_evidence")
        if expected:
            identity_mismatches = {
                key: [expected[key], result.get(key)]
                for key in (
                    "experimental_execution_id",
                    "scenario_digest",
                    "profile_digest",
                )
                if result.get(key) != expected[key]
            }
            metric_mismatches = metrics_match(expected["metrics"], actual_metrics)
            if identity_mismatches or metric_mismatches:
                return {
                    "entry": entry,
                    "started_at": started_at,
                    "duration_ms": (time.perf_counter_ns() - started_ns) // 1_000_000,
                    "status": "INVALID",
                    "phase": "PORTABLE_PARITY",
                    "code": "local_thermal_evidence_mismatch",
                    "message": json.dumps(
                        {"identity": identity_mismatches, "metrics": metric_mismatches},
                        sort_keys=True,
                    )[:2000],
                    "adapter_result": adapter_result,
                    "result": result,
                    "metrics": actual_metrics,
                }
        return {
            "entry": entry,
            "started_at": started_at,
            "duration_ms": (time.perf_counter_ns() - started_ns) // 1_000_000,
            "status": "SUCCESS",
            "phase": None,
            "code": None,
            "message": None,
            "adapter_result": adapter_result,
            "result": result,
            "metrics": actual_metrics,
        }
    except ValueError as error:
        status, phase, code = "INVALID", "IDENTITY", "immutable_identity_mismatch"
        message = str(error)[:2000]
    except Exception as error:
        status, phase, code = "FAILED", "EXECUTION", type(error).__name__
        message = str(error)[:2000]
    return {
        "entry": entry,
        "started_at": started_at,
        "duration_ms": (time.perf_counter_ns() - started_ns) // 1_000_000,
        "status": status,
        "phase": phase,
        "code": code,
        "message": message,
        "adapter_result": None,
        "result": None,
        "metrics": None,
    }


with tracking_context as tracking_run:
    now = datetime.now(timezone.utc)
    campaign_row = {
        "campaign_id": campaign_id,
        "manifest_digest": manifest_digest,
        "question": manifest["question"],
        "parameter_space_version": manifest.get(
            "parameter_space_version", "racing-v3-thermal-adequacy-v1"
        ),
        "scenario_id": "embedded-thermal-plan",
        "scenario_version": "v1",
        "scenario_digest": manifest["source_evidence"]["artifact_digest"],
        "model_id": manifest["model"]["id"],
        "model_version": manifest["model"]["version"],
        "model_digest": manifest["model"]["digest"],
        "data_pack_id": "pitgun.racing-v3-engine-thermal-profile",
        "data_pack_version": "v8",
        "data_pack_digest": manifest["source_evidence"]["base_profile_digest"],
        "runner_version": "v3-decision-surface-probe/v1",
        "source_git_revision": source_git_revision,
        "status": "RUNNING",
        "planned_run_count": manifest["planned_run_count"],
        "created_at": existing_campaign.get("created_at", now) if existing_campaign else now,
        "updated_at": now,
        "mlflow_run_id": tracking_run.info.run_id,
        "completed_at": None,
    }
    campaign_source = spark.createDataFrame([campaign_row], campaign_row_schema)
    (
        DeltaTable.forName(spark, campaigns_table)
        .alias("target")
        .merge(campaign_source.alias("source"), "target.campaign_id = source.campaign_id")
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
                "local_replay_count": manifest["local_replay_run_count"],
                "new_evidence_count": manifest["new_evidence_run_count"],
                "unique_profile_count": manifest["unique_profile_count"],
                "unique_scenario_count": manifest["unique_scenario_count"],
                "probe_artifact_digest": probe_identity["digest"],
                "adapter_version": adapter_version,
            }
        )
        mlflow.log_dict(manifest, f"inputs/{campaign_name}-manifest.json")

    existing = (
        spark.table(runs_table)
        .where(F.col("campaign_id") == campaign_id)
        .select("experimental_configuration_id", "seed", "execution_status")
        .collect()
    )
    natural_keys = {(row["experimental_configuration_id"], row["seed"]) for row in existing}
    if len(existing) != len(natural_keys):
        raise RuntimeError("thermal ledger contains duplicate natural keys")
    planned_keys = {(row["configuration_id"], row["seed"]) for row in plan}
    if natural_keys - planned_keys:
        raise RuntimeError("thermal ledger contains unplanned rows")
    accepted = {
        (row["experimental_configuration_id"], row["seed"])
        for row in existing
        if row["execution_status"] == "SUCCESS"
    }
    pending = [
        entry
        for entry in plan
        if (entry["configuration_id"], entry["seed"]) not in accepted
    ]

    def persist_attempts(attempts: list[dict]) -> None:
        run_rows = []
        metric_rows = []
        metric_definitions = {
            "total_time_ms": ("racing.total-time", "ms", "total"),
            "maximum_speed_kph": ("racing.maximum-speed", "km/h", "maximum"),
            "maximum_engine_temperature_c": (
                "racing.maximum-engine-temperature", "celsius", "maximum"
            ),
            "engine_derated_time_s": ("racing.engine-derated-time", "s", "total"),
            "engine_derated_fraction": (
                "racing.engine-derated-fraction", "ratio", "total"
            ),
            "generated_engine_heat_kj": (
                "racing.generated-engine-heat", "kJ", "total"
            ),
            "removed_engine_heat_kj": (
                "racing.removed-engine-heat", "kJ", "total"
            ),
            "fixed_drag_area_m2": ("racing.fixed-drag-area", "m2", "maximum"),
        }
        for attempt in attempts:
            entry = attempt["entry"]
            result = attempt["result"]
            adapter_result = attempt["adapter_result"]
            completed_at = datetime.now(timezone.utc)
            execution_id = result["experimental_execution_id"] if result else None
            if result and attempt["status"] == "SUCCESS":
                for key, value in attempt["metrics"].items():
                    metric_id, unit, statistic = metric_definitions[key]
                    metric_rows.append(
                        {
                            "campaign_id": campaign_id,
                            "experimental_execution_id": execution_id,
                            "experimental_configuration_id": entry["configuration_id"],
                            "response_id": entry["parameter_set_id"],
                            "seed": entry["seed"],
                            "metric_id": metric_id,
                            "metric_value": float(value),
                            "metric_unit": unit,
                            "statistic": statistic,
                            "recorded_at": completed_at,
                        }
                    )
            metadata = {
                key: entry[key]
                for key in (
                    "execution_key", "parameter_set_id", "parameter_origin",
                    "anchor_id", "vehicle_family", "circuit_archetype", "split",
                    "workload", "laps", "cooling_points",
                )
            }
            run_rows.append(
                {
                    "campaign_id": campaign_id,
                    "experimental_configuration_id": entry["configuration_id"],
                    "response_id": entry["parameter_set_id"],
                    "response_digest": entry["profile_ref"],
                    "seed": entry["seed"],
                    "experimental_execution_id": execution_id,
                    "scenario_digest": entry["scenario_ref"],
                    "adapter_version": adapter_version,
                    "probe_artifact_digest": probe_identity["digest"],
                    "canonical_result_digest": adapter_result["canonical_result_digest"] if adapter_result else None,
                    "source_git_revision": source_git_revision,
                    "circuit_id": entry["circuit_id"],
                    "era": entry["era"],
                    "setup_json": json.dumps(metadata, sort_keys=True, separators=(",", ":")),
                    "strategy_json": "{}",
                    "execution_status": attempt["status"],
                    "failure_phase": attempt["phase"],
                    "failure_code": attempt["code"],
                    "failure_message": attempt["message"],
                    "duration_ms": attempt["duration_ms"],
                    "result_json": json.dumps(result, sort_keys=True, separators=(",", ":")) if result else None,
                    "started_at": attempt["started_at"],
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

    batch_size = 128
    with concurrent.futures.ThreadPoolExecutor(max_workers=max_workers) as executor:
        for offset in range(0, len(pending), batch_size):
            attempts = list(executor.map(execute_entry, pending[offset : offset + batch_size]))
            persist_attempts(attempts)

    persisted = (
        spark.table(runs_table)
        .where(F.col("campaign_id") == campaign_id)
        .select(
            "experimental_configuration_id", "seed", "execution_status",
            "failure_code", "result_json",
        )
        .collect()
    )
    counts = Counter(row["execution_status"] for row in persisted)
    if len(persisted) != manifest["planned_run_count"]:
        raise RuntimeError("thermal ledger does not reconcile with the plan")

    entry_by_key = {(row["configuration_id"], row["seed"]): row for row in plan}
    split_counts = Counter()
    family_counts = Counter()
    pathology_counts = Counter()
    local_parity_failures = 0
    for row in persisted:
        entry = entry_by_key[(row["experimental_configuration_id"], row["seed"])]
        split_counts[entry["split"]] += row["execution_status"] == "SUCCESS"
        family_counts[entry["vehicle_family"]] += row["execution_status"] == "SUCCESS"
        local_parity_failures += row["failure_code"] == "local_thermal_evidence_mismatch"
        if row["execution_status"] == "SUCCESS":
            metrics = thermal_metrics(json.loads(row["result_json"]))
            if (
                metrics["maximum_engine_temperature_c"]
                > manifest["adequacy_contract"]["global_pathology_guards"][
                    "maximum_engine_temperature_c"
                ]
                or metrics["engine_derated_fraction"]
                > manifest["adequacy_contract"]["global_pathology_guards"][
                    "maximum_engine_derated_fraction"
                ]
            ):
                pathology_counts[entry["vehicle_family"]] += 1

    completed = (
        counts["SUCCESS"] == manifest["planned_run_count"]
        and local_parity_failures == 0
    )
    report = {
        "schema_version": "pitgun.racing-v3-thermal-databricks-report/v1",
        "campaign_id": campaign_id,
        "manifest_digest": manifest_digest,
        "status": "COMPLETED" if completed else "FAILED",
        "planned_execution_count": manifest["planned_run_count"],
        "terminal_counts": dict(counts),
        "successful_split_counts": dict(split_counts),
        "successful_vehicle_family_counts": dict(family_counts),
        "pathological_execution_counts": dict(pathology_counts),
        "local_parity_failure_count": local_parity_failures,
        "decision": "REVIEW_REQUIRED" if completed else "REJECTED",
        "per_family_verdicts_selected": False,
        "automatic_catalog_promotion": False,
    }
    mlflow.log_metrics(
        {
            "successful_execution_count": counts["SUCCESS"],
            "invalid_execution_count": counts["INVALID"],
            "failed_execution_count": counts["FAILED"],
            "local_parity_failure_count": local_parity_failures,
            "pathological_execution_count": sum(pathology_counts.values()),
        }
    )
    mlflow.set_tag("pitgun.local_parity", str(local_parity_failures == 0).lower())
    mlflow.set_tag("pitgun.decision", report["decision"])
    mlflow.log_dict(report, f"reports/{campaign_name}.json")

    completed_at = datetime.now(timezone.utc)
    completion_source = spark.createDataFrame(
        [{
            "campaign_id": campaign_id,
            "status": report["status"],
            "updated_at": completed_at,
            "completed_at": completed_at,
        }],
        "campaign_id STRING, status STRING, updated_at TIMESTAMP, completed_at TIMESTAMP",
    )
    (
        DeltaTable.forName(spark, campaigns_table)
        .alias("target")
        .merge(completion_source.alias("source"), "target.campaign_id = source.campaign_id")
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
    raise RuntimeError(f"V3 thermal campaign did not complete: {report_json}")
dbutils.notebook.exit(report_json)
