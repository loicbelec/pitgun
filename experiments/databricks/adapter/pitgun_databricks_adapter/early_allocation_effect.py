"""Load, validate, and analyze the early marginal-allocation campaign."""

from __future__ import annotations

import copy
from collections import defaultdict
import hashlib
import importlib.resources
import json
import statistics
from typing import Any


CAMPAIGN_NAME = "racing-early-allocation-effect-v1"
SOURCE_CAMPAIGN_NAME = "racing-budget-effect-v2"
SCHEMA_VERSION = "pitgun.early-allocation-effect-campaign/v1"
SCENARIO_IDENTITY = {
    "id": "racing.early-allocation-effect-campaign",
    "version": "1.0.0",
}
POINT_KEYS = ("aero_points", "chassis_points", "cooling_points", "engine_points")
AXES = ("aero", "chassis", "cooling", "engine")
TREATMENTS = frozenset(
    {"reference"}
    | {f"{direction}_{axis}" for direction in ("add", "remove") for axis in AXES}
)


class EarlyAllocationEffectManifestError(ValueError):
    """Raised when campaign identity or a controlled comparison differs."""


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
        raise EarlyAllocationEffectManifestError(
            "scenario must contain one controlled player"
        )
    return players[0]


def _projection(scenario: dict[str, Any]) -> dict[str, Any]:
    projection = copy.deepcopy(scenario)
    player = _player(projection)
    player.pop("budget_cap")
    for key in POINT_KEYS:
        player["tuning"].pop(key)
    return projection


def _validate_manifest(
    manifest: dict[str, Any],
    resources: dict[str, bytes],
    source_manifest_digest: str,
) -> None:
    if manifest.get("schema_version") != SCHEMA_VERSION:
        raise EarlyAllocationEffectManifestError("unsupported early-allocation campaign")
    if manifest.get("campaign_id") != "racing-early-allocation-effect-2026-v1":
        raise EarlyAllocationEffectManifestError("early-allocation identity changed")
    if manifest.get("planned_block_count") != 15:
        raise EarlyAllocationEffectManifestError("campaign must contain 15 blocks")
    runs = manifest.get("runs", [])
    if manifest.get("planned_run_count") != 135 or len(runs) != 135:
        raise EarlyAllocationEffectManifestError("campaign must contain 135 runs")
    if manifest.get("source", {}).get("manifest_digest") != source_manifest_digest:
        raise EarlyAllocationEffectManifestError("source budget evidence changed")

    controlled = manifest.get("controlled_input", {})
    if (
        controlled.get("progression") != "early"
        or controlled.get("reference_budget") != 4
        or controlled.get("reference_allocation")
        != {key: 1 for key in POINT_KEYS}
        or set(controlled.get("treatments", {})) != TREATMENTS
        or controlled.get("player_reference") != "neutral"
        or controlled.get("player_strategy") != "balanced-one-stop"
        or controlled.get("opponent_field")
        != "frozen-economy-backed-four-point-field"
        or controlled.get("only_allowed_comparison_difference")
        != "one player development point on one named physical axis"
    ):
        raise EarlyAllocationEffectManifestError("controlled input changed")
    governance = manifest.get("governance", {})
    if any(value is not False for value in governance.values()):
        raise EarlyAllocationEffectManifestError("campaign governance is unsafe")

    matrix = manifest.get("matrix", {})
    if (
        len(matrix.get("circuits", [])) != 5
        or len(matrix.get("seeds", [])) != 3
        or matrix.get("era") != 1
        or matrix.get("treatment_count") != 9
    ):
        raise EarlyAllocationEffectManifestError("campaign matrix changed")

    run_keys = [run.get("run_key") for run in runs]
    if len(set(run_keys)) != 135 or set(resources) != set(run_keys):
        raise EarlyAllocationEffectManifestError("resources and run keys differ")
    groups: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for run in runs:
        resource = run["scenario_resource"]
        data = resources.get(resource)
        if data is None or _sha256(data) != run["scenario_resource_digest"]:
            raise EarlyAllocationEffectManifestError(f"scenario changed: {resource}")
        scenario = json.loads(data)
        catalog = manifest["catalog"]
        if scenario.get("scenario") != SCENARIO_IDENTITY:
            raise EarlyAllocationEffectManifestError(
                f"scenario identity changed: {resource}"
            )
        if scenario.get("model") != {
            "id": catalog["model_id"],
            "version": catalog["model_version"],
            "digest": catalog["model_digest"],
        }:
            raise EarlyAllocationEffectManifestError(f"model changed: {resource}")
        if scenario.get("data_pack") != {
            "id": "pitgun.racing.simulation",
            "version": catalog["version"],
            "digest": catalog["simulation_pack_digest"],
        }:
            raise EarlyAllocationEffectManifestError(f"data pack changed: {resource}")

        treatment = controlled["treatments"].get(run["treatment"])
        player = _player(scenario)
        allocation = {key: int(player["tuning"][key]) for key in POINT_KEYS}
        if (
            treatment is None
            or run["direction"] != treatment["direction"]
            or run["axis"] != treatment["axis"]
            or run["player_budget"] != treatment["budget"]
            or run["player_allocation"] != treatment["allocation"]
            or player["budget_cap"] != treatment["budget"]
            or allocation != treatment["allocation"]
            or sum(allocation.values()) != treatment["budget"]
        ):
            raise EarlyAllocationEffectManifestError(
                f"invalid marginal treatment: {resource}"
            )
        opponents = [
            row for row in scenario["request"]["competitors"] if not row["is_player"]
        ]
        if len(opponents) != 9:
            raise EarlyAllocationEffectManifestError(f"invalid field: {resource}")
        for opponent in opponents:
            points = [int(opponent["tuning"][key]) for key in POINT_KEYS]
            if opponent["budget_cap"] != 4 or sum(points) != 4:
                raise EarlyAllocationEffectManifestError(
                    f"opponent field changed: {resource}/{opponent['id']}"
                )
        if _sha256(_compact(_projection(scenario))) != run["block_invariant_digest"]:
            raise EarlyAllocationEffectManifestError(
                f"block invariant changed: {resource}"
            )
        groups[run["block_key"]].append(run)

    if len(groups) != 15:
        raise EarlyAllocationEffectManifestError("block keys are incomplete")
    for block_key, block in groups.items():
        if len(block) != 9 or {row["treatment"] for row in block} != TREATMENTS:
            raise EarlyAllocationEffectManifestError(
                f"block treatments are incomplete: {block_key}"
            )
        if len({row["block_invariant_digest"] for row in block}) != 1:
            raise EarlyAllocationEffectManifestError(
                f"block invariant is inconsistent: {block_key}"
            )


