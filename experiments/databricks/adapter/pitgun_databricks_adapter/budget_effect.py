"""Load the immutable controlled Racing development-budget campaign."""

from __future__ import annotations

import copy
from collections import defaultdict
import hashlib
import importlib.resources
import json
import statistics
from typing import Any


CAMPAIGN_NAME = "racing-budget-effect-v1"
SCHEMA_VERSION = "pitgun.budget-effect-campaign/v1"
TREATMENTS = {"field-090": 90, "field-100": 100, "field-110": 110}
POINT_KEYS = ("aero_points", "chassis_points", "cooling_points", "engine_points")


class BudgetEffectManifestError(ValueError):
    """Raised when campaign identities or controlled triplets differ."""


def _sha256(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


def _compact(value: Any) -> bytes:
    return json.dumps(
        value, sort_keys=True, separators=(",", ":"), ensure_ascii=False
    ).encode()


def _player(scenario: dict[str, Any]) -> dict[str, Any]:
    players = [
        row for row in scenario["request"]["competitors"] if row.get("is_player")
    ]
    if len(players) != 1 or players[0].get("id") != "player":
        raise BudgetEffectManifestError("scenario must contain one controlled player")
    return players[0]


def _projection(scenario: dict[str, Any]) -> dict[str, Any]:
    projection = copy.deepcopy(scenario)
    player = _player(projection)
    player.pop("budget_cap")
    for key in POINT_KEYS:
        player["tuning"].pop(key)
    return projection


def _balanced_points(budget: int) -> dict[str, int]:
    quotient, remainder = divmod(budget, 4)
    return {
        key: quotient + (1 if index < remainder else 0)
        for index, key in enumerate(POINT_KEYS)
    }


def _treated_budget(field_median: int, percentage: int) -> int:
    return (field_median * percentage + 50) // 100


def _validate_manifest(
    manifest: dict[str, Any],
    resources: dict[str, bytes],
    source_manifest_digest: str,
) -> None:
    if manifest.get("schema_version") != SCHEMA_VERSION:
        raise BudgetEffectManifestError("unsupported budget-effect campaign")
    if manifest.get("planned_triplet_count") != 45:
        raise BudgetEffectManifestError("budget campaign must contain 45 triplets")
    runs = manifest.get("runs", [])
    if manifest.get("planned_run_count") != 135 or len(runs) != 135:
        raise BudgetEffectManifestError("budget campaign must contain 135 runs")
    if manifest.get("source", {}).get("manifest_digest") != source_manifest_digest:
        raise BudgetEffectManifestError("source strategy campaign identity changed")
    controlled = manifest.get("controlled_input", {})
    if controlled.get("only_allowed_triplet_difference") != (
        "player.development_budget_and_balanced_point_allocation"
    ):
        raise BudgetEffectManifestError("controlled treatment boundary changed")
    if {
        row.get("id"): row.get("field_median_percentage")
        for row in controlled.get("treatments", [])
    } != TREATMENTS:
        raise BudgetEffectManifestError("budget treatments changed")
    if (
        controlled.get("player_reference") != "neutral"
        or controlled.get("player_strategy") != "balanced-one-stop"
        or controlled.get("allocation") != "deterministic-balanced-four-axis"
    ):
        raise BudgetEffectManifestError("controlled player boundary changed")
    governance = manifest.get("governance", {})
    if any(
        governance.get(key) is not False
        for key in (
            "private_player_data_allowed",
            "automatic_game_or_catalog_promotion",
            "automatic_budget_target_selection_allowed",
        )
    ):
        raise BudgetEffectManifestError("budget campaign governance is unsafe")

    run_keys = [run.get("run_key") for run in runs]
    if len(set(run_keys)) != 135 or set(resources) != set(run_keys):
        raise BudgetEffectManifestError("scenario resources and run keys differ")
    triplets: dict[str, list[dict[str, Any]]] = {}
    for run in runs:
        resource = run["scenario_resource"]
        data = resources.get(resource)
        if data is None or _sha256(data) != run["scenario_resource_digest"]:
            raise BudgetEffectManifestError(f"scenario changed: {resource}")
        scenario = json.loads(data)
        if scenario.get("scenario") != {
            "id": "racing.budget-effect-campaign",
            "version": "1.0.0",
        }:
            raise BudgetEffectManifestError(f"scenario identity changed: {resource}")
        catalog = manifest["catalog"]
        if scenario.get("model") != {
            "id": catalog["model_id"],
            "version": catalog["model_version"],
            "digest": catalog["model_digest"],
        }:
            raise BudgetEffectManifestError(f"model identity changed: {resource}")
        if scenario.get("data_pack") != {
            "id": "pitgun.racing.simulation",
            "version": catalog["version"],
            "digest": catalog["simulation_pack_digest"],
        }:
            raise BudgetEffectManifestError(f"data pack changed: {resource}")
        player = _player(scenario)
        expected_budget = _treated_budget(
            int(run["field_median_budget"]), int(run["treatment_percentage"])
        )
        allocation = {key: int(player["tuning"][key]) for key in POINT_KEYS}
        if (
            run["treatment_percentage"] != TREATMENTS.get(run["treatment"])
            or player["budget_cap"] != expected_budget
            or run["player_budget"] != expected_budget
            or allocation != _balanced_points(expected_budget)
            or run["player_allocation"] != allocation
            or sum(allocation.values()) != expected_budget
        ):
            raise BudgetEffectManifestError(f"invalid budget treatment: {resource}")
        if _sha256(_compact(_projection(scenario))) != run[
            "triplet_invariant_digest"
        ]:
            raise BudgetEffectManifestError(f"triplet invariant changed: {resource}")
        triplets.setdefault(run["triplet_key"], []).append(run)

    if len(triplets) != 45:
        raise BudgetEffectManifestError("triplet keys are incomplete")
    for triplet_key, triplet in triplets.items():
        if {run["treatment"] for run in triplet} != set(TREATMENTS):
            raise BudgetEffectManifestError(
                f"triplet treatments are incomplete: {triplet_key}"
            )
        if len({run["triplet_invariant_digest"] for run in triplet}) != 1:
            raise BudgetEffectManifestError(f"triplet input changed: {triplet_key}")
        if len({run["source_opponent_contract_digest"] for run in triplet}) != 1:
            raise BudgetEffectManifestError(
                f"opponent field changed inside triplet: {triplet_key}"
            )
        if len({run["player_budget"] for run in triplet}) != 3:
            raise BudgetEffectManifestError(
                f"triplet budgets are not distinct: {triplet_key}"
            )


def load_budget_effect_campaign() -> tuple[dict[str, Any], str]:
    """Return the checksummed campaign after validating all 45 triplets."""

    package = importlib.resources.files("pitgun_databricks_adapter")
    campaign_root = package.joinpath("campaigns")
    manifest_name = CAMPAIGN_NAME + ".json"
    manifest_bytes = campaign_root.joinpath(manifest_name).read_bytes()
    checksum_parts = (
        campaign_root.joinpath(CAMPAIGN_NAME + ".sha256").read_text().split()
    )
    digest = hashlib.sha256(manifest_bytes).hexdigest()
    if (
        len(checksum_parts) != 2
        or checksum_parts[1] != manifest_name
        or checksum_parts[0] != digest
    ):
        raise BudgetEffectManifestError("campaign manifest checksum mismatch")

    source_bytes = campaign_root.joinpath("racing-strategy-effect-v1.json").read_bytes()
    source_digest = _sha256(source_bytes)
    manifest = json.loads(manifest_bytes)
    resources = {
        run["scenario_resource"]: package.joinpath(
            "scenarios", f"{run['scenario_resource']}.json"
        ).read_bytes()
        for run in manifest.get("runs", [])
    }
    _validate_manifest(manifest, resources, source_digest)
    return manifest, "sha256:" + digest


def materialize_budget_effect_plan(
    manifest: dict[str, Any],
) -> list[dict[str, Any]]:
    """Return the explicit reviewed budget execution plan."""

    return [dict(run) for run in manifest["runs"]]


def extract_budget_effect_evidence(
    entry: dict[str, Any], result: dict[str, Any], manifest: dict[str, Any]
) -> dict[str, Any]:
    """Validate one result and return normalized causal budget evidence."""

    catalog = manifest["catalog"]
    expected_identities = {
        "scenario": {"id": "racing.budget-effect-campaign", "version": "1.0.0"},
        "model": {
            "id": catalog["model_id"],
            "version": catalog["model_version"],
            "digest": catalog["model_digest"],
        },
        "data_pack": {
            "id": "pitgun.racing.simulation",
            "version": catalog["version"],
            "digest": catalog["simulation_pack_digest"],
        },
        "seed": str(entry["seed"]),
    }
    mismatches = {
        key: result.get(key)
        for key, expected in expected_identities.items()
        if result.get(key) != expected
    }
    if mismatches:
        raise BudgetEffectManifestError(
            "runner identity differs from the budget plan: " + repr(mismatches)
        )
    standings = result.get("summary", {}).get("standings", [])
    players = [row for row in standings if row.get("competitor_id") == "player"]
    if len(standings) != 10 or len(players) != 1:
        raise BudgetEffectManifestError("result must contain one player in ten standings")
    player = players[0]
    total_times = [int(row["total_time_ms"]) for row in standings]
    metrics = {
        "racing.budget-effect.player-position": (float(player["position"]), "rank"),
        "racing.budget-effect.player-win": (
            1.0 if player["position"] == 1 else 0.0,
            "boolean",
        ),
        "racing.budget-effect.player-podium": (
            1.0 if player["position"] <= 3 else 0.0,
            "boolean",
        ),
        "racing.budget-effect.player-gap-to-leader": (
            float(player["gap_to_leader_ms"]),
            "ms",
        ),
        "racing.budget-effect.player-best-lap": (
            float(player["best_lap_ms"]),
            "ms",
        ),
        "racing.budget-effect.player-total-time": (
            float(player["total_time_ms"]),
            "ms",
        ),
        "racing.budget-effect.field-spread": (
            float(max(total_times) - min(total_times)),
            "ms",
        ),
        "racing.budget-effect.player-budget": (float(entry["player_budget"]), "point"),
    }
    return {
        "run_key": entry["run_key"],
        "triplet_key": entry["triplet_key"],
        "configuration_id": result["configuration_id"],
        "run_id": result["run_id"],
        "circuit_id": entry["circuit_id"],
        "progression": entry["progression"],
        "seed": int(entry["seed"]),
        "treatment": entry["treatment"],
        "treatment_percentage": int(entry["treatment_percentage"]),
        "field_median_budget": int(entry["field_median_budget"]),
        "player_budget": int(entry["player_budget"]),
        "player_position": int(player["position"]),
        "player_gap_to_leader_ms": int(player["gap_to_leader_ms"]),
        "player_best_lap_ms": int(player["best_lap_ms"]),
        "player_total_time_ms": int(player["total_time_ms"]),
        "field_spread_ms": max(total_times) - min(total_times),
        "metrics": metrics,
    }


def _dose_summary(rows: list[dict[str, Any]]) -> dict[str, Any]:
    return {
        "triplet_count": len(rows),
        "mean_position_delta_090_minus_100": statistics.fmean(
            row["position_delta_090_minus_100"] for row in rows
        ),
        "mean_position_delta_110_minus_100": statistics.fmean(
            row["position_delta_110_minus_100"] for row in rows
        ),
        "median_total_time_delta_090_minus_100_ms": statistics.median(
            row["total_time_delta_090_minus_100_ms"] for row in rows
        ),
        "median_total_time_delta_110_minus_100_ms": statistics.median(
            row["total_time_delta_110_minus_100_ms"] for row in rows
        ),
        "median_total_time_span_110_minus_090_ms": statistics.median(
            row["total_time_span_110_minus_090_ms"] for row in rows
        ),
        "median_best_lap_delta_090_minus_100_ms": statistics.median(
            row["best_lap_delta_090_minus_100_ms"] for row in rows
        ),
        "median_best_lap_delta_110_minus_100_ms": statistics.median(
            row["best_lap_delta_110_minus_100_ms"] for row in rows
        ),
        "monotonic_total_time_rate": sum(row["total_time_monotonic"] for row in rows)
        / len(rows),
        "higher_budget_faster_rate": sum(
            row["total_time_delta_110_minus_100_ms"] < 0 for row in rows
        )
        / len(rows),
        "lower_budget_slower_rate": sum(
            row["total_time_delta_090_minus_100_ms"] > 0 for row in rows
        )
        / len(rows),
    }


def summarize_budget_effect(
    manifest: dict[str, Any], evidence: list[dict[str, Any]], lineage: dict[str, Any]
) -> dict[str, Any]:
    """Produce exact budget dose-response effects and seed stability evidence."""

    expected = {run["run_key"] for run in manifest["runs"]}
    actual = {row["run_key"] for row in evidence}
    if len(actual) != len(evidence) or actual != expected:
        raise BudgetEffectManifestError(
            "budget summary requires one successful result per planned run"
        )
    groups: dict[str, dict[str, dict[str, Any]]] = defaultdict(dict)
    for row in evidence:
        groups[row["triplet_key"]][row["treatment"]] = row
    triplets = []
    for triplet_key, group in sorted(groups.items()):
        if set(group) != set(TREATMENTS):
            raise BudgetEffectManifestError(f"incomplete result triplet: {triplet_key}")
        low, baseline, high = (
            group["field-090"],
            group["field-100"],
            group["field-110"],
        )
        triplets.append(
            {
                "triplet_key": triplet_key,
                "circuit_id": baseline["circuit_id"],
                "progression": baseline["progression"],
                "seed": baseline["seed"],
                "budgets": {
                    "field-090": low["player_budget"],
                    "field-100": baseline["player_budget"],
                    "field-110": high["player_budget"],
                },
                "position_delta_090_minus_100": low["player_position"]
                - baseline["player_position"],
                "position_delta_110_minus_100": high["player_position"]
                - baseline["player_position"],
                "total_time_delta_090_minus_100_ms": low["player_total_time_ms"]
                - baseline["player_total_time_ms"],
                "total_time_delta_110_minus_100_ms": high["player_total_time_ms"]
                - baseline["player_total_time_ms"],
                "total_time_span_110_minus_090_ms": high["player_total_time_ms"]
                - low["player_total_time_ms"],
                "best_lap_delta_090_minus_100_ms": low["player_best_lap_ms"]
                - baseline["player_best_lap_ms"],
                "best_lap_delta_110_minus_100_ms": high["player_best_lap_ms"]
                - baseline["player_best_lap_ms"],
                "total_time_monotonic": low["player_total_time_ms"]
                >= baseline["player_total_time_ms"]
                >= high["player_total_time_ms"],
            }
        )
    if len(triplets) != manifest["planned_triplet_count"]:
        raise BudgetEffectManifestError("triplet result count does not reconcile")

    by_circuit = {}
    by_progression = {}
    by_circuit_progression = {}
    circuits = sorted({row["circuit_id"] for row in triplets})
    progressions = sorted({row["progression"] for row in triplets})
    for circuit in circuits:
        by_circuit[circuit] = _dose_summary(
            [row for row in triplets if row["circuit_id"] == circuit]
        )
    for progression in progressions:
        by_progression[progression] = _dose_summary(
            [row for row in triplets if row["progression"] == progression]
        )
    stable_count = 0
    for circuit in circuits:
        for progression in progressions:
            selected = [
                row
                for row in triplets
                if row["circuit_id"] == circuit and row["progression"] == progression
            ]
            if len(selected) != len(manifest["matrix"]["seeds"]):
                raise BudgetEffectManifestError("seed group is incomplete")
            low_deltas = [row["total_time_delta_090_minus_100_ms"] for row in selected]
            high_deltas = [row["total_time_delta_110_minus_100_ms"] for row in selected]
            low_stable = all(value >= 0 for value in low_deltas) or all(
                value <= 0 for value in low_deltas
            )
            high_stable = all(value >= 0 for value in high_deltas) or all(
                value <= 0 for value in high_deltas
            )
            stable = low_stable and high_stable
            stable_count += int(stable)
            by_circuit_progression[f"{circuit}:{progression}"] = {
                **_dose_summary(selected),
                "seed_direction_stable": stable,
                "seed_deltas_090_minus_100_ms": low_deltas,
                "seed_deltas_110_minus_100_ms": high_deltas,
            }

    return {
        "schema_version": "pitgun.budget-effect-report/v1",
        "campaign_id": manifest["campaign_id"],
        "lineage": lineage,
        "sample": {
            "successful_run_count": len(evidence),
            "triplet_count": len(triplets),
        },
        "comparison": "90% and 110% of field median minus 100% baseline",
        "negative_total_time_delta_is_faster": True,
        "overall": _dose_summary(triplets),
        "by_circuit": by_circuit,
        "by_progression": by_progression,
        "by_circuit_progression": by_circuit_progression,
        "seed_direction_stability": {
            "group_count": len(by_circuit_progression),
            "stable_group_count": stable_count,
            "stable_group_rate": stable_count / len(by_circuit_progression),
        },
        "claims": {
            "evidence": [
                f"{len(triplets)} exact triplets change only player development "
                "budget and balanced allocation.",
                f"{stable_count}/{len(by_circuit_progression)} "
                "circuit/progression groups preserve both dose directions "
                "across all seeds.",
            ],
            "inference": [
                "Stable dose response may inform a future progression rule after human review."
            ],
            "unresolved": [
                "Balanced four-axis allocation does not identify optimal specialist allocations.",
                "The three tested relative budgets do not by themselves select "
                "a game difficulty target.",
            ],
        },
        "causal_interpretation_allowed": True,
        "budget_target_selected": False,
        "automatic_game_or_catalog_promotion": False,
    }
