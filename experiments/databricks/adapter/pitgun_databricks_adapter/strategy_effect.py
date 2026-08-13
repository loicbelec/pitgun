"""Load the immutable controlled Racing player-strategy campaign."""

from __future__ import annotations

import copy
from collections import defaultdict
import hashlib
import importlib.resources
import json
import statistics
from typing import Any


CAMPAIGN_NAME = "racing-strategy-effect-v1"
SCHEMA_VERSION = "pitgun.strategy-effect-campaign/v1"
STRATEGIES = {"balanced-one-stop", "late-one-stop"}


class StrategyEffectManifestError(ValueError):
    """Raised when causal campaign identities or pair invariants differ."""


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
        raise StrategyEffectManifestError("scenario must contain one controlled player")
    return players[0]


def _projection(scenario: dict[str, Any]) -> dict[str, Any]:
    projection = copy.deepcopy(scenario)
    _player(projection).pop("stint_strategy")
    return projection


def _validate_manifest(
    manifest: dict[str, Any],
    resources: dict[str, bytes],
    source_manifest_digest: str,
) -> None:
    if manifest.get("schema_version") != SCHEMA_VERSION:
        raise StrategyEffectManifestError("unsupported strategy-effect campaign")
    if manifest.get("planned_pair_count") != 45:
        raise StrategyEffectManifestError("strategy campaign must contain 45 pairs")
    runs = manifest.get("runs", [])
    if manifest.get("planned_run_count") != 90 or len(runs) != 90:
        raise StrategyEffectManifestError("strategy campaign must contain 90 runs")
    if manifest.get("source", {}).get("manifest_digest") != source_manifest_digest:
        raise StrategyEffectManifestError("source opponent audit identity changed")
    controlled = manifest.get("controlled_input", {})
    if controlled != {
        "player_reference": "neutral",
        "strategy_profiles": ["balanced-one-stop", "late-one-stop"],
        "only_allowed_pair_difference": "player.stint_strategy",
        "opponent_field_source": "balanced-one-stop",
    }:
        raise StrategyEffectManifestError("controlled input boundary changed")
    governance = manifest.get("governance", {})
    if any(
        governance.get(key) is not False
        for key in (
            "private_player_data_allowed",
            "automatic_game_or_catalog_promotion",
            "policy_selection_allowed",
        )
    ):
        raise StrategyEffectManifestError("strategy campaign governance is unsafe")

    run_keys = [run.get("run_key") for run in runs]
    if len(set(run_keys)) != 90 or set(resources) != set(run_keys):
        raise StrategyEffectManifestError("scenario resources and run keys differ")
    pairs: dict[str, list[dict[str, Any]]] = {}
    for run in runs:
        resource = run["scenario_resource"]
        data = resources.get(resource)
        if data is None or _sha256(data) != run["scenario_resource_digest"]:
            raise StrategyEffectManifestError(f"scenario changed: {resource}")
        scenario = json.loads(data)
        if scenario.get("scenario") != {
            "id": "racing.strategy-effect-campaign",
            "version": "1.0.0",
        }:
            raise StrategyEffectManifestError(f"scenario identity changed: {resource}")
        catalog = manifest["catalog"]
        if scenario.get("model") != {
            "id": catalog["model_id"],
            "version": catalog["model_version"],
            "digest": catalog["model_digest"],
        }:
            raise StrategyEffectManifestError(f"model identity changed: {resource}")
        if scenario.get("data_pack") != {
            "id": "pitgun.racing.simulation",
            "version": catalog["version"],
            "digest": catalog["simulation_pack_digest"],
        }:
            raise StrategyEffectManifestError(f"data pack changed: {resource}")
        player = _player(scenario)
        if _sha256(_compact(player["stint_strategy"])) != run[
            "player_strategy_digest"
        ]:
            raise StrategyEffectManifestError(f"strategy digest changed: {resource}")
        if _sha256(_compact(_projection(scenario))) != run["pair_invariant_digest"]:
            raise StrategyEffectManifestError(f"pair invariant changed: {resource}")
        pairs.setdefault(run["pair_key"], []).append(
            {"run": run, "scenario": scenario}
        )

    if len(pairs) != 45:
        raise StrategyEffectManifestError("pair keys are incomplete")
    for pair_key, pair in pairs.items():
        if {row["run"]["strategy_profile"] for row in pair} != STRATEGIES:
            raise StrategyEffectManifestError(f"pair variants are incomplete: {pair_key}")
        invariant_digests = {row["run"]["pair_invariant_digest"] for row in pair}
        strategy_digests = {row["run"]["player_strategy_digest"] for row in pair}
        source_contracts = {
            row["run"]["source_opponent_contract_digest"] for row in pair
        }
        if len(invariant_digests) != 1 or len(source_contracts) != 1:
            raise StrategyEffectManifestError(f"pair input changed: {pair_key}")
        if len(strategy_digests) != 2:
            raise StrategyEffectManifestError(f"pair strategies are identical: {pair_key}")