def load_early_allocation_effect_campaign() -> tuple[dict[str, Any], str]:
    """Return the checksummed campaign after validating all packaged resources."""

    package = importlib.resources.files("pitgun_databricks_adapter")
    campaign_root = package.joinpath("campaigns")
    manifest_name = CAMPAIGN_NAME + ".json"
    manifest_bytes = campaign_root.joinpath(manifest_name).read_bytes()
    checksum = campaign_root.joinpath(CAMPAIGN_NAME + ".sha256").read_text().split()
    digest = hashlib.sha256(manifest_bytes).hexdigest()
    if len(checksum) != 2 or checksum != [digest, manifest_name]:
        raise EarlyAllocationEffectManifestError("campaign checksum mismatch")
    source_bytes = campaign_root.joinpath(SOURCE_CAMPAIGN_NAME + ".json").read_bytes()
    manifest = json.loads(manifest_bytes)
    resources = {
        run["scenario_resource"]: package.joinpath(
            "scenarios", f"{run['scenario_resource']}.json"
        ).read_bytes()
        for run in manifest.get("runs", [])
    }
    _validate_manifest(manifest, resources, _sha256(source_bytes))
    return manifest, "sha256:" + digest


def materialize_early_allocation_effect_plan(
    manifest: dict[str, Any],
) -> list[dict[str, Any]]:
    """Return the reviewed immutable execution plan."""

    return [dict(run) for run in manifest["runs"]]


