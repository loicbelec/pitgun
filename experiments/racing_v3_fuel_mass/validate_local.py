#!/usr/bin/env python3
"""Validate Model V3 power-based fuel and lap-level mass evolution."""

from __future__ import annotations

import argparse
import concurrent.futures
import json
import pathlib
import sys
from typing import Any


ROOT = pathlib.Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))

from experiments.racing_v3_validation import validate_local as shared  # noqa: E402


PROFILE = (
    ROOT
    / "experiments"
    / "racing_v3_decision_surface"
    / "profile-v6.power-based-fuel-mass.json"
)
OUTPUT = pathlib.Path(__file__).parent / "results" / "local-fuel-mass-v1.json"
SCHEMA_VERSION = "pitgun.racing-v3-fuel-mass-validation/v1"
FUEL_LOADS_KG = (40.0, 70.0, 100.0)
LAPS = 12


def build_plan(base: dict[str, Any]) -> list[dict[str, Any]]:
    plan = []
    for circuit_id, circuit_slug, archetype in shared.CIRCUITS:
        for vehicle_id, era in shared.VEHICLES:
            for fuel_load_kg in FUEL_LOADS_KG:
                scenario = shared.configure_scenario(
                    base,
                    circuit_id=circuit_id,
                    vehicle_id=vehicle_id,
                    era=era,
                    laps=LAPS,
                )
                scenario["request"]["initial_fuel_mass_kg"] = fuel_load_kg
                plan.append(
                    {
                        "case_id": f"fuel-{int(fuel_load_kg)}kg",
                        "family": "fuel_mass.load",
                        "level": fuel_load_kg,
                        "circuit_id": circuit_id,
                        "circuit_slug": circuit_slug,
                        "circuit_archetype": archetype,
                        "vehicle_id": vehicle_id,
                        "era": era,
                        "seed": 42,
                        "initial_fuel_mass_kg": fuel_load_kg,
                        "scenario": scenario,
                    }
                )
    assert len(plan) == 48
    return plan


def summarize(points: list[dict[str, Any]], profile: dict[str, Any]) -> dict[str, Any]:
    groups = []
    maximum_accounting_error_kg = 0.0
    for circuit_id, circuit_slug, _ in shared.CIRCUITS:
        for vehicle_id, _ in shared.VEHICLES:
            rows = sorted(
                (
                    point
                    for point in points
                    if point["circuit_id"] == circuit_id
                    and point["vehicle_id"] == vehicle_id
                ),
                key=lambda point: point["initial_fuel_mass_kg"],
            )
            loads = []
            for row in rows:
                diagnostics = row["fuel_mass_diagnostics"]
                trajectory = diagnostics["fuel_mass_after_lap_kg"]
                expected_consumption = (
                    diagnostics["engine_output_work_kj"]
                    / 3_600.0
                    * profile["fuel_mass"][
                        "brake_specific_fuel_consumption_kg_per_kwh"
                    ]
                    + row["total_time_ms"]
                    / 1_000.0
                    * profile["fuel_mass"]["idle_fuel_flow_kg_per_s"]
                )
                accounting_error_kg = abs(
                    diagnostics["fuel_consumed_kg"] - expected_consumption
                )
                maximum_accounting_error_kg = max(
                    maximum_accounting_error_kg, accounting_error_kg
                )
                loads.append(
                    {
                        "initial_fuel_mass_kg": row["initial_fuel_mass_kg"],
                        "total_time_ms": row["total_time_ms"],
                        "fuel_consumed_kg": diagnostics["fuel_consumed_kg"],
                        "engine_output_work_kj": diagnostics["engine_output_work_kj"],
                        "final_fuel_mass_kg": diagnostics["final_fuel_mass_kg"],
                        "minimum_total_vehicle_mass_kg": diagnostics[
                            "minimum_total_vehicle_mass_kg"
                        ],
                        "maximum_total_vehicle_mass_kg": diagnostics[
                            "maximum_total_vehicle_mass_kg"
                        ],
                        "lap_count": len(trajectory),
                        "trajectory_is_monotonic": all(
                            right <= left for left, right in zip(trajectory, trajectory[1:])
                        ),
                        "accounting_error_kg": accounting_error_kg,
                    }
                )
            groups.append(
                {
                    "circuit_id": circuit_id,
                    "circuit_slug": circuit_slug,
                    "vehicle_id": vehicle_id,
                    "light_to_heavy_time_delta_ms": loads[-1]["total_time_ms"]
                    - loads[0]["total_time_ms"],
                    "loads": loads,
                }
            )
    return {
        "group_count": len(groups),
        "minimum_light_to_heavy_time_delta_ms": min(
            group["light_to_heavy_time_delta_ms"] for group in groups
        ),
        "maximum_accounting_error_kg": maximum_accounting_error_kg,
        "all_trajectories_monotonic": all(
            load["trajectory_is_monotonic"]
            for group in groups
            for load in group["loads"]
        ),
        "groups": groups,
    }


