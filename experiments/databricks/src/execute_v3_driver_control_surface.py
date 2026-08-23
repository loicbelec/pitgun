# Databricks notebook source
# ruff: noqa: F821
"""Execute and rank the governed Model V3 driver-control surface."""

# COMMAND ----------

from collections import Counter, defaultdict
import concurrent.futures
from datetime import datetime, timezone
import importlib.metadata
import json
import math
import re
import statistics
import time

from delta.tables import DeltaTable
import mlflow
from pyspark.sql import functions as F

from pitgun_databricks_adapter import (
    execute_packaged_v3_driver_control,
    inspect_packaged_v3_driver_control_probe,
    load_driver_control_campaign,
    materialize_driver_control_plan,
    validate_packaged_v3_driver_control_profiles,
)


dbutils.widgets.text("catalog_name", "workspace")
dbutils.widgets.text("calibration_schema", "pitgun_calibration")
dbutils.widgets.text("experiment_id", "")
dbutils.widgets.text("campaign_name", "racing-v3-driver-control-surface-v2")
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


def driver_metrics(result: dict) -> dict[str, float]:
    laps = [float(value) for value in result["player_lap_times_ms"]]
    resolution = result["driver_control_resolutions"]["player"]
    diagnostics = result["driver_control_diagnostics"]["player"]
    tire = result["tire_diagnostics"]
    wear = result["tire_degradation_diagnostics"]
    return {
        "total_time_ms": float(result["total_time_ms"]),
        "best_lap_ms": min(laps),
        "mean_lap_ms": statistics.fmean(laps),
        "lap_time_stddev_ms": statistics.pstdev(laps),
        "final_tire_temperature_c": float(result["final_tire_temperature_c"]),
        "final_tire_wear_pct": float(result["final_tire_wear_pct"]),
        "requested_commitment": float(resolution["requested_commitment"]),
        "control_error_amplitude": float(resolution["control_error_amplitude"]),
        "correction_workload_multiplier": float(
            resolution["correction_workload_multiplier"]
        ),
        "correction_contact_workload_mj": float(
            tire["correction_contact_workload_mj"]
        ),
        "requested_correction_wear_fraction": float(
            wear["requested_correction_wear_fraction"]
        ),
        "mean_cornering_utilization": float(
            diagnostics["cornering"]["mean_realized"]
        ),
        "mean_braking_utilization": float(
            diagnostics["braking"]["mean_realized"]
        ),
        "mean_traction_utilization": float(
            diagnostics["traction"]["mean_realized"]
        ),
    }


def metrics_match(expected: dict, actual: dict) -> dict[str, list[float]]:
    return {
        key: [float(value), float(actual[key])]
        for key, value in expected.items()
        if not math.isclose(float(value), float(actual[key]), rel_tol=1e-9, abs_tol=1e-9)
    }


def pathological(metrics: dict[str, float]) -> bool:
    finite = all(math.isfinite(value) for value in metrics.values())
    utilizations = (
        metrics["mean_cornering_utilization"],
        metrics["mean_braking_utilization"],
        metrics["mean_traction_utilization"],
    )
    return (
        not finite
        or not 0.0 <= metrics["final_tire_temperature_c"] <= 180.0
        or not 0.0 <= metrics["final_tire_wear_pct"] < 100.0
        or metrics["correction_workload_multiplier"] < 1.0
        or any(not 0.0 <= value <= 1.0 for value in utilizations)
    )


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

manifest, manifest_digest = load_driver_control_campaign(campaign_name)
plan = materialize_driver_control_plan(manifest)
campaign_id = manifest["campaign_id"]
adapter_version = importlib.metadata.version("pitgun-databricks-adapter")
source_git_revision = adapter_version.split("+g", 1)[-1]
probe_identity = inspect_packaged_v3_driver_control_probe()
profile_preflight = validate_packaged_v3_driver_control_profiles(campaign_name)
if profile_preflight["validated_profile_count"] != manifest["unique_profile_count"]:
    raise RuntimeError("Rust profile preflight did not reconcile with the manifest")

# COMMAND ----------

required_tables = {"campaigns", "experimental_runs", "experimental_metrics"}
actual_tables = {row[1] for row in spark.sql(f"SHOW TABLES IN {calibration}").collect()}
missing_tables = required_tables - actual_tables
if missing_tables:
    raise RuntimeError("run bootstrap first; missing tables: " + ", ".join(sorted(missing_tables)))

