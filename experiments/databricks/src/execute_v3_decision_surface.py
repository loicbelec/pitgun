# Databricks notebook source
"""Execute the immutable Racing V3 multi-era decision-surface campaign."""

# COMMAND ----------

from collections import Counter
import concurrent.futures
from datetime import datetime, timezone
import hashlib
import importlib.metadata
import json
import math
import re
import time

from delta.tables import DeltaTable
import mlflow
from pyspark.sql import functions as F

from pitgun_databricks_adapter import (
    execute_packaged_v3_decision_surface,
    inspect_packaged_v3_decision_surface_probe,
    load_decision_surface_campaign,
    materialize_decision_surface_plan,
)


dbutils.widgets.text("catalog_name", "workspace")
dbutils.widgets.text("calibration_schema", "pitgun_calibration")
dbutils.widgets.text("experiment_id", "")
dbutils.widgets.text("campaign_name", "racing-v3-decision-surface-v2")
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


def canonical_pretty(value: object) -> bytes:
    return (json.dumps(value, indent=2, ensure_ascii=False) + "\n").encode()


def sha256(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


def portable_point(value: object) -> object:
    if isinstance(value, float):
        return round(value, 9)
    if isinstance(value, dict):
        return {
            key: portable_point(item)
            for key, item in value.items()
            if key != "result_digest"
        }
    if isinstance(value, list):
        return [portable_point(item) for item in value]
    return value


def compact_point(entry: dict, result: dict) -> dict:
    mechanical = result["mechanical_diagnostics"]
    tire = result["tire_diagnostics"]
    fuel = result["fuel_mass_diagnostics"]
    degradation = result["tire_degradation_diagnostics"]
    return dict(entry["metadata"], seed=int(entry["seed"])) | {
        "total_time_ms": result["total_time_ms"],
        "maximum_speed_kph": result["observed_maximum_speed_kph"],
        "maximum_engine_temperature_c": mechanical[
            "maximum_engine_temperature_c"
        ],
        "engine_derated_time_s": mechanical["engine_derated_time_s"],
        "maximum_tire_utilization": tire["maximum_combined_utilization"],
        "fuel_consumed_kg": fuel["fuel_consumed_kg"],
        "final_tire_wear_pct": result["final_tire_wear_pct"],
        "thermal_wear_multiplier": degradation[
            "maximum_thermal_wear_multiplier"
        ],
        "result_digest": sha256(canonical_pretty(result)),
    }


def point_sort_key(point: dict) -> tuple:
    return (
        point["family"],
        point["split"],
        point["circuit_id"],
        point["vehicle_id"],
        point["progression"],
        point["case_id"],
        point["seed"],
    )


catalog_name = validated_identifier("catalog_name", catalog_name)
calibration_schema = validated_identifier("calibration_schema", calibration_schema)
if calibration_schema.lower() in {"default", "information_schema"}:
    raise ValueError("protected schema cannot be an experimental campaign target")
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

manifest, manifest_digest = load_decision_surface_campaign(campaign_name)
plan = materialize_decision_surface_plan(manifest)
campaign_id = manifest["campaign_id"]
adapter_version = importlib.metadata.version("pitgun-databricks-adapter")
source_git_revision = adapter_version.split("+g", 1)[-1]
probe_identity = inspect_packaged_v3_decision_surface_probe()

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
            "decision-surface campaign changed after execution began: "
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


def execute_entry(entry: dict) -> dict:
    started_at = datetime.now(timezone.utc)
    invocation_started = time.perf_counter_ns()
    try:
        adapter_result = execute_packaged_v3_decision_surface(
            entry["execution_key"], campaign_name
        )
        result = adapter_result["result"]
        expected = {
            "experimental_execution_id": entry[
                "expected_experimental_execution_id"
            ],
            "scenario_digest": entry["scenario_ref"],
            "profile_digest": entry["profile_ref"],
            "seed": entry["seed"],
            "model": manifest["model"],
        }
        mismatches = {
            key: result.get(key)
            for key, value in expected.items()
            if result.get(key) != value
        }
        if mismatches:
            return {
                "entry": entry,
                "started_at": started_at,
                "duration_ms": (time.perf_counter_ns() - invocation_started)
                // 1_000_000,
                "status": "INVALID",
                "phase": "IDENTITY",
                "code": "immutable_identity_mismatch",
                "message": (
                    "probe identity differs from immutable plan: "
                    + json.dumps(mismatches, sort_keys=True)
                )[:2000],
                "adapter_result": adapter_result,
                "result": result,
                "compact": None,
                "raw_result_match": False,
            }
        raw_result_match = (
            sha256(canonical_pretty(result)) == entry["expected_probe_result_digest"]
        )
        compact = compact_point(entry, result)
        actual_metrics = {
            key: compact[key] for key in entry["expected_metrics"]
        }
        metric_mismatches = {
            key: [entry["expected_metrics"][key], actual]
            for key, actual in actual_metrics.items()
            if not (
                actual == entry["expected_metrics"][key]
                if key == "total_time_ms"
                else math.isclose(
                    float(actual),
                    float(entry["expected_metrics"][key]),
                    rel_tol=1e-9,
                    abs_tol=1e-9,
                )
            )
        }
        portable_digest = sha256(canonical_pretty(portable_point(compact)))
        if (
            metric_mismatches
            or portable_digest != entry["expected_portable_point_digest"]
        ):
            return {
                "entry": entry,
                "started_at": started_at,
                "duration_ms": (time.perf_counter_ns() - invocation_started)
                // 1_000_000,
                "status": "INVALID",
                "phase": "PORTABLE_PARITY",
                "code": "portable_metric_mismatch",
                "message": json.dumps(metric_mismatches, sort_keys=True)[:2000],
                "adapter_result": adapter_result,
                "result": result,
                "compact": compact,
                "raw_result_match": raw_result_match,
            }
        return {
            "entry": entry,
            "started_at": started_at,
            "duration_ms": (time.perf_counter_ns() - invocation_started) // 1_000_000,
            "status": "SUCCESS",
            "phase": None,
            "code": None,
            "message": None,
            "adapter_result": adapter_result,
            "result": result,
            "compact": compact,
            "raw_result_match": raw_result_match,
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
        "duration_ms": (time.perf_counter_ns() - invocation_started) // 1_000_000,
        "status": status,
        "phase": phase,
        "code": code,
        "message": message,
        "adapter_result": None,
        "result": None,
        "compact": None,
        "raw_result_match": False,
    }


with tracking_context as tracking_run:
    now = datetime.now(timezone.utc)
    campaign_row = {
        "campaign_id": campaign_id,
        "manifest_digest": manifest_digest,
        "question": manifest["question"],
        "parameter_space_version": manifest["parameter_space_version"],
        "scenario_id": "multi-era-explicit-plan",
        "scenario_version": "v1",
        "scenario_digest": manifest["local_evidence"]["point_set_digest"],
        "model_id": manifest["model"]["id"],
        "model_version": manifest["model"]["version"],
        "model_digest": manifest["model"]["digest"],
        "data_pack_id": manifest["data_pack"]["id"],
        "data_pack_version": manifest["data_pack"]["version"],
        "data_pack_digest": manifest["data_pack"]["digest"],
        "runner_version": "v3-decision-surface-probe/v1",
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
                "unique_configuration_count": manifest[
                    "unique_configuration_count"
                ],
                "unique_scenario_count": manifest["unique_scenario_count"],
                "probe_artifact_digest": probe_identity["digest"],
                "adapter_version": adapter_version,
                "local_evidence_digest": manifest["local_evidence"][
                    "artifact_digest"
                ],
            }
        )
        mlflow.log_dict(manifest, "inputs/v3-decision-surface-manifest.json")

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
    planned_keys = {(row["configuration_id"], row["seed"]) for row in plan}
    if natural_keys - planned_keys:
        raise RuntimeError("experimental ledger contains unplanned rows")
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
        for attempt in attempts:
            entry = attempt["entry"]
            result = attempt["result"]
            adapter_result = attempt["adapter_result"]
            completed_at = datetime.now(timezone.utc)
            execution_id = result["experimental_execution_id"] if result else None
            if result and attempt["status"] == "SUCCESS":
                mechanical = result["mechanical_diagnostics"]
                tire = result["tire_diagnostics"]
                fuel = result["fuel_mass_diagnostics"]
                degradation = result["tire_degradation_diagnostics"]
                metrics = (
                    ("racing.total-time", result["total_time_ms"], "ms", "total"),
                    (
                        "racing.maximum-speed",
                        result["observed_maximum_speed_kph"],
                        "km/h",
                        "maximum",
                    ),
                    (
                        "racing.maximum-engine-temperature",
                        mechanical["maximum_engine_temperature_c"],
                        "celsius",
                        "maximum",
                    ),
                    (
                        "racing.engine-derated-time",
                        mechanical["engine_derated_time_s"],
                        "s",
                        "total",
                    ),
                    (
                        "racing.maximum-tire-utilization",
                        tire["maximum_combined_utilization"],
                        "ratio",
                        "maximum",
                    ),
                    (
                        "racing.fuel-consumed",
                        fuel["fuel_consumed_kg"],
                        "kg",
                        "total",
                    ),
                    (
                        "racing.final-tire-wear",
                        result["final_tire_wear_pct"],
                        "percent",
                        "final",
                    ),
                    (
                        "racing.maximum-thermal-wear-multiplier",
                        degradation["maximum_thermal_wear_multiplier"],
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
                                "configuration_id"
                            ],
                            "response_id": entry["family"],
                            "seed": entry["seed"],
                            "metric_id": metric_id,
                            "metric_value": float(value),
                            "metric_unit": unit,
                            "statistic": statistic,
                            "recorded_at": completed_at,
                        }
                    )
            strategy = entry["scenario"]["request"]["competitors"][0].get(
                "stint_strategy", {}
            )
            run_rows.append(
                {
                    "campaign_id": campaign_id,
                    "experimental_configuration_id": entry["configuration_id"],
                    "response_id": entry["family"],
                    "response_digest": entry["profile_ref"],
                    "seed": entry["seed"],
                    "experimental_execution_id": execution_id,
                    "scenario_digest": entry["scenario_ref"],
                    "adapter_version": adapter_version,
                    "probe_artifact_digest": probe_identity["digest"],
                    "canonical_result_digest": adapter_result[
                        "canonical_result_digest"
                    ]
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
                    "execution_status": attempt["status"],
                    "failure_phase": attempt["phase"],
                    "failure_code": attempt["code"],
                    "failure_message": attempt["message"],
                    "duration_ms": attempt["duration_ms"],
                    "result_json": json.dumps(
                        result, sort_keys=True, separators=(",", ":")
                    )
                    if result
                    else None,
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
                .whenMatchedUpdateAll(
                    condition="target.execution_status <> 'SUCCESS'"
                )
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

    batch_size = 256
    with concurrent.futures.ThreadPoolExecutor(max_workers=max_workers) as executor:
        for offset in range(0, len(pending), batch_size):
            attempts = list(executor.map(execute_entry, pending[offset : offset + batch_size]))
            persist_attempts(attempts)

    persisted = (
        spark.table(runs_table)
        .where(F.col("campaign_id") == campaign_id)
        .select(
            "experimental_configuration_id",
            "seed",
            "execution_status",
            "result_json",
        )
        .collect()
    )
    counts = Counter(row["execution_status"] for row in persisted)
    if len(persisted) != manifest["planned_run_count"]:
        raise RuntimeError("decision-surface ledger does not reconcile with the plan")

    entry_by_key = {(row["configuration_id"], row["seed"]): row for row in plan}
    compact_points = []
    parity_failures = []
    raw_result_match_count = 0
    for row in persisted:
        if row["execution_status"] != "SUCCESS":
            continue
        key = (row["experimental_configuration_id"], row["seed"])
        entry = entry_by_key[key]
        result = json.loads(row["result_json"])
        point = compact_point(entry, result)
        if sha256(canonical_pretty(portable_point(point))) != entry[
            "expected_portable_point_digest"
        ]:
            parity_failures.append(entry["execution_key"])
        if sha256(canonical_pretty(result)) == entry["expected_probe_result_digest"]:
            raw_result_match_count += 1
        compact_points.append(point)
    compact_points.sort(key=point_sort_key)
    point_set_digest = sha256(canonical_pretty(portable_point(compact_points)))
    expected_point_set_digest = manifest["local_evidence"][
        "portable_point_set_digest"
    ]
    portable_local_parity = (
        not parity_failures
        and len(compact_points) == manifest["planned_run_count"]
        and point_set_digest == expected_point_set_digest
    )
    completed = (
        counts["SUCCESS"] == manifest["planned_run_count"]
        and portable_local_parity
    )
    report = {
        "schema_version": "pitgun.racing-v3-decision-surface-databricks-report/v1",
        "campaign_id": campaign_id,
        "manifest_digest": manifest_digest,
        "status": "COMPLETED" if completed else "FAILED",
        "planned_execution_count": manifest["planned_run_count"],
        "terminal_counts": dict(counts),
        "local_raw_point_set_digest": manifest["local_evidence"]["point_set_digest"],
        "local_portable_point_set_digest": expected_point_set_digest,
        "databricks_portable_point_set_digest": point_set_digest,
        "portable_local_point_parity": portable_local_parity,
        "raw_probe_result_match_count": raw_result_match_count,
        "parity_failure_count": len(parity_failures),
        "local_summary_digests": manifest["local_evidence"]["summary_digests"],
        "calibration_split_count": sum(
            point["split"] == "calibration" for point in compact_points
        ),
        "held_out_split_count": sum(
            point["split"] == "held_out" for point in compact_points
        ),
        "decision": "REVIEW_REQUIRED" if completed else "REJECTED",
        "automatic_catalog_promotion": False,
    }
    mlflow.log_metrics(
        {
            "successful_execution_count": counts["SUCCESS"],
            "invalid_execution_count": counts["INVALID"],
            "failed_execution_count": counts["FAILED"],
            "parity_failure_count": len(parity_failures),
            "calibration_split_count": report["calibration_split_count"],
            "held_out_split_count": report["held_out_split_count"],
        }
    )
    mlflow.set_tag(
        "pitgun.portable_local_point_parity", str(portable_local_parity).lower()
    )
    mlflow.set_tag("pitgun.decision", report["decision"])
    mlflow.log_dict(report, "reports/v3-decision-surface.json")

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
    raise RuntimeError(f"V3 decision-surface campaign did not complete: {report_json}")
dbutils.notebook.exit(report_json)
