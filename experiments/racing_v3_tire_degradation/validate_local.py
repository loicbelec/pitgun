#!/usr/bin/env python3
"""Explore compound-dependent Model V3 tire degradation without recompilation."""

from __future__ import annotations

import argparse
import concurrent.futures
import copy
import json
import pathlib
import statistics
import subprocess
import tempfile
from typing import Any


ROOT = pathlib.Path(__file__).resolve().parents[2]
import sys

sys.path.insert(0, str(ROOT))

from experiments.racing_v3_validation import validate_local as shared  # noqa: E402


PROFILE = (
    ROOT
    / "experiments"
    / "racing_v3_decision_surface"
    / "profile-v7.compound-degradation.json"
)
OUTPUT = pathlib.Path(__file__).parent / "results" / "local-tire-degradation-v1.json"
SCHEMA_VERSION = "pitgun.racing-v3-tire-degradation-validation/v1"
LAPS_LONGRUN = 24
LAPS_STRATEGY = 30
FUEL_LOADS_KG = (70.0, 100.0)
COMPOUNDS = ("soft", "medium", "hard")
STOP_WINDOWS = (8, 12, 16, 20, 22)
DRIVER_LEVELS = {
    "controlled": {
        "cornering_utilization": 0.94,
        "braking_utilization": 0.93,
        "traction_utilization": 0.94,
        "control_error": 0.02,
    },
    "precise": {
        "cornering_utilization": 0.99,
        "braking_utilization": 0.98,
        "traction_utilization": 0.99,
        "control_error": 0.005,
    },
}
THERMAL_GAINS = (0.0, 0.35, 0.7)
WORKLOAD_ENERGIES_J = (6_000_000_000.0, 8_000_000_000.0, 10_000_000_000.0)


def profile_for(
    base: dict[str, Any],
    *,
    driver_level: str | None = None,
    thermal_gain: float | None = None,
    workload_energy_j: float | None = None,
) -> dict[str, Any]:
    profile = copy.deepcopy(base)
    if driver_level is not None:
        profile["driver_control_override"] = DRIVER_LEVELS[driver_level]
    if thermal_gain is not None:
        profile["tire_degradation"]["thermal_deviation_wear_gain"] = thermal_gain
    if workload_energy_j is not None:
        profile["tire_contact"]["workload_energy_to_full_wear_j"] = workload_energy_j
    return profile


def scenario_for(
    base: dict[str, Any],
    *,
    circuit_id: str,
    vehicle_id: str,
    era: int,
    laps: int,
    fuel_load_kg: float,
    stints: tuple[tuple[str, int], ...],
) -> dict[str, Any]:
    scenario = shared.configure_scenario(
        base,
        circuit_id=circuit_id,
        vehicle_id=vehicle_id,
        era=era,
        laps=laps,
    )
    scenario["request"]["initial_fuel_mass_kg"] = fuel_load_kg
    scenario["request"]["competitors"][0]["stint_strategy"] = (
        shared.explicit_stint_strategy(stints)
    )
    return scenario


