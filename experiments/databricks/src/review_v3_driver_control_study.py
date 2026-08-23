# Databricks notebook source
# MAGIC %md
# MAGIC # When calibration reveals a missing physical mechanism
# MAGIC
# MAGIC Pitgun executes the same deterministic Rust simulation for the player and
# MAGIC every opponent. This study asked a narrow question: **can coefficient
# MAGIC calibration alone make an aggressive driver fast over a short run, but
# MAGIC costly over a race distance?**
# MAGIC
# MAGIC The answer is useful precisely because it is negative. We explored 33
# MAGIC bounded driver-control profiles over 1,584 paired simulations. Every
# MAGIC profile increased correction workload and tire wear as intended, yet
# MAGIC `ATTACK` remained fastest in every comparison. The experiment therefore
# MAGIC rejects coefficient-only calibration and motivates one explainable,
# MAGIC accumulated physical consequence in the next candidate model.

# COMMAND ----------

from collections import defaultdict
import json
import re
import statistics

import pandas as pd
import plotly.express as px
from pyspark.sql import functions as F

from pitgun_databricks_adapter import load_driver_control_study_review


dbutils.widgets.text("catalog_name", "workspace")
dbutils.widgets.text("calibration_schema", "pitgun_calibration")

catalog_name = dbutils.widgets.get("catalog_name")
calibration_schema = dbutils.widgets.get("calibration_schema")
review = load_driver_control_study_review()
expected = review["expected_evidence"]
lineage = review["lineage"]


def validated_identifier(label: str, value: str) -> str:
    if not re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", value):
        raise ValueError(f"{label} is not a portable SQL identifier: {value!r}")
    return value


def read_snapshot(table_name: str, version: int):
    return (
        spark.read.format("delta")
        .option("versionAsOf", version)
        .table(table_name)
    )


def driver_metrics(result: dict) -> dict[str, float]:
    laps = [float(value) for value in result["player_lap_times_ms"]]
    resolution = result["driver_control_resolutions"]["player"]
    diagnostics = result["driver_control_diagnostics"]["player"]
    tire = result["tire_diagnostics"]
    wear = result["tire_degradation_diagnostics"]
    return {
        "mean_lap_ms": statistics.fmean(laps),
        "final_tire_wear_pct": float(result["final_tire_wear_pct"]),
        "control_error_amplitude": float(resolution["control_error_amplitude"]),
        "correction_workload_mj": float(
            tire["correction_contact_workload_mj"]
        ),
        "correction_wear_fraction": float(
            wear["requested_correction_wear_fraction"]
        ),
        "mean_cornering_utilization": float(
            diagnostics["cornering"]["mean_realized"]
        ),
    }


catalog_name = validated_identifier("catalog_name", catalog_name)
calibration_schema = validated_identifier("calibration_schema", calibration_schema)
calibration = f"`{catalog_name}`.`{calibration_schema}`"
campaign_id = review["campaign_id"]

campaigns = read_snapshot(
    f"{calibration}.campaigns", lineage["campaigns_table_version"]
)
runs = read_snapshot(
    f"{calibration}.experimental_runs",
    lineage["experimental_runs_table_version"],
)
metrics = read_snapshot(
    f"{calibration}.experimental_metrics",
    lineage["experimental_metrics_table_version"],
)

campaign_rows = campaigns.where(F.col("campaign_id") == campaign_id).collect()
if len(campaign_rows) != 1:
    raise RuntimeError("expected exactly one governed campaign row")
campaign = campaign_rows[0].asDict()
if campaign["status"] != review["campaign_status"]:
    raise RuntimeError("campaign is not the reviewed completed campaign")
if campaign["manifest_digest"] != review["campaign_manifest_digest"]:
    raise RuntimeError("campaign manifest digest differs from the review")

run_rows = (
    runs.where(F.col("campaign_id") == campaign_id)
    .select(
        "experimental_configuration_id",
        "experimental_execution_id",
        "seed",
        "setup_json",
        "result_json",
        "execution_status",
    )
    .collect()
)
successful = [row for row in run_rows if row["execution_status"] == "SUCCESS"]
if len(run_rows) != expected["planned_execution_count"]:
    raise RuntimeError("run ledger does not reconcile with the reviewed plan")
if len(successful) != expected["successful_execution_count"]:
    raise RuntimeError("successful execution count changed")

metric_rows = metrics.where(F.col("campaign_id") == campaign_id)
metric_count = metric_rows.count()
metric_execution_count = metric_rows.select(
    "experimental_execution_id"
).distinct().count()
if metric_count != expected["normalized_metric_count"]:
    raise RuntimeError("normalized metric count changed")
