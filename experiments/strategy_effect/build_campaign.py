#!/usr/bin/env python3
"""Freeze the controlled Racing V2 player-strategy campaign."""

from __future__ import annotations

import copy
import hashlib
import json
import pathlib
from typing import Any


ROOT = pathlib.Path(__file__).resolve().parents[2]
SOURCE_MANIFEST = (
    ROOT / "experiments" / "databricks" / "campaigns" / "racing-opponent-audit-v1.json"
)
SOURCE_CHECKSUM = SOURCE_MANIFEST.with_suffix(".sha256")
SOURCE_SCENARIOS = ROOT / "experiments" / "opponent_audit" / "campaign_scenarios"
SCENARIO_ROOT = ROOT / "experiments" / "strategy_effect" / "scenarios"
MANIFEST_PATH = (
    ROOT / "experiments" / "databricks" / "campaigns" / "racing-strategy-effect-v1.json"
)
CHECKSUM_PATH = MANIFEST_PATH.with_suffix(".sha256")
SCHEMA_VERSION = "pitgun.strategy-effect-campaign/v1"
STRATEGIES = ("balanced-one-stop", "late-one-stop")


class StrategyEffectBuildError(ValueError):
    """Raised when the frozen source cannot produce a causal pair."""


def canonical_bytes(value: Any) -> bytes:
    return (json.dumps(value, indent=2, ensure_ascii=False) + "\n").encode()


def compact_bytes(value: Any) -> bytes:
    return json.dumps(
        value, sort_keys=True, separators=(",", ":"), ensure_ascii=False
    ).encode()


