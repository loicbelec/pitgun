"""Load and analyze the immutable Catalog 1.9 opponent acceptance campaign."""

from __future__ import annotations

from collections import defaultdict
import hashlib
import importlib.resources
import json
import re
import statistics
from typing import Any


CAMPAIGN_NAME = "racing-opponent-acceptance-v1"
SCHEMA_VERSION = "pitgun.opponent-acceptance-campaign/v1"
REPORT_VERSION = "pitgun.opponent-acceptance-report/v1"
CATALOG_RESOURCE = "racing-v1-9-0"
SCENARIO_ID = "racing.opponent-acceptance-matrix"
SCENARIO_VERSION = "1.0.0"
RESOURCE_PATTERN = re.compile(r"[a-z0-9]+(?:-[a-z0-9]+)*")

EXPECTED_CATALOG = {
    "id": "pitgun.racing",
    "version": "1.9.0",
    "manifest_digest": (
        "sha256:30621c0a2c6ff232eacecfb92737372e0f8766e121b45c8b2fb48cc762ca49ac"
    ),
    "simulation_pack_digest": (
        "sha256:b0341f1e867fc0217daaf83d1ffb34e807826c45cd3cf4b8a380b504f2e27d00"
    ),
    "model_id": "pitgun.racing-v3-candidate",
    "model_version": "0.15.0",
    "model_digest": (
        "sha256:3038739c9059c12cf47feb4de6a3fd791d9f94290d9e83405a61bd966eea540f"
    ),
    "opponent_policy_id": "pitgun.racing.opponents.competitive",
    "opponent_policy_version": "2.0.0",
    "opponent_policy_resource_digest": (
        "sha256:3ae8204156754fcf6758bcb452e0b648582e0e8afa196365ea9167e2203a3370"
    ),
}
EXPECTED_CIRCUITS = {"BUDAPEST", "MONACO", "MONZA", "SINGAPORE", "SUZUKA"}
EXPECTED_PROGRESSION = {"early": (1, 4), "mid": (3, 27), "late": (5, 37)}
EXPECTED_SEEDS = {42, 4242, 20260825}
EXPECTED_REFERENCES = {"naive", "balanced", "circuit-informed"}


class OpponentAcceptanceError(ValueError):
    """Raised when a packaged input or native result crosses the review boundary."""


