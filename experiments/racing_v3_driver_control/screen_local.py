#!/usr/bin/env python3
"""Screen deterministic Racing Model V3 driver controls locally."""

from __future__ import annotations

import argparse
import concurrent.futures
import copy
import hashlib
import json
import pathlib
import statistics
import subprocess
import tempfile
from collections import Counter, defaultdict
from typing import Any


ROOT = pathlib.Path(__file__).resolve().parents[2]
BASE_SCENARIO = (
    ROOT / "apps" / "pitgun-cli" / "scenarios" / "racing-batch-v1" / "balanced.json"
)
PROFILE = pathlib.Path(__file__).parent / "profile-v10.driver-control.json"
DRIVERS = pathlib.Path(__file__).parent / "driver-archetypes-v1.json"
DEFAULT_OUTPUT = pathlib.Path(__file__).parent / "results" / "local-driver-control-screen-v1.json"
SCHEMA_VERSION = "pitgun.racing-v3-driver-control-local-screen/v1"
PROBE_SCHEMA_VERSION = "pitgun.racing-v3-driver-control-probe/v1"
MAX_JOBS = 16

CIRCUITS = (
    ("mc-1929", "monaco", "low-speed-high-downforce", 78),
    ("jp-1962", "suzuka", "mixed", 53),
    ("it-1922", "monza", "high-speed-low-downforce", 53),
)
HORIZONS = (("short", 5), ("race-length", None))
COMPOUNDS = ("soft", "medium", "hard")
MODES = ("manage", "balanced", "attack")
SEEDS = (7, 42, 99)
# Screening reserve, not a proposed game or catalog fuel target. Keeping a
# large common reserve prevents fuel depletion from censoring the longest
# driver/tire comparisons while preserving identical mass within every pair.
INITIAL_FUEL_MASS_KG = 150.0

CONSISTENCY_CONTROLS = (
    ("consistency_low", 0.62),
    ("consistency_high", 0.96),
)
TIRE_MANAGEMENT_CONTROLS = (
    ("tire_management_low", 0.58),
    ("tire_management_high", 0.98),
)


class ScreenError(RuntimeError):
    """Raised when governed local evidence cannot be produced."""


def canonical_pretty(value: object) -> bytes:
    return (json.dumps(value, indent=2, ensure_ascii=False) + "\n").encode()


def sha256(value: object) -> str:
    return "sha256:" + hashlib.sha256(canonical_pretty(value)).hexdigest()


def explicit_stint_strategy(tire_id: str, laps: int) -> dict[str, Any]:
    return {"stints": [{"tire_id": tire_id, "laps": laps}], "pit_laps": []}


def scenario_for(
    base: dict[str, Any],
    *,
    circuit_id: str,
    laps: int,
    tire_id: str,
    driver_id: str,
) -> dict[str, Any]:
    scenario = copy.deepcopy(base)
    request = scenario["request"]
    request.update(
        {
            "track_id": circuit_id,
            "vehicle_id": "f1_2026",
            "era": 5,
            "laps": laps,
            "hz": 5.0,
            "initial_fuel_mass_kg": INITIAL_FUEL_MASS_KG,
        }
    )
    competitor = request["competitors"][0]
    competitor.update(
        {
            "driver_id": driver_id,
            "budget_cap": 100.0,
            "tuning": {
                "engine_points": 25.0,
                "cooling_points": 25.0,
                "aero_points": 25.0,
                "chassis_points": 25.0,
                "downforce_slider": 0.5,
                "gear_ratio_slider": 0.5,
            },
            "stint_strategy": explicit_stint_strategy(tire_id, laps),
        }
    )
    request.pop("pit_strategy", None)
    request.pop("competitor_vehicle_components", None)
    return scenario


def experiment_for(driver: dict[str, Any], mode: str) -> dict[str, Any]:
    return {
        "drivers": {driver["id"]: driver},
        "competitor_modes": {"player": mode},
    }


