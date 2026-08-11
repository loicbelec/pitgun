# Databricks notebook source
"""Execute the immutable Racing reference campaign into governed Delta tables."""

# COMMAND ----------

from collections import Counter, defaultdict
from datetime import datetime, timezone
import html
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
    execute_packaged_racing,
    inspect_packaged_runner,
    load_reference_campaign,
    materialize_plan,
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


def render_family_pace_svg(families: dict) -> str:
    """Render a dependency-free mean/range plot suitable for MLflow artifacts."""

    width = 900
    height = 120 + 90 * len(families)
    plot_left = 210
    plot_right = width - 70
    observations = [
        value["mean_lap_time_ms"] + offset * value["lap_time_range_ms"] / 2
        for value in families.values()
        for offset in (-1, 1)
    ]
    minimum = min(observations) - 100
    maximum = max(observations) + 100

    def x_position(value: float) -> float:
        return plot_left + (value - minimum) * (plot_right - plot_left) / (
            maximum - minimum
        )

    rows = []
    for index, (family, values) in enumerate(sorted(families.items())):
        y = 105 + index * 90
        mean = values["mean_lap_time_ms"]
        half_range = values["lap_time_range_ms"] / 2
        low = x_position(mean - half_range)
        high = x_position(mean + half_range)
        mean_x = x_position(mean)
        rows.extend(
            [
                f'<text x="25" y="{y + 6}" class="label">{html.escape(family)}</text>',
                f'<line x1="{low:.2f}" y1="{y}" x2="{high:.2f}" y2="{y}" class="range"/>',
                f'<line x1="{low:.2f}" y1="{y - 9}" x2="{low:.2f}" y2="{y + 9}" class="range"/>',
                f'<line x1="{high:.2f}" y1="{y - 9}" x2="{high:.2f}" y2="{y + 9}" class="range"/>',
                f'<circle cx="{mean_x:.2f}" cy="{y}" r="7" class="mean"/>',
                f'<text x="{plot_right}" y="{y + 28}" text-anchor="end" class="value">{mean:,.2f} ms mean · {values["lap_time_range_ms"]:.0f} ms range</text>',
            ]
        )

    return "\n".join(
        [
            f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">',
            "<style>.title{font:700 22px sans-serif;fill:#182934}.subtitle,.value{font:14px monospace;fill:#55636b}.label{font:700 17px monospace;fill:#182934}.axis,.range{stroke:#87939a;stroke-width:3}.mean{fill:#d54b2a;stroke:#182934;stroke-width:2}</style>",
            '<rect width="100%" height="100%" fill="#f6ead7"/>',
            '<text x="25" y="38" class="title">Pitgun Racing reference campaign</text>',
            '<text x="25" y="64" class="subtitle">Mean deterministic lap time and observed three-seed range · lower is faster</text>',
            f'<line x1="{plot_left}" y1="78" x2="{plot_right}" y2="78" class="axis"/>',
            *rows,
            "</svg>",
        ]
    )


catalog_name = validated_identifier("catalog_name", catalog_name)
calibration_schema = validated_identifier("calibration_schema", calibration_schema)
if calibration_schema.lower() in {"default", "information_schema"}:
    raise ValueError(f"protected schema cannot be a campaign target: {calibration_schema!r}")
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
  campaign_id STRING, configuration_id STRING, configuration_family STRING,
  seed STRING, run_id STRING, scenario_id STRING, scenario_version STRING,
  scenario_digest STRING, model_id STRING, model_version STRING,
  model_digest STRING, data_pack_id STRING, data_pack_version STRING,
  data_pack_digest STRING, runner_version STRING, adapter_version STRING,
  runner_artifact_digest STRING, canonical_result_digest STRING,
  source_git_revision STRING, circuit_id STRING, era INT, setup_json STRING,
  strategy_json STRING, execution_status STRING, failure_phase STRING,
  failure_code STRING, failure_message STRING, duration_ms BIGINT,
  result_json STRING, started_at TIMESTAMP, completed_at TIMESTAMP,
  ingested_at TIMESTAMP
