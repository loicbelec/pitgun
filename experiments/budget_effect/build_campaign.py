#!/usr/bin/env python3
"""Freeze the controlled Racing V2 development-budget campaign."""

from __future__ import annotations

import copy
import hashlib
import json
import pathlib
import statistics
from typing import Any


ROOT = pathlib.Path(__file__).resolve().parents[2]
SOURCE_MANIFEST = (
    ROOT / "experiments" / "databricks" / "campaigns" / "racing-strategy-effect-v1.json"
)
SOURCE_CHECKSUM = SOURCE_MANIFEST.with_suffix(".sha256")
SOURCE_SCENARIOS = ROOT / "experiments" / "strategy_effect" / "scenarios"
SCENARIO_ROOT = ROOT / "experiments" / "budget_effect" / "scenarios"
MANIFEST_PATH = (
    ROOT / "experiments" / "databricks" / "campaigns" / "racing-budget-effect-v1.json"
)
CHECKSUM_PATH = MANIFEST_PATH.with_suffix(".sha256")
SCHEMA_VERSION = "pitgun.budget-effect-campaign/v1"
TREATMENTS = {
    "field-090": 90,
    "field-100": 100,
    "field-110": 110,
}
POINT_KEYS = ("aero_points", "chassis_points", "cooling_points", "engine_points")


class BudgetEffectBuildError(ValueError):
    """Raised when the source cannot produce one controlled budget triplet."""


def canonical_bytes(value: Any) -> bytes:
    return (json.dumps(value, indent=2, ensure_ascii=False) + "\n").encode()


def compact_bytes(value: Any) -> bytes:
    return json.dumps(
        value, sort_keys=True, separators=(",", ":"), ensure_ascii=False
    ).encode()


