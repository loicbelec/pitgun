#!/usr/bin/env python3
"""Run the pre-registered modern-V6T thermal refinement screen."""

from __future__ import annotations

import argparse
import concurrent.futures
import copy
import hashlib
import importlib.util
import json
import math
import pathlib
from typing import Any


ROOT = pathlib.Path(__file__).resolve().parents[2]
EXPERIMENT = pathlib.Path(__file__).resolve().parent
CONTRACT = EXPERIMENT / "contract-v1.json"
DEFAULT_OUTPUT = EXPERIMENT / "results" / "local-refinement-v1.json"
BASE_SCENARIO = (
    ROOT / "apps" / "pitgun-cli" / "scenarios" / "racing-batch-v1" / "balanced.json"
)
BASE_PROFILE = (
    ROOT
    / "experiments"
    / "racing_v3_decision_surface"
    / "profile-v8.engine-thermal-resolution.json"
)
THERMAL_SCREEN = ROOT / "experiments" / "racing_v3_thermal" / "screen_local.py"
SOURCE_CAMPAIGN = (
    ROOT
    / "experiments"
    / "databricks"
    / "campaigns"
    / "racing-v3-thermal-adequacy-v1.json"
)
SCHEMA_VERSION = "pitgun.racing-v3-thermal-refinement-local/v1"
MAX_JOBS = 16


class RefinementError(RuntimeError):
    """Raised when the pre-registered experiment cannot be reproduced."""


def canonical_pretty(value: object) -> bytes:
    return (json.dumps(value, indent=2, ensure_ascii=False) + "\n").encode()


