# Databricks notebook source
"""Review pinned Model V3 thermal evidence without mutating governed state."""

# COMMAND ----------

import json
import re

import plotly.express as px
from pyspark.sql import functions as F
from pyspark.sql import types as T

from pitgun_databricks_adapter import (
    load_thermal_surface_campaign,
    load_thermal_surface_review,
)


dbutils.widgets.text("catalog_name", "workspace")
dbutils.widgets.text("calibration_schema", "pitgun_calibration")
dbutils.widgets.text("campaign_name", "racing-v3-thermal-adequacy-v1")
dbutils.widgets.text("review_name", "racing-v3-thermal-adequacy-review-v1")
dbutils.widgets.text("campaigns_table_version", "")
dbutils.widgets.text("experimental_runs_table_version", "")
dbutils.widgets.text("experimental_metrics_table_version", "")

catalog_name = dbutils.widgets.get("catalog_name")
calibration_schema = dbutils.widgets.get("calibration_schema")
campaign_name = dbutils.widgets.get("campaign_name")
review_name = dbutils.widgets.get("review_name")
campaigns_version_text = dbutils.widgets.get("campaigns_table_version")
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
campaigns_version = validated_version("campaigns_table_version", campaigns_version_text)
runs_version = validated_version("experimental_runs_table_version", runs_version_text)
metrics_version = validated_version(
    "experimental_metrics_table_version", metrics_version_text
)

manifest, manifest_digest = load_thermal_surface_campaign(campaign_name)
review, review_digest = load_thermal_surface_review(review_name)
campaign_id = manifest["campaign_id"]
if review["campaign_id"] != campaign_id:
    raise RuntimeError("thermal review and campaign identities differ")
if review["manifest_digest"] != manifest_digest:
    raise RuntimeError("thermal review and campaign manifest digests differ")
requested_versions = {
    "campaigns": campaigns_version,
    "experimental_runs": runs_version,
    "experimental_metrics": metrics_version,
}
if requested_versions != review["evidence_versions"]:
    raise RuntimeError("job parameters do not match the reviewed Delta snapshots")

calibration = f"`{catalog_name}`.`{calibration_schema}`"
campaigns_table = f"{calibration}.campaigns"
runs_table = f"{calibration}.experimental_runs"
metrics_table = f"{calibration}.experimental_metrics"

# COMMAND ----------

campaign_rows = (
    spark.read.option("versionAsOf", campaigns_version)
    .table(campaigns_table)
    .where(F.col("campaign_id") == campaign_id)
    .collect()
)
if len(campaign_rows) != 1:
    raise RuntimeError("pinned campaign snapshot must contain exactly one campaign")
campaign = campaign_rows[0].asDict()
if campaign["manifest_digest"] != manifest_digest or campaign["status"] != "COMPLETED":
    raise RuntimeError("pinned campaign is not the completed reviewed campaign")

runs = (
    spark.read.option("versionAsOf", runs_version)
    .table(runs_table)
    .where(F.col("campaign_id") == campaign_id)
)
metrics = (
    spark.read.option("versionAsOf", metrics_version)
    .table(metrics_table)
    .where(F.col("campaign_id") == campaign_id)
)

execution_count = runs.count()
successful_count = runs.where(F.col("execution_status") == "SUCCESS").count()
metric_count = metrics.count()
metric_execution_count = metrics.select("experimental_execution_id").distinct().count()
expected = review["observed_evidence"]
if execution_count != expected["planned_execution_count"]:
    raise RuntimeError("pinned runs do not reconcile with the immutable plan")
if successful_count != expected["successful_execution_count"]:
    raise RuntimeError("pinned runs do not reproduce the reviewed success count")
if metric_execution_count != successful_count or metric_count != successful_count * 8:
    raise RuntimeError("pinned metrics do not provide eight metrics per successful run")

# COMMAND ----------

metadata_schema = T.StructType(
    [
        T.StructField("vehicle_family", T.StringType()),
        T.StructField("split", T.StringType()),
        T.StructField("workload", T.StringType()),
        T.StructField("cooling_points", T.IntegerType()),
    ]
)
run_metadata = (
    runs.where(F.col("execution_status") == "SUCCESS")
    .withColumn("metadata", F.from_json("setup_json", metadata_schema))
    .select(
        "experimental_execution_id",
        "response_id",
        "seed",
        "circuit_id",
        F.col("metadata.vehicle_family").alias("vehicle_family"),
        F.col("metadata.split").alias("split"),
        F.col("metadata.workload").alias("workload"),
        F.col("metadata.cooling_points").alias("cooling_points"),
    )
)
metric_pivot = (
    metrics.groupBy("experimental_execution_id")
    .pivot(
        "metric_id",
        [
            "racing.total-time",
            "racing.maximum-engine-temperature",
            "racing.engine-derated-fraction",
        ],
    )
    .agg(F.max("metric_value"))
    .select(
        "experimental_execution_id",
        F.col("`racing.total-time`").alias("total_time_ms"),
        F.col("`racing.maximum-engine-temperature`").alias(
            "maximum_temperature_c"
        ),
        F.col("`racing.engine-derated-fraction`").alias("derated_fraction"),
    )
)
evidence = run_metadata.join(metric_pivot, "experimental_execution_id")
pathological = (F.col("maximum_temperature_c") > F.lit(180.0)) | (
    F.col("derated_fraction") > F.lit(0.5)
)