def build_plan(
    base_scenario: dict[str, Any], base_profile: dict[str, Any]
) -> list[dict[str, Any]]:
    plan: list[dict[str, Any]] = []
    for circuit_id, circuit_slug, archetype in shared.CIRCUITS:
        for vehicle_id, era in shared.VEHICLES:
            for fuel_load_kg in FUEL_LOADS_KG:
                for compound in COMPOUNDS:
                    plan.append(
                        {
                            "case_id": f"longrun-{compound}-{int(fuel_load_kg)}kg",
                            "family": "compound.longrun",
                            "circuit_id": circuit_id,
                            "circuit_slug": circuit_slug,
                            "circuit_archetype": archetype,
                            "vehicle_id": vehicle_id,
                            "era": era,
                            "compound": compound,
                            "fuel_load_kg": fuel_load_kg,
                            "seed": 42,
                            "scenario": scenario_for(
                                base_scenario,
                                circuit_id=circuit_id,
                                vehicle_id=vehicle_id,
                                era=era,
                                laps=LAPS_LONGRUN,
                                fuel_load_kg=fuel_load_kg,
                                stints=((compound, LAPS_LONGRUN),),
                            ),
                            "profile": profile_for(base_profile),
                        }
                    )

            for stop_lap in STOP_WINDOWS:
                plan.append(
                    {
                        "case_id": f"strategy-medium-soft-l{stop_lap}",
                        "family": "strategy.window",
                        "circuit_id": circuit_id,
                        "circuit_slug": circuit_slug,
                        "circuit_archetype": archetype,
                        "vehicle_id": vehicle_id,
                        "era": era,
                        "stop_lap": stop_lap,
                        "fuel_load_kg": 100.0,
                        "seed": 42,
                        "scenario": scenario_for(
                            base_scenario,
                            circuit_id=circuit_id,
                            vehicle_id=vehicle_id,
                            era=era,
                            laps=LAPS_STRATEGY,
                            fuel_load_kg=100.0,
                            stints=(("medium", stop_lap), ("soft", LAPS_STRATEGY - stop_lap)),
                        ),
                        "profile": profile_for(base_profile),
                    }
                )

        for compound in COMPOUNDS:
            for driver_level in DRIVER_LEVELS:
                plan.append(
                    {
                        "case_id": f"driver-{driver_level}-{compound}",
                        "family": "driver.control",
                        "circuit_id": circuit_id,
                        "circuit_slug": circuit_slug,
                        "circuit_archetype": archetype,
                        "vehicle_id": "f1_2026",
                        "era": 5,
                        "compound": compound,
                        "driver_level": driver_level,
                        "fuel_load_kg": 100.0,
                        "seed": 42,
                        "scenario": scenario_for(
                            base_scenario,
                            circuit_id=circuit_id,
                            vehicle_id="f1_2026",
                            era=5,
                            laps=LAPS_LONGRUN,
                            fuel_load_kg=100.0,
                            stints=((compound, LAPS_LONGRUN),),
                        ),
                        "profile": profile_for(base_profile, driver_level=driver_level),
                    }
                )

        for thermal_gain in THERMAL_GAINS:
            for workload_energy_j in WORKLOAD_ENERGIES_J:
                plan.append(
                    {
                        "case_id": (
                            f"screen-thermal-{thermal_gain:g}-workload-"
                            f"{int(workload_energy_j / 1_000_000_000)}gj"
                        ),
                        "family": "parameter.screen",
                        "circuit_id": circuit_id,
                        "circuit_slug": circuit_slug,
                        "circuit_archetype": archetype,
                        "vehicle_id": "f1_2026",
                        "era": 5,
                        "compound": "medium",
                        "thermal_gain": thermal_gain,
                        "workload_energy_to_full_wear_j": workload_energy_j,
                        "fuel_load_kg": 100.0,
                        "seed": 42,
                        "scenario": scenario_for(
                            base_scenario,
                            circuit_id=circuit_id,
                            vehicle_id="f1_2026",
                            era=5,
                            laps=LAPS_LONGRUN,
                            fuel_load_kg=100.0,
                            stints=(("medium", LAPS_LONGRUN),),
                        ),
                        "profile": profile_for(
                            base_profile,
                            thermal_gain=thermal_gain,
                            workload_energy_j=workload_energy_j,
                        ),
                    }
                )

    assert len(plan) == 236
    return plan


def execute_point(runner: pathlib.Path, point: dict[str, Any]) -> dict[str, Any]:
    with tempfile.TemporaryDirectory(prefix="pitgun-v3-tire-") as directory:
        root = pathlib.Path(directory)
        scenario_path = root / "scenario.json"
        profile_path = root / "profile.json"
        scenario_path.write_bytes(shared.canonical_pretty(point["scenario"]))
        profile_path.write_bytes(shared.canonical_pretty(point["profile"]))
        completed = subprocess.run(
            [str(runner), str(scenario_path), str(profile_path), str(point["seed"])],
            cwd=ROOT,
            check=False,
            capture_output=True,
            timeout=180,
        )
    if completed.returncode != 0:
        raise shared.ValidationError(
            f"probe failed for {point['case_id']} {point['circuit_slug']} "
            f"{point['vehicle_id']}: "
            + completed.stderr.decode(errors="replace").strip()
        )
    if completed.stderr:
        raise shared.ValidationError(
            f"successful probe wrote to stderr for {point['case_id']}"
        )
    try:
        result = json.loads(completed.stdout)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise shared.ValidationError("probe returned invalid JSON") from error
    if result.get("schema_version") != shared.PROBE_SCHEMA_VERSION:
        raise shared.ValidationError("probe returned an unsupported result")
    diagnostics = result.get("tire_degradation_diagnostics")
    if not isinstance(diagnostics, dict):
        raise shared.ValidationError("V7 probe returned no tire-degradation lineage")
    metadata = {
        key: value
        for key, value in point.items()
        if key not in {"scenario", "profile"}
    }
    metadata["stint_pace"] = shared.stint_pace_summary(
        result["player_lap_times_ms"], result["player_pit_laps"]
    )
    return metadata | {"execution_status": "succeeded"} | result