def _sha256(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


def _validate_manifest(
    manifest: dict[str, Any], resource_digests: dict[str, str]
) -> None:
    if manifest.get("schema_version") != SCHEMA_VERSION:
        raise OpponentAcceptanceError("unsupported opponent acceptance campaign")
    if manifest.get("catalog") != EXPECTED_CATALOG:
        raise OpponentAcceptanceError(
            "opponent acceptance catalog or model identity changed"
        )
    runs = manifest.get("runs", [])
    if manifest.get("planned_run_count") != 135 or len(runs) != 135:
        raise OpponentAcceptanceError("opponent acceptance plan must contain 135 runs")

    run_keys = [run.get("run_key") for run in runs]
    resources = [run.get("scenario_resource") for run in runs]
    if len(set(run_keys)) != 135 or len(set(resources)) != 135:
        raise OpponentAcceptanceError("opponent acceptance identities are not unique")
    if any(
        not isinstance(resource, str) or not RESOURCE_PATTERN.fullmatch(resource)
        for resource in resources
    ):
        raise OpponentAcceptanceError("opponent acceptance resource is unsafe")
    for run in runs:
        resource = run["scenario_resource"]
        if resource_digests.get(resource) != run.get("scenario_resource_digest"):
            raise OpponentAcceptanceError(
                f"packaged scenario changed or is missing: {resource}"
            )

    expected_axes = {
        (circuit, progression, seed, reference)
        for circuit in EXPECTED_CIRCUITS
        for progression in EXPECTED_PROGRESSION
        for seed in EXPECTED_SEEDS
        for reference in EXPECTED_REFERENCES
    }
    actual_axes = {
        (
            run.get("circuit_id"),
            run.get("progression"),
            run.get("seed"),
            run.get("player_reference"),
        )
        for run in runs
    }
    if actual_axes != expected_axes:
        raise OpponentAcceptanceError("opponent acceptance matrix is incomplete")

    paired_fields: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for run in runs:
        expected_era, expected_budget = EXPECTED_PROGRESSION[run["progression"]]
        if run.get("era") != expected_era or run.get("player_budget") != expected_budget:
            raise OpponentAcceptanceError(
                f"progression identity changed in {run['run_key']}"
            )
        paired_fields[run["source_field_id"]].append(run)
    if len(paired_fields) != 45:
        raise OpponentAcceptanceError("opponent acceptance must contain 45 pairs")
    for field_id, rows in paired_fields.items():
        if {row["player_reference"] for row in rows} != EXPECTED_REFERENCES:
            raise OpponentAcceptanceError(f"paired field is incomplete: {field_id}")
        if len({row["source_opponent_contract_digest"] for row in rows}) != 1:
            raise OpponentAcceptanceError(f"opponents changed in field: {field_id}")
        if len({row["source_opponent_component_digest"] for row in rows}) != 1:
            raise OpponentAcceptanceError(f"components changed in field: {field_id}")

    gates = manifest.get("acceptance_gates", {})
    if set(gates.values()) != {True}:
        raise OpponentAcceptanceError("opponent acceptance gates changed")
    governance = manifest.get("governance", {})
    if any(value is not False for value in governance.values()):
        raise OpponentAcceptanceError("opponent acceptance governance is unsafe")


def load_opponent_acceptance_campaign() -> tuple[dict[str, Any], str]:
    """Return the validated packaged plan and exact manifest digest."""

    package = importlib.resources.files("pitgun_databricks_adapter")
    campaign_root = package.joinpath("campaigns")
    manifest_name = CAMPAIGN_NAME + ".json"
    manifest_bytes = campaign_root.joinpath(manifest_name).read_bytes()
    checksum = campaign_root.joinpath(CAMPAIGN_NAME + ".sha256").read_text().split()
    if len(checksum) != 2 or checksum[1] != manifest_name:
        raise OpponentAcceptanceError("campaign checksum format is invalid")
    digest = hashlib.sha256(manifest_bytes).hexdigest()
    if checksum[0] != digest:
        raise OpponentAcceptanceError("campaign manifest checksum mismatch")

    manifest = json.loads(manifest_bytes)
    resource_digests = {
        run["scenario_resource"]: _sha256(
            package.joinpath(
                "scenarios", f"{run['scenario_resource']}.json"
            ).read_bytes()
        )
        for run in manifest.get("runs", [])
        if isinstance(run.get("scenario_resource"), str)
        and RESOURCE_PATTERN.fullmatch(run["scenario_resource"])
    }
    _validate_manifest(manifest, resource_digests)
    return manifest, "sha256:" + digest


def materialize_opponent_acceptance_plan(
    manifest: dict[str, Any]
) -> list[dict[str, Any]]:
    """Return the explicit immutable run plan in reviewed order."""

    return [dict(run) for run in manifest["runs"]]


def extract_opponent_acceptance_evidence(
    entry: dict[str, Any], result: dict[str, Any], manifest: dict[str, Any]
) -> dict[str, Any]:
    """Validate one native result and normalize gameplay evidence."""

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
    if result.get("scenario") != {"id": SCENARIO_ID, "version": SCENARIO_VERSION}:
        mismatches["scenario"] = result.get("scenario")
    if mismatches:
        raise OpponentAcceptanceError(
            "native identity differs from the immutable acceptance plan: "
            + repr(mismatches)
        )

    standings = result.get("summary", {}).get("standings", [])
    players = [row for row in standings if row.get("competitor_id") == "player"]
    if len(standings) != 10 or len(players) != 1:
        raise OpponentAcceptanceError("result must contain one player in ten standings")
    player = players[0]
    ordered = sorted(standings, key=lambda row: int(row["position"]))
    leader = ordered[0]
    last = ordered[-1]
    opponent_median = float(entry["opponent_budget_median"])
    player_budget = float(entry["player_budget"])
    budget_delta = player_budget - opponent_median
    metrics = {
        "racing.opponent-acceptance.player-position": (
            float(player["position"]), "rank"
        ),
        "racing.opponent-acceptance.player-win": (
            1.0 if int(player["position"]) == 1 else 0.0, "boolean"
        ),
        "racing.opponent-acceptance.player-podium": (
            1.0 if int(player["position"]) <= 3 else 0.0, "boolean"
        ),
        "racing.opponent-acceptance.player-gap-to-leader": (
            float(player["gap_to_leader_ms"]), "ms"
        ),
        "racing.opponent-acceptance.player-best-lap": (
            float(player["best_lap_ms"]), "ms"
        ),
        "racing.opponent-acceptance.field-spread": (
            float(int(last["total_time_ms"]) - int(leader["total_time_ms"])), "ms"
        ),
        "racing.opponent-acceptance.player-budget": (player_budget, "point"),
        "racing.opponent-acceptance.opponent-budget-minimum": (
            float(entry["opponent_budget_min"]), "point"
        ),
        "racing.opponent-acceptance.opponent-budget-median": (
            opponent_median, "point"
        ),
        "racing.opponent-acceptance.opponent-budget-maximum": (
            float(entry["opponent_budget_max"]), "point"
        ),
        "racing.opponent-acceptance.player-budget-delta-to-opponent-median": (
            budget_delta, "point"
        ),
    }
    return {
        "run_key": entry["run_key"],
        "source_field_id": entry["source_field_id"],
        "configuration_id": result["configuration_id"],
        "run_id": result["run_id"],
        "circuit_id": entry["circuit_id"],
        "circuit_class": entry["circuit_class"],
        "progression": entry["progression"],
        "player_reference": entry["player_reference"],
        "player_position": int(player["position"]),
        "player_gap_to_leader_ms": int(player["gap_to_leader_ms"]),
        "field_spread_ms": int(last["total_time_ms"]) - int(leader["total_time_ms"]),
        "player_budget": int(entry["player_budget"]),
        "opponent_budget_median": opponent_median,
        "metrics": metrics,
    }


def _aggregate(rows: list[dict[str, Any]]) -> dict[str, Any]:
    positions = [row["player_position"] for row in rows]
    gaps = [row["player_gap_to_leader_ms"] for row in rows]
    return {
        "successful_run_count": len(rows),
        "win_rate": sum(position == 1 for position in positions) / len(rows),
        "podium_rate": sum(position <= 3 for position in positions) / len(rows),
        "mean_position": statistics.fmean(positions),
        "median_gap_to_leader_ms": statistics.median(gaps),
        "mean_budget_delta_to_opponent_median": statistics.fmean(
            row["player_budget"] - row["opponent_budget_median"] for row in rows
        ),
    }


def summarize_opponent_acceptance(
    evidence: list[dict[str, Any]], retry_identity: dict[str, Any]
) -> dict[str, Any]:
    """Aggregate the complete matrix and expose observed gates for human review."""

    if len(evidence) != 135:
        raise OpponentAcceptanceError("acceptance summary requires all 135 results")
    by_key = {row["run_key"]: row for row in evidence}
    if len(by_key) != 135:
        raise OpponentAcceptanceError("acceptance evidence contains duplicate keys")

    overall = {
        reference: _aggregate(
            [row for row in evidence if row["player_reference"] == reference]
        )
        for reference in sorted(EXPECTED_REFERENCES)
    }
    by_circuit = {
        reference: {
            circuit: _aggregate(
                [
                    row
                    for row in evidence
                    if row["player_reference"] == reference
                    and row["circuit_id"] == circuit
                ]
            )
            for circuit in sorted(EXPECTED_CIRCUITS)
        }
        for reference in sorted(EXPECTED_REFERENCES)
    }
    by_progression = {
        reference: {
            progression: _aggregate(
                [
                    row
                    for row in evidence
                    if row["player_reference"] == reference
                    and row["progression"] == progression
                ]
            )
            for progression in sorted(EXPECTED_PROGRESSION)
        }
        for reference in sorted(EXPECTED_REFERENCES)
    }

    fields: dict[str, dict[str, dict[str, Any]]] = defaultdict(dict)
    for row in evidence:
        fields[row["source_field_id"]][row["player_reference"]] = row
    informed_better_fields = 0
    informed_better_circuits = set()
    paired_effects = []
    for field_id, rows in sorted(fields.items()):
        if set(rows) != EXPECTED_REFERENCES:
            raise OpponentAcceptanceError(f"incomplete result pair: {field_id}")
        naive = rows["naive"]
        informed = rows["circuit-informed"]
        position_gain = naive["player_position"] - informed["player_position"]
        gap_gain_ms = (
            naive["player_gap_to_leader_ms"]
            - informed["player_gap_to_leader_ms"]
        )
        is_better = position_gain > 0 or (position_gain == 0 and gap_gain_ms > 0)
        if is_better:
            informed_better_fields += 1
            informed_better_circuits.add(informed["circuit_id"])
        paired_effects.append(
            {
                "source_field_id": field_id,
                "circuit_id": informed["circuit_id"],
                "progression": informed["progression"],
                "position_gain_over_naive": position_gain,
                "gap_gain_over_naive_ms": gap_gain_ms,
                "circuit_informed_better": is_better,
            }
        )

    naive_wins = sum(
        row["player_position"] == 1
        for row in evidence
        if row["player_reference"] == "naive"
    )
    budget_parity = {
        "maximum_absolute_player_delta_to_opponent_median": max(
            abs(row["player_budget"] - row["opponent_budget_median"])
            for row in evidence
        ),
        "mean_player_delta_to_opponent_median": statistics.fmean(
            row["player_budget"] - row["opponent_budget_median"]
            for row in evidence
        ),
    }
    observed_gates = {
        "deterministic_retry_identity": bool(retry_identity.get("all_identical")),
        "no_universal_naive_victory_pattern": naive_wins < 45,
        "informed_value_across_multiple_circuit_classes": (
            len(informed_better_circuits) >= 2
        ),
        "budget_parity_reported": True,
        "human_verdict_required": True,
    }
    return {
        "schema_version": REPORT_VERSION,
        "overall": overall,
        "by_circuit": by_circuit,
        "by_progression": by_progression,
        "paired_effects": paired_effects,
        "budget_parity": budget_parity,
        "retry_identity": retry_identity,
        "observed_gates": observed_gates,
        "diagnostics": {
            "naive_win_count": naive_wins,
            "paired_field_count": len(fields),
            "circuit_informed_better_field_count": informed_better_fields,
            "circuit_informed_better_circuit_count": len(informed_better_circuits),
        },
        "decision": "REVIEW_REQUIRED",
        "automatic_game_or_catalog_promotion": False,
        "automatic_policy_mutation": False,
    }