def load_strategy_effect_campaign() -> tuple[dict[str, Any], str]:
    """Return the checksummed campaign after validating every causal pair."""

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
        raise StrategyEffectManifestError("campaign manifest checksum mismatch")

    source_name = "racing-opponent-audit-v1.json"
    source_bytes = campaign_root.joinpath(source_name).read_bytes()
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


def materialize_strategy_effect_plan(
    manifest: dict[str, Any],
) -> list[dict[str, Any]]:
    """Return the already explicit reviewed execution plan."""

    return [dict(run) for run in manifest["runs"]]


def extract_strategy_effect_evidence(
    entry: dict[str, Any], result: dict[str, Any], manifest: dict[str, Any]
) -> dict[str, Any]:
    """Validate one result and return normalized causal strategy evidence."""

    catalog = manifest["catalog"]
    expected_identities = {
        "scenario": {"id": "racing.strategy-effect-campaign", "version": "1.0.0"},
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
        raise StrategyEffectManifestError(
            "runner identity differs from the strategy plan: " + repr(mismatches)
        )
    standings = result.get("summary", {}).get("standings", [])
    players = [row for row in standings if row.get("competitor_id") == "player"]
    if len(standings) != 10 or len(players) != 1:
        raise StrategyEffectManifestError("result must contain one player in ten standings")
    player = players[0]
    total_times = [int(row["total_time_ms"]) for row in standings]
    metrics = {
        "racing.strategy-effect.player-position": (
            float(player["position"]),
            "rank",
        ),
        "racing.strategy-effect.player-win": (
            1.0 if player["position"] == 1 else 0.0,
            "boolean",
        ),
        "racing.strategy-effect.player-podium": (
            1.0 if player["position"] <= 3 else 0.0,
            "boolean",
        ),
        "racing.strategy-effect.player-gap-to-leader": (
            float(player["gap_to_leader_ms"]),
            "ms",
        ),
        "racing.strategy-effect.player-best-lap": (
            float(player["best_lap_ms"]),
            "ms",
        ),
        "racing.strategy-effect.player-total-time": (
            float(player["total_time_ms"]),
            "ms",
        ),
        "racing.strategy-effect.field-spread": (
            float(max(total_times) - min(total_times)),
            "ms",
        ),
    }
    return {
        "run_key": entry["run_key"],
        "pair_key": entry["pair_key"],
        "configuration_id": result["configuration_id"],
        "run_id": result["run_id"],
        "circuit_id": entry["circuit_id"],
        "progression": entry["progression"],
        "seed": int(entry["seed"]),
        "strategy_profile": entry["strategy_profile"],
        "player_position": int(player["position"]),
        "player_gap_to_leader_ms": int(player["gap_to_leader_ms"]),
        "player_best_lap_ms": int(player["best_lap_ms"]),
        "player_total_time_ms": int(player["total_time_ms"]),
        "field_spread_ms": max(total_times) - min(total_times),
        "metrics": metrics,
    }


def _paired_summary(rows: list[dict[str, Any]]) -> dict[str, Any]:
    return {
        "pair_count": len(rows),
        "mean_position_delta_late_minus_balanced": statistics.fmean(
            row["position_delta"] for row in rows
        ),
        "median_gap_delta_late_minus_balanced_ms": statistics.median(
            row["gap_delta_ms"] for row in rows
        ),
        "median_total_time_delta_late_minus_balanced_ms": statistics.median(
            row["total_time_delta_ms"] for row in rows
        ),
        "median_best_lap_delta_late_minus_balanced_ms": statistics.median(
            row["best_lap_delta_ms"] for row in rows
        ),
        "late_strategy_faster_rate": sum(
            row["total_time_delta_ms"] < 0 for row in rows
        )
        / len(rows),
    }


def summarize_strategy_effect(
    manifest: dict[str, Any], evidence: list[dict[str, Any]], lineage: dict[str, Any]
) -> dict[str, Any]:
    """Produce exact late-minus-balanced paired effects and stability evidence."""

    expected = {run["run_key"] for run in manifest["runs"]}
    actual = {row["run_key"] for row in evidence}
    if len(actual) != len(evidence) or actual != expected:
        raise StrategyEffectManifestError(
            "strategy summary requires one successful result per planned run"
        )
    groups: dict[str, dict[str, dict[str, Any]]] = defaultdict(dict)
    for row in evidence:
        groups[row["pair_key"]][row["strategy_profile"]] = row
    pairs = []
    for pair_key, pair in sorted(groups.items()):
        if set(pair) != STRATEGIES:
            raise StrategyEffectManifestError(f"incomplete result pair: {pair_key}")
        balanced = pair["balanced-one-stop"]
        late = pair["late-one-stop"]
        pairs.append(
            {
                "pair_key": pair_key,
                "circuit_id": balanced["circuit_id"],
                "progression": balanced["progression"],
                "seed": balanced["seed"],
                "position_delta": late["player_position"]
                - balanced["player_position"],
                "gap_delta_ms": late["player_gap_to_leader_ms"]
                - balanced["player_gap_to_leader_ms"],
                "total_time_delta_ms": late["player_total_time_ms"]
                - balanced["player_total_time_ms"],
                "best_lap_delta_ms": late["player_best_lap_ms"]
                - balanced["player_best_lap_ms"],
            }
        )
    if len(pairs) != manifest["planned_pair_count"]:
        raise StrategyEffectManifestError("paired result count does not reconcile")

    by_circuit = {}
    by_progression = {}
    by_circuit_progression = {}
    for circuit in sorted({row["circuit_id"] for row in pairs}):
        by_circuit[circuit] = _paired_summary(
            [row for row in pairs if row["circuit_id"] == circuit]
        )
    progression_ids = sorted({row["progression"] for row in pairs})
    for progression in progression_ids:
        by_progression[progression] = _paired_summary(
            [row for row in pairs if row["progression"] == progression]
        )
    stable_count = 0
    for circuit in sorted(by_circuit):
        for progression in progression_ids:
            selected = [
                row
                for row in pairs
                if row["circuit_id"] == circuit
                and row["progression"] == progression
            ]
            if len(selected) != len(manifest["matrix"]["seeds"]):
                raise StrategyEffectManifestError("seed group is incomplete")
            deltas = [row["total_time_delta_ms"] for row in selected]
            stable = all(value <= 0 for value in deltas) or all(
                value >= 0 for value in deltas
            )
            stable_count += int(stable)
            by_circuit_progression[f"{circuit}:{progression}"] = {
                **_paired_summary(selected),
                "seed_direction_stable": stable,
                "seed_deltas_ms": deltas,
            }

    return {
        "schema_version": "pitgun.strategy-effect-report/v1",
        "campaign_id": manifest["campaign_id"],
        "lineage": lineage,
        "sample": {"successful_run_count": len(evidence), "pair_count": len(pairs)},
        "comparison": "late-one-stop minus balanced-one-stop",
        "negative_total_time_delta_is_faster": True,
        "overall": _paired_summary(pairs),
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
                f"{len(pairs)} exact pairs change only the controlled player strategy.",
                f"{stable_count}/{len(by_circuit_progression)} "
                "circuit/progression groups preserve the total-time direction "
                "across all seeds.",
            ],
            "inference": [
                "Stable circuit/progression effects may inform a future "
                "strategy rule after human review."
            ],
            "unresolved": [
                "The experiment compares two authored one-stop timings only; "
                "it does not optimize compounds or stop count.",
                "The neutral setup bounds the causal claim and does not prove "
                "the same effect for every setup.",
            ],
        },
        "causal_interpretation_allowed": True,
        "policy_selected": False,
        "automatic_game_or_catalog_promotion": False,
    }
