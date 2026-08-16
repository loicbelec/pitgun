"""Load and analyze the immutable economy-backed Racing budget campaign V2."""

from __future__ import annotations

import copy
from collections import defaultdict
import hashlib
import importlib.resources
import json
import statistics
from typing import Any


CAMPAIGN_NAME = "racing-budget-effect-v2"
SOURCE_CAMPAIGN_NAME = "racing-strategy-effect-v1"
SCHEMA_VERSION = "pitgun.budget-effect-campaign/v2"
SCENARIO_IDENTITY = {"id": "racing.budget-effect-campaign", "version": "2.0.0"}
POINT_KEYS = ("aero_points", "chassis_points", "cooling_points", "engine_points")
PROGRESSION_ARTIFACT = {
    "repository": "loicbelec/pitgun-game",
    "git_revision": "1eebd08e4a375a5570a73c7a6adcd16cc8736e8a",
    "path": "docs/gameplay/ai-calibration-progression-v1.json",
    "schema_version": "pitgun.ai-calibration-progression/v1",
    "artifact_digest": (
        "sha256:1e5a082ff05b8c66d43ad0d69306af608c2b9fd4253a4d479d3c0f9c03daf23c"
    ),
}
TREATMENTS = {
    "early": {"below": 3, "reference": 4, "above": 5},
    "mid": {"below": 24, "reference": 27, "above": 30},
    "late": {"below": 33, "reference": 37, "above": 41},
}


class BudgetEffectV2ManifestError(ValueError):
    """Raised when V2 campaign identities or controlled triplets differ."""


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
        raise BudgetEffectV2ManifestError(
            "V2 scenario must contain one controlled player"
        )
    return players[0]


def _projection(scenario: dict[str, Any]) -> dict[str, Any]:
    projection = copy.deepcopy(scenario)
    player = _player(projection)
    player.pop("budget_cap")
    for key in POINT_KEYS:
        player["tuning"].pop(key)
    return projection


def _balanced_points(budget: int) -> dict[str, int]:
    quotient, remainder = divmod(budget, len(POINT_KEYS))
    return {
        key: quotient + (1 if index < remainder else 0)
        for index, key in enumerate(POINT_KEYS)
    }


