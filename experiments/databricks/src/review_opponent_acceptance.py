# Databricks notebook source
"""Render the pinned Catalog 1.9 opponent acceptance decision."""

# COMMAND ----------

import html
import json
import re
import statistics

import pandas as pd
import plotly.express as px
from pyspark.sql import functions as F

from pitgun_databricks_adapter import (
    extract_opponent_acceptance_evidence,
    load_opponent_acceptance_campaign,
    load_opponent_acceptance_review,
    summarize_opponent_acceptance,
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
requested_versions = {
    "campaigns": validated_version("campaigns_table_version", campaigns_version_text),
    "runs": validated_version("runs_table_version", runs_version_text),
    "metrics": validated_version("metrics_table_version", metrics_version_text),
}
calibration = f"`{catalog_name}`.`{calibration_schema}`"
campaigns_table = f"{calibration}.campaigns"
runs_table = f"{calibration}.runs"
metrics_table = f"{calibration}.metrics"

manifest, manifest_digest = load_opponent_acceptance_campaign()
review, review_digest = load_opponent_acceptance_review()
campaign_id = manifest["campaign_id"]
if review["campaign_id"] != campaign_id:
    raise RuntimeError("review and campaign identities differ")
if review["manifest_digest"] != manifest_digest:
    raise RuntimeError("review and campaign manifest digests differ")
if review["evidence_versions"] != requested_versions:
    raise RuntimeError("job parameters do not match the reviewed Delta snapshots")

# COMMAND ----------

campaign_rows = (
    spark.read.option("versionAsOf", requested_versions["campaigns"])
    .table(campaigns_table)
    .where(F.col("campaign_id") == campaign_id)
    .collect()
)
if len(campaign_rows) != 1:
    raise RuntimeError("pinned campaigns snapshot must contain exactly one campaign")
campaign = campaign_rows[0].asDict()
if campaign["status"] != "COMPLETED":
    raise RuntimeError("pinned opponent acceptance campaign is not completed")
if campaign["manifest_digest"] != manifest_digest:
    raise RuntimeError("pinned campaign manifest digest changed")
if campaign["mlflow_run_id"] != review["databricks"]["mlflow_run_id"]:
    raise RuntimeError("pinned campaign MLflow lineage changed")
if not review["source_merge_commit"].startswith(campaign["source_git_revision"]):
    raise RuntimeError("pinned campaign source revision changed")

run_rows = (
    spark.read.option("versionAsOf", requested_versions["runs"])
    .table(runs_table)
    .where(F.col("campaign_id") == campaign_id)
    .select(
        "execution_key",
        "execution_status",
        "run_id",
        "canonical_result_digest",
        "result_json",
    )
    .collect()
)
expected = review["observed_evidence"]
if len(run_rows) != expected["planned_run_count"]:
    raise RuntimeError("pinned runs do not reconcile with the immutable plan")
if sum(row["execution_status"] == "SUCCESS" for row in run_rows) != expected[
    "successful_run_count"
]:
    raise RuntimeError("pinned runs do not reproduce the reviewed success count")
if len({row["execution_key"] for row in run_rows}) != len(run_rows):
    raise RuntimeError("pinned runs contain duplicate execution keys")

metric_rows = (
    spark.read.option("versionAsOf", requested_versions["metrics"])
    .table(metrics_table)
    .where(F.col("campaign_id") == campaign_id)
    .select("run_id", "metric_id")
    .collect()
)
if len(metric_rows) != expected["metric_row_count"]:
    raise RuntimeError("pinned metrics do not reproduce the reviewed metric count")
metrics_by_run = {}
for row in metric_rows:
    metrics_by_run.setdefault(row["run_id"], set()).add(row["metric_id"])
expected_metric_ids = {
    "racing.opponent-acceptance.player-position",
    "racing.opponent-acceptance.player-win",
    "racing.opponent-acceptance.player-podium",
    "racing.opponent-acceptance.player-gap-to-leader",
    "racing.opponent-acceptance.player-best-lap",
    "racing.opponent-acceptance.field-spread",
    "racing.opponent-acceptance.player-budget",
    "racing.opponent-acceptance.opponent-budget-minimum",
    "racing.opponent-acceptance.opponent-budget-median",
    "racing.opponent-acceptance.opponent-budget-maximum",
    "racing.opponent-acceptance.player-budget-delta-to-opponent-median",
}
if len(metrics_by_run) != expected["successful_run_count"] or any(
    metric_ids != expected_metric_ids
    for metric_ids in metrics_by_run.values()
):
    raise RuntimeError("pinned metrics are incomplete or contain duplicate identities")

entry_by_key = {entry["run_key"]: entry for entry in manifest["runs"]}
row_by_key = {row["execution_key"]: row for row in run_rows}
if set(row_by_key) != set(entry_by_key):
    raise RuntimeError("pinned run keys differ from the checksummed campaign")

retry_identity = expected["retry_identity"]
for sentinel in retry_identity["sentinels"]:
    row = row_by_key.get(sentinel["run_key"])
    if row is None or row["run_id"] != sentinel["run_id"]:
        raise RuntimeError("pinned deterministic retry run identity changed")
    if row["canonical_result_digest"] != sentinel["canonical_result_digest"]:
        raise RuntimeError("pinned deterministic retry result digest changed")

evidence = [
    extract_opponent_acceptance_evidence(
        entry_by_key[row["execution_key"]],
        json.loads(row["result_json"]),
        manifest,
    )
    for row in run_rows
]
summary = summarize_opponent_acceptance(evidence, retry_identity)
if summary["diagnostics"] != expected["diagnostics"]:
    raise RuntimeError("pinned acceptance diagnostics differ from the human review")
if summary["budget_parity"] != expected["budget_parity"]:
    raise RuntimeError("pinned budget parity differs from the human review")
if not all(summary["observed_gates"].values()):
    raise RuntimeError("one or more pre-registered acceptance gates failed")

# COMMAND ----------

reference_labels = {
    "naive": "Naive",
    "balanced": "Balanced",
    "circuit-informed": "Circuit-informed",
}
circuit_order = ["BUDAPEST", "MONACO", "MONZA", "SINGAPORE", "SUZUKA"]
reference_order = ["naive", "balanced", "circuit-informed"]

position_records = []
for circuit in circuit_order:
    for reference in reference_order:
        aggregate = summary["by_circuit"][reference][circuit]
        position_records.append(
            {
                "circuit": circuit.title(),
                "reference": reference_labels[reference],
                "mean_position": aggregate["mean_position"],
                "median_gap_seconds": aggregate["median_gap_to_leader_ms"] / 1000.0,
                "podium_rate": aggregate["podium_rate"],
            }
        )
position_frame = pd.DataFrame.from_records(position_records)
position_matrix = position_frame.pivot(
    index="circuit", columns="reference", values="mean_position"
).reindex(
    index=[circuit.title() for circuit in circuit_order],
    columns=[reference_labels[value] for value in reference_order],
)
position_figure = px.imshow(
    position_matrix,
    text_auto=".2f",
    aspect="auto",
    color_continuous_scale="RdYlGn_r",
    zmin=1,
    zmax=10,
    title="Catalog 1.9 — mean player finishing position",
    labels={"x": "Player reference", "y": "Circuit", "color": "Mean position"},
    template="plotly_white",
    height=520,
)
position_figure.update_layout(font_family="Arial", title_x=0.02)
displayHTML(position_figure.to_html(include_plotlyjs="cdn", full_html=False))

# COMMAND ----------

paired_groups = {}
for effect in summary["paired_effects"]:
    key = (effect["circuit_id"], effect["progression"])
    paired_groups.setdefault(key, []).append(effect["position_gain_over_naive"])
gain_records = [
    {
        "circuit": circuit.title(),
        "progression": progression.title(),
        "median_position_gain": statistics.median(values),
    }
    for (circuit, progression), values in paired_groups.items()
]
gain_frame = pd.DataFrame.from_records(gain_records)
gain_matrix = gain_frame.pivot(
    index="circuit", columns="progression", values="median_position_gain"
).reindex(
    index=[circuit.title() for circuit in circuit_order],
    columns=["Early", "Mid", "Late"],
)
gain_figure = px.imshow(
    gain_matrix,
    text_auto="+.1f",
    aspect="auto",
    color_continuous_scale="RdYlGn",
    color_continuous_midpoint=0,
    title="Circuit-informed gain over the naive player",
    labels={
        "x": "Progression",
        "y": "Circuit",
        "color": "Positions gained",
    },
    template="plotly_white",
    height=520,
)
gain_figure.update_layout(font_family="Arial", title_x=0.02)
displayHTML(gain_figure.to_html(include_plotlyjs="cdn", full_html=False))

# COMMAND ----------

verdict = review["human_decision"]
verdict_rows = []
for circuit in circuit_order:
    circuit_verdict = review["circuit_verdicts"][circuit]
    verdict_rows.append(
        "<tr>"
        f"<td>{html.escape(circuit.title())}</td>"
        f"<td><strong>{html.escape(circuit_verdict['verdict'])}</strong></td>"
        f"<td>{html.escape(circuit_verdict['reason'])}</td>"
        "</tr>"
    )
displayHTML(
    """
    <section style="font-family:Arial,sans-serif;max-width:1100px">
      <div style="border-left:8px solid #d3542f;background:#f7f1e7;padding:20px 24px;margin:16px 0">
        <div style="font-size:13px;letter-spacing:.12em;color:#6d655c">HUMAN BALANCE VERDICT</div>
        <div style="font-size:32px;font-weight:800;margin:6px 0">%s</div>
        <div style="font-size:16px;line-height:1.5">%s</div>
      </div>
      <table style="border-collapse:collapse;width:100%%;font-size:14px">
        <thead><tr style="background:#18232b;color:white;text-align:left">
          <th style="padding:10px">Circuit</th><th style="padding:10px">Verdict</th><th style="padding:10px">Evidence</th>
        </tr></thead>
        <tbody>%s</tbody>
      </table>
      <p style="color:#6d655c;margin-top:18px">
        Budget parity: maximum absolute delta %.1f points. Deterministic retries: %s.
        No policy, catalog, or game release was mutated by this review.
      </p>
    </section>
    """
    % (
        html.escape(verdict["verdict"]),
        html.escape(verdict["reason"]),
        "".join(verdict_rows),
        summary["budget_parity"]["maximum_absolute_player_delta_to_opponent_median"],
        "identical" if retry_identity["all_identical"] else "different",
    )
)

# COMMAND ----------

report = {
    "schema_version": "pitgun.opponent-acceptance-review-report/v1",
    "review_id": review["id"],
    "review_digest": review_digest,
    "campaign_id": campaign_id,
    "manifest_digest": manifest_digest,
    "mlflow_run_id": campaign["mlflow_run_id"],
    "evidence_versions": requested_versions,
    "successful_run_count": len(evidence),
    "metric_row_count": len(metric_rows),
    "human_decision": review["human_decision"],
    "circuit_verdicts": review["circuit_verdicts"],
    "next_gate": review["next_gate"],
    "automatic_policy_mutation": False,
    "automatic_catalog_promotion": False,
    "automatic_game_promotion": False,
}
dbutils.notebook.exit(json.dumps(report, sort_keys=True, separators=(",", ":")))