def sha256(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


def load_thermal_screen():
    specification = importlib.util.spec_from_file_location(
        "racing_v3_thermal_screen", THERMAL_SCREEN
    )
    if specification is None or specification.loader is None:
        raise RefinementError("cannot load the governed thermal runner")
    module = importlib.util.module_from_spec(specification)
    specification.loader.exec_module(module)
    return module


def load_contract(path: pathlib.Path = CONTRACT) -> tuple[dict[str, Any], bytes]:
    payload = path.read_bytes()
    expected = path.with_suffix(".sha256").read_text().split()
    if len(expected) != 2 or expected[1] != path.name or expected[0] != hashlib.sha256(payload).hexdigest():
        raise RefinementError("refinement contract does not match its checksum")
    contract = json.loads(payload)
    if contract.get("schema_version") != "pitgun.racing-v3-thermal-refinement-contract/v1":
        raise RefinementError("unsupported refinement contract")
    if not contract["reserved_final_validation"]["must_not_run_during_local_selection"]:
        raise RefinementError("final validation is not reserved")
    if contract["family_policy"]["modern_v6t"]["changed_axis"] != "soft_limit_offset_c":
        raise RefinementError("local refinement must remain one-dimensional")
    if contract["selection_rule"]["zero_cooling_derated_fraction_cap"] is not None:
        raise RefinementError("zero-cooling severity was incorrectly made a pathology guard")
    source = contract["source_evidence"]
    review_path = ROOT / source["review_artifact"]
    if sha256(SOURCE_CAMPAIGN.read_bytes()) != source["campaign_digest"]:
        raise RefinementError("source Databricks campaign changed after review")
    if sha256(review_path.read_bytes()) != source["review_digest"]:
        raise RefinementError("source Databricks review changed after pre-registration")
    return contract, payload


def parameter_sets(contract: dict[str, Any], base_profile: dict[str, Any]) -> list[dict[str, Any]]:
    anchor = contract["anchor_parameters"]
    variants = []
    for value in contract["candidate_values"]:
        profile = copy.deepcopy(base_profile)
        profile["engine_thermal_resolution"].update(anchor)
        profile["engine_thermal_resolution"]["soft_limit_offset_c"] = value
        identifier = "adaptive-038" if value == anchor["soft_limit_offset_c"] else f"soft-limit-{value:+.1f}c"
        variants.append(
            {
                "parameter_set_id": identifier,
                "soft_limit_offset_c": value,
                "distance_from_anchor_c": abs(value - anchor["soft_limit_offset_c"]),
                "profile": profile,
            }
        )
    return variants


def build_plan(contract: dict[str, Any], base_scenario: dict[str, Any], base_profile: dict[str, Any]) -> list[dict[str, Any]]:
    thermal = load_thermal_screen()
    selection = contract["local_selection"]
    reserved = contract["reserved_final_validation"]
    if any(item["id"] == reserved["circuit"]["id"] for item in selection["circuits"]):
        raise RefinementError("reserved circuit leaked into local selection")
    if reserved["seed"] in selection["seeds"]:
        raise RefinementError("reserved seed leaked into local selection")

    plan = []
    for candidate in parameter_sets(contract, base_profile):
        for circuit in selection["circuits"]:
            for seed in selection["seeds"]:
                for cooling_points in selection["cooling_points"]:
                    scenario = thermal.configure_scenario(
                        base_scenario,
                        circuit_id=circuit["id"],
                        vehicle_id=selection["vehicle_id"],
                        era=selection["era"],
                        laps=selection["laps"],
                        cooling_points=cooling_points,
                    )
                    plan.append(
                        {
                            "stage": "modern-v6t-selection",
                            "parameter_set_id": candidate["parameter_set_id"],
                            "soft_limit_offset_c": candidate["soft_limit_offset_c"],
                            "distance_from_anchor_c": candidate["distance_from_anchor_c"],
                            "context_id": f"modern-v6t-{circuit['slug']}",
                            "partition": "reviewed-selection",
                            "workload": "long",
                            "laps": selection["laps"],
                            "cooling_points": cooling_points,
                            "seed": seed,
                            "circuit_id": circuit["id"],
                            "circuit_slug": circuit["slug"],
                            "circuit_archetype": circuit["archetype"],
                            "vehicle_id": selection["vehicle_id"],
                            "era": selection["era"],
                            "scenario": scenario,
                            "profile": candidate["profile"],
                        }
                    )
    return plan


def metrics(point: dict[str, Any]) -> dict[str, float]:
    diagnostics = point["mechanical_diagnostics"]
    elapsed_s = point["total_time_ms"] / 1000.0
    derated_s = diagnostics["engine_derated_time_s"]
    return {
        "total_time_ms": float(point["total_time_ms"]),
        "maximum_engine_temperature_c": float(diagnostics["maximum_engine_temperature_c"]),
        "engine_derated_fraction": float(derated_s / elapsed_s if elapsed_s else 0.0),
        "generated_engine_heat_kj": float(diagnostics["generated_engine_heat_kj"]),
        "removed_engine_heat_kj": float(diagnostics["removed_engine_heat_kj"]),
        "fixed_drag_area_m2": float(diagnostics["fixed_drag_area_m2"]),
    }


def evaluate_candidate(points: list[dict[str, Any]]) -> dict[str, Any]:
    grouped: dict[tuple[str, int], dict[int, dict[str, Any]]] = {}
    for point in points:
        grouped.setdefault((point["circuit_id"], point["seed"]), {})[
            point["cooling_points"]
        ] = point
    evaluations = []
    for (circuit_id, seed), cooling in sorted(grouped.items()):
        if set(cooling) != {0, 10, 20}:
            raise RefinementError("incomplete cooling triplet")
        values = {level: metrics(point) for level, point in cooling.items()}
        finite = all(math.isfinite(value) for item in values.values() for value in item.values())
        temperature_safe = max(item["maximum_engine_temperature_c"] for item in values.values()) <= 180.0
        temperature_monotonic = values[0]["maximum_engine_temperature_c"] >= values[10]["maximum_engine_temperature_c"] >= values[20]["maximum_engine_temperature_c"]
        derating_monotonic = values[0]["engine_derated_fraction"] >= values[10]["engine_derated_fraction"] >= values[20]["engine_derated_fraction"]
        interior_optimum = values[10]["total_time_ms"] < values[0]["total_time_ms"] and values[10]["total_time_ms"] < values[20]["total_time_ms"]
        passed = finite and temperature_safe and temperature_monotonic and derating_monotonic and interior_optimum
        evaluations.append(
            {
                "circuit_id": circuit_id,
                "seed": seed,
                "passed": passed,
                "guards": {
                    "finite": finite,
                    "temperature_safe": temperature_safe,
                    "temperature_monotonic": temperature_monotonic,
                    "derating_monotonic": derating_monotonic,
                    "interior_optimum": interior_optimum,
                },
                "gain_zero_to_ten_ms": values[0]["total_time_ms"] - values[10]["total_time_ms"],
                "penalty_ten_to_twenty_ms": values[20]["total_time_ms"] - values[10]["total_time_ms"],
                "maximum_zero_cooling_derated_fraction": values[0]["engine_derated_fraction"],
            }
        )
    first = points[0]
    return {
        "parameter_set_id": first["parameter_set_id"],
        "soft_limit_offset_c": first["soft_limit_offset_c"],
        "distance_from_anchor_c": first["distance_from_anchor_c"],
        "passed": all(item["passed"] for item in evaluations),
        "triplets": evaluations,
    }


def build_report(runner: pathlib.Path, jobs: int, contract_path: pathlib.Path = CONTRACT) -> dict[str, Any]:
    if not runner.is_file():
        raise RefinementError(f"missing V3 probe runner: {runner}")
    if not 1 <= jobs <= MAX_JOBS:
        raise ValueError(f"jobs must be between 1 and {MAX_JOBS}")
    contract, contract_bytes = load_contract(contract_path)
    scenario_bytes = BASE_SCENARIO.read_bytes()
    profile_bytes = BASE_PROFILE.read_bytes()
    plan = build_plan(contract, json.loads(scenario_bytes), json.loads(profile_bytes))
    thermal = load_thermal_screen()
    with concurrent.futures.ThreadPoolExecutor(max_workers=jobs) as executor:
        points = list(executor.map(lambda point: thermal.execute_point(runner, point), plan))
    points.sort(key=lambda item: (item["parameter_set_id"], item["circuit_id"], item["seed"], item["cooling_points"]))
    candidates = []
    for identifier in sorted({point["parameter_set_id"] for point in points}):
        candidates.append(evaluate_candidate([point for point in points if point["parameter_set_id"] == identifier]))
    passing = sorted((item for item in candidates if item["passed"]), key=lambda item: (item["distance_from_anchor_c"], item["parameter_set_id"]))
    return {
        "schema_version": SCHEMA_VERSION,
        "contract_id": contract["contract_id"],
        "contract_digest": sha256(contract_bytes),
        "model": contract["model"],
        "runner_digest": sha256(runner.read_bytes()),
        "base_scenario_digest": sha256(scenario_bytes),
        "base_profile_digest": sha256(profile_bytes),
        "execution_count": len(points),
        "candidate_count": len(candidates),
        "selection_verdict": "PASS" if passing else "REFINE",
        "selected_parameter_set_id": passing[0]["parameter_set_id"] if passing else None,
        "candidates": candidates,
        "governance": {
            "silverstone_executed_locally": False,
            "automatic_promotion": False,
            "databricks_validation_required": True,
        },
        "points": [{key: value for key, value in point.items() if key not in {"schema_version", "model"}} for point in points],
    }


def write_or_check(report: dict[str, Any], output: pathlib.Path, check: bool) -> None:
    payload = canonical_pretty(report)
    digest = (sha256(payload) + "\n").encode()
    if check:
        if output.read_bytes() != payload or output.with_suffix(".sha256").read_bytes() != digest:
            raise RefinementError("stored refinement artifacts do not match replay")
        return
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_bytes(payload)
    output.with_suffix(".sha256").write_bytes(digest)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--runner", type=pathlib.Path, default=ROOT / "target" / "release" / "examples" / "v3_decision_surface_probe")
    parser.add_argument("--jobs", type=int, default=8)
    parser.add_argument("--contract", type=pathlib.Path, default=CONTRACT)
    parser.add_argument("--output", type=pathlib.Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--check", action="store_true")
    arguments = parser.parse_args()
    try:
        report = build_report(arguments.runner.resolve(), arguments.jobs, arguments.contract.resolve())
        write_or_check(report, arguments.output.resolve(), arguments.check)
    except (OSError, ValueError, RefinementError) as error:
        print(f"error: {error}", file=__import__("sys").stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
