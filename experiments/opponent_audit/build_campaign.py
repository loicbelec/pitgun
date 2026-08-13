#!/usr/bin/env python3
"""Freeze the explicit Racing V2 opponent-audit campaign and scenarios."""

from __future__ import annotations

import hashlib
import json
import pathlib
from typing import Any

from run_smoke import (
    DEFAULT_SOURCE,
    MODEL_DIGEST,
    SOURCE_ARTIFACT_DIGEST,
    SOURCE_SCHEMA,
    OpponentSmokeError,
    balanced_points,
    canonical_pretty,
    sha256,
    validate_source,
)


ROOT = pathlib.Path(__file__).resolve().parents[2]
SCENARIO_ROOT = ROOT / "experiments" / "opponent_audit" / "campaign_scenarios"
MANIFEST_PATH = (
    ROOT / "experiments" / "databricks" / "campaigns" / "racing-opponent-audit-v1.json"
)
CHECKSUM_PATH = MANIFEST_PATH.with_suffix(".sha256")
SOURCE_REVISION = "d0a6eee23279ba838561a3662069af05a3f5d31e"
SCHEMA_VERSION = "pitgun.opponent-audit-campaign/v1"

CIRCUIT_BASELINES = {
    "BUDAPEST": {"downforce_slider": 0.72, "gear_ratio_slider": 0.40},
    "MONACO": {"downforce_slider": 0.86, "gear_ratio_slider": 0.28},
    "MONZA": {"downforce_slider": 0.22, "gear_ratio_slider": 0.78},
    "SINGAPORE": {"downforce_slider": 0.82, "gear_ratio_slider": 0.34},
    "SUZUKA": {"downforce_slider": 0.62, "gear_ratio_slider": 0.50},
}
PLAYER_REFERENCES = {
    "neutral": {
        "description": "Equal budget and allocation with neutral setup sliders.",
        "setup_source": "authored-control",
    },
    "circuit-informed": {
        "description": (
            "Equal budget and allocation with the public circuit baseline authored "
            "by the game; diagnostic only, not a validated optimum."
        ),
        "setup_source": "loicbelec/pitgun-game:src/engine/aiSetupBaselines.ts",
    },
}


def build_scenario(
    source: dict[str, Any],
    selected: dict[str, Any],
    player_reference: str,
) -> dict[str, Any]:
    if sha256(canonical_pretty(selected["contracts"])) != selected["contractDigest"]:
        raise OpponentSmokeError(f"contract digest mismatch for {selected['id']}")
    if player_reference not in PLAYER_REFERENCES:
        raise OpponentSmokeError(f"unknown player reference: {player_reference}")

    budget = int(selected["input"]["playerBudget"])
    points = balanced_points(budget)
    circuit_id = selected["identity"]["circuitId"]
    sliders = (
        {"downforce_slider": 0.5, "gear_ratio_slider": 0.5}
        if player_reference == "neutral"
        else CIRCUIT_BASELINES[circuit_id]
    )
    player = {
        "id": "player",
        "driver_id": "default",
        "name": f"Controlled {player_reference.title()} Reference",
        "team_id": "pitgun",
        "is_player": True,
        "tuning": {
            "engine_points": points["engine"],
            "cooling_points": points["cooling"],
            "aero_points": points["aero"],
            "chassis_points": points["chassis"],
            **sliders,
        },
        "budget_cap": budget,
        "stint_strategy": selected["input"]["playerStrategy"],
    }
    opponents = [
        {
            key: value
            for key, value in opponent.items()
            if key not in {"style", "points"}
        }
        for opponent in selected["contracts"]
    ]
    catalog = source["catalog"]
    return {
        "schema_version": "pitgun.racing-resolved-scenario/v1",
        "scenario": {"id": "racing.opponent-audit-campaign", "version": "1.0.0"},
        "model": {
            "id": catalog["modelId"],
            "version": catalog["modelVersion"],
            "digest": MODEL_DIGEST,
        },
        "data_pack": {
            "id": "pitgun.racing.simulation",
            "version": catalog["version"],
            "digest": catalog["simulationPackDigest"],
        },
        "clock": {"tick_numerator_us": 200000, "tick_denominator": 1},
        "request": {
            "track_id": circuit_id,
            "laps": selected["input"]["laps"],
            "competitors": [player, *opponents],
            "vehicle_id": selected["input"]["vehicleId"],
            "era": selected["input"]["era"],
            "hz": 5.0,
        },
    }