def _validate_manifest(
    manifest: dict[str, Any],
    resources: dict[str, bytes],
    source_manifest_digest: str,
) -> None:
    if manifest.get("schema_version") != SCHEMA_VERSION:
        raise BudgetEffectV2ManifestError("unsupported budget-effect V2 campaign")
    if manifest.get("campaign_id") != "racing-budget-effect-2026-v2":
        raise BudgetEffectV2ManifestError("budget-effect V2 identity changed")
    if manifest.get("planned_triplet_count") != 45:
        raise BudgetEffectV2ManifestError("V2 campaign must contain 45 triplets")
    runs = manifest.get("runs", [])
    if manifest.get("planned_run_count") != 135 or len(runs) != 135:
        raise BudgetEffectV2ManifestError("V2 campaign must contain 135 runs")
    source = manifest.get("source", {})
    if source.get("manifest_digest") != source_manifest_digest:
        raise BudgetEffectV2ManifestError("source strategy campaign changed")
    if source.get("gameplay_progression") != PROGRESSION_ARTIFACT:
        raise BudgetEffectV2ManifestError("gameplay progression provenance changed")
    controlled = manifest.get("controlled_input", {})
    if controlled.get("treatments_by_progression") != TREATMENTS:
        raise BudgetEffectV2ManifestError("economy-backed treatments changed")
    if controlled.get("only_allowed_triplet_difference") != (
        "player.development_budget_and_balanced_point_allocation"
    ):
        raise BudgetEffectV2ManifestError("controlled V2 treatment boundary changed")
    if (
        controlled.get("player_reference") != "neutral"
        or controlled.get("player_strategy") != "balanced-one-stop"
        or controlled.get("allocation") != "deterministic-balanced-four-axis"
        or controlled.get("opponent_field_normalization")
        != {
            "budget": "economy-backed-progression-reference",
            "allocation": "source-relative-largest-remainder-with-integer-quantization",
            "all_opponents_share_reference_total": True,
            "early_quantization_caveat": (
                "four total points quantize the audited field to one point per axis"
            ),
        }
    ):
        raise BudgetEffectV2ManifestError("controlled V2 input changed")
    governance = manifest.get("governance", {})
    if any(
        governance.get(key) is not False
        for key in (
            "private_player_data_allowed",
            "automatic_game_or_catalog_promotion",
            "automatic_budget_target_selection_allowed",
        )
    ):
        raise BudgetEffectV2ManifestError("V2 campaign governance is unsafe")

    progression_matrix = manifest.get("matrix", {}).get("progression", [])
    if (
        len(progression_matrix) != len(TREATMENTS)
        or {row.get("id") for row in progression_matrix} != set(TREATMENTS)
        or any("playerBudget" in row for row in progression_matrix)
        or any(
            row.get("referenceBudget") != TREATMENTS[row["id"]]["reference"]
            for row in progression_matrix
        )
    ):
        raise BudgetEffectV2ManifestError("V2 progression matrix is not economy-backed")

    run_keys = [run.get("run_key") for run in runs]
    if len(set(run_keys)) != 135 or set(resources) != set(run_keys):
        raise BudgetEffectV2ManifestError("V2 resources and run keys differ")
    triplets: dict[str, list[dict[str, Any]]] = {}
    for run in runs:
        resource = run["scenario_resource"]
        data = resources.get(resource)
        if data is None or _sha256(data) != run["scenario_resource_digest"]:
            raise BudgetEffectV2ManifestError(f"V2 scenario changed: {resource}")
        scenario = json.loads(data)
        if scenario.get("scenario") != SCENARIO_IDENTITY:
            raise BudgetEffectV2ManifestError(
                f"V2 scenario identity changed: {resource}"
            )
        catalog = manifest["catalog"]
        if scenario.get("model") != {
            "id": catalog["model_id"],
            "version": catalog["model_version"],
            "digest": catalog["model_digest"],
        }:
            raise BudgetEffectV2ManifestError(f"V2 model changed: {resource}")
        if scenario.get("data_pack") != {
            "id": "pitgun.racing.simulation",
            "version": catalog["version"],
            "digest": catalog["simulation_pack_digest"],
        }:
            raise BudgetEffectV2ManifestError(f"V2 data pack changed: {resource}")

        expected_budget = TREATMENTS[run["progression"]][run["treatment"]]
        reference_budget = TREATMENTS[run["progression"]]["reference"]
        player = _player(scenario)
        allocation = {key: int(player["tuning"][key]) for key in POINT_KEYS}
        if (
            run["reference_budget"] != reference_budget
            or run["opponent_budget"] != reference_budget
            or run["player_budget"] != expected_budget
            or player["budget_cap"] != expected_budget
            or allocation != _balanced_points(expected_budget)
            or run["player_allocation"] != allocation
            or max(allocation.values()) > 20
        ):
            raise BudgetEffectV2ManifestError(f"invalid V2 treatment: {resource}")
        opponents = [
            row for row in scenario["request"]["competitors"] if not row["is_player"]
        ]
        if len(opponents) != 9:
            raise BudgetEffectV2ManifestError(f"invalid V2 field: {resource}")
        for opponent in opponents:
            points = [int(opponent["tuning"][key]) for key in POINT_KEYS]
            if (
                int(opponent["budget_cap"]) != reference_budget
                or sum(points) != reference_budget
                or max(points) > 20
            ):
                raise BudgetEffectV2ManifestError(
                    f"invalid normalized opponent: {resource}/{opponent['id']}"
                )
        if _sha256(_compact(_projection(scenario))) != run[
            "triplet_invariant_digest"
        ]:
            raise BudgetEffectV2ManifestError(
                f"V2 triplet invariant changed: {resource}"
            )
        triplets.setdefault(run["triplet_key"], []).append(run)

    if len(triplets) != 45:
        raise BudgetEffectV2ManifestError("V2 triplet keys are incomplete")
    for triplet_key, triplet in triplets.items():
        if {run["treatment"] for run in triplet} != set(
            TREATMENTS[triplet[0]["progression"]]
        ):
            raise BudgetEffectV2ManifestError(
                f"V2 triplet treatments are incomplete: {triplet_key}"
            )
        if len({run["triplet_invariant_digest"] for run in triplet}) != 1:
            raise BudgetEffectV2ManifestError(
                f"V2 triplet input changed: {triplet_key}"
            )
        if len({run["player_budget"] for run in triplet}) != 3:
            raise BudgetEffectV2ManifestError(
                f"V2 triplet budgets are not distinct: {triplet_key}"
            )


