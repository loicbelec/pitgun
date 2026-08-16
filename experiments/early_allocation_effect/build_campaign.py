#!/usr/bin/env python3
"""Freeze the Racing early-game marginal development-allocation campaign."""

from __future__ import annotations

import copy
import hashlib
import json
import pathlib
from typing import Any


ROOT = pathlib.Path(__file__).resolve().parents[2]
SOURCE_MANIFEST = (
    ROOT / "experiments" / "databricks" / "campaigns" / "racing-budget-effect-v2.json"
)
SOURCE_CHECKSUM = SOURCE_MANIFEST.with_suffix(".sha256")
SOURCE_SCENARIOS = ROOT / "experiments" / "budget_effect_v2" / "scenarios"
SCENARIO_ROOT = ROOT / "experiments" / "early_allocation_effect" / "scenarios"
MANIFEST_PATH = (
    ROOT
    / "experiments"
    / "databricks"
    / "campaigns"
    / "racing-early-allocation-effect-v1.json"
)
CHECKSUM_PATH = MANIFEST_PATH.with_suffix(".sha256")

SCHEMA_VERSION = "pitgun.early-allocation-effect-campaign/v1"
CAMPAIGN_ID = "racing-early-allocation-effect-2026-v1"
SCENARIO_IDENTITY = {
    "id": "racing.early-allocation-effect-campaign",
    "version": "1.0.0",
}
SOURCE_MANIFEST_DIGEST = (
    "sha256:dc419513206268fd9a12f98a585dbadebf11585397f78637db84627db136e59b"
)
SOURCE_EXECUTION = {
    "issue": "loicbelec/pitgun#230",
    "mlflow_run_id": "ea3e8ae16fba45e4878656219b421359",
    "job_run_id": "739348618705542",
    "idempotent_replay_run_id": "999013012120037",
}
POINT_KEYS = ("aero_points", "chassis_points", "cooling_points", "engine_points")
AXES = tuple(key.removesuffix("_points") for key in POINT_KEYS)
REFERENCE_ALLOCATION = {key: 1 for key in POINT_KEYS}


class EarlyAllocationBuildError(ValueError):
    """Raised when the source cannot produce an exact marginal campaign."""


def canonical_bytes(value: Any) -> bytes:
    return (json.dumps(value, indent=2, ensure_ascii=False) + "\n").encode()


def compact_bytes(value: Any) -> bytes:
    return json.dumps(
        value, sort_keys=True, separators=(",", ":"), ensure_ascii=False
    ).encode()