def first_crossover_lap(softer: list[int], harder: list[int]) -> int | None:
    for index, (soft_time, hard_time) in enumerate(zip(softer, harder), start=1):
        if index == 1:
            continue
        if soft_time >= hard_time:
            return index
    return None


def summarize_compounds(points: list[dict[str, Any]]) -> dict[str, Any]:
    groups = []
    for circuit_id, circuit_slug, _ in shared.CIRCUITS:
        for vehicle_id, _ in shared.VEHICLES:
            for fuel_load_kg in FUEL_LOADS_KG:
                rows = {
                    point["compound"]: point
                    for point in points
                    if point["family"] == "compound.longrun"
                    and point["circuit_id"] == circuit_id
                    and point["vehicle_id"] == vehicle_id
                    and point["fuel_load_kg"] == fuel_load_kg
                }
                compounds = {}
                for compound in COMPOUNDS:
                    row = rows[compound]
                    diagnostics = row["tire_degradation_diagnostics"]
                    compounds[compound] = {
                        "total_time_ms": row["total_time_ms"],
                        "final_wear_fraction": diagnostics[
                            "wear_before_service_after_lap"
                        ][-1],
                        "requested_baseline_wear_fraction": diagnostics[
                            "requested_baseline_wear_fraction"
                        ],
                        "requested_workload_wear_fraction": diagnostics[
                            "requested_workload_wear_fraction"
                        ],
                        "minimum_thermal_wear_multiplier": diagnostics[
                            "minimum_thermal_wear_multiplier"
                        ],
                        "maximum_thermal_wear_multiplier": diagnostics[
                            "maximum_thermal_wear_multiplier"
                        ],
                        "pace_drift_ms": row["stint_pace"][0]["pace_drift_ms"],
                    }
                groups.append(
                    {
                        "circuit_id": circuit_id,
                        "circuit_slug": circuit_slug,
                        "vehicle_id": vehicle_id,
                        "fuel_load_kg": fuel_load_kg,
                        "soft_to_medium_crossover_lap": first_crossover_lap(
                            rows["soft"]["player_lap_times_ms"],
                            rows["medium"]["player_lap_times_ms"],
                        ),
                        "medium_to_hard_crossover_lap": first_crossover_lap(
                            rows["medium"]["player_lap_times_ms"],
                            rows["hard"]["player_lap_times_ms"],
                        ),
                        "wear_order_is_soft_medium_hard": (
                            compounds["soft"]["final_wear_fraction"]
                            > compounds["medium"]["final_wear_fraction"]
                            > compounds["hard"]["final_wear_fraction"]
                        ),
                        "compounds": compounds,
                    }
                )
    return {
        "group_count": len(groups),
        "ordered_wear_group_count": sum(
            group["wear_order_is_soft_medium_hard"] for group in groups
        ),
        "soft_medium_crossover_group_count": sum(
            group["soft_to_medium_crossover_lap"] is not None for group in groups
        ),
        "medium_hard_crossover_group_count": sum(
            group["medium_to_hard_crossover_lap"] is not None for group in groups
        ),
        "groups": groups,
    }


