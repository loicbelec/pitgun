"""Load the immutable controlled Racing development-budget campaign."""

from __future__ import annotations

import copy
import hashlib
import importlib.resources
import json
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