family_summary = (
    evidence.groupBy("vehicle_family")
    .agg(
        F.count("*").alias("execution_count"),
        F.count_if(pathological).alias("pathological_execution_count"),
        F.round(F.max("maximum_temperature_c"), 3).alias("maximum_temperature_c"),
        F.round(F.max("derated_fraction"), 6).alias("maximum_derated_fraction"),
    )
    .orderBy("vehicle_family")
)
family_rows = family_summary.collect()
observed_family_counts = {
    row["vehicle_family"]: row["execution_count"] for row in family_rows
}
observed_pathology_counts = {
    row["vehicle_family"]: row["pathological_execution_count"]
    for row in family_rows
}
if observed_family_counts != expected["vehicle_family_execution_counts"]:
    raise RuntimeError("vehicle-family counts differ from the recorded review")
if observed_pathology_counts != expected["pathological_execution_counts"]:
    raise RuntimeError("pathology counts differ from the recorded review")

display(family_summary)

# COMMAND ----------

cooling_summary = (
    evidence.where(F.col("workload").isin("long", "full-race"))
    .groupBy("vehicle_family", "split", "cooling_points")
    .agg(
        F.count("*").alias("execution_count"),
        F.count_if(pathological).alias("pathological_execution_count"),
        F.round(F.expr("percentile_approx(total_time_ms, 0.5)"), 1).alias(
            "median_total_time_ms"
        ),
        F.round(F.expr("percentile_approx(maximum_temperature_c, 0.5)"), 3).alias(
            "median_maximum_temperature_c"
        ),
        F.round(F.expr("percentile_approx(derated_fraction, 0.5)"), 6).alias(
            "median_derated_fraction"
        ),
    )
    .orderBy("vehicle_family", "split", "cooling_points")
)
display(cooling_summary)

candidate_response_ids = sorted(
    {
        response_id
        for response_id in (
            review.get("next_gate", {}).get("candidate_centres", [])
            + [
                row.get("candidate_parameter_set_id")
                for row in review["per_family_verdicts"].values()
            ]
        )
        if response_id
    }
)
if not candidate_response_ids:
    raise RuntimeError("thermal review does not identify any response to render")
candidate_surface = (
    evidence.where(
        F.col("workload").isin("long", "full-race")
        & F.col("response_id").isin(*candidate_response_ids)
    )
    .groupBy(
        "vehicle_family",
        "response_id",
        "split",
        "circuit_id",
        "cooling_points",
    )
    .agg(F.avg("total_time_ms").alias("total_time_ms"))
    .orderBy(
        "vehicle_family",
        "response_id",
        "split",
        "circuit_id",
        "cooling_points",
    )
)
candidate_plot = candidate_surface.toPandas()
figure = px.line(
    candidate_plot,
    x="cooling_points",
    y="total_time_ms",
    color="response_id",
    line_dash="circuit_id",
    facet_col="vehicle_family",
    markers=True,
    title="Pinned thermal candidate response by family and circuit",
    labels={
        "cooling_points": "Cooling development points",
        "total_time_ms": "Total time (ms)",
        "response_id": "Parameter set",
        "circuit_id": "Circuit",
    },
)
displayHTML(figure.to_html(include_plotlyjs="cdn", full_html=False))
display(candidate_surface)

# COMMAND ----------

report = {
    "schema_version": "pitgun.racing-v3-thermal-databricks-review-report/v1",
    "campaign_id": campaign_id,
    "manifest_digest": manifest_digest,
    "review_id": review["id"],
    "review_digest": review_digest,
    "evidence_versions": requested_versions,
    "execution_count": execution_count,
    "metric_row_count": metric_count,
    "per_family_verdicts": review["per_family_verdicts"],
    "next_gate": review["next_gate"],
    "automatic_catalog_promotion": False,
}
dbutils.notebook.exit(json.dumps(report, sort_keys=True, separators=(",", ":")))
