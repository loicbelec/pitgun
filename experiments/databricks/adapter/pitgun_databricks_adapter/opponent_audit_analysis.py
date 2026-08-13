"""Pure evidence extraction and aggregation for the opponent audit."""

from __future__ import annotations

from collections import defaultdict
import statistics
from typing import Any


class OpponentAuditResultError(ValueError):
    """Raised when a runner result does not match the immutable audit entry."""


def extract_opponent_audit_evidence(
    entry: dict[str, Any], result: dict[str, Any], manifest: dict[str, Any]
) -> dict[str, Any]:
    """Validate one result and return normalized gameplay evidence."""

    mismatches = {}
    if result.get("model") != {
        "id": manifest["catalog"]["model_id"],
        "version": manifest["catalog"]["model_version"],
        "digest": manifest["catalog"]["model_digest"],
    }:
        mismatches["model"] = result.get("model")
    if result.get("data_pack") != {
        "id": "pitgun.racing.simulation",
        "version": manifest["catalog"]["version"],
        "digest": manifest["catalog"]["simulation_pack_digest"],
    }:
        mismatches["data_pack"] = result.get("data_pack")
    if result.get("seed") != str(entry["seed"]):
        mismatches["seed"] = result.get("seed")
    if result.get("scenario") != {
        "id": "racing.opponent-audit-campaign",
        "version": "1.0.0",
    }:
        mismatches["scenario"] = result.get("scenario")
    if mismatches:
        raise OpponentAuditResultError(
            "runner identity differs from immutable opponent audit plan: "
            + repr(mismatches)
        )

    standings = result.get("summary", {}).get("standings", [])
    players = [row for row in standings if row.get("competitor_id") == "player"]
    if len(standings) != 10 or len(players) != 1:
        raise OpponentAuditResultError(
            "result must contain one player in ten standings"
        )
    player = players[0]
    total_times = [int(row["total_time_ms"]) for row in standings]
    metrics = {
        "racing.opponent-audit.player-position": (float(player["position"]), "rank"),
        "racing.opponent-audit.player-win": (
            1.0 if player["position"] == 1 else 0.0,
            "boolean",
        ),
        "racing.opponent-audit.player-podium": (
            1.0 if player["position"] <= 3 else 0.0,
            "boolean",
        ),
        "racing.opponent-audit.player-gap-to-leader": (
            float(player["gap_to_leader_ms"]),
            "ms",
        ),
        "racing.opponent-audit.player-best-lap": (
            float(player["best_lap_ms"]),
            "ms",
        ),
        "racing.opponent-audit.field-spread": (
            float(max(total_times) - min(total_times)),
            "ms",
        ),
        "racing.opponent-audit.distinct-opponent-tunings": (
            float(entry["distinct_opponent_tunings"]),
            "count",
        ),
        "racing.opponent-audit.distinct-opponent-strategies": (
            float(entry["distinct_opponent_strategies"]),
            "count",
        ),
        "racing.opponent-audit.opponent-budget-minimum": (
            float(entry["opponent_budget_min"]),
            "point",
        ),
        "racing.opponent-audit.opponent-budget-maximum": (
            float(entry["opponent_budget_max"]),
            "point",
        ),
    }
    return {
        "run_key": entry["run_key"],
        "configuration_id": result["configuration_id"],
        "run_id": result["run_id"],
        "circuit_id": entry["circuit_id"],
        "progression": entry["progression"],
        "strategy_profile": entry["strategy_profile"],
        "player_reference": entry["player_reference"],
        "player_position": int(player["position"]),
        "player_gap_to_leader_ms": int(player["gap_to_leader_ms"]),
        "field_spread_ms": max(total_times) - min(total_times),
        "metrics": metrics,
    }


def summarize_opponent_audit(evidence: list[dict[str, Any]]) -> dict[str, Any]:
    """Aggregate successful evidence by controlled reference and circuit."""

    grouped: dict[tuple[str, str], list[dict[str, Any]]] = defaultdict(list)
    for row in evidence:
        grouped[(row["player_reference"], row["circuit_id"])].append(row)

    references: dict[str, dict[str, Any]] = defaultdict(dict)
    for (reference, circuit), rows in sorted(grouped.items()):
        positions = [row["player_position"] for row in rows]
        gaps = [row["player_gap_to_leader_ms"] for row in rows]
        spreads = [row["field_spread_ms"] for row in rows]
        references[reference][circuit] = {
            "successful_run_count": len(rows),
            "win_rate": sum(position == 1 for position in positions) / len(rows),
            "podium_rate": sum(position <= 3 for position in positions) / len(rows),
            "mean_position": statistics.fmean(positions),
            "median_gap_to_leader_ms": statistics.median(gaps),
            "mean_field_spread_ms": statistics.fmean(spreads),
        }

    overall = {}
    for reference in sorted(references):
        rows = [row for row in evidence if row["player_reference"] == reference]
        positions = [row["player_position"] for row in rows]
        gaps = [row["player_gap_to_leader_ms"] for row in rows]
        overall[reference] = {
            "successful_run_count": len(rows),
            "win_rate": sum(position == 1 for position in positions) / len(rows),
            "podium_rate": sum(position <= 3 for position in positions) / len(rows),
            "mean_position": statistics.fmean(positions),
            "median_gap_to_leader_ms": statistics.median(gaps),
        }
    return {
        "schema_version": "pitgun.opponent-audit-report/v1",
        "overall": overall,
        "references": dict(references),
        "automatic_game_or_catalog_promotion": False,
    }