def summarize_strategies(points: list[dict[str, Any]]) -> dict[str, Any]:
    groups = []
    for circuit_id, circuit_slug, _ in shared.CIRCUITS:
        for vehicle_id, _ in shared.VEHICLES:
            rows = sorted(
                (
                    point
                    for point in points
                    if point["family"] == "strategy.window"
                    and point["circuit_id"] == circuit_id
                    and point["vehicle_id"] == vehicle_id
                ),
                key=lambda point: point["stop_lap"],
            )
            fastest = min(rows, key=lambda point: point["total_time_ms"])
            groups.append(
                {
                    "circuit_id": circuit_id,
                    "circuit_slug": circuit_slug,
                    "vehicle_id": vehicle_id,
                    "fastest_stop_lap": fastest["stop_lap"],
                    "strategy_time_range_ms": max(row["total_time_ms"] for row in rows)
                    - min(row["total_time_ms"] for row in rows),
                    "windows": [
                        {
                            "stop_lap": row["stop_lap"],
                            "total_time_ms": row["total_time_ms"],
                            "first_stint_pace_drift_ms": row["stint_pace"][0][
                                "pace_drift_ms"
                            ],
                        }
                        for row in rows
                    ],
                }
            )
    fastest_windows = sorted({group["fastest_stop_lap"] for group in groups})
    return {
        "group_count": len(groups),
        "distinct_fastest_stop_count": len(fastest_windows),
        "fastest_stop_laps": fastest_windows,
        "minimum_strategy_time_range_ms": min(
            group["strategy_time_range_ms"] for group in groups
        ),
        "groups": groups,
    }


def summarize_driver(points: list[dict[str, Any]]) -> dict[str, Any]:
    groups = []
    for circuit_id, circuit_slug, _ in shared.CIRCUITS:
        for compound in COMPOUNDS:
            rows = {
                point["driver_level"]: point
                for point in points
                if point["family"] == "driver.control"
                and point["circuit_id"] == circuit_id
                and point["compound"] == compound
            }
            groups.append(
                {
                    "circuit_id": circuit_id,
                    "circuit_slug": circuit_slug,
                    "compound": compound,
                    "controlled_total_time_ms": rows["controlled"]["total_time_ms"],
                    "precise_total_time_ms": rows["precise"]["total_time_ms"],
                    "controlled_minus_precise_ms": rows["controlled"]["total_time_ms"]
                    - rows["precise"]["total_time_ms"],
                    "controlled_final_wear_fraction": rows["controlled"][
                        "tire_degradation_diagnostics"
                    ]["wear_before_service_after_lap"][-1],
                    "precise_final_wear_fraction": rows["precise"][
                        "tire_degradation_diagnostics"
                    ]["wear_before_service_after_lap"][-1],
                }
            )
    return {
        "group_count": len(groups),
        "observable_pace_group_count": sum(
            group["controlled_minus_precise_ms"] != 0 for group in groups
        ),
        "groups": groups,
    }


def summarize_screen(points: list[dict[str, Any]]) -> dict[str, Any]:
    groups = []
    for circuit_id, circuit_slug, _ in shared.CIRCUITS:
        rows = [
            point
            for point in points
            if point["family"] == "parameter.screen"
            and point["circuit_id"] == circuit_id
        ]
        final_wear = [
            row["tire_degradation_diagnostics"]["wear_before_service_after_lap"][-1]
            for row in rows
        ]
        groups.append(
            {
                "circuit_id": circuit_id,
                "circuit_slug": circuit_slug,
                "total_time_range_ms": max(row["total_time_ms"] for row in rows)
                - min(row["total_time_ms"] for row in rows),
                "final_wear_range_fraction": max(final_wear) - min(final_wear),
                "points": [
                    {
                        "thermal_gain": row["thermal_gain"],
                        "workload_energy_to_full_wear_j": row[
                            "workload_energy_to_full_wear_j"
                        ],
                        "total_time_ms": row["total_time_ms"],
                        "final_wear_fraction": row["tire_degradation_diagnostics"][
                            "wear_before_service_after_lap"
                        ][-1],
                    }
                    for row in sorted(
                        rows,
                        key=lambda point: (
                            point["thermal_gain"],
                            point["workload_energy_to_full_wear_j"],
                        ),
                    )
                ],
            }
        )
    return {
        "group_count": len(groups),
        "all_groups_observable": all(
            group["total_time_range_ms"] > 0
            and group["final_wear_range_fraction"] > 0.0
            for group in groups
        ),
        "groups": groups,
    }