def sha256(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


def treatment_matrix() -> dict[str, dict[str, Any]]:
    treatments: dict[str, dict[str, Any]] = {
        "reference": {
            "direction": "reference",
            "axis": None,
            "budget": 4,
            "allocation": dict(REFERENCE_ALLOCATION),
        }
    }
    for direction, delta in (("add", 1), ("remove", -1)):
        for axis, key in zip(AXES, POINT_KEYS):
            allocation = dict(REFERENCE_ALLOCATION)
            allocation[key] += delta
            treatments[f"{direction}_{axis}"] = {
                "direction": direction,
                "axis": axis,
                "budget": sum(allocation.values()),
                "allocation": allocation,
            }
    return treatments


TREATMENTS = treatment_matrix()


def controlled_player(scenario: dict[str, Any]) -> dict[str, Any]:
    players = [row for row in scenario["request"]["competitors"] if row["is_player"]]
    if len(players) != 1 or players[0]["id"] != "player":
        raise EarlyAllocationBuildError("source scenario must contain one player")
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
    source_digest = "sha256:" + actual_digest
    if expected_name != SOURCE_MANIFEST.name or expected_digest != actual_digest:
        raise EarlyAllocationBuildError("budget V2 campaign checksum mismatch")
    if source_digest != SOURCE_MANIFEST_DIGEST:
        raise EarlyAllocationBuildError("budget V2 campaign provenance changed")
    manifest = json.loads(manifest_bytes)
    if manifest.get("campaign_id") != "racing-budget-effect-2026-v2":
        raise EarlyAllocationBuildError("unexpected source campaign")
    return manifest, source_digest


def reference_source_index(manifest: dict[str, Any]) -> dict[str, dict[str, Any]]:
    runs = {
        run["triplet_key"]: run
        for run in manifest["runs"]
        if run["progression"] == "early" and run["treatment"] == "reference"
    }
    if len(runs) != 15:
        raise EarlyAllocationBuildError("expected 15 early reference scenarios")
    return runs


def load_source_scenario(run: dict[str, Any]) -> dict[str, Any]:
    path = SOURCE_SCENARIOS / f"{run['scenario_resource']}.json"
    data = path.read_bytes()
    if sha256(data) != run["scenario_resource_digest"]:
        raise EarlyAllocationBuildError(f"source resource changed: {path.name}")
    scenario = json.loads(data)
    player = controlled_player(scenario)
    allocation = {key: int(player["tuning"][key]) for key in POINT_KEYS}
    if int(player["budget_cap"]) != 4 or allocation != REFERENCE_ALLOCATION:
        raise EarlyAllocationBuildError(f"source is not a 1/1/1/1 reference: {path.name}")
    return scenario


def build_manifest(
    source: dict[str, Any],
    source_digest: str,
    scenario_root: pathlib.Path = SCENARIO_ROOT,
) -> dict[str, Any]:
    runs = []
    expected_paths = set()
    scenario_root.mkdir(parents=True, exist_ok=True)
    for block_key, source_run in sorted(reference_source_index(source).items()):
        base = load_source_scenario(source_run)
        base["scenario"] = dict(SCENARIO_IDENTITY)
        player = controlled_player(base)
        player["name"] = "Early Marginal Allocation Reference"
        invariant_digest = sha256(compact_bytes(invariant_projection(base)))

        for treatment, specification in TREATMENTS.items():
            scenario = copy.deepcopy(base)
            treated_player = controlled_player(scenario)
            treated_player["budget_cap"] = specification["budget"]
            treated_player["tuning"].update(specification["allocation"])
            if sha256(compact_bytes(invariant_projection(scenario))) != invariant_digest:
                raise EarlyAllocationBuildError(
                    f"non-allocation input changed inside block: {block_key}"
                )
            resource = f"early-allocation-{block_key}--{treatment}"
            path = scenario_root / f"{resource}.json"
            scenario_bytes = canonical_bytes(scenario)
            path.write_bytes(scenario_bytes)
            expected_paths.add(path)
            runs.append(
                {
                    "run_key": resource,
                    "block_key": block_key,
                    "scenario_resource": resource,
                    "scenario_resource_digest": sha256(scenario_bytes),
                    "block_invariant_digest": invariant_digest,
                    "treatment": treatment,
                    "direction": specification["direction"],
                    "axis": specification["axis"],
                    "player_budget": specification["budget"],
                    "player_allocation": specification["allocation"],
                    "opponent_budget": 4,
                    "source_opponent_contract_digest": source_run[
                        "source_opponent_contract_digest"
                    ],
                    "source_resource_digest": source_run["scenario_resource_digest"],
                    "circuit_id": source_run["circuit_id"],
                    "progression": "early",
                    "era": int(source_run["era"]),
                    "seed": int(source_run["seed"]),
                }
            )

    unexpected = set(scenario_root.glob("*.json")) - expected_paths
    if unexpected:
        raise EarlyAllocationBuildError(
            "unplanned early-allocation scenarios: "
            + ", ".join(path.name for path in sorted(unexpected))
        )
    source_matrix = source["matrix"]
    circuits = sorted({run["circuit_id"] for run in runs})
    seeds = sorted({run["seed"] for run in runs})
    return {
        "schema_version": SCHEMA_VERSION,
        "campaign_id": CAMPAIGN_ID,
        "question": (
            "Which physical development axis gives or removes marginal performance "
            "at the four-point early-game boundary?"
        ),
        "source": {
            "campaign_id": source["campaign_id"],
            "manifest_digest": source_digest,
            "execution_evidence": dict(SOURCE_EXECUTION),
            "gameplay_progression": copy.deepcopy(
                source["source"]["gameplay_progression"]
            ),
        },
        "catalog": copy.deepcopy(source["catalog"]),
        "controlled_input": {
            "progression": "early",
            "reference_budget": 4,
            "reference_allocation": dict(REFERENCE_ALLOCATION),
            "treatments": copy.deepcopy(TREATMENTS),
            "player_reference": "neutral",
            "player_strategy": "balanced-one-stop",
            "opponent_field": "frozen-economy-backed-four-point-field",
            "only_allowed_comparison_difference": (
                "one player development point on one named physical axis"
            ),
        },
        "matrix": {
            "circuits": circuits,
            "seeds": seeds,
            "era": next(
                row["era"]
                for row in source_matrix["progression"]
                if row["id"] == "early"
            ),
            "treatment_count": len(TREATMENTS),
        },
        "planned_block_count": 15,
        "planned_run_count": len(runs),
        "runs": runs,
        "governance": {
            "private_player_data_allowed": False,
            "automatic_game_or_catalog_promotion": False,
            "automatic_opponent_policy_selection_allowed": False,
            "solver_change_allowed": False,
            "late_era_change_allowed": False,
            "agent_driving_change_allowed": False,
        },
    }


def validate_manifest(
    manifest: dict[str, Any], resources: dict[str, bytes], source_digest: str
) -> None:
    if manifest.get("schema_version") != SCHEMA_VERSION:
        raise EarlyAllocationBuildError("unsupported early-allocation schema")
    if manifest.get("campaign_id") != CAMPAIGN_ID:
        raise EarlyAllocationBuildError("early-allocation campaign identity changed")
    if manifest.get("planned_block_count") != 15:
        raise EarlyAllocationBuildError("campaign must contain 15 blocks")
    runs = manifest.get("runs", [])
    if manifest.get("planned_run_count") != 135 or len(runs) != 135:
        raise EarlyAllocationBuildError("campaign must contain 135 runs")
    source = manifest.get("source", {})
    if (
        source.get("manifest_digest") != source_digest
        or source.get("execution_evidence") != SOURCE_EXECUTION
    ):
        raise EarlyAllocationBuildError("source evidence changed")
    controlled = manifest.get("controlled_input", {})
    if (
        controlled.get("progression") != "early"
        or controlled.get("reference_budget") != 4
        or controlled.get("reference_allocation") != REFERENCE_ALLOCATION
        or controlled.get("treatments") != TREATMENTS
        or controlled.get("only_allowed_comparison_difference")
        != "one player development point on one named physical axis"
    ):
        raise EarlyAllocationBuildError("controlled allocation boundary changed")
    governance = manifest.get("governance", {})
    if not governance or any(value is not False for value in governance.values()):
        raise EarlyAllocationBuildError("campaign governance is unsafe")

    run_keys = [run.get("run_key") for run in runs]
    if len(set(run_keys)) != 135 or set(resources) != set(run_keys):
        raise EarlyAllocationBuildError("resources and run keys differ")
    blocks: dict[str, list[dict[str, Any]]] = {}
    for run in runs:
        resource = run["scenario_resource"]
        data = resources.get(resource)
        if data is None or sha256(data) != run["scenario_resource_digest"]:
            raise EarlyAllocationBuildError(f"scenario changed: {resource}")
        scenario = json.loads(data)
        if scenario.get("scenario") != SCENARIO_IDENTITY:
            raise EarlyAllocationBuildError(f"scenario identity changed: {resource}")
        catalog = manifest["catalog"]
        if scenario.get("model") != {
            "id": catalog["model_id"],
            "version": catalog["model_version"],
            "digest": catalog["model_digest"],
        }:
            raise EarlyAllocationBuildError(f"model identity changed: {resource}")
        specification = TREATMENTS.get(run["treatment"])
        if specification is None:
            raise EarlyAllocationBuildError(f"unknown treatment: {resource}")
        player = controlled_player(scenario)
        allocation = {key: int(player["tuning"][key]) for key in POINT_KEYS}
        if (
            run["direction"] != specification["direction"]
            or run["axis"] != specification["axis"]
            or run["player_budget"] != specification["budget"]
            or run["player_allocation"] != specification["allocation"]
            or int(player["budget_cap"]) != specification["budget"]
            or allocation != specification["allocation"]
        ):
            raise EarlyAllocationBuildError(f"invalid treatment: {resource}")
        opponents = [
            row for row in scenario["request"]["competitors"] if not row["is_player"]
        ]
        if len(opponents) != 9:
            raise EarlyAllocationBuildError(f"invalid opponent field: {resource}")
        for opponent in opponents:
            points = [int(opponent["tuning"][key]) for key in POINT_KEYS]
            if int(opponent["budget_cap"]) != 4 or sum(points) != 4:
                raise EarlyAllocationBuildError(
                    f"opponent field changed: {resource}/{opponent['id']}"
                )
        if sha256(compact_bytes(invariant_projection(scenario))) != run[
            "block_invariant_digest"
        ]:
            raise EarlyAllocationBuildError(f"block invariant changed: {resource}")
        blocks.setdefault(run["block_key"], []).append(run)

    if len(blocks) != 15:
        raise EarlyAllocationBuildError("campaign block keys are incomplete")
    expected_treatments = set(TREATMENTS)
    for block_key, block in blocks.items():
        if {run["treatment"] for run in block} != expected_treatments:
            raise EarlyAllocationBuildError(f"incomplete block: {block_key}")
        if len({run["block_invariant_digest"] for run in block}) != 1:
            raise EarlyAllocationBuildError(f"block input changed: {block_key}")


def main() -> None:
    source, source_digest = load_source()
    manifest = build_manifest(source, source_digest)
    resources = {
        path.stem: path.read_bytes() for path in SCENARIO_ROOT.glob("*.json")
    }
    validate_manifest(manifest, resources, source_digest)
    manifest_bytes = canonical_bytes(manifest)
    MANIFEST_PATH.write_bytes(manifest_bytes)
    CHECKSUM_PATH.write_text(
        f"{hashlib.sha256(manifest_bytes).hexdigest()}  {MANIFEST_PATH.name}\n"
    )
    print(
        f"froze {manifest['planned_block_count']} early-allocation blocks and "
        f"{manifest['planned_run_count']} runs; manifest {sha256(manifest_bytes)}"
    )


if __name__ == "__main__":
    main()
