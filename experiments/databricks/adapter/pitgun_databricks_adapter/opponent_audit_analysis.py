"""Pure evidence extraction and aggregation for the opponent audit."""

from __future__ import annotations

from collections import defaultdict
import math
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


def _mean(rows: list[dict[str, Any]], key: str) -> float:
    return statistics.fmean(float(row[key]) for row in rows)


def _pearson(rows: list[dict[str, Any]], left: str, right: str) -> float | None:
    xs = [float(row[left]) for row in rows]
    ys = [float(row[right]) for row in rows]
    if len(rows) < 2 or len(set(xs)) < 2 or len(set(ys)) < 2:
        return None
    x_mean = statistics.fmean(xs)
    y_mean = statistics.fmean(ys)
    numerator = sum((x - x_mean) * (y - y_mean) for x, y in zip(xs, ys))
    denominator = math.sqrt(
        sum((x - x_mean) ** 2 for x in xs)
        * sum((y - y_mean) ** 2 for y in ys)
    )
    return numerator / denominator if denominator else None


def diagnose_opponent_audit(
    manifest: dict[str, Any],
    evidence: list[dict[str, Any]],
    lineage: dict[str, Any],
) -> dict[str, Any]:
    """Decompose an exact completed audit without selecting an AI policy.

    Setup comparisons are paired because only the controlled player reference
    changes. Strategy and progression comparisons remain descriptive: their
    source contracts or physical context also change.
    """

    expected = {entry["run_key"]: entry for entry in manifest["runs"]}
    actual = {row["run_key"]: row for row in evidence}
    if len(actual) != len(evidence) or set(actual) != set(expected):
        raise OpponentAuditResultError(
            "diagnosis requires exactly one successful result for every manifest run"
        )

    rows = []
    for run_key, row in actual.items():
        entry = expected[run_key]
        rows.append(
            {
                **row,
                "seed": int(entry["seed"]),
                "source_scenario_id": entry["source_scenario_id"],
                "source_contract_digest": entry["source_contract_digest"],
                "distinct_opponent_tunings": int(
                    entry["distinct_opponent_tunings"]
                ),
                "distinct_opponent_strategies": int(
                    entry["distinct_opponent_strategies"]
                ),
                "opponent_budget_min": int(entry["opponent_budget_min"]),
                "opponent_budget_max": int(entry["opponent_budget_max"]),
            }
        )

    # Exact paired setup effect: same opponents, player budget, strategy, and seed.
    setup_groups: dict[str, dict[str, dict[str, Any]]] = defaultdict(dict)
    for row in rows:
        setup_groups[row["source_scenario_id"]][row["player_reference"]] = row
    setup_pairs = []
    for source_id, pair in sorted(setup_groups.items()):
        if set(pair) != {"neutral", "circuit-informed"}:
            raise OpponentAuditResultError(f"incomplete setup pair: {source_id}")
        neutral = pair["neutral"]
        informed = pair["circuit-informed"]
        if neutral["source_contract_digest"] != informed["source_contract_digest"]:
            raise OpponentAuditResultError(f"opponents changed inside setup pair: {source_id}")
        setup_pairs.append(
            {
                "source_scenario_id": source_id,
                "circuit_id": neutral["circuit_id"],
                "progression": neutral["progression"],
                "strategy_profile": neutral["strategy_profile"],
                "seed": neutral["seed"],
                "position_delta_informed_minus_neutral": (
                    informed["player_position"] - neutral["player_position"]
                ),
                "gap_delta_informed_minus_neutral_ms": (
                    informed["player_gap_to_leader_ms"]
                    - neutral["player_gap_to_leader_ms"]
                ),
            }
        )

    setup_by_circuit = {}
    for circuit in sorted({row["circuit_id"] for row in setup_pairs}):
        circuit_rows = [row for row in setup_pairs if row["circuit_id"] == circuit]
        setup_by_circuit[circuit] = {
            "pair_count": len(circuit_rows),
            "mean_position_delta_informed_minus_neutral": _mean(
                circuit_rows, "position_delta_informed_minus_neutral"
            ),
            "median_gap_delta_informed_minus_neutral_ms": statistics.median(
                row["gap_delta_informed_minus_neutral_ms"] for row in circuit_rows
            ),
            "position_improvement_rate": sum(
                row["position_delta_informed_minus_neutral"] < 0
                for row in circuit_rows
            )
            / len(circuit_rows),
        }

    stability_groups: dict[tuple[str, str, str], list[int]] = defaultdict(list)
    for row in setup_pairs:
        stability_groups[
            (row["circuit_id"], row["progression"], row["strategy_profile"])
        ].append(row["gap_delta_informed_minus_neutral_ms"])
    stable_groups = 0
    for deltas in stability_groups.values():
        if len(deltas) != len(manifest["matrix"]["seeds"]):
            raise OpponentAuditResultError("setup stability group has missing seeds")
        stable_groups += int(
            all(value >= 0 for value in deltas)
            or all(value <= 0 for value in deltas)
        )

    # Strategy pairs are intentionally labelled as confounded when the source
    # opponent contract differs between the two authored strategy scenarios.
    strategy_groups: dict[
        tuple[str, str, int, str], dict[str, dict[str, Any]]
    ] = defaultdict(dict)
    for row in rows:
        strategy_groups[
            (
                row["circuit_id"],
                row["progression"],
                row["seed"],
                row["player_reference"],
            )
        ][row["strategy_profile"]] = row
    strategy_pairs = []
    for key, pair in sorted(strategy_groups.items()):
        if set(pair) != {"balanced-one-stop", "late-one-stop"}:
            raise OpponentAuditResultError(f"incomplete strategy pair: {key}")
        balanced = pair["balanced-one-stop"]
        late = pair["late-one-stop"]
        strategy_pairs.append(
            {
                "circuit_id": key[0],
                "progression": key[1],
                "seed": key[2],
                "player_reference": key[3],
                "same_source_contract": (
                    balanced["source_contract_digest"]
                    == late["source_contract_digest"]
                ),
                "position_delta_late_minus_balanced": (
                    late["player_position"] - balanced["player_position"]
                ),
                "gap_delta_late_minus_balanced_ms": (
                    late["player_gap_to_leader_ms"]
                    - balanced["player_gap_to_leader_ms"]
                ),
            }
        )

    progression = {}
    player_budgets = {
        row["id"]: int(row["playerBudget"])
        for row in manifest["matrix"]["progression"]
    }
    for band in sorted(player_budgets):
        band_rows = [row for row in rows if row["progression"] == band]
        progression[band] = {
            "run_count": len(band_rows),
            "player_budget": player_budgets[band],
            "opponent_budget_minimum": min(
                row["opponent_budget_min"] for row in band_rows
            ),
            "opponent_budget_maximum": max(
                row["opponent_budget_max"] for row in band_rows
            ),
            "mean_player_position": _mean(band_rows, "player_position"),
            "median_gap_to_leader_ms": statistics.median(
                row["player_gap_to_leader_ms"] for row in band_rows
            ),
        }

    diversity = {
        "run_count": len(rows),
        "distinct_opponent_tuning_counts": sorted(
            {row["distinct_opponent_tunings"] for row in rows}
        ),
        "distinct_opponent_strategy_counts": sorted(
            {row["distinct_opponent_strategies"] for row in rows}
        ),
        "strategy_diversity_correlations": {
            "player_position": _pearson(
                rows, "distinct_opponent_strategies", "player_position"
            ),
            "player_gap_to_leader_ms": _pearson(
                rows,
                "distinct_opponent_strategies",
                "player_gap_to_leader_ms",
            ),
            "field_spread_ms": _pearson(
                rows, "distinct_opponent_strategies", "field_spread_ms"
            ),
        },
        "correlations_are_descriptive_not_causal": True,
    }

    same_contract_count = sum(row["same_source_contract"] for row in strategy_pairs)
    evidence_statements = [
        f"{len(setup_pairs)} exact setup pairs preserve the source opponent contract.",
        f"{stable_groups}/{len(stability_groups)} setup-effect groups keep the "
        "same gap direction across all seeds.",
        "Opponent tuning diversity is constant across the campaign."
        if len(diversity["distinct_opponent_tuning_counts"]) == 1
        else "Opponent tuning diversity varies across the campaign.",
        f"{same_contract_count}/{len(strategy_pairs)} strategy pairs preserve "
        "the source opponent contract.",
    ]
    inference_statements = [
        "Circuit-specific setup alignment is a plausible contributor only where "
        "paired effects are stable across seeds.",
        "Strategy-diversity correlations can prioritize a follow-up experiment "
        "but cannot identify a causal strategy effect.",
    ]
    unresolved = [
        "Progression changes budget, era, vehicle, and opponent contracts "
        "together; this campaign cannot isolate one cause.",
        "A causal strategy comparison requires frozen opponents with only the "
        "controlled player strategy changed."
        if same_contract_count != len(strategy_pairs)
        else "Strategy pairs preserve contracts, but the campaign was not "
        "authored as a causal strategy experiment.",
        "Constant tuning-count diversity cannot explain run-to-run performance "
        "variation; setup quality still requires a dedicated measure.",
    ]

    return {
        "schema_version": "pitgun.opponent-diagnosis-report/v1",
        "campaign_id": manifest["campaign_id"],
        "lineage": lineage,
        "sample": {
            "successful_run_count": len(rows),
            "setup_pair_count": len(setup_pairs),
            "strategy_pair_count": len(strategy_pairs),
        },
        "setup_alignment": {
            "comparison": "circuit-informed minus neutral",
            "negative_delta_is_better_for_player": True,
            "by_circuit": setup_by_circuit,
            "seed_direction_stability": {
                "group_count": len(stability_groups),
                "stable_group_count": stable_groups,
                "stable_group_rate": stable_groups / len(stability_groups),
            },
        },
        "strategy_response": {
            "comparison": "late-one-stop minus balanced-one-stop",
            "pair_count": len(strategy_pairs),
            "same_source_contract_pair_count": same_contract_count,
            "mean_position_delta": _mean(
                strategy_pairs, "position_delta_late_minus_balanced"
            ),
            "median_gap_delta_ms": statistics.median(
                row["gap_delta_late_minus_balanced_ms"]
                for row in strategy_pairs
            ),
            "causal_interpretation_allowed": same_contract_count
            == len(strategy_pairs),
        },
        "progression_and_budget": {
            "groups": progression,
            "causal_interpretation_allowed": False,
        },
        "diversity": diversity,
        "claims": {
            "evidence": evidence_statements,
            "inference": inference_statements,
            "unresolved": unresolved,
        },
        "automatic_game_or_catalog_promotion": False,
        "policy_selected": False,
    }