def extract_early_allocation_effect_evidence(
    entry: dict[str, Any], result: dict[str, Any], manifest: dict[str, Any]
) -> dict[str, Any]:
    """Validate one runner result and normalize its controlled evidence."""

    catalog = manifest["catalog"]
    expected = {
        "scenario": SCENARIO_IDENTITY,
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
        for key, value in expected.items()
        if result.get(key) != value
    }
    if mismatches:
        raise EarlyAllocationEffectManifestError(
            "runner identity differs from plan: " + repr(mismatches)
        )
    standings = result.get("summary", {}).get("standings", [])
    players = [row for row in standings if row.get("competitor_id") == "player"]
    if len(standings) != 10 or len(players) != 1:
        raise EarlyAllocationEffectManifestError(
            "result must contain one player in ten standings"
        )
    player = players[0]
    total_times = [int(row["total_time_ms"]) for row in standings]
    metrics = {
        "racing.early-allocation.player-position": (
            float(player["position"]),
            "rank",
        ),
        "racing.early-allocation.player-gap-to-leader": (
            float(player["gap_to_leader_ms"]),
            "ms",
        ),
        "racing.early-allocation.player-best-lap": (
            float(player["best_lap_ms"]),
            "ms",
        ),
        "racing.early-allocation.player-total-time": (
            float(player["total_time_ms"]),
            "ms",
        ),
        "racing.early-allocation.field-spread": (
            float(max(total_times) - min(total_times)),
            "ms",
        ),
        "racing.early-allocation.player-budget": (
            float(entry["player_budget"]),
            "point",
        ),
    }
    return {
        "run_key": entry["run_key"],
        "block_key": entry["block_key"],
        "configuration_id": result["configuration_id"],
        "run_id": result["run_id"],
        "circuit_id": entry["circuit_id"],
        "seed": int(entry["seed"]),
        "treatment": entry["treatment"],
        "direction": entry["direction"],
        "axis": entry["axis"],
        "player_budget": int(entry["player_budget"]),
        "player_position": int(player["position"]),
        "player_gap_to_leader_ms": int(player["gap_to_leader_ms"]),
        "player_best_lap_ms": int(player["best_lap_ms"]),
        "player_total_time_ms": int(player["total_time_ms"]),
        "field_spread_ms": max(total_times) - min(total_times),
        "metrics": metrics,
    }


def _axis_summary(rows: list[dict[str, Any]]) -> dict[str, Any]:
    return {
        "comparison_count": len(rows),
        "median_add_minus_reference_total_time_ms": statistics.median(
            row["add_minus_reference_total_time_ms"] for row in rows
        ),
        "median_remove_minus_reference_total_time_ms": statistics.median(
            row["remove_minus_reference_total_time_ms"] for row in rows
        ),
        "median_marginal_benefit_ms_per_point": statistics.median(
            row["marginal_benefit_ms_per_point"] for row in rows
        ),
        "direction_consistency_rate": sum(
            row["direction_consistent"] for row in rows
        )
        / len(rows),
        "add_faster_rate": sum(
            row["add_minus_reference_total_time_ms"] <= 0 for row in rows
        )
        / len(rows),
        "remove_slower_rate": sum(
            row["remove_minus_reference_total_time_ms"] >= 0 for row in rows
        )
        / len(rows),
    }