def build_report(runner: pathlib.Path, jobs: int) -> dict[str, Any]:
    if not runner.is_file():
        raise shared.ValidationError(f"missing V3 validation probe: {runner}")
    if jobs < 1 or jobs > shared.MAX_JOBS:
        raise ValueError(f"jobs must be between 1 and {shared.MAX_JOBS}")
    scenario_bytes = shared.BASE_SCENARIO.read_bytes()
    profile_bytes = PROFILE.read_bytes()
    profile = json.loads(profile_bytes)
    plan = build_plan(json.loads(scenario_bytes))
    with concurrent.futures.ThreadPoolExecutor(max_workers=jobs) as executor:
        points = list(
            executor.map(
                lambda point: shared.execute_point(runner, PROFILE, point), plan
            )
        )
    points.sort(
        key=lambda point: (
            point["circuit_id"],
            point["vehicle_id"],
            point["initial_fuel_mass_kg"],
        )
    )
    if any(point["execution_status"] != "succeeded" for point in points):
        raise shared.ValidationError("fuel/mass campaign contains unsupported executions")
    identities = {json.dumps(point["model"], sort_keys=True) for point in points}
    if len(identities) != 1:
        raise shared.ValidationError("fuel/mass campaign mixed model identities")
    summary = summarize(points, profile)
    return {
        "schema_version": SCHEMA_VERSION,
        "campaign": {
            "purpose": "bounded power-based fuel and mass validation",
            "model": json.loads(next(iter(identities))),
            "runner": {
                "path": str(runner.relative_to(ROOT)),
                "digest": shared.sha256(runner.read_bytes()),
            },
            "base_scenario_path": str(shared.BASE_SCENARIO.relative_to(ROOT)),
            "base_scenario_digest": shared.sha256(scenario_bytes),
            "profile_path": str(PROFILE.relative_to(ROOT)),
            "profile_digest": shared.sha256(profile_bytes),
            "execution_count": len(points),
            "simulated_lap_count": sum(len(point["player_lap_times_ms"]) for point in points),
            "fuel_loads_kg": list(FUEL_LOADS_KG),
            "laps_per_execution": LAPS,
            "circuits": [identifier for identifier, _, _ in shared.CIRCUITS],
            "vehicles": [identifier for identifier, _ in shared.VEHICLES],
        },
        "verdicts": [
            {
                "capability": "power_based_fuel_accounting",
                "verdict": "PASS"
                if summary["maximum_accounting_error_kg"] <= 0.001
                else "REFINE",
                "reason": "Consumed fuel reconciles with integrated engine work and idle flow.",
            },
            {
                "capability": "lap_level_mass_lineage",
                "verdict": "PASS"
                if summary["all_trajectories_monotonic"]
                else "STRUCTURAL_CHANGE_REQUIRED",
                "reason": "Fuel mass is bounded and observable after every simulated lap.",
            },
            {
                "capability": "fuel_load_pace_response",
                "verdict": "PASS"
                if summary["minimum_light_to_heavy_time_delta_ms"] > 0
                else "REFINE",
                "reason": "Heavier reviewed initial loads are slower in every circuit/vehicle group.",
            },
            {
                "capability": "cross_era_specific_consumption",
                "verdict": "REFINE",
                "reason": "The first candidate deliberately uses one reviewed specific-consumption law across eras; engine-specific calibration is not yet claimed.",
            },
            {
                "capability": "hybrid_energy_accounting",
                "verdict": "STRUCTURAL_CHANGE_REQUIRED",
                "reason": "Battery storage, recovery and deployment remain explicitly deferred to #246.",
            },
            {
                "capability": "production_change",
                "verdict": "REFINE",
                "reason": "This offline campaign does not authorize catalog, game or hosted-verification changes.",
            },
        ],
        "fuel_mass_summary": summary,
        "points": points,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--runner",
        type=pathlib.Path,
        default=ROOT / "target" / "release" / "examples" / "v3_validation_probe",
    )
    parser.add_argument("--jobs", type=int, default=4)
    parser.add_argument("--output", type=pathlib.Path, default=OUTPUT)
    parser.add_argument("--check", action="store_true")
    arguments = parser.parse_args()
    try:
        report = build_report(arguments.runner.resolve(), arguments.jobs)
        shared.write_or_check(report, arguments.output.resolve(), arguments.check)
    except (OSError, ValueError, shared.ValidationError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