def sha256(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


def controlled_player(scenario: dict[str, Any]) -> dict[str, Any]:
    players = [
        row for row in scenario["request"]["competitors"] if row["is_player"]
    ]
    if len(players) != 1 or players[0]["id"] != "player":
        raise StrategyEffectBuildError("source scenario must contain one player")
    return players[0]


def causal_projection(scenario: dict[str, Any]) -> dict[str, Any]:
    projection = copy.deepcopy(scenario)
    player = controlled_player(projection)
    player.pop("stint_strategy")
    return projection


def load_source() -> tuple[dict[str, Any], str]:
    manifest_bytes = SOURCE_MANIFEST.read_bytes()
    expected_digest, expected_name = SOURCE_CHECKSUM.read_text().split()
    actual_digest = hashlib.sha256(manifest_bytes).hexdigest()
    if expected_name != SOURCE_MANIFEST.name or expected_digest != actual_digest:
        raise StrategyEffectBuildError("opponent audit manifest checksum mismatch")
    manifest = json.loads(manifest_bytes)
    if manifest.get("planned_run_count") != 180:
        raise StrategyEffectBuildError("opponent audit source is incomplete")
    return manifest, "sha256:" + actual_digest


def source_index(manifest: dict[str, Any]) -> dict[tuple[str, str, int, str], dict[str, Any]]:
    index = {}
    for run in manifest["runs"]:
        if run["player_reference"] != "neutral":
            continue
        key = (
            run["circuit_id"],
            run["progression"],
            int(run["seed"]),
            run["strategy_profile"],
        )
        index[key] = run
    if len(index) != 90:
        raise StrategyEffectBuildError("expected 90 neutral source scenarios")
    return index


def load_source_scenario(run: dict[str, Any]) -> dict[str, Any]:
    path = SOURCE_SCENARIOS / f"{run['scenario_resource']}.json"
    data = path.read_bytes()
    if sha256(data) != run["scenario_resource_digest"]:
        raise StrategyEffectBuildError(f"source resource changed: {path.name}")
    return json.loads(data)


def build_manifest(source: dict[str, Any], source_digest: str) -> dict[str, Any]:
    indexed = source_index(source)
    runs = []
    expected_paths = set()
    for circuit in sorted(row["id"] for row in source["matrix"]["circuits"]):
        for progression in ("early", "mid", "late"):
            for seed in sorted(int(value) for value in source["matrix"]["seeds"]):
                pair_key = f"{circuit.lower()}-{progression}-{seed}"
                balanced_run = indexed[(circuit, progression, seed, STRATEGIES[0])]
                late_run = indexed[(circuit, progression, seed, STRATEGIES[1])]
                base = load_source_scenario(balanced_run)
                late_source = load_source_scenario(late_run)
                base["scenario"] = {
                    "id": "racing.strategy-effect-campaign",
                    "version": "1.0.0",
                }
                player = controlled_player(base)
                player["name"] = "Controlled Strategy Reference"
                strategies = {
                    STRATEGIES[0]: copy.deepcopy(player["stint_strategy"]),
                    STRATEGIES[1]: copy.deepcopy(
                        controlled_player(late_source)["stint_strategy"]
                    ),
                }
                pair_projection_digest = sha256(compact_bytes(causal_projection(base)))
                for strategy_profile in STRATEGIES:
                    scenario = copy.deepcopy(base)
                    controlled_player(scenario)["stint_strategy"] = strategies[
                        strategy_profile
                    ]
                    actual_projection_digest = sha256(
                        compact_bytes(causal_projection(scenario))
                    )
                    if actual_projection_digest != pair_projection_digest:
                        raise StrategyEffectBuildError(
                            f"non-strategy input changed inside pair: {pair_key}"
                        )
                    resource = f"strategy-effect-{pair_key}--{strategy_profile}"
                    scenario_bytes = canonical_bytes(scenario)
                    path = SCENARIO_ROOT / f"{resource}.json"
                    path.write_bytes(scenario_bytes)
                    expected_paths.add(path)
                    runs.append(
                        {
                            "run_key": resource,
                            "pair_key": pair_key,
                            "scenario_resource": resource,
                            "scenario_resource_digest": sha256(scenario_bytes),
                            "pair_invariant_digest": pair_projection_digest,
                            "player_strategy_digest": sha256(
                                compact_bytes(strategies[strategy_profile])
                            ),
                            "source_opponent_contract_digest": balanced_run[
                                "source_contract_digest"
                            ],
                            "source_balanced_resource_digest": balanced_run[
                                "scenario_resource_digest"
                            ],
                            "source_strategy_resource_digest": (
                                balanced_run["scenario_resource_digest"]
                                if strategy_profile == STRATEGIES[0]
                                else late_run["scenario_resource_digest"]
                            ),
                            "circuit_id": circuit,
                            "progression": progression,
                            "era": int(balanced_run["era"]),
                            "seed": seed,
                            "strategy_profile": strategy_profile,
                        }
                    )

    unexpected = set(SCENARIO_ROOT.glob("*.json")) - expected_paths
    if unexpected:
        raise StrategyEffectBuildError(
            "unplanned strategy scenarios: "
            + ", ".join(path.name for path in sorted(unexpected))
        )
    return {
        "schema_version": SCHEMA_VERSION,
        "campaign_id": "racing-strategy-effect-2026-v1",
        "question": (
            "What is the causal effect of changing only the controlled player's "
            "one-stop strategy against a frozen opponent field?"
        ),
        "source": {
            "campaign_id": source["campaign_id"],
            "manifest_digest": source_digest,
            "repository": source["source"]["repository"],
            "revision": source["source"]["revision"],
        },
        "catalog": source["catalog"],
        "controlled_input": {
            "player_reference": "neutral",
            "strategy_profiles": list(STRATEGIES),
            "only_allowed_pair_difference": "player.stint_strategy",
            "opponent_field_source": "balanced-one-stop",
        },
        "matrix": {
            "circuits": source["matrix"]["circuits"],
            "progression": source["matrix"]["progression"],
            "seeds": source["matrix"]["seeds"],
        },
        "planned_pair_count": 45,
        "planned_run_count": len(runs),
        "runs": runs,
        "governance": {
            "private_player_data_allowed": False,
            "automatic_game_or_catalog_promotion": False,
            "policy_selection_allowed": False,
        },
    }


def main() -> None:
    source, source_digest = load_source()
    SCENARIO_ROOT.mkdir(parents=True, exist_ok=True)
    manifest = build_manifest(source, source_digest)
    manifest_bytes = canonical_bytes(manifest)
    MANIFEST_PATH.write_bytes(manifest_bytes)
    CHECKSUM_PATH.write_text(
        f"{hashlib.sha256(manifest_bytes).hexdigest()}  {MANIFEST_PATH.name}\n"
    )
    print(
        f"froze {manifest['planned_pair_count']} causal strategy pairs and "
        f"{manifest['planned_run_count']} runs; manifest {sha256(manifest_bytes)}"
    )


if __name__ == "__main__":
    main()