existing_rows = spark.table(campaigns_table).where(F.col("campaign_id") == campaign_id).collect()
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
        raise RuntimeError("driver-control campaign changed after execution began: " + json.dumps(mismatches, sort_keys=True))

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


def execute_entry(entry: dict) -> dict:
    started_at = datetime.now(timezone.utc)
    started_ns = time.perf_counter_ns()
    try:
        adapter_result = execute_packaged_v3_driver_control(
            entry["execution_key"], campaign_name
        )
        result = adapter_result["result"]
        if result.get("model") != manifest["model"]:
            raise ValueError("Rust probe returned a different model identity")
        actual_metrics = driver_metrics(result)
        expected = entry.get("expected_local_evidence")
        if expected:
            identity_mismatches = {
                key: [expected[key], result.get(key)]
                for key in (
                    "experimental_execution_id", "scenario_digest", "profile_digest",
                    "driver_experiment_digest",
                )
                if result.get(key) != expected[key]
            }
            metric_mismatches = metrics_match(expected["metrics"], actual_metrics)
            if identity_mismatches or metric_mismatches:
                return {
                    "entry": entry, "started_at": started_at,
                    "duration_ms": (time.perf_counter_ns() - started_ns) // 1_000_000,
                    "status": "INVALID", "phase": "PORTABLE_PARITY",
                    "code": "local_driver_control_evidence_mismatch",
                    "message": json.dumps(
                        {"identity": identity_mismatches, "metrics": metric_mismatches},
                        sort_keys=True,
                    )[:2000],
                    "adapter_result": adapter_result, "result": result,
                    "metrics": actual_metrics,
                }
        return {
            "entry": entry, "started_at": started_at,
            "duration_ms": (time.perf_counter_ns() - started_ns) // 1_000_000,
            "status": "SUCCESS", "phase": None, "code": None, "message": None,
            "adapter_result": adapter_result, "result": result,
            "metrics": actual_metrics,
        }
    except ValueError as error:
        status, phase, code = "INVALID", "IDENTITY", "immutable_identity_mismatch"
        message = str(error)[:2000]
    except Exception as error:
        status, phase, code = "FAILED", "EXECUTION", type(error).__name__
        message = str(error)[:2000]
    return {
        "entry": entry, "started_at": started_at,
        "duration_ms": (time.perf_counter_ns() - started_ns) // 1_000_000,
        "status": status, "phase": phase, "code": code, "message": message,
        "adapter_result": None, "result": None, "metrics": None,
    }


# COMMAND ----------