def sha256(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


def balanced_points(budget: int) -> dict[str, int]:
    quotient, remainder = divmod(budget, 4)
    return {
        key: quotient + (1 if index < remainder else 0)
        for index, key in enumerate(POINT_KEYS)
    }


def treated_budget(field_median: int, percentage: int) -> int:
    return (field_median * percentage + 50) // 100


def controlled_player(scenario: dict[str, Any]) -> dict[str, Any]:
    players = [row for row in scenario["request"]["competitors"] if row["is_player"]]
    if len(players) != 1 or players[0]["id"] != "player":
        raise BudgetEffectBuildError("source scenario must contain one player")
    return players[0]


def invariant_projection(scenario: dict[str, Any]) -> dict[str, Any]:
    projection = copy.deepcopy(scenario)
    player = controlled_player(projection)
    player.pop("budget_cap")
    for key in POINT_KEYS:
        player["tuning"].pop(key)
    return projection


def load_source() -> tuple[dict[str, Any], str]:
    manifest_bytes = SOURCE_MANIFEST.read_bytes()
    expected_digest, expected_name = SOURCE_CHECKSUM.read_text().split()
    actual_digest = hashlib.sha256(manifest_bytes).hexdigest()
    if expected_name != SOURCE_MANIFEST.name or expected_digest != actual_digest:
        raise BudgetEffectBuildError("strategy campaign checksum mismatch")
    manifest = json.loads(manifest_bytes)
    if manifest.get("planned_pair_count") != 45:
        raise BudgetEffectBuildError("strategy source campaign is incomplete")
    return manifest, "sha256:" + actual_digest


def balanced_source_index(manifest: dict[str, Any]) -> dict[str, dict[str, Any]]:
    runs = {
        run["pair_key"]: run
        for run in manifest["runs"]
        if run["strategy_profile"] == "balanced-one-stop"
    }
    if len(runs) != 45:
        raise BudgetEffectBuildError("expected 45 balanced source scenarios")
    return runs


def load_source_scenario(run: dict[str, Any]) -> dict[str, Any]:
    path = SOURCE_SCENARIOS / f"{run['scenario_resource']}.json"
    data = path.read_bytes()
    if sha256(data) != run["scenario_resource_digest"]:
        raise BudgetEffectBuildError(f"source resource changed: {path.name}")
    return json.loads(data)


def build_manifest(source: dict[str, Any], source_digest: str) -> dict[str, Any]:
    runs = []
    expected_paths = set()
    for triplet_key, source_run in sorted(balanced_source_index(source).items()):
        base = load_source_scenario(source_run)
        base["scenario"] = {
            "id": "racing.budget-effect-campaign",
            "version": "1.0.0",
        }
        player = controlled_player(base)
        player["name"] = "Controlled Development Budget Reference"
        opponents = [
            row for row in base["request"]["competitors"] if not row["is_player"]
        ]
        opponent_budgets = [int(row["budget_cap"]) for row in opponents]
        field_median = int(statistics.median(opponent_budgets))
        invariant_digest = sha256(compact_bytes(invariant_projection(base)))
        treatment_budgets = {
            treatment: treated_budget(field_median, percentage)
            for treatment, percentage in TREATMENTS.items()
        }
        if len(set(treatment_budgets.values())) != len(TREATMENTS):
            raise BudgetEffectBuildError(f"budget treatments collapsed: {triplet_key}")

        for treatment, percentage in TREATMENTS.items():
            scenario = copy.deepcopy(base)
            treated_player = controlled_player(scenario)
            budget = treatment_budgets[treatment]
            treated_player["budget_cap"] = budget
            treated_player["tuning"].update(balanced_points(budget))
            actual_invariant = sha256(compact_bytes(invariant_projection(scenario)))
            if actual_invariant != invariant_digest:
                raise BudgetEffectBuildError(
                    f"non-budget input changed inside triplet: {triplet_key}"
                )
            resource = f"budget-effect-{triplet_key}--{treatment}"
            path = SCENARIO_ROOT / f"{resource}.json"
            scenario_bytes = canonical_bytes(scenario)
            path.write_bytes(scenario_bytes)
            expected_paths.add(path)
            runs.append(
                {
                    "run_key": resource,
                    "triplet_key": triplet_key,
                    "scenario_resource": resource,
                    "scenario_resource_digest": sha256(scenario_bytes),
                    "triplet_invariant_digest": invariant_digest,
                    "treatment": treatment,
                    "treatment_percentage": percentage,
                    "field_median_budget": field_median,
                    "player_budget": budget,
                    "player_allocation": balanced_points(budget),
                    "source_opponent_contract_digest": source_run[
                        "source_opponent_contract_digest"
                    ],
                    "source_resource_digest": source_run[
                        "scenario_resource_digest"
                    ],
                    "circuit_id": source_run["circuit_id"],
                    "progression": source_run["progression"],
                    "era": int(source_run["era"]),
                    "seed": int(source_run["seed"]),
                }
            )

    unexpected = set(SCENARIO_ROOT.glob("*.json")) - expected_paths
    if unexpected:
        raise BudgetEffectBuildError(
            "unplanned budget scenarios: "
            + ", ".join(path.name for path in sorted(unexpected))
        )
    return {
        "schema_version": SCHEMA_VERSION,
        "campaign_id": "racing-budget-effect-2026-v1",
        "question": (
            "How does controlled player competitiveness respond to development "
            "budget at 90, 100, and 110 percent of a frozen field median?"
        ),
        "source": {
            "campaign_id": source["campaign_id"],
            "manifest_digest": source_digest,
            "upstream_opponent_campaign": source["source"],
        },
        "catalog": source["catalog"],
        "controlled_input": {
            "player_reference": "neutral",
            "player_strategy": "balanced-one-stop",
            "treatments": [
                {"id": treatment, "field_median_percentage": percentage}
                for treatment, percentage in TREATMENTS.items()
            ],
            "rounding": "nearest-integer-half-up",
            "allocation": "deterministic-balanced-four-axis",
            "only_allowed_triplet_difference": (
                "player.development_budget_and_balanced_point_allocation"
            ),
            "opponent_field_source": "balanced-one-stop",
        },
        "matrix": source["matrix"],
        "planned_triplet_count": 45,
        "planned_run_count": len(runs),
        "runs": runs,
        "governance": {
            "private_player_data_allowed": False,
            "automatic_game_or_catalog_promotion": False,
            "automatic_budget_target_selection_allowed": False,
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
        f"froze {manifest['planned_triplet_count']} budget triplets and "
        f"{manifest['planned_run_count']} runs; manifest {sha256(manifest_bytes)}"
    )


if __name__ == "__main__":
    main()