def summarize_early_allocation_effect(
    manifest: dict[str, Any], evidence: list[dict[str, Any]], lineage: dict[str, Any]
) -> dict[str, Any]:
    """Produce axis, circuit, and seed-stability marginal evidence."""

    expected = {run["run_key"] for run in manifest["runs"]}
    actual = {row["run_key"] for row in evidence}
    if len(actual) != len(evidence) or actual != expected:
        raise EarlyAllocationEffectManifestError(
            "summary requires one successful result per planned run"
        )
    grouped: dict[str, dict[str, dict[str, Any]]] = defaultdict(dict)
    for row in evidence:
        grouped[row["block_key"]][row["treatment"]] = row

    comparisons = []
    for block_key, group in sorted(grouped.items()):
        if set(group) != TREATMENTS:
            raise EarlyAllocationEffectManifestError(
                f"incomplete result block: {block_key}"
            )
        reference = group["reference"]
        for axis in AXES:
            added = group[f"add_{axis}"]
            removed = group[f"remove_{axis}"]
            add_delta = added["player_total_time_ms"] - reference["player_total_time_ms"]
            remove_delta = (
                removed["player_total_time_ms"] - reference["player_total_time_ms"]
            )
            comparisons.append(
                {
                    "block_key": block_key,
                    "circuit_id": reference["circuit_id"],
                    "seed": reference["seed"],
                    "axis": axis,
                    "add_minus_reference_total_time_ms": add_delta,
                    "remove_minus_reference_total_time_ms": remove_delta,
                    "marginal_benefit_ms_per_point": (remove_delta - add_delta) / 2,
                    "add_minus_reference_best_lap_ms": added["player_best_lap_ms"]
                    - reference["player_best_lap_ms"],
                    "remove_minus_reference_best_lap_ms": removed[
                        "player_best_lap_ms"
                    ]
                    - reference["player_best_lap_ms"],
                    "add_position_delta": added["player_position"]
                    - reference["player_position"],
                    "remove_position_delta": removed["player_position"]
                    - reference["player_position"],
                    "direction_consistent": add_delta <= 0 <= remove_delta,
                }
            )
    if len(comparisons) != manifest["planned_block_count"] * len(AXES):
        raise EarlyAllocationEffectManifestError("comparison count does not reconcile")

    by_axis = {
        axis: _axis_summary([row for row in comparisons if row["axis"] == axis])
        for axis in AXES
    }
    by_circuit_axis = {}
    stable_count = 0
    for circuit in manifest["matrix"]["circuits"]:
        for axis in AXES:
            selected = [
                row
                for row in comparisons
                if row["circuit_id"] == circuit and row["axis"] == axis
            ]
            if len(selected) != len(manifest["matrix"]["seeds"]):
                raise EarlyAllocationEffectManifestError("seed group is incomplete")
            add_values = [
                row["add_minus_reference_total_time_ms"] for row in selected
            ]
            remove_values = [
                row["remove_minus_reference_total_time_ms"] for row in selected
            ]
            stable = (
                (all(value >= 0 for value in add_values) or all(value <= 0 for value in add_values))
                and (
                    all(value >= 0 for value in remove_values)
                    or all(value <= 0 for value in remove_values)
                )
            )
            stable_count += int(stable)
            by_circuit_axis[f"{circuit}:{axis}"] = {
                **_axis_summary(selected),
                "seed_direction_stable": stable,
                "seed_add_deltas_ms": add_values,
                "seed_remove_deltas_ms": remove_values,
            }

    ranking = sorted(
        AXES,
        key=lambda axis: by_axis[axis]["median_marginal_benefit_ms_per_point"],
        reverse=True,
    )
    return {
        "schema_version": "pitgun.early-allocation-effect-report/v1",
        "campaign_id": manifest["campaign_id"],
        "lineage": lineage,
        "sample": {
            "successful_run_count": len(evidence),
            "block_count": len(grouped),
            "axis_comparison_count": len(comparisons),
        },
        "negative_add_delta_is_faster": True,
        "positive_remove_delta_is_slower": True,
        "by_axis": by_axis,
        "by_circuit_axis": by_circuit_axis,
        "evidence_ranking_by_median_marginal_benefit": ranking,
        "seed_direction_stability": {
            "group_count": len(by_circuit_axis),
            "stable_group_count": stable_count,
            "stable_group_rate": stable_count / len(by_circuit_axis),
        },
        "claims": {
            "evidence": [
                f"{len(comparisons)} paired axis comparisons reconcile all immutable runs.",
                f"{stable_count}/{len(by_circuit_axis)} circuit/axis groups preserve both directions across seeds.",
            ],
            "inference": [
                "The reviewed ranking may inform public opponent allocation profiles."
            ],
            "unresolved": [
                "The campaign measures a local four-point early-game boundary only.",
                "Five circuits do not establish universal axis weights.",
                "Interactions between simultaneous axis investments remain unmeasured.",
            ],
        },
        "causal_interpretation_allowed": True,
        "allocation_profile_selected": False,
        "automatic_game_or_catalog_promotion": False,
    }
