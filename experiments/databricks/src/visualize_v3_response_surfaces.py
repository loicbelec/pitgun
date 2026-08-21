# Databricks notebook source
"""Read-only visual review of the governed Racing V3 response surfaces."""

# COMMAND ----------

import json
import re

import pandas as pd
import plotly.express as px
from pyspark.sql import functions as F


dbutils.widgets.text("catalog_name", "workspace")
dbutils.widgets.text("calibration_schema", "pitgun_calibration")
dbutils.widgets.text("campaign_id", "racing-v3-decision-surface-2026-v1")
dbutils.widgets.text("campaigns_table_version", "")
dbutils.widgets.text("runs_table_version", "")
dbutils.widgets.text("vehicle_id", "f1_2026")
dbutils.widgets.text("circuit_id", "it-1922")

catalog_name = dbutils.widgets.get("catalog_name")
calibration_schema = dbutils.widgets.get("calibration_schema")
campaign_id = dbutils.widgets.get("campaign_id")
campaigns_table_version = dbutils.widgets.get("campaigns_table_version")
runs_table_version = dbutils.widgets.get("runs_table_version")
selected_vehicle = dbutils.widgets.get("vehicle_id")
selected_circuit = dbutils.widgets.get("circuit_id")


def validated_identifier(label: str, value: str) -> str:
    if not re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", value):
        raise ValueError(f"{label} is not a portable SQL identifier: {value!r}")
    return value


def read_snapshot(table_name: str, version: str):
    reader = spark.read.format("delta")
    if version:
        if not version.isdigit():
            raise ValueError("Delta table versions must be non-negative integers")
        reader = reader.option("versionAsOf", int(version))
    return reader.table(table_name)


catalog_name = validated_identifier("catalog_name", catalog_name)
calibration_schema = validated_identifier("calibration_schema", calibration_schema)
calibration = f"`{catalog_name}`.`{calibration_schema}`"
campaigns_table = f"{calibration}.campaigns"
runs_table = f"{calibration}.experimental_runs"

campaigns = read_snapshot(campaigns_table, campaigns_table_version)
runs = read_snapshot(runs_table, runs_table_version)
campaign_rows = campaigns.where(F.col("campaign_id") == campaign_id).collect()
if len(campaign_rows) != 1:
    raise RuntimeError("expected exactly one governed campaign ledger row")
campaign = campaign_rows[0].asDict()
if campaign["status"] != "COMPLETED":
    raise RuntimeError("response surfaces require a completed governed campaign")

rows = (
    runs.where(
        (F.col("campaign_id") == campaign_id)
        & (F.col("execution_status") == "SUCCESS")
    )
    .select("seed", "setup_json", "result_json")
    .collect()
)
if len(rows) != campaign["planned_run_count"]:
    raise RuntimeError("successful evidence does not reconcile with the campaign plan")

# COMMAND ----------

records = []
for row in rows:
    metadata = json.loads(row["setup_json"])
    result = json.loads(row["result_json"])
    mechanical = result["mechanical_diagnostics"]
    degradation = result["tire_degradation_diagnostics"]
    records.append(
        metadata
        | {
            "seed": int(row["seed"]),
            "total_time_ms": float(result["total_time_ms"]),
            "maximum_speed_kph": float(result["observed_maximum_speed_kph"]),
            "maximum_engine_temperature_c": float(
                mechanical["maximum_engine_temperature_c"]
            ),
            "engine_derated_time_s": float(mechanical["engine_derated_time_s"]),
            "thermal_wear_multiplier": float(
                degradation["maximum_thermal_wear_multiplier"]
            ),
        }
    )

evidence = pd.DataFrame.from_records(records)
snapshot = {
    "campaign_id": campaign_id,
    "campaign_status": campaign["status"],
    "manifest_digest": campaign["manifest_digest"],
    "mlflow_run_id": campaign["mlflow_run_id"],
    "successful_execution_count": len(evidence),
    "campaigns_table_version": campaigns_table_version or "latest-at-read-time",
    "runs_table_version": runs_table_version or "latest-at-read-time",
}
display(spark.createDataFrame([snapshot]))

# COMMAND ----------