if metric_execution_count != expected["successful_execution_count"]:
    raise RuntimeError("normalized metric execution count changed")
per_execution_counts = {
    row["count"]
    for row in metric_rows.groupBy("experimental_execution_id").count().collect()
}
if per_execution_counts != {expected["metrics_per_execution"]}:
    raise RuntimeError("normalized metric cardinality changed")

# COMMAND ----------
# MAGIC %md
# MAGIC ## 1. A governed and reproducible experiment
# MAGIC
# MAGIC Each comparison keeps circuit, horizon, driver and seed fixed. Only the
# MAGIC driving mode changes between `MANAGE`, `BALANCED` and `ATTACK`. Delta
# MAGIC snapshots make the evidence immutable; the campaign and Rust model are
# MAGIC identified by content digests; MLflow retains the campaign report.

# COMMAND ----------

lineage_card = {
    "campaign": campaign_id,
    "manifest": review["campaign_manifest_digest"],
    "model": f"{campaign['model_id']} {campaign['model_version']}",
    "successful Rust simulations": len(successful),
    "normalized metrics": metric_count,
    "parameter profiles": expected["parameter_set_count"],
    "paired groups": expected["paired_group_count"],
    "Databricks job run": lineage["databricks_job_run_id"],
    "metric backfill run": lineage["metric_backfill_job_run_id"],
    "Delta snapshots": (
        f"campaigns@{lineage['campaigns_table_version']}, "
        f"runs@{lineage['experimental_runs_table_version']}, "
        f"metrics@{lineage['experimental_metrics_table_version']}"
    ),
    "MLflow run": campaign["mlflow_run_id"],
}
display(spark.createDataFrame([lineage_card]))

# COMMAND ----------

records = []
for row in successful:
    metadata = json.loads(row["setup_json"])
    records.append(
        metadata
        | driver_metrics(json.loads(row["result_json"]))
        | {
            "experimental_execution_id": row["experimental_execution_id"],
            "seed": row["seed"],
        }
    )

evidence = pd.DataFrame.from_records(records)
group_columns = [
    "parameter_set_id",
    "circuit_slug",
    "horizon",
    "driver_id",
    "seed",
]
grouped: dict[tuple, dict[str, dict]] = defaultdict(dict)
for record in records:
    group = tuple(record[column] for column in group_columns)
    grouped[group][record["mode"]] = record
if len(grouped) != expected["paired_group_count"]:
    raise RuntimeError("paired group count changed")
if any(set(modes) != {"manage", "balanced", "attack"} for modes in grouped.values()):
    raise RuntimeError("a paired group no longer contains all three modes")

paired_records = []
for group, modes in grouped.items():
    winner = min(modes, key=lambda mode: (modes[mode]["mean_lap_ms"], mode))
    paired_records.append(
        dict(zip(group_columns, group))
        | {
            "winner": winner,
            "attack_advantage_over_manage_ms": (
                modes["manage"]["mean_lap_ms"]
                - modes["attack"]["mean_lap_ms"]
            ),
            "attack_extra_wear_percentage_points": (
                modes["attack"]["final_tire_wear_pct"]
                - modes["manage"]["final_tire_wear_pct"]
            ),
            "attack_extra_correction_workload_mj": (
                modes["attack"]["correction_workload_mj"]
                - modes["manage"]["correction_workload_mj"]
            ),
            "physical_ordering": (
                modes["attack"]["control_error_amplitude"]
                > modes["manage"]["control_error_amplitude"]
                and modes["attack"]["correction_workload_mj"]
                > modes["manage"]["correction_workload_mj"]
            ),
        }
    )
paired = pd.DataFrame.from_records(paired_records)

# COMMAND ----------
# MAGIC %md
# MAGIC ## 2. The intended response exists
# MAGIC
# MAGIC More commitment produces more control error and more correction
# MAGIC workload. The model therefore does not ignore the driving decision. The
# MAGIC question is whether that cost becomes large enough, over time, to change
# MAGIC the optimal decision.

# COMMAND ----------

ordering_failures = int((~paired["physical_ordering"]).sum())
if ordering_failures != expected["physical_ordering_failure_count"]:
    raise RuntimeError("physical ordering conclusion changed")

workload_figure = px.box(
    paired,
    x="horizon",
    y="attack_extra_correction_workload_mj",
    color="horizon",
    points="all",
    title="ATTACK adds deterministic correction workload",
    labels={
        "horizon": "Simulation horizon",
        "attack_extra_correction_workload_mj": (
            "ATTACK minus MANAGE correction workload (MJ)"
        ),
    },
    template="plotly_white",
    color_discrete_map={"short": "#d97745", "race-length": "#243746"},
)
workload_figure.update_layout(showlegend=False)
displayHTML(workload_figure.to_html(include_plotlyjs="cdn", full_html=False))