"""
metric_row_schema = """
  campaign_id STRING, run_id STRING, configuration_id STRING, seed STRING,
  metric_id STRING, metric_value DOUBLE, metric_unit STRING,
  sample_count BIGINT, statistic STRING, recorded_at TIMESTAMP
"""

manifest, manifest_digest = load_reference_campaign()
plan = materialize_plan(manifest)
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
actual_tables = {
    row[1] for row in spark.sql(f"SHOW TABLES IN {calibration}").collect()
}
missing_tables = required_tables - actual_tables
if missing_tables:
    raise RuntimeError(
        "run bootstrap_job before the campaign; missing tables: "
        + ", ".join(sorted(missing_tables))
    )

existing_campaigns = (
    spark.table(campaigns_table)
    .where(F.col("campaign_id") == campaign_id)
    .collect()
)
if len(existing_campaigns) > 1:
    raise RuntimeError(f"campaign ledger contains duplicate key: {campaign_id}")

existing_campaign = existing_campaigns[0].asDict() if existing_campaigns else None
if existing_campaign:
    immutable_expectations = {
        "manifest_digest": manifest_digest,
        "question": manifest["question"],
        "parameter_space_version": manifest["parameter_space_version"],
        "model_digest": manifest["model"]["digest"],
        "data_pack_digest": manifest["data_pack"]["digest"],
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
tracking_context = (
    mlflow.start_run(run_id=existing_mlflow_run_id)
    if existing_mlflow_run_id
    else mlflow.start_run(
        experiment_id=experiment_id,
        run_name=campaign_id,
        tags={
            "pitgun.campaign_id": campaign_id,
            "pitgun.manifest_digest": manifest_digest,
            "pitgun.domain": "racing-calibration",
        },
    )
)

# COMMAND ----------

campaign_started = time.perf_counter_ns()
with tracking_context as tracking_run:
    mlflow_run_id = tracking_run.info.run_id
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

    mlflow.log_params(
        {
            "campaign_id": campaign_id,
            "manifest_digest": manifest_digest,
            "parameter_space_version": manifest["parameter_space_version"],
            "circuit_id": manifest["circuit_id"],
            "era": manifest["era"],
            "configuration_family_count": len(manifest["configuration_families"]),
            "seed_count": len(manifest["seeds"]),
            "runner_version": runner_identity["version"],
            "runner_artifact_digest": runner_identity["digest"],
            "adapter_version": adapter_version,
            "source_git_revision": source_git_revision,
        }
    )
    mlflow.log_dict(manifest, "inputs/campaign-manifest.json")

    existing_run_rows = (
        spark.table(runs_table)
        .where(F.col("campaign_id") == campaign_id)
        .select("configuration_id", "seed", "run_id", "execution_status")
        .collect()
    )
    if len(existing_run_rows) != len(
        {(row["configuration_id"], row["seed"]) for row in existing_run_rows}
    ):
        raise RuntimeError("run ledger contains duplicate natural keys")

    planned_keys = {
        (entry["expected_configuration_id"], entry["seed"]) for entry in plan
    }
    unexpected_keys = {
        (row["configuration_id"], row["seed"])
        for row in existing_run_rows
        if (row["configuration_id"], row["seed"]) not in planned_keys
    }
    if unexpected_keys:
        raise RuntimeError(f"campaign ledger contains unplanned rows: {unexpected_keys}")

    accepted_keys = {
        (row["configuration_id"], row["seed"])
        for row in existing_run_rows
        if row["execution_status"] == "SUCCESS"
    }
    skipped_run_count = len(accepted_keys)
    run_rows = []
    metric_rows = []

    for entry in plan:
        natural_key = (entry["expected_configuration_id"], entry["seed"])
        if natural_key in accepted_keys:
            continue

        started_at = datetime.now(timezone.utc)
        invocation_started = time.perf_counter_ns()
        execution_status = "FAILED"
        failure_phase = None
        failure_code = None
        failure_message = None
        run_id = None
        result_json = None
        adapter_result = None

        try:
            adapter_result = execute_packaged_racing(
                int(entry["seed"]), entry["configuration_family"]
            )
            result = adapter_result["result"]
            identity_mismatches = {}
            if result["configuration_id"] != entry["expected_configuration_id"]:
                identity_mismatches["configuration_id"] = result["configuration_id"]
            if result["scenario_digest"] != entry["expected_scenario_digest"]:
                identity_mismatches["scenario_digest"] = result["scenario_digest"]
            if result["model"] != manifest["model"]:
                identity_mismatches["model"] = result["model"]
            if result["data_pack"] != manifest["data_pack"]:
                identity_mismatches["data_pack"] = result["data_pack"]
            if result["seed"] != entry["seed"]:
                identity_mismatches["seed"] = result["seed"]
            if identity_mismatches:
                raise ValueError(
                    "runner identity differs from immutable plan: "
                    + json.dumps(identity_mismatches, sort_keys=True)
                )

            execution_status = "SUCCESS"
            run_id = result["run_id"]
            result_json = json.dumps(result, sort_keys=True, separators=(",", ":"))
            recorded_at = datetime.now(timezone.utc)
            metric_rows.extend(
                [
                    {
                        "campaign_id": campaign_id,
                        "run_id": run_id,
                        "configuration_id": result["configuration_id"],
                        "seed": entry["seed"],
                        "metric_id": "racing.lap-time",
                        "metric_value": float(result["summary"]["total_time_ms"]),
                        "metric_unit": "ms",
                        "sample_count": 1,
                        "statistic": "total",
                        "recorded_at": recorded_at,
                    },
                    {
                        "campaign_id": campaign_id,
                        "run_id": run_id,
                        "configuration_id": result["configuration_id"],
                        "seed": entry["seed"],
                        "metric_id": "racing.telemetry-frame-count",
                        "metric_value": float(
                            result["summary"]["telemetry_frame_count"]
                        ),
                        "metric_unit": "frame",
                        "sample_count": 1,
                        "statistic": "count",
                        "recorded_at": recorded_at,
                    },
                ]
            )
            for metric in result["summary"]["metrics"]["metrics"]:
                metric_rows.append(
                    {
                        "campaign_id": campaign_id,
                        "run_id": run_id,
                        "configuration_id": result["configuration_id"],
                        "seed": entry["seed"],
                        "metric_id": metric["id"],
                        "metric_value": float(metric["value"]),
                        "metric_unit": metric["unit"],
                        "sample_count": int(metric["sample_count"]),
                        "statistic": metric["statistic"],
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
                "configuration_id": entry["expected_configuration_id"],
                "configuration_family": entry["configuration_family"],
                "seed": entry["seed"],
                "run_id": run_id,
                "scenario_id": manifest["scenario"]["id"],
                "scenario_version": manifest["scenario"]["version"],
                "scenario_digest": entry["expected_scenario_digest"],
                "model_id": manifest["model"]["id"],
                "model_version": manifest["model"]["version"],
                "model_digest": manifest["model"]["digest"],
                "data_pack_id": manifest["data_pack"]["id"],
                "data_pack_version": manifest["data_pack"]["version"],
                "data_pack_digest": manifest["data_pack"]["digest"],
                "runner_version": runner_identity["version"],
                "adapter_version": adapter_version,
                "runner_artifact_digest": runner_identity["digest"],
                "canonical_result_digest": adapter_result[
                    "canonical_result_digest"
                ]
                if adapter_result
                else None,
                "source_git_revision": source_git_revision,
                "circuit_id": manifest["circuit_id"],
                "era": manifest["era"],
                "setup_json": json.dumps(
                    entry["setup"], sort_keys=True, separators=(",", ":")
                ),
                "strategy_json": json.dumps(
                    entry["strategy"], sort_keys=True, separators=(",", ":")
                ),
                "execution_status": execution_status,
                "failure_phase": failure_phase,
                "failure_code": failure_code,
                "failure_message": failure_message,
                "duration_ms": (time.perf_counter_ns() - invocation_started)
                // 1_000_000,
                "result_json": result_json,
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
                "AND target.configuration_id = source.configuration_id "
                "AND target.seed = source.seed",
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
        .select("configuration_family", "execution_status", "result_json")
        .collect()
    )
    counts = Counter(row["execution_status"] for row in persisted_runs)
    planned_count = manifest["planned_run_count"]
    successful_count = counts["SUCCESS"]
    invalid_count = counts["INVALID"]
    failed_count = counts["FAILED"]
    if len(persisted_runs) != planned_count:
        raise RuntimeError(
            f"campaign ledger has {len(persisted_runs)} rows; expected {planned_count}"
        )
    if successful_count + invalid_count + failed_count != planned_count:
        raise RuntimeError("campaign terminal counts do not reconcile")

    family_results = defaultdict(list)
    for row in persisted_runs:
        if row["execution_status"] == "SUCCESS":
            family_results[row["configuration_family"]].append(
                json.loads(row["result_json"])
            )

    family_report = {}
    for family, results in sorted(family_results.items()):
        lap_times = [result["summary"]["total_time_ms"] for result in results]
        maximum_speeds = [
            metric["value"]
            for result in results
            for metric in result["summary"]["metrics"]["metrics"]
            if metric["id"] == "racing.observed-maximum-speed"
        ]
        summary = {
            "successful_seed_count": len(results),
            "mean_lap_time_ms": statistics.fmean(lap_times),
            "lap_time_stddev_ms": statistics.pstdev(lap_times),
            "lap_time_range_ms": max(lap_times) - min(lap_times),
            "mean_maximum_speed_kmh": statistics.fmean(maximum_speeds),
        }
        family_report[family] = summary
        metric_prefix = family.replace("-", "_")
        for metric_name, value in summary.items():
            if metric_name != "successful_seed_count" and math.isfinite(value):
                mlflow.log_metric(f"{metric_prefix}.{metric_name}", value)

    mean_paces = [value["mean_lap_time_ms"] for value in family_report.values()]
    family_pace_spread_ms = max(mean_paces) - min(mean_paces) if mean_paces else 0
    campaign_duration_ms = (time.perf_counter_ns() - campaign_started) // 1_000_000
    final_status = "COMPLETED" if successful_count == planned_count else "PARTIAL"
    report = {
        "schema_version": "pitgun.calibration-campaign-report/v1",
        "campaign_id": campaign_id,
        "manifest_digest": manifest_digest,
        "mlflow_run_id": mlflow_run_id,
        "status": final_status,
        "planned_run_count": planned_count,
        "successful_run_count": successful_count,
        "invalid_run_count": invalid_count,
        "failed_run_count": failed_count,
        "skipped_accepted_run_count": skipped_run_count,
        "attempted_run_count": len(run_rows),
        "campaign_duration_ms": campaign_duration_ms,
        "family_pace_spread_ms": family_pace_spread_ms,
        "families": family_report,
    }
    mlflow.log_metrics(
        {
            "planned_run_count": planned_count,
            "successful_run_count": successful_count,
            "invalid_run_count": invalid_count,
            "failed_run_count": failed_count,
            "family_pace_spread_ms": family_pace_spread_ms,
            "campaign_duration_ms": campaign_duration_ms,
        }
    )
    mlflow.log_dict(report, "reports/campaign-report.json")
    mlflow.log_text(render_family_pace_svg(family_report), "plots/family-pace.svg")

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
    raise RuntimeError(f"reference campaign did not complete: {report_json}")
dbutils.notebook.exit(report_json)
