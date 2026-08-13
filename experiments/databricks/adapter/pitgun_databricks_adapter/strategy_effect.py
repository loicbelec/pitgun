"""Load the immutable controlled Racing player-strategy campaign."""

from __future__ import annotations

import copy
import hashlib
import importlib.resources
import json
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