def build_manifest(source: dict[str, Any]) -> dict[str, Any]:
    runs: list[dict[str, Any]] = []
    expected_paths: set[pathlib.Path] = set()
    for selected in sorted(source["scenarios"], key=lambda item: item["id"]):
        identity = selected["identity"]
        opponent_budgets = [
            int(contract["budget_cap"]) for contract in selected["contracts"]
        ]
        distinct_tunings = len(
            {
                json.dumps(contract["tuning"], sort_keys=True, separators=(",", ":"))
                for contract in selected["contracts"]
            }
        )
        distinct_strategies = len(
            {
                json.dumps(
                    contract["stint_strategy"], sort_keys=True, separators=(",", ":")
                )
                for contract in selected["contracts"]
            }
        )
        for player_reference in PLAYER_REFERENCES:
            resource = f"{selected['id']}--{player_reference}"
            path = SCENARIO_ROOT / f"{resource}.json"
            scenario_bytes = canonical_pretty(
                build_scenario(source, selected, player_reference)
            )
            path.write_bytes(scenario_bytes)
            expected_paths.add(path)
            runs.append(
                {
                    "run_key": resource,
                    "scenario_resource": resource,
                    "scenario_resource_digest": sha256(scenario_bytes),
                    "source_scenario_id": selected["id"],
                    "source_contract_digest": selected["contractDigest"],
                    "circuit_id": identity["circuitId"],
                    "progression": identity["progression"],
                    "era": selected["input"]["era"],
                    "seed": identity["seed"],
                    "strategy_profile": identity["strategyProfile"],
                    "player_reference": player_reference,
                    "opponent_budget_min": min(opponent_budgets),
                    "opponent_budget_max": max(opponent_budgets),
                    "distinct_opponent_tunings": distinct_tunings,
                    "distinct_opponent_strategies": distinct_strategies,
                }
            )

    unexpected = set(SCENARIO_ROOT.glob("*.json")) - expected_paths
    if unexpected:
        raise OpponentSmokeError(
            "unplanned campaign scenarios: "
            + ", ".join(path.name for path in sorted(unexpected))
        )

    return {
        "schema_version": SCHEMA_VERSION,
        "campaign_id": "racing-opponent-audit-2026-v1",
        "question": (
            "How competitive and diverse are the current game opponents against "
            "neutral and circuit-informed controlled player references?"
        ),
        "source": {
            "repository": "loicbelec/pitgun-game",
            "revision": SOURCE_REVISION,
            "schema_version": SOURCE_SCHEMA,
            "artifact_digest": SOURCE_ARTIFACT_DIGEST,
        },
        "catalog": {
            "id": source["catalog"]["id"],
            "version": source["catalog"]["version"],
            "manifest_digest": source["catalog"]["manifestDigest"],
            "simulation_pack_digest": source["catalog"]["simulationPackDigest"],
            "model_id": source["catalog"]["modelId"],
            "model_version": source["catalog"]["modelVersion"],
            "model_digest": MODEL_DIGEST,
        },
        "player_references": [
            {"id": identifier, **definition}
            for identifier, definition in PLAYER_REFERENCES.items()
        ],
        "matrix": {
            "circuits": source["matrix"]["circuits"],
            "progression": source["matrix"]["progression"],
            "seeds": source["matrix"]["seeds"],
            "strategy_profiles": source["matrix"]["strategyProfiles"],
        },
        "planned_run_count": len(runs),
        "runs": runs,
        "governance": {
            "private_player_data_allowed": False,
            "automatic_game_or_catalog_promotion": False,
            "circuit_informed_reference_is_validated_optimum": False,
        },
    }


def main() -> None:
    source = json.loads(DEFAULT_SOURCE.read_text())
    validate_source(source)
    if len(source.get("scenarios", [])) != 90:
        raise OpponentSmokeError("campaign requires exactly 90 source scenarios")
    SCENARIO_ROOT.mkdir(parents=True, exist_ok=True)
    manifest = build_manifest(source)
    manifest_bytes = canonical_pretty(manifest)
    MANIFEST_PATH.write_bytes(manifest_bytes)
    CHECKSUM_PATH.write_text(
        f"{hashlib.sha256(manifest_bytes).hexdigest()}  {MANIFEST_PATH.name}\n"
    )
    print(
        f"froze {manifest['planned_run_count']} explicit opponent audit runs; "
        f"manifest {sha256(manifest_bytes)}"
    )


if __name__ == "__main__":
    main()