# COMMAND ----------
# MAGIC %md
# MAGIC ## 3. But the cost never changes the winner
# MAGIC
# MAGIC Positive values below mean `ATTACK` is faster than `MANAGE`. Every dot is
# MAGIC positive — including race-length simulations. This is the decisive
# MAGIC result: the workload and wear path is present, but remains too indirect
# MAGIC to counter the immediate force-utilization benefit.

# COMMAND ----------

advantage_figure = px.strip(
    paired,
    x="horizon",
    y="attack_advantage_over_manage_ms",
    color="horizon",
    facet_col="horizon",
    title="ATTACK wins every paired short and race-length comparison",
    labels={
        "horizon": "Simulation horizon",
        "attack_advantage_over_manage_ms": (
            "Mean-lap advantage over MANAGE (ms; positive = ATTACK faster)"
        ),
    },
    template="plotly_white",
    color_discrete_map={"short": "#d97745", "race-length": "#243746"},
)
advantage_figure.add_hline(y=0, line_dash="dash", line_color="#111827")
advantage_figure.update_layout(showlegend=False)
displayHTML(advantage_figure.to_html(include_plotlyjs="cdn", full_html=False))

# COMMAND ----------

profile_rows = []
for parameter_set_id, profile_pairs in paired.groupby("parameter_set_id"):
    short = profile_pairs[profile_pairs["horizon"] == "short"]
    race = profile_pairs[profile_pairs["horizon"] == "race-length"]
    profile_rows.append(
        {
            "parameter_set_id": parameter_set_id,
            "short_attack_win_count": int((short["winner"] == "attack").sum()),
            "race_length_attack_win_count": int(
                (race["winner"] == "attack").sum()
            ),
            "median_short_attack_gain_ms": float(
                short["attack_advantage_over_manage_ms"].median()
            ),
            "median_long_attack_wear_cost_percentage_points": float(
                race["attack_extra_wear_percentage_points"].median()
            ),
        }
    )
profiles = pd.DataFrame.from_records(profile_rows)
if len(profiles) != expected["parameter_set_count"]:
    raise RuntimeError("parameter profile count changed")
if set(profiles["short_attack_win_count"]) != {
    expected["short_group_count_per_parameter_set"]
}:
    raise RuntimeError("short-run winner conclusion changed")
if set(profiles["race_length_attack_win_count"]) != {
    expected["race_length_group_count_per_parameter_set"]
}:
    raise RuntimeError("race-length winner conclusion changed")

surface_figure = px.scatter(
    profiles,
    x="median_short_attack_gain_ms",
    y="median_long_attack_wear_cost_percentage_points",
    hover_name="parameter_set_id",
    title="33 coefficient profiles: more wear, but no long-run trade-off",
    labels={
        "median_short_attack_gain_ms": "Median short ATTACK gain (ms / lap)",
        "median_long_attack_wear_cost_percentage_points": (
            "Median race-length ATTACK wear cost (percentage points)"
        ),
    },
    template="plotly_white",
    color_discrete_sequence=["#d97745"],
)
displayHTML(surface_figure.to_html(include_plotlyjs="cdn", full_html=False))

# COMMAND ----------
# MAGIC %md
# MAGIC ## 4. Decision: improve the physics before calibrating again
# MAGIC
# MAGIC No coefficient profile passed the pre-registered gate. Selecting the
# MAGIC “least bad” point would only disguise a structural limitation. The next
# MAGIC candidate will therefore add a deterministic, observable and accumulated
# MAGIC cost of sustained corrections. It will first be screened locally, then
# MAGIC evaluated on a new immutable Databricks campaign.
# MAGIC
# MAGIC This is a reduced-order engineering model used to exercise Pitgun's
# MAGIC simulation and data platform. The result is **not** a claim of real-world
# MAGIC Formula 1 calibration.

# COMMAND ----------

summary = {
    "review_id": review["review_id"],
    "campaign_id": campaign_id,
    "successful_execution_count": len(successful),
    "normalized_metric_count": metric_count,
    "parameter_set_count": len(profiles),
    "paired_group_count": len(paired),
    "selection_gate_pass_count": 0,
    "candidate_selected": False,
    "decision": review["reviewed_conclusion"]["decision"],
    "automatic_catalog_promotion": False,
}
display(spark.createDataFrame([summary]))
dbutils.notebook.exit(json.dumps(summary, sort_keys=True, separators=(",", ":")))