def load_budget_effect_v2_campaign() -> tuple[dict[str, Any], str]:
    """Return the checksummed economy-backed campaign after validation."""

    package = importlib.resources.files("pitgun_databricks_adapter")
    campaign_root = package.joinpath("campaigns")
    manifest_name = CAMPAIGN_NAME + ".json"
    manifest_bytes = campaign_root.joinpath(manifest_name).read_bytes()
    checksum_parts = campaign_root.joinpath(CAMPAIGN_NAME + ".sha256").read_text().split()
    digest = hashlib.sha256(manifest_bytes).hexdigest()
    if (
        len(checksum_parts) != 2
        or checksum_parts[1] != manifest_name
        or checksum_parts[0] != digest
    ):
        raise BudgetEffectV2ManifestError("V2 campaign checksum mismatch")

    source_bytes = campaign_root.joinpath(SOURCE_CAMPAIGN_NAME + ".json").read_bytes()
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


def materialize_budget_effect_v2_plan(
    manifest: dict[str, Any],
) -> list[dict[str, Any]]:
    """Return the explicit reviewed V2 execution plan."""

    return [dict(run) for run in manifest["runs"]]


def extract_budget_effect_v2_evidence(
    entry: dict[str, Any], result: dict[str, Any], manifest: dict[str, Any]
) -> dict[str, Any]:
    """Validate one result and return normalized economy-backed evidence."""

    catalog = manifest["catalog"]
    expected_identities = {
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
        for key, expected in expected_identities.items()
        if result.get(key) != expected
    }
    if mismatches:
        raise BudgetEffectV2ManifestError(
            "runner identity differs from the V2 plan: " + repr(mismatches)
        )
    standings = result.get("summary", {}).get("standings", [])
    players = [row for row in standings if row.get("competitor_id") == "player"]
    if len(standings) != 10 or len(players) != 1:
        raise BudgetEffectV2ManifestError(
            "V2 result must contain one player in ten standings"
        )
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
        "racing.budget-effect.opponent-reference-budget": (
            float(entry["opponent_budget"]),
            "point",
        ),
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
        "reference_budget": int(entry["reference_budget"]),
        "opponent_budget": int(entry["opponent_budget"]),
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
        "mean_position_delta_below_minus_reference": statistics.fmean(
            row["position_delta_below_minus_reference"] for row in rows
        ),
        "mean_position_delta_above_minus_reference": statistics.fmean(
            row["position_delta_above_minus_reference"] for row in rows
        ),
        "median_total_time_delta_below_minus_reference_ms": statistics.median(
            row["total_time_delta_below_minus_reference_ms"] for row in rows
        ),
        "median_total_time_delta_above_minus_reference_ms": statistics.median(
            row["total_time_delta_above_minus_reference_ms"] for row in rows
        ),
        "median_total_time_span_above_minus_below_ms": statistics.median(
            row["total_time_span_above_minus_below_ms"] for row in rows
        ),
        "median_best_lap_delta_below_minus_reference_ms": statistics.median(
            row["best_lap_delta_below_minus_reference_ms"] for row in rows
        ),
        "median_best_lap_delta_above_minus_reference_ms": statistics.median(
            row["best_lap_delta_above_minus_reference_ms"] for row in rows
        ),
        "monotonic_total_time_rate": sum(row["total_time_monotonic"] for row in rows)
        / len(rows),
        "above_faster_rate": sum(
            row["total_time_delta_above_minus_reference_ms"] < 0 for row in rows
        )
        / len(rows),
        "below_slower_rate": sum(
            row["total_time_delta_below_minus_reference_ms"] > 0 for row in rows
        )
        / len(rows),
    }