# Marginal value of one development point. Positive values mean the axis helps.
baseline = evidence[
    (evidence["family"] == "development.transfer")
    & (evidence["case_id"] == "balanced")
][
    [
        "split",
        "circuit_id",
        "vehicle_id",
        "progression",
        "seed",
        "total_time_ms",
    ]
].rename(columns={"total_time_ms": "baseline_total_time_ms"})
marginals = evidence[evidence["family"] == "development.marginal"].merge(
    baseline,
    on=["split", "circuit_id", "vehicle_id", "progression", "seed"],
    how="inner",
    validate="many_to_one",
)
marginals["marginal_benefit_ms"] = marginals.apply(
    lambda row: row["baseline_total_time_ms"] - row["total_time_ms"]
    if row["direction"] == "plus"
    else row["total_time_ms"] - row["baseline_total_time_ms"],
    axis=1,
)
marginal_summary = (
    marginals.groupby(
        ["split", "vehicle_id", "progression", "budget", "axis"],
        as_index=False,
    )["marginal_benefit_ms"]
    .median()
    .sort_values(["vehicle_id", "split", "axis", "budget"])
)
figure = px.line(
    marginal_summary,
    x="budget",
    y="marginal_benefit_ms",
    color="axis",
    facet_row="vehicle_id",
    facet_col="split",
    markers=True,
    title="Marginal value of one development point across progression",
    labels={
        "budget": "Development budget",
        "marginal_benefit_ms": "Median benefit per point (ms / 3 laps)",
    },
    template="plotly_white",
    height=1050,
)
figure.add_hline(y=0, line_dash="dot", line_color="#666")
displayHTML(figure.to_html(include_plotlyjs="cdn", full_html=False))

# COMMAND ----------

# Cooling/thermal review: this is the baseline diagnostic used to design the
# next coefficient experiment; it does not select or mutate any coefficient.
cooling = marginals[marginals["axis"] == "cooling"].copy()
cooling["cooling_change"] = cooling["direction"].map(
    {"minus": "-1 cooling", "plus": "+1 cooling"}
)
thermal_summary = (
    cooling.groupby(
        ["split", "vehicle_id", "progression", "budget", "cooling_change"],
        as_index=False,
    )[
        [
            "maximum_engine_temperature_c",
            "engine_derated_time_s",
            "thermal_wear_multiplier",
            "marginal_benefit_ms",
        ]
    ]
    .median()
    .sort_values(["vehicle_id", "split", "budget", "cooling_change"])
)
thermal_figure = px.line(
    thermal_summary,
    x="budget",
    y="maximum_engine_temperature_c",
    color="cooling_change",
    facet_row="vehicle_id",
    facet_col="split",
    markers=True,
    title="Cooling response and maximum engine temperature",
    labels={
        "budget": "Development budget",
        "maximum_engine_temperature_c": "Median maximum temperature (°C)",
    },
    template="plotly_white",
    height=1050,
)
displayHTML(thermal_figure.to_html(include_plotlyjs="cdn", full_html=False))
display(spark.createDataFrame(thermal_summary))

# COMMAND ----------

# The 5x5 setup surface for one reviewed circuit/vehicle selection.
setup = evidence[
    (evidence["family"] == "setup.grid")
    & (evidence["vehicle_id"] == selected_vehicle)
    & (evidence["circuit_id"] == selected_circuit)
]
if setup.empty:
    raise ValueError("selected circuit/vehicle has no setup-grid evidence")
heatmap = setup.pivot_table(
    index="downforce_slider",
    columns="gear_ratio_slider",
    values="total_time_ms",
    aggfunc="median",
)
heatmap = heatmap - heatmap.to_numpy().min()
setup_figure = px.imshow(
    heatmap,
    text_auto=".0f",
    aspect="auto",
    color_continuous_scale="RdYlGn_r",
    title=(
        f"Setup regret surface — {selected_vehicle} / {selected_circuit} "
        "(milliseconds over best 3-lap result)"
    ),
    labels={
        "x": "Gear ratio slider",
        "y": "Downforce slider",
        "color": "Regret (ms)",
    },
    template="plotly_white",
)
displayHTML(setup_figure.to_html(include_plotlyjs="cdn", full_html=False))

# COMMAND ----------

dbutils.notebook.exit(json.dumps(snapshot, sort_keys=True, separators=(",", ":")))