def point(
    *,
    family: str,
    circuit_id: str,
    circuit_slug: str,
    circuit_archetype: str,
    laps: int,
    horizon: str,
    tire_id: str,
    driver: dict[str, Any],
    mode: str,
    seed: int,
    base_scenario: dict[str, Any],
) -> dict[str, Any]:
    metadata = {
        "family": family,
        "circuit_id": circuit_id,
        "circuit_slug": circuit_slug,
        "circuit_archetype": circuit_archetype,
        "laps": laps,
        "horizon": horizon,
        "tire_id": tire_id,
        "driver_id": driver["id"],
        "driver_traits": driver["traits"],
        "mode": mode,
        "seed": seed,
        "initial_fuel_mass_kg": INITIAL_FUEL_MASS_KG,
    }
    metadata["configuration_id"] = sha256(metadata)
    return metadata | {
        "scenario": scenario_for(
            base_scenario,
            circuit_id=circuit_id,
            laps=laps,
            tire_id=tire_id,
            driver_id=driver["id"],
        ),
        "driver_experiment": experiment_for(driver, mode),
    }


def build_plan(
    base_scenario: dict[str, Any], archetypes: list[dict[str, Any]]
) -> list[dict[str, Any]]:
    plan: list[dict[str, Any]] = []
    for circuit_id, circuit_slug, circuit_archetype, race_laps in CIRCUITS:
        for horizon, configured_laps in HORIZONS:
            laps = race_laps if configured_laps is None else configured_laps
            for tire_id in COMPOUNDS:
                for driver in archetypes:
                    for mode in MODES:
                        for seed in SEEDS:
                            plan.append(
                                point(
                                    family="archetype.full-factorial",
                                    circuit_id=circuit_id,
                                    circuit_slug=circuit_slug,
                                    circuit_archetype=circuit_archetype,
                                    laps=laps,
                                    horizon=horizon,
                                    tire_id=tire_id,
                                    driver=driver,
                                    mode=mode,
                                    seed=seed,
                                    base_scenario=base_scenario,
                                )
                            )

        for driver_id, consistency in CONSISTENCY_CONTROLS:
            driver = {
                "schema_version": "pitgun.racing-driver/v2",
                "id": driver_id,
                "traits": {
                    "limit_exploitation": 0.85,
                    "consistency": consistency,
                    "tire_management": 0.80,
                },
            }
            for horizon, configured_laps in HORIZONS:
                laps = race_laps if configured_laps is None else configured_laps
                for seed in SEEDS:
                    plan.append(
                        point(
                            family="trait-isolation.consistency",
                            circuit_id=circuit_id,
                            circuit_slug=circuit_slug,
                            circuit_archetype=circuit_archetype,
                            laps=laps,
                            horizon=horizon,
                            tire_id="medium",
                            driver=driver,
                            mode="balanced",
                            seed=seed,
                            base_scenario=base_scenario,
                        )
                    )

        for driver_id, tire_management in TIRE_MANAGEMENT_CONTROLS:
            driver = {
                "schema_version": "pitgun.racing-driver/v2",
                "id": driver_id,
                "traits": {
                    "limit_exploitation": 0.85,
                    "consistency": 0.82,
                    "tire_management": tire_management,
                },
            }
            for seed in SEEDS:
                plan.append(
                    point(
                        family="trait-isolation.tire-management",
                        circuit_id=circuit_id,
                        circuit_slug=circuit_slug,
                        circuit_archetype=circuit_archetype,
                        laps=race_laps,
                        horizon="race-length",
                        tire_id="medium",
                        driver=driver,
                        mode="attack",
                        seed=seed,
                        base_scenario=base_scenario,
                    )
                )

    if len(plan) != 702:
        raise ScreenError(f"unexpected campaign size: {len(plan)}")
    return plan