def summarize_budget_effect_v2(
    manifest: dict[str, Any], evidence: list[dict[str, Any]], lineage: dict[str, Any]
) -> dict[str, Any]:
    """Produce V2 dose-response and seed-stability evidence."""

    expected = {run["run_key"] for run in manifest["runs"]}
    actual = {row["run_key"] for row in evidence}
    if len(actual) != len(evidence) or actual != expected:
        raise BudgetEffectV2ManifestError(
            "V2 summary requires one successful result per planned run"
        )
    groups: dict[str, dict[str, dict[str, Any]]] = defaultdict(dict)
    for row in evidence:
        groups[row["triplet_key"]][row["treatment"]] = row
    triplets = []
    for triplet_key, group in sorted(groups.items()):
        if set(group) != {"below", "reference", "above"}:
            raise BudgetEffectV2ManifestError(
                f"incomplete V2 result triplet: {triplet_key}"
            )
        below, reference, above = (
            group["below"],
            group["reference"],
            group["above"],
        )
        triplets.append(
            {
                "triplet_key": triplet_key,
                "circuit_id": reference["circuit_id"],
                "progression": reference["progression"],
                "seed": reference["seed"],
                "budgets": {
                    "below": below["player_budget"],
                    "reference": reference["player_budget"],
                    "above": above["player_budget"],
                },
                "position_delta_below_minus_reference": below["player_position"]
                - reference["player_position"],
                "position_delta_above_minus_reference": above["player_position"]
                - reference["player_position"],
                "total_time_delta_below_minus_reference_ms": below[
                    "player_total_time_ms"
                ]
                - reference["player_total_time_ms"],
                "total_time_delta_above_minus_reference_ms": above[
                    "player_total_time_ms"
                ]
                - reference["player_total_time_ms"],
                "total_time_span_above_minus_below_ms": above["player_total_time_ms"]
                - below["player_total_time_ms"],
                "best_lap_delta_below_minus_reference_ms": below[
                    "player_best_lap_ms"
                ]
                - reference["player_best_lap_ms"],
                "best_lap_delta_above_minus_reference_ms": above[
                    "player_best_lap_ms"
                ]
                - reference["player_best_lap_ms"],
                "total_time_monotonic": below["player_total_time_ms"]
                >= reference["player_total_time_ms"]
                >= above["player_total_time_ms"],
            }
        )
    if len(triplets) != manifest["planned_triplet_count"]:
        raise BudgetEffectV2ManifestError("V2 triplet count does not reconcile")

    circuits = sorted({row["circuit_id"] for row in triplets})
    progressions = sorted({row["progression"] for row in triplets})
    by_circuit = {
        circuit: _dose_summary(
            [row for row in triplets if row["circuit_id"] == circuit]
        )
        for circuit in circuits
    }
    by_progression = {
        progression: _dose_summary(
            [row for row in triplets if row["progression"] == progression]
        )
        for progression in progressions
    }
    stable_count = 0
    by_circuit_progression = {}
    for circuit in circuits:
        for progression in progressions:
            selected = [
                row
                for row in triplets
                if row["circuit_id"] == circuit and row["progression"] == progression
            ]
            if len(selected) != len(manifest["matrix"]["seeds"]):
                raise BudgetEffectV2ManifestError("V2 seed group is incomplete")
            below_deltas = [
                row["total_time_delta_below_minus_reference_ms"] for row in selected
            ]
            above_deltas = [
                row["total_time_delta_above_minus_reference_ms"] for row in selected
            ]
            below_stable = all(value >= 0 for value in below_deltas) or all(
                value <= 0 for value in below_deltas
            )
            above_stable = all(value >= 0 for value in above_deltas) or all(
                value <= 0 for value in above_deltas
            )
            stable = below_stable and above_stable
            stable_count += int(stable)
            by_circuit_progression[f"{circuit}:{progression}"] = {
                **_dose_summary(selected),
                "seed_direction_stable": stable,
                "seed_deltas_below_minus_reference_ms": below_deltas,
                "seed_deltas_above_minus_reference_ms": above_deltas,
            }

    return {
        "schema_version": "pitgun.budget-effect-report/v2",
        "campaign_id": manifest["campaign_id"],
        "lineage": lineage,
        "sample": {
            "successful_run_count": len(evidence),
            "triplet_count": len(triplets),
        },
        "comparison": "below and above economy-backed progression reference",
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
                f"{len(triplets)} exact economy-backed triplets change only player "
                "development budget and balanced allocation.",
                f"{stable_count}/{len(by_circuit_progression)} circuit/progression "
                "groups preserve both dose directions across all seeds.",
            ],
            "inference": [
                "Stable unsaturated response may inform future opponent progression "
                "after human review."
            ],
            "unresolved": [
                "Four-point early fields quantize the audited development allocations "
                "to one point per axis.",
                "Balanced player allocation does not identify optimal specialist allocations.",
                "The tested budgets do not automatically select game difficulty.",
            ],
        },
        "causal_interpretation_allowed": True,
        "budget_target_selected": False,
        "automatic_game_or_catalog_promotion": False,
    }