def verdicts(
    compounds: dict[str, Any],
    strategies: dict[str, Any],
    driver: dict[str, Any],
    screen: dict[str, Any],
) -> list[dict[str, str]]:
    all_wear_ordered = (
        compounds["ordered_wear_group_count"] == compounds["group_count"]
    )
    crossover_count = (
        compounds["soft_medium_crossover_group_count"]
        + compounds["medium_hard_crossover_group_count"]
    )
    return [
        {
            "capability": "compound_degradation_lineage",
            "verdict": "PASS" if all_wear_ordered else "REFINE",
            "reason": (
                "Every reviewed group orders accumulated wear as soft > medium > hard."
                if all_wear_ordered
                else "At least one reviewed group does not preserve the expected compound wear ordering."
            ),
        },
        {
            "capability": "compound_crossover",
            "verdict": "PASS" if crossover_count else "REFINE",
            "reason": (
                f"{crossover_count} reviewed softer-versus-harder pairs cross within 24 laps."
                if crossover_count
                else "No reviewed compound pair crosses within the bounded 24-lap horizon."
            ),
        },
        {
            "capability": "one_stop_window_diversity",
            "verdict": (
                "PASS"
                if strategies["distinct_fastest_stop_count"] > 1
                else "REFINE"
            ),
            "reason": (
                "The fastest reviewed stop lap varies across circuit and vehicle groups."
                if strategies["distinct_fastest_stop_count"] > 1
                else "One reviewed stop lap remains universally fastest."
            ),
        },
        {
            "capability": "driver_contact_coupling",
            "verdict": (
                "PASS"
                if driver["observable_pace_group_count"] == driver["group_count"]
                else "REFINE"
            ),
            "reason": "Named driver-control limits remain observable through physical force usage.",
        },
        {
            "capability": "runtime_parameter_exploration",
            "verdict": "PASS" if screen["all_groups_observable"] else "REFINE",
            "reason": "Thermal and workload coefficients are varied through JSON profiles without recompiling Rust.",
        },
        {
            "capability": "production_change",
            "verdict": "REFINE",
            "reason": "This offline candidate does not change the game, catalog selection, Authority or Verifier.",
        },
    ]


def build_report(runner: pathlib.Path, jobs: int) -> dict[str, Any]:
    if not runner.is_file():
        raise shared.ValidationError(f"missing V3 validation probe: {runner}")
    if jobs < 1 or jobs > shared.MAX_JOBS:
        raise ValueError(f"jobs must be between 1 and {shared.MAX_JOBS}")
    scenario_bytes = shared.BASE_SCENARIO.read_bytes()
    profile_bytes = PROFILE.read_bytes()
    plan = build_plan(json.loads(scenario_bytes), json.loads(profile_bytes))
    with concurrent.futures.ThreadPoolExecutor(max_workers=jobs) as executor:
        points = list(executor.map(lambda point: execute_point(runner, point), plan))
    points.sort(
        key=lambda point: (
            point["family"],
            point["circuit_id"],
            point["vehicle_id"],
            point["case_id"],
        )
    )
    identities = {json.dumps(point["model"], sort_keys=True) for point in points}
    if len(identities) != 1:
        raise shared.ValidationError("tire campaign mixed model identities")
    compounds = summarize_compounds(points)
    strategies = summarize_strategies(points)
    driver = summarize_driver(points)
    screen = summarize_screen(points)
    return {
        "schema_version": SCHEMA_VERSION,
        "campaign": {
            "purpose": "bounded compound-dependent tire degradation and parameter exploration",
            "model": json.loads(next(iter(identities))),
            "runner": {
                "path": str(runner.relative_to(ROOT)),
                "digest": shared.sha256(runner.read_bytes()),
            },
            "base_scenario_path": str(shared.BASE_SCENARIO.relative_to(ROOT)),
            "base_scenario_digest": shared.sha256(scenario_bytes),
            "base_profile_path": str(PROFILE.relative_to(ROOT)),
            "base_profile_digest": shared.sha256(profile_bytes),
            "execution_count": len(points),
            "simulated_lap_count": sum(
                len(point["player_lap_times_ms"]) for point in points
            ),
            "circuits": [identifier for identifier, _, _ in shared.CIRCUITS],
            "vehicles": [identifier for identifier, _ in shared.VEHICLES],
            "fuel_loads_kg": list(FUEL_LOADS_KG),
            "compounds": list(COMPOUNDS),
            "stop_windows": list(STOP_WINDOWS),
            "driver_levels": sorted(DRIVER_LEVELS),
            "thermal_gains": list(THERMAL_GAINS),
            "workload_energies_j": list(WORKLOAD_ENERGIES_J),
        },
        "verdicts": verdicts(compounds, strategies, driver, screen),
        "compound_summary": compounds,
        "strategy_summary": strategies,
        "driver_summary": driver,
        "parameter_screen_summary": screen,
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