def execute_point(
    runner: pathlib.Path, profile_path: pathlib.Path, item: dict[str, Any]
) -> dict[str, Any]:
    with tempfile.TemporaryDirectory(prefix="pitgun-v3-driver-") as directory:
        temporary = pathlib.Path(directory)
        scenario_path = temporary / "scenario.json"
        experiment_path = temporary / "driver-experiment.json"
        scenario_path.write_bytes(canonical_pretty(item["scenario"]))
        experiment_path.write_bytes(canonical_pretty(item["driver_experiment"]))
        completed = subprocess.run(
            [
                str(runner),
                str(scenario_path),
                str(profile_path),
                str(experiment_path),
                str(item["seed"]),
            ],
            cwd=ROOT,
            check=False,
            capture_output=True,
            timeout=240,
        )
    if completed.returncode != 0:
        raise ScreenError(
            f"probe failed for {item['configuration_id']}: "
            + completed.stderr.decode(errors="replace").strip()
        )
    if completed.stderr:
        raise ScreenError(
            f"successful probe wrote to stderr for {item['configuration_id']}"
        )
    try:
        probe = json.loads(completed.stdout)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ScreenError("probe returned invalid JSON") from error
    if probe.get("schema_version") != PROBE_SCHEMA_VERSION:
        raise ScreenError("probe returned an unsupported result")

    lap_times = probe.pop("player_lap_times_ms")
    resolution = probe["driver_control_resolutions"]["player"]
    diagnostics = probe["driver_control_diagnostics"]["player"]
    tire = probe["tire_diagnostics"]
    wear = probe["tire_degradation_diagnostics"]
    metadata = {
        key: value
        for key, value in item.items()
        if key not in {"scenario", "driver_experiment"}
    }
    return metadata | {
        "experimental_execution_id": probe["experimental_execution_id"],
        "model": probe["model"],
        "scenario_digest": probe["scenario_digest"],
        "profile_digest": probe["profile_digest"],
        "driver_experiment_digest": probe["driver_experiment_digest"],
        "total_time_ms": probe["total_time_ms"],
        "best_lap_ms": min(lap_times),
        "mean_lap_ms": statistics.fmean(lap_times),
        "lap_time_stddev_ms": statistics.pstdev(lap_times),
        "final_tire_temperature_c": probe["final_tire_temperature_c"],
        "final_tire_wear_pct": probe["final_tire_wear_pct"],
        "requested_commitment": resolution["requested_commitment"],
        "control_error_amplitude": resolution["control_error_amplitude"],
        "correction_workload_multiplier": resolution[
            "correction_workload_multiplier"
        ],
        "mean_cornering_utilization": diagnostics["cornering"]["mean_realized"],
        "mean_braking_utilization": diagnostics["braking"]["mean_realized"],
        "mean_traction_utilization": diagnostics["traction"]["mean_realized"],
        "base_contact_workload_mj": tire["base_contact_workload_mj"],
        "correction_contact_workload_mj": tire[
            "correction_contact_workload_mj"
        ],
        "correction_generated_heat_kj": tire["correction_generated_heat_kj"],
        "requested_correction_wear_fraction": wear[
            "requested_correction_wear_fraction"
        ],
    }


def paired(rows: list[dict[str, Any]], family: str) -> dict[tuple[Any, ...], dict[str, Any]]:
    result = {}
    for row in rows:
        if row["family"] != family:
            continue
        key = (
            row["circuit_id"],
            row["horizon"],
            row["tire_id"],
            row["driver_id"],
            row["seed"],
        )
        result[key + (row["mode"],)] = row
    return result


def ratio(numerator: int, denominator: int) -> float:
    return numerator / denominator if denominator else 0.0


def range_summary(values: list[float]) -> dict[str, float]:
    return {
        "minimum": min(values),
        "median": statistics.median(values),
        "maximum": max(values),
    }