with tracking_context as tracking_run:
    now = datetime.now(timezone.utc)
    campaign_row = {
        "campaign_id": campaign_id,
        "manifest_digest": manifest_digest,
        "question": manifest["question"],
        "parameter_space_version": manifest["parameter_space_version"],
        "scenario_id": "embedded-driver-control-plan",
        "scenario_version": "v1",
        "scenario_digest": manifest["source_evidence"]["local_report_digest"],
        "model_id": manifest["model"]["id"],
        "model_version": manifest["model"]["version"],
        "model_digest": manifest["model"]["digest"],
        "data_pack_id": "pitgun.racing-driver-control-profile",
        "data_pack_version": "v1",
        "data_pack_digest": manifest["source_evidence"]["base_profile_digest"],
        "runner_version": "v3-driver-control-probe/v1",
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
        DeltaTable.forName(spark, campaigns_table).alias("target")
        .merge(campaign_source.alias("source"), "target.campaign_id = source.campaign_id")
        .whenMatchedUpdate(set={
            "status": "source.status", "updated_at": "source.updated_at",
            "mlflow_run_id": "source.mlflow_run_id",
        })
        .whenNotMatchedInsertAll().execute()
    )
    if not existing_mlflow_run_id:
        mlflow.log_params({
            "campaign_id": campaign_id,
            "manifest_digest": manifest_digest,
            "planned_execution_count": manifest["planned_run_count"],
            "local_replay_count": manifest["local_replay_run_count"],
            "unique_profile_count": manifest["unique_profile_count"],
            "probe_artifact_digest": probe_identity["digest"],
            "adapter_version": adapter_version,
            "rust_validated_profile_count": profile_preflight[
                "validated_profile_count"
            ],
        })
        mlflow.log_dict(manifest, f"inputs/{campaign_name}-manifest.json")
        mlflow.log_dict(
            profile_preflight, f"inputs/{campaign_name}-rust-profile-preflight.json"
        )

    existing = (
        spark.table(runs_table).where(F.col("campaign_id") == campaign_id)
        .select("experimental_configuration_id", "seed", "execution_status").collect()
    )
    natural_keys = {(row["experimental_configuration_id"], row["seed"]) for row in existing}
    if len(existing) != len(natural_keys):
        raise RuntimeError("driver-control ledger contains duplicate natural keys")
    planned_keys = {(row["configuration_id"], row["seed"]) for row in plan}
    if natural_keys - planned_keys:
        raise RuntimeError("driver-control ledger contains unplanned rows")
    accepted = {
        (row["experimental_configuration_id"], row["seed"])
        for row in existing if row["execution_status"] == "SUCCESS"
    }
    pending = [
        entry for entry in plan
        if (entry["configuration_id"], entry["seed"]) not in accepted
    ]

    metric_definitions = {
        "total_time_ms": ("racing.total-time", "ms", "total"),
        "best_lap_ms": ("racing.best-lap", "ms", "minimum"),
        "mean_lap_ms": ("racing.mean-lap", "ms", "mean"),
        "lap_time_stddev_ms": ("racing.lap-time-dispersion", "ms", "stddev"),
        "final_tire_temperature_c": ("racing.final-tire-temperature", "celsius", "final"),
        "final_tire_wear_pct": ("racing.final-tire-wear", "percent", "final"),
        "requested_commitment": ("racing.driver-commitment", "ratio", "requested"),
        "control_error_amplitude": ("racing.control-error-amplitude", "ratio", "resolved"),
        "correction_workload_multiplier": ("racing.correction-workload-multiplier", "ratio", "resolved"),
        "correction_contact_workload_mj": ("racing.correction-contact-workload", "MJ", "total"),
        "requested_correction_wear_fraction": ("racing.correction-wear-fraction", "ratio", "total"),
        "mean_cornering_utilization": ("racing.cornering-utilization", "ratio", "mean"),
        "mean_braking_utilization": ("racing.braking-utilization", "ratio", "mean"),
        "mean_traction_utilization": ("racing.traction-utilization", "ratio", "mean"),
    }

    def normalized_metric_rows(
        entry: dict, result: dict, recorded_at: datetime
    ) -> list[dict]:
        execution_id = result["experimental_execution_id"]
        return [
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
                "recorded_at": recorded_at,
            }
            for key, value in driver_metrics(result).items()
            for metric_id, unit, statistic in [metric_definitions[key]]
        ]

    def persist_metric_rows(metric_rows: list[dict]) -> None:
        if not metric_rows:
            return
        source = spark.createDataFrame(metric_rows, metric_row_schema)
        (
            DeltaTable.forName(spark, metrics_table).alias("target")
            .merge(
                source.alias("source"),
                "target.campaign_id = source.campaign_id "
                "AND target.experimental_execution_id = source.experimental_execution_id "
                "AND target.metric_id = source.metric_id",
            )
            .whenNotMatchedInsertAll().execute()
        )

    def persist_attempts(attempts: list[dict]) -> None:
        run_rows, metric_rows = [], []
        for attempt in attempts:
            entry, result = attempt["entry"], attempt["result"]
            adapter_result = attempt["adapter_result"]
            completed_at = datetime.now(timezone.utc)
            execution_id = result["experimental_execution_id"] if result else None
            if result and attempt["status"] == "SUCCESS":
                metric_rows.extend(normalized_metric_rows(entry, result, completed_at))
            metadata = {
                key: entry[key] for key in (
                    "execution_key", "parameter_set_id", "split", "circuit_slug",
                    "circuit_archetype", "horizon", "laps", "driver_id", "mode", "tire_id",
                )
            }
            coefficients = entry["profile"]["driver_control_profile"]
            run_rows.append({
                "campaign_id": campaign_id,
                "experimental_configuration_id": entry["configuration_id"],
                "response_id": entry["parameter_set_id"],
                "response_digest": entry["profile_ref"], "seed": entry["seed"],
                "experimental_execution_id": execution_id,
                "scenario_digest": entry["scenario_ref"],
                "adapter_version": adapter_version,
                "probe_artifact_digest": probe_identity["digest"],
                "canonical_result_digest": adapter_result["canonical_result_digest"] if adapter_result else None,
                "source_git_revision": source_git_revision,
                "circuit_id": entry["circuit_id"], "era": 5,
                "setup_json": json.dumps(metadata, sort_keys=True, separators=(",", ":")),
                "strategy_json": json.dumps(coefficients, sort_keys=True, separators=(",", ":")),
                "execution_status": attempt["status"], "failure_phase": attempt["phase"],
                "failure_code": attempt["code"], "failure_message": attempt["message"],
                "duration_ms": attempt["duration_ms"],
                "result_json": json.dumps(result, sort_keys=True, separators=(",", ":")) if result else None,
                "started_at": attempt["started_at"], "completed_at": completed_at,
                "ingested_at": completed_at,
            })
        if run_rows:
            source = spark.createDataFrame(run_rows, run_row_schema)
            (
                DeltaTable.forName(spark, runs_table).alias("target")
                .merge(source.alias("source"),
                    "target.campaign_id = source.campaign_id AND target.experimental_configuration_id = source.experimental_configuration_id AND target.seed = source.seed")
                .whenMatchedUpdateAll(condition="target.execution_status <> 'SUCCESS'")
                .whenNotMatchedInsertAll().execute()
            )
        persist_metric_rows(metric_rows)

    with concurrent.futures.ThreadPoolExecutor(max_workers=max_workers) as executor:
        for offset in range(0, len(pending), 128):
            persist_attempts(list(executor.map(execute_entry, pending[offset : offset + 128])))

    persisted = (
        spark.table(runs_table).where(F.col("campaign_id") == campaign_id)
        .select(
            "experimental_configuration_id", "seed", "execution_status",
            "failure_code", "experimental_execution_id", "result_json",
        )
        .collect()
    )
    counts = Counter(row["execution_status"] for row in persisted)
    if len(persisted) != manifest["planned_run_count"]:
        raise RuntimeError("driver-control ledger does not reconcile with the plan")

    entry_by_key = {(row["configuration_id"], row["seed"]): row for row in plan}
    successful_rows = [row for row in persisted if row["execution_status"] == "SUCCESS"]
    expected_metric_keys = {
        (row["experimental_execution_id"], metric_id)
        for row in successful_rows
        for metric_id, _, _ in metric_definitions.values()
    }
    metric_key_counts = (
        spark.table(metrics_table).where(F.col("campaign_id") == campaign_id)
        .groupBy("experimental_execution_id", "metric_id").count().collect()
    )
    duplicate_metric_keys = [row.asDict() for row in metric_key_counts if row["count"] != 1]
    if duplicate_metric_keys:
        raise RuntimeError("driver-control metrics contain duplicate natural keys")
    actual_metric_keys = {
        (row["experimental_execution_id"], row["metric_id"])
        for row in metric_key_counts
    }
    if actual_metric_keys - expected_metric_keys:
        raise RuntimeError("driver-control metrics contain unexpected natural keys")
    missing_metric_keys = expected_metric_keys - actual_metric_keys
    backfill_rows = []
    backfill_recorded_at = datetime.now(timezone.utc)
    for row in successful_rows:
        entry = entry_by_key[(row["experimental_configuration_id"], row["seed"])]
        result = json.loads(row["result_json"])
        if result["experimental_execution_id"] != row["experimental_execution_id"]:
            raise RuntimeError("stored driver-control result identity changed")
        backfill_rows.extend(
            metric_row
            for metric_row in normalized_metric_rows(
                entry, result, backfill_recorded_at
            )
            if (
                metric_row["experimental_execution_id"], metric_row["metric_id"]
            ) in missing_metric_keys
        )
    if len(backfill_rows) != len(missing_metric_keys):
        raise RuntimeError("driver-control metric backfill does not reconcile")
    persist_metric_rows(backfill_rows)
    normalized_metric_rows_count = (
        spark.table(metrics_table).where(F.col("campaign_id") == campaign_id).count()
    )
    normalized_metric_execution_count = (
        spark.table(metrics_table).where(F.col("campaign_id") == campaign_id)
        .select("experimental_execution_id").distinct().count()
    )
    if (
        normalized_metric_rows_count != len(expected_metric_keys)
        or normalized_metric_execution_count != len(successful_rows)
    ):
        raise RuntimeError("driver-control normalized metrics are incomplete")

    grouped = defaultdict(dict)
    global_grouped = defaultdict(dict)
    pathology_counts = Counter()
    local_parity_failures = 0
    for row in persisted:
        entry = entry_by_key[(row["experimental_configuration_id"], row["seed"])]
        local_parity_failures += row["failure_code"] == "local_driver_control_evidence_mismatch"
        if row["execution_status"] != "SUCCESS":
            continue
        metrics = driver_metrics(json.loads(row["result_json"]))
        pathology_counts[entry["parameter_set_id"]] += pathological(metrics)
        group = (
            entry["parameter_set_id"], entry["circuit_id"], entry["horizon"],
            entry["driver_id"], entry["seed"],
        )
        grouped[group][entry["mode"]] = metrics
        global_group = (
            entry["parameter_set_id"], entry["circuit_id"], entry["horizon"],
            entry["seed"],
        )
        global_grouped[global_group][
            f"{entry['driver_id']}:{entry['mode']}"
        ] = metrics

    evaluations = {}
    for parameter_set in manifest["parameter_sets"]:
        parameter_set_id = parameter_set["parameter_set_id"]
        short_groups = long_groups = short_attack_wins = long_attack_wins = 0
        global_short_groups = global_long_groups = 0
        global_short_attack_wins = global_long_attack_wins = 0
        global_winning_drivers = Counter()
        global_winning_modes = Counter()
        ordering_failures = 0
        pace_gains, long_wear_costs = [], []
        for group, modes in grouped.items():
            if group[0] != parameter_set_id or set(modes) != {"manage", "balanced", "attack"}:
                continue
            horizon = group[2]
            winner = min(modes, key=lambda mode: (modes[mode]["mean_lap_ms"], mode))
            if horizon == "short":
                short_groups += 1
                short_attack_wins += winner == "attack"
                pace_gains.append(modes["manage"]["mean_lap_ms"] - modes["attack"]["mean_lap_ms"])
            else:
                long_groups += 1
                long_attack_wins += winner == "attack"
                long_wear_costs.append(modes["attack"]["final_tire_wear_pct"] - modes["manage"]["final_tire_wear_pct"])
            if not (
                modes["attack"]["control_error_amplitude"] > modes["manage"]["control_error_amplitude"]
                and modes["attack"]["correction_contact_workload_mj"] > modes["manage"]["correction_contact_workload_mj"]
            ):
                ordering_failures += 1
        for group, candidates in global_grouped.items():
            if group[0] != parameter_set_id:
                continue
            winner = min(
                candidates,
                key=lambda candidate: (candidates[candidate]["mean_lap_ms"], candidate),
            )
            driver_id, mode = winner.split(":", 1)
            global_winning_drivers[driver_id] += 1
            global_winning_modes[mode] += 1
            if group[2] == "short":
                global_short_groups += 1
                global_short_attack_wins += mode == "attack"
            else:
                global_long_groups += 1
                global_long_attack_wins += mode == "attack"

        selection_contract = manifest["selection_contract"]
        global_mode_gate = True
        if selection_contract.get("attack_must_win_some_short_global_contexts"):
            global_mode_gate = global_mode_gate and global_short_attack_wins > 0
        if selection_contract.get(
            "attack_must_not_win_every_race_length_global_context"
        ):
            global_mode_gate = (
                global_mode_gate
                and global_long_groups > 0
                and global_long_attack_wins < global_long_groups
            )
        minimum_winning_drivers = int(
            selection_contract.get("minimum_global_winning_driver_count", 0)
        )
        global_driver_gate = len(global_winning_drivers) >= minimum_winning_drivers
        paired_mode_gate = True
        if selection_contract.get("attack_must_win_some_short_groups"):
            paired_mode_gate = paired_mode_gate and short_attack_wins > 0
        if selection_contract.get("attack_must_not_win_every_race_length_group"):
            paired_mode_gate = (
                paired_mode_gate
                and long_groups > 0
                and long_attack_wins < long_groups
            )
        gate = (
            paired_mode_gate
            and global_mode_gate
            and global_driver_gate
            and ordering_failures == 0
            and pathology_counts[parameter_set_id] == 0
        )
        evaluations[parameter_set_id] = {
            "parameter_set_id": parameter_set_id,
            "origin": parameter_set["origin"],
            "short_group_count": short_groups,
            "short_attack_win_count": short_attack_wins,
            "race_length_group_count": long_groups,
            "race_length_attack_win_count": long_attack_wins,
            "global_short_context_count": global_short_groups,
            "global_short_attack_win_count": global_short_attack_wins,
            "global_race_length_context_count": global_long_groups,
            "global_race_length_attack_win_count": global_long_attack_wins,
            "global_winning_driver_count": len(global_winning_drivers),
            "global_winning_driver_counts": dict(sorted(global_winning_drivers.items())),
            "global_winning_mode_counts": dict(sorted(global_winning_modes.items())),
            "physical_ordering_failure_count": ordering_failures,
            "pathological_execution_count": pathology_counts[parameter_set_id],
            "median_short_attack_gain_ms": statistics.median(pace_gains) if pace_gains else None,
            "median_long_attack_wear_cost_percentage_points": statistics.median(long_wear_costs) if long_wear_costs else None,
            "selection_gate_passed": gate,
            "parameters": parameter_set["parameters"],
        }
    ranking = sorted(
        evaluations.values(),
        key=lambda row: (
            not row["selection_gate_passed"],
            row["global_race_length_attack_win_count"],
            -row["global_winning_driver_count"],
            abs(
                row["global_short_attack_win_count"]
                - row["global_short_context_count"] / 2
            ),
            row["parameter_set_id"],
        ),
    )
    completed = counts["SUCCESS"] == manifest["planned_run_count"] and local_parity_failures == 0
    report = {
        "schema_version": "pitgun.racing-v3-driver-control-databricks-report/v1",
        "campaign_id": campaign_id, "manifest_digest": manifest_digest,
        "status": "COMPLETED" if completed else "FAILED",
        "planned_execution_count": manifest["planned_run_count"],
        "terminal_counts": dict(counts),
        "local_parity_failure_count": local_parity_failures,
        "normalized_metric_row_count": normalized_metric_rows_count,
        "normalized_metric_execution_count": normalized_metric_execution_count,
        "backfilled_metric_row_count": len(backfill_rows),
        "ranked_parameter_sets": ranking,
        "selection_gate_pass_count": sum(row["selection_gate_passed"] for row in ranking),
        "candidate_selected": False,
        "independent_702_case_validation_completed": False,
        "decision": "HUMAN_REVIEW_REQUIRED" if completed else "REJECTED",
        "automatic_catalog_promotion": False,
    }
    mlflow.log_metrics({
        "successful_execution_count": counts["SUCCESS"],
        "invalid_execution_count": counts["INVALID"],
        "failed_execution_count": counts["FAILED"],
        "local_parity_failure_count": local_parity_failures,
        "selection_gate_pass_count": report["selection_gate_pass_count"],
        "normalized_metric_row_count": normalized_metric_rows_count,
        "normalized_metric_execution_count": normalized_metric_execution_count,
        "backfilled_metric_row_count": len(backfill_rows),
    })
    mlflow.set_tag("pitgun.decision", report["decision"])
    mlflow.log_dict(report, f"reports/{campaign_name}.json")

    completed_at = datetime.now(timezone.utc)
    completion_source = spark.createDataFrame([{
        "campaign_id": campaign_id, "status": report["status"],
        "updated_at": completed_at, "completed_at": completed_at,
    }], "campaign_id STRING, status STRING, updated_at TIMESTAMP, completed_at TIMESTAMP")
    (
        DeltaTable.forName(spark, campaigns_table).alias("target")
        .merge(completion_source.alias("source"), "target.campaign_id = source.campaign_id")
        .whenMatchedUpdate(set={
            "status": "source.status", "updated_at": "source.updated_at",
            "completed_at": "source.completed_at",
        }).execute()
    )

report_json = json.dumps(report, sort_keys=True, separators=(",", ":"))
if report["status"] != "COMPLETED":
    raise RuntimeError(f"V3 driver-control campaign did not complete: {report_json}")
dbutils.notebook.exit(report_json)