def analyze(rows: list[dict[str, Any]]) -> dict[str, Any]:
    full = [row for row in rows if row["family"] == "archetype.full-factorial"]
    by_mode = paired(rows, "archetype.full-factorial")
    pair_keys = {
        key[:-1]
        for key in by_mode
        if key[-1] == "manage"
    }
    short_pairs = [key for key in pair_keys if key[1] == "short"]
    long_pairs = [key for key in pair_keys if key[1] == "race-length"]

    attack_faster = sum(
        by_mode[key + ("attack",)]["mean_lap_ms"]
        < by_mode[key + ("manage",)]["mean_lap_ms"]
        for key in short_pairs
    )
    attack_costlier = sum(
        by_mode[key + ("attack",)]["correction_contact_workload_mj"]
        > by_mode[key + ("manage",)]["correction_contact_workload_mj"]
        and by_mode[key + ("attack",)]["control_error_amplitude"]
        > by_mode[key + ("manage",)]["control_error_amplitude"]
        for key in pair_keys
    )
    manage_preserves = sum(
        by_mode[key + ("manage",)]["final_tire_wear_pct"]
        < by_mode[key + ("attack",)]["final_tire_wear_pct"]
        for key in long_pairs
    )
    short_attack_pace_gain_ms = [
        by_mode[key + ("manage",)]["mean_lap_ms"]
        - by_mode[key + ("attack",)]["mean_lap_ms"]
        for key in short_pairs
    ]
    long_attack_pace_gain_ms = [
        by_mode[key + ("manage",)]["mean_lap_ms"]
        - by_mode[key + ("attack",)]["mean_lap_ms"]
        for key in long_pairs
    ]
    long_attack_wear_cost_pct = [
        by_mode[key + ("attack",)]["final_tire_wear_pct"]
        - by_mode[key + ("manage",)]["final_tire_wear_pct"]
        for key in long_pairs
    ]
    long_attack_correction_workload_cost_mj = [
        by_mode[key + ("attack",)]["correction_contact_workload_mj"]
        - by_mode[key + ("manage",)]["correction_contact_workload_mj"]
        for key in long_pairs
    ]

    winner_counts: Counter[str] = Counter()
    winning_mode_counts: Counter[str] = Counter()
    scenarios: dict[tuple[Any, ...], list[dict[str, Any]]] = defaultdict(list)
    for row in full:
        scenario_key = (
            row["circuit_id"],
            row["horizon"],
            row["tire_id"],
            row["seed"],
        )
        scenarios[scenario_key].append(row)
    for candidates in scenarios.values():
        winner = min(candidates, key=lambda row: (row["mean_lap_ms"], row["driver_id"], row["mode"]))
        winner_counts[f"{winner['driver_id']}:{winner['mode']}"] += 1
        winning_mode_counts[winner["mode"]] += 1

    consistency = {
        (row["circuit_id"], row["horizon"], row["seed"], row["driver_id"]): row
        for row in rows
        if row["family"] == "trait-isolation.consistency"
    }
    consistency_keys = {
        key[:-1] for key in consistency if key[-1] == "consistency_low"
    }
    lower_error = sum(
        consistency[key + ("consistency_high",)]["control_error_amplitude"]
        < consistency[key + ("consistency_low",)]["control_error_amplitude"]
        for key in consistency_keys
    )
    changed_dispersion = sum(
        consistency[key + ("consistency_high",)]["lap_time_stddev_ms"]
        != consistency[key + ("consistency_low",)]["lap_time_stddev_ms"]
        for key in consistency_keys
    )

    tire_management = {
        (row["circuit_id"], row["seed"], row["driver_id"]): row
        for row in rows
        if row["family"] == "trait-isolation.tire-management"
    }
    tire_keys = {
        key[:-1] for key in tire_management if key[-1] == "tire_management_low"
    }
    lower_correction_wear = sum(
        tire_management[key + ("tire_management_high",)][
            "requested_correction_wear_fraction"
        ]
        < tire_management[key + ("tire_management_low",)][
            "requested_correction_wear_fraction"
        ]
        for key in tire_keys
    )

    checks = {
        "attack_improves_short_run_pace": {
            "passed": attack_faster > 0,
            "count": attack_faster,
            "total": len(short_pairs),
            "ratio": ratio(attack_faster, len(short_pairs)),
        },
        "attack_pays_error_and_workload_cost": {
            "passed": attack_costlier == len(pair_keys),
            "count": attack_costlier,
            "total": len(pair_keys),
            "ratio": ratio(attack_costlier, len(pair_keys)),
        },
        "manage_preserves_tires": {
            "passed": manage_preserves > 0,
            "count": manage_preserves,
            "total": len(long_pairs),
            "ratio": ratio(manage_preserves, len(long_pairs)),
        },
        "consistency_reduces_error_and_changes_dispersion": {
            "passed": lower_error == len(consistency_keys) and changed_dispersion > 0,
            "lower_error_count": lower_error,
            "changed_dispersion_count": changed_dispersion,
            "total": len(consistency_keys),
        },
        "tire_management_reduces_correction_wear": {
            "passed": lower_correction_wear == len(tire_keys),
            "count": lower_correction_wear,
            "total": len(tire_keys),
        },
        "no_universal_driver_mode_winner": {
            "passed": max(winner_counts.values(), default=0) < len(scenarios),
            "scenario_count": len(scenarios),
            "winner_counts": dict(sorted(winner_counts.items())),
        },
        "no_universal_mode_winner": {
            "passed": max(winning_mode_counts.values(), default=0) < len(scenarios),
            "scenario_count": len(scenarios),
            "winner_counts": dict(sorted(winning_mode_counts.items())),
        },
    }
    return {
        "checks": checks,
        "effect_summary": {
            "short_attack_pace_gain_ms": range_summary(short_attack_pace_gain_ms),
            "race_length_attack_pace_gain_ms": range_summary(
                long_attack_pace_gain_ms
            ),
            "race_length_attack_wear_cost_percentage_points": range_summary(
                long_attack_wear_cost_pct
            ),
            "race_length_attack_correction_workload_cost_mj": range_summary(
                long_attack_correction_workload_cost_mj
            ),
        },
        "review_verdict": "PASS" if all(check["passed"] for check in checks.values()) else "REFINE",
    }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--jobs", type=int, default=4)
    parser.add_argument("--output", type=pathlib.Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--check", action="store_true")
    parser.add_argument(
        "--limit",
        type=int,
        help="run only the first N configurations for probe development",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if not 1 <= args.jobs <= MAX_JOBS:
        raise ScreenError(f"--jobs must be in [1, {MAX_JOBS}]")
    base_scenario = json.loads(BASE_SCENARIO.read_bytes())
    driver_document = json.loads(DRIVERS.read_bytes())
    archetypes = driver_document["drivers"]
    plan = build_plan(base_scenario, archetypes)
    if args.limit is not None:
        plan = plan[: args.limit]

    runner = (
        ROOT
        / "target"
        / "release"
        / "examples"
        / "v3_driver_control_probe"
    )
    if not runner.is_file():
        raise ScreenError(
            "missing release probe; run `cargo build --locked --release "
            "-p pitgun-racing-simulator --example v3_driver_control_probe`"
        )

    rows: list[dict[str, Any]] = []
    with concurrent.futures.ThreadPoolExecutor(max_workers=args.jobs) as executor:
        futures = [executor.submit(execute_point, runner, PROFILE, item) for item in plan]
        for index, future in enumerate(concurrent.futures.as_completed(futures), start=1):
            rows.append(future.result())
            if index % 50 == 0 or index == len(futures):
                print(f"completed {index}/{len(futures)}", flush=True)
    rows.sort(key=lambda row: row["configuration_id"])

    report = {
        "schema_version": SCHEMA_VERSION,
        "campaign": {
            "model": "pitgun.racing-v3-candidate@0.12.0",
            "profile_digest": sha256(json.loads(PROFILE.read_bytes())),
            "driver_archetype_digest": sha256(driver_document),
            "configuration_count": len(rows),
            "simulated_lap_count": sum(row["laps"] for row in rows),
            "complete": args.limit is None,
        },
        "analysis": analyze(rows) if args.limit is None else None,
        "runs": rows,
    }
    encoded = canonical_pretty(report)
    if args.check:
        if not args.output.is_file() or args.output.read_bytes() != encoded:
            raise ScreenError(f"{args.output} is missing or not reproducible")
        print(f"reproduced {args.output}")
        return 0
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_bytes(encoded)
    print(f"wrote {args.output}")
    if report["analysis"] is not None:
        print(f"review verdict: {report['analysis']['review_verdict']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
