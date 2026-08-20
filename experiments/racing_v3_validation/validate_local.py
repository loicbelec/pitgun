#!/usr/bin/env python3
"""Validate the frozen Racing Model V3 candidate on held-out and long-run cases."""

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
from typing import Any


ROOT = pathlib.Path(__file__).resolve().parents[2]
BASE_SCENARIO = (
    ROOT / "apps" / "pitgun-cli" / "scenarios" / "racing-batch-v1" / "balanced.json"
)
BASE_PROFILE = (
    ROOT
    / "experiments"
    / "racing_v3_decision_surface"
    / "profile-v5.active-vehicle-tire-fidelity.json"
)
DEFAULT_OUTPUT = pathlib.Path(__file__).parent / "results" / "local-validation-v2.json"
SCHEMA_VERSION = "pitgun.racing-v3-local-validation/v2"
PROBE_SCHEMA_VERSION = "pitgun.racing-v3-validation-probe/v1"
MAX_JOBS = 16

CIRCUITS = (
    ("be-1925", "spa", "high-speed-elevation"),
    ("es-1991", "barcelona", "mixed"),
    ("sg-2008", "singapore", "street-high-downforce"),
    ("mx-1962", "mexico", "altitude"),
)
VEHICLES = (
    ("classic_v8_1960", 1),
    ("classic_v8_1970", 3),
    ("modern_v6t", 5),
    ("f1_2026", 5),
)
SETUPS = (
    ("neutral", 0.5, 0.5),
    ("low-short", 0.0, 0.0),
    ("low-long", 0.0, 1.0),
    ("high-short", 1.0, 0.0),
    ("high-long", 1.0, 1.0),
)
SETUP_SEEDS = (7, 42)
TIRE_STRATEGIES = (
    ("soft-no-stop", (("soft", 24),)),
    ("medium-no-stop", (("medium", 24),)),
    ("hard-no-stop", (("hard", 24),)),
    ("medium-soft-l8", (("medium", 8), ("soft", 16))),
    ("medium-soft-l12", (("medium", 12), ("soft", 12))),
    ("medium-soft-l16", (("medium", 16), ("soft", 8))),
)
PROGRESSION_ANCHORS = (
    ("era1-classic60", 1, "classic_v8_1960"),
    ("era2-classic70", 2, "classic_v8_1970"),
    ("era3-classic70", 3, "classic_v8_1970"),
    ("era4-classic70", 4, "classic_v8_1970"),
    ("era5-modern", 5, "modern_v6t"),
    ("era5-2026", 5, "f1_2026"),
)


class ValidationError(RuntimeError):
    """Raised when governed validation evidence cannot be produced."""


def canonical_pretty(value: object) -> bytes:
    return (json.dumps(value, indent=2, ensure_ascii=False) + "\n").encode()


def sha256(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


def configure_scenario(
    base: dict[str, Any], *, circuit_id: str, vehicle_id: str, era: int, laps: int
) -> dict[str, Any]:
    scenario = copy.deepcopy(base)
    request = scenario["request"]
    request.update(
        {
            "track_id": circuit_id,
            "vehicle_id": vehicle_id,
            "era": era,
            "laps": laps,
            "hz": 5.0,
        }
    )
    competitor = request["competitors"][0]
    competitor["budget_cap"] = 40.0
    competitor["tuning"] = {
        "engine_points": 10.0,
        "cooling_points": 10.0,
        "aero_points": 10.0,
        "chassis_points": 10.0,
        "downforce_slider": 0.5,
        "gear_ratio_slider": 0.5,
    }
    competitor.pop("stint_strategy", None)
    request.pop("pit_strategy", None)
    return scenario


def explicit_stint_strategy(stints: tuple[tuple[str, int], ...]) -> dict[str, Any]:
    cumulative = 0
    pit_laps = []
    for _, laps in stints[:-1]:
        cumulative += laps
        pit_laps.append(cumulative)
    return {
        "stints": [{"tire_id": tire, "laps": laps} for tire, laps in stints],
        "pit_laps": pit_laps,
    }


def build_plan(base: dict[str, Any]) -> list[dict[str, Any]]:
    plan: list[dict[str, Any]] = []
    for circuit_id, circuit_slug, archetype in CIRCUITS:
        for vehicle_id, era in VEHICLES:
            for setup_id, downforce, gearing in SETUPS:
                for seed in SETUP_SEEDS:
                    scenario = configure_scenario(
                        base,
                        circuit_id=circuit_id,
                        vehicle_id=vehicle_id,
                        era=era,
                        laps=6,
                    )
                    scenario["request"]["competitors"][0]["tuning"].update(
                        {"downforce_slider": downforce, "gear_ratio_slider": gearing}
                    )
                    plan.append(
                        {
                            "case_id": f"setup-{setup_id}",
                            "family": "heldout.setup",
                            "level": setup_id,
                            "circuit_id": circuit_id,
                            "circuit_slug": circuit_slug,
                            "circuit_archetype": archetype,
                            "vehicle_id": vehicle_id,
                            "era": era,
                            "seed": seed,
                            "scenario": scenario,
                        }
                    )

            for strategy_id, stints in TIRE_STRATEGIES:
                scenario = configure_scenario(
                    base,
                    circuit_id=circuit_id,
                    vehicle_id=vehicle_id,
                    era=era,
                    laps=24,
                )
                scenario["request"]["competitors"][0]["stint_strategy"] = (
                    explicit_stint_strategy(stints)
                )
                plan.append(
                    {
                        "case_id": f"tire-{strategy_id}",
                        "family": "longrun.tire",
                        "level": strategy_id,
                        "circuit_id": circuit_id,
                        "circuit_slug": circuit_slug,
                        "circuit_archetype": archetype,
                        "vehicle_id": vehicle_id,
                        "era": era,
                        "seed": 42,
                        "stints": [
                            {"tire_id": tire, "laps": laps} for tire, laps in stints
                        ],
                        "scenario": scenario,
                    }
                )

        for anchor_id, era, vehicle_id in PROGRESSION_ANCHORS:
            scenario = configure_scenario(
                base,
                circuit_id=circuit_id,
                vehicle_id=vehicle_id,
                era=era,
                laps=6,
            )
            plan.append(
                {
                    "case_id": anchor_id,
                    "family": "progression.anchor",
                    "level": anchor_id,
                    "circuit_id": circuit_id,
                    "circuit_slug": circuit_slug,
                    "circuit_archetype": archetype,
                    "vehicle_id": vehicle_id,
                    "era": era,
                    "seed": 99,
                    "scenario": scenario,
                }
            )
    assert len(plan) == 280
    return plan


def execute_point(
    runner: pathlib.Path, profile_path: pathlib.Path, point: dict[str, Any]
) -> dict[str, Any]:
    with tempfile.TemporaryDirectory(prefix="pitgun-v3-validation-") as directory:
        scenario_path = pathlib.Path(directory) / "scenario.json"
        scenario_path.write_bytes(canonical_pretty(point["scenario"]))
        completed = subprocess.run(
            [str(runner), str(scenario_path), str(profile_path), str(point["seed"])],
            cwd=ROOT,
            check=False,
            capture_output=True,
            timeout=180,
        )
    if completed.returncode != 0:
        error = completed.stderr.decode(errors="replace").strip()
        if "cannot resolve V3 physical vehicle" not in error:
            raise ValidationError(
                f"probe failed for {point['case_id']} {point['circuit_slug']} "
                f"{point['vehicle_id']}: {error}"
            )
        return {
            key: value for key, value in point.items() if key not in {"scenario"}
        } | {
            "execution_status": "unsupported",
            "error": error.removeprefix("error: "),
        }
    if completed.stderr:
        raise ValidationError(f"successful probe wrote to stderr for {point['case_id']}")
    try:
        result = json.loads(completed.stdout)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ValidationError("probe returned invalid JSON") from error
    if result.get("schema_version") != PROBE_SCHEMA_VERSION:
        raise ValidationError("probe returned an unsupported result")
    metadata = {
        key: value
        for key, value in point.items()
        if key not in {"scenario"}
    }
    if point["family"] == "longrun.tire":
        metadata["stint_pace"] = stint_pace_summary(
            result["player_lap_times_ms"], result["player_pit_laps"]
        )
    return metadata | {"execution_status": "succeeded"} | result


def stint_pace_summary(
    lap_times_ms: list[int], pit_laps: list[int], window: int = 3
) -> list[dict[str, Any]]:
    boundaries = [0, *pit_laps, len(lap_times_ms)]
    summaries = []
    for index, (start, end) in enumerate(zip(boundaries, boundaries[1:]), start=1):
        clean = list(lap_times_ms[start:end])
        if start == 0 and clean:
            clean = clean[1:]
        if not clean:
            summaries.append(
                {"stint": index, "clean_lap_count": 0, "pace_drift_ms": None}
            )
            continue
        first = statistics.median(clean[:window])
        last = statistics.median(clean[-window:])
        summaries.append(
            {
                "stint": index,
                "clean_lap_count": len(clean),
                "first_window_median_ms": first,
                "last_window_median_ms": last,
                "pace_drift_ms": last - first,
            }
        )
    return summaries


def summarize_setups(points: list[dict[str, Any]]) -> dict[str, Any]:
    groups = []
    for circuit_id, circuit_slug, _ in CIRCUITS:
        for vehicle_id, _ in VEHICLES:
            selected = [
                point
                for point in points
                if point["family"] == "heldout.setup"
                and point["circuit_id"] == circuit_id
                and point["vehicle_id"] == vehicle_id
                and point["execution_status"] == "succeeded"
            ]
            if not selected:
                groups.append(
                    {
                        "circuit_id": circuit_id,
                        "circuit_slug": circuit_slug,
                        "vehicle_id": vehicle_id,
                        "status": "unsupported",
                    }
                )
                continue
            levels = []
            for setup_id, _, _ in SETUPS:
                values = [p["total_time_ms"] for p in selected if p["level"] == setup_id]
                levels.append(
                    {"setup": setup_id, "median_total_time_ms": statistics.median(values)}
                )
            fastest = min(levels, key=lambda value: (value["median_total_time_ms"], value["setup"]))
            neutral = next(value for value in levels if value["setup"] == "neutral")
            groups.append(
                {
                    "circuit_id": circuit_id,
                    "circuit_slug": circuit_slug,
                    "vehicle_id": vehicle_id,
                    "status": "succeeded",
                    "fastest_setup": fastest["setup"],
                    "neutral_regret_ms": (
                        neutral["median_total_time_ms"] - fastest["median_total_time_ms"]
                    ),
                    "levels": levels,
                }
            )
    optima = {
        group["fastest_setup"]
        for group in groups
        if group["status"] == "succeeded"
    }
    return {
        "group_count": len(groups),
        "distinct_fastest_setup_count": len(optima),
        "fastest_setups": sorted(optima),
        "groups": groups,
    }


def summarize_tires(points: list[dict[str, Any]]) -> dict[str, Any]:
    groups = []
    for circuit_id, circuit_slug, _ in CIRCUITS:
        for vehicle_id, _ in VEHICLES:
            selected = [
                point
                for point in points
                if point["family"] == "longrun.tire"
                and point["circuit_id"] == circuit_id
                and point["vehicle_id"] == vehicle_id
                and point["execution_status"] == "succeeded"
            ]
            if not selected:
                groups.append(
                    {
                        "circuit_id": circuit_id,
                        "circuit_slug": circuit_slug,
                        "vehicle_id": vehicle_id,
                        "status": "unsupported",
                    }
                )
                continue
            strategies = [
                {
                    "strategy": point["level"],
                    "total_time_ms": point["total_time_ms"],
                    "final_tire_wear_pct": point["final_tire_wear_pct"],
                    "final_tire_temperature_c": point["final_tire_temperature_c"],
                    "stint_pace": point["stint_pace"],
                }
                for point in sorted(selected, key=lambda value: value["level"])
            ]
            one_stop = [value for value in strategies if "medium-soft" in value["strategy"]]
            no_stop = [value for value in strategies if "no-stop" in value["strategy"]]
            fastest = min(one_stop, key=lambda value: (value["total_time_ms"], value["strategy"]))
            groups.append(
                {
                    "circuit_id": circuit_id,
                    "circuit_slug": circuit_slug,
                    "vehicle_id": vehicle_id,
                    "status": "succeeded",
                    "fastest_one_stop": fastest["strategy"],
                    "no_stop_compound_time_range_ms": max(
                        value["total_time_ms"] for value in no_stop
                    )
                    - min(value["total_time_ms"] for value in no_stop),
                    "no_stop_compound_final_wear_range_pct": max(
                        value["final_tire_wear_pct"] for value in no_stop
                    )
                    - min(value["final_tire_wear_pct"] for value in no_stop),
                    "strategies": strategies,
                }
            )
    windows = {
        group["fastest_one_stop"]
        for group in groups
        if group["status"] == "succeeded"
    }
    succeeded_groups = [group for group in groups if group["status"] == "succeeded"]
    return {
        "group_count": len(groups),
        "distinct_fastest_one_stop_count": len(windows),
        "fastest_one_stops": sorted(windows),
        "minimum_no_stop_compound_time_range_ms": min(
            group["no_stop_compound_time_range_ms"] for group in succeeded_groups
        ),
        "minimum_no_stop_compound_final_wear_range_pct": min(
            group["no_stop_compound_final_wear_range_pct"]
            for group in succeeded_groups
        ),
        "groups": groups,
    }


def progression_summary(points: list[dict[str, Any]]) -> dict[str, Any]:
    selected = [point for point in points if point["family"] == "progression.anchor"]
    succeeded = [point for point in selected if point["execution_status"] == "succeeded"]
    unsupported = [point for point in selected if point["execution_status"] == "unsupported"]
    observed_eras = sorted({point["era"] for point in succeeded})
    return {
        "execution_count": len(selected),
        "succeeded_execution_count": len(succeeded),
        "unsupported_execution_count": len(unsupported),
        "observed_eras": observed_eras,
        "era_3_and_4_explicitly_covered": 3 in observed_eras and 4 in observed_eras,
        "anchors": [
            {
                "case_id": point["case_id"],
                "circuit_id": point["circuit_id"],
                "vehicle_id": point["vehicle_id"],
                "era": point["era"],
                "total_time_ms": point["total_time_ms"],
            }
            for point in succeeded
        ],
        "unsupported_anchors": [
            {
                "case_id": point["case_id"],
                "circuit_id": point["circuit_id"],
                "vehicle_id": point["vehicle_id"],
                "era": point["era"],
                "error": point["error"],
            }
            for point in unsupported
        ],
    }


def campaign_verdicts(
    setup: dict[str, Any],
    tires: dict[str, Any],
    progression: dict[str, Any],
    unsupported_vehicle_ids: list[str],
) -> list[dict[str, str]]:
    setup_diverse = setup["distinct_fastest_setup_count"] > 1
    tires_active = (
        tires["minimum_no_stop_compound_time_range_ms"] > 5
        and tires["minimum_no_stop_compound_final_wear_range_pct"] > 0.01
    )
    strategy_diverse = tires["distinct_fastest_one_stop_count"] > 1
    return [
        {
            "capability": "active_vehicle_compatibility",
            "verdict": "REFINE" if unsupported_vehicle_ids else "PASS",
            "reason": (
                "The frozen candidate cannot resolve active vehicle resources: "
                + ", ".join(unsupported_vehicle_ids)
                if unsupported_vehicle_ids
                else "Every active physical vehicle resource executes on the held-out circuits."
            ),
        },
        {
            "capability": "held_out_setup_response",
            "verdict": (
                "REFINE" if unsupported_vehicle_ids else "PASS" if setup_diverse else "REFINE"
            ),
            "reason": (
                "Supported held-out groups produce multiple coarse setup optima, but active-vehicle coverage is incomplete."
                if unsupported_vehicle_ids and setup_diverse
                else "Held-out circuit and vehicle groups do not share one universal coarse setup optimum."
                if setup_diverse
                else "Every held-out circuit and vehicle group shares the same coarse setup optimum."
            ),
        },
        {
            "capability": "long_run_tire_activation",
            "verdict": (
                "REFINE"
                if unsupported_vehicle_ids and tires_active
                else "PASS"
                if tires_active
                else "STRUCTURAL_CHANGE_REQUIRED"
            ),
            "reason": (
                "Compounds are observable in every supported long-run group, but active-vehicle coverage is incomplete."
                if unsupported_vehicle_ids and tires_active
                else "Compounds produce observable long-run time and final-wear differences."
                if tires_active
                else "Explicit no-stop soft, medium and hard strategies are identical: the simulator starts from the vehicle default tire instead of the first declared stint tire."
            ),
        },
        {
            "capability": "one_stop_window_diversity",
            "verdict": "PASS" if strategy_diverse else "REFINE",
            "reason": (
                "The fastest reviewed one-stop window varies across circuit and vehicle groups."
                if strategy_diverse
                else "One reviewed one-stop window is universally fastest."
            ),
        },
        {
            "capability": "active_era_coverage",
            "verdict": (
                "PASS"
                if progression["era_3_and_4_explicitly_covered"]
                and not unsupported_vehicle_ids
                else "REFINE"
            ),
            "reason": (
                "Eras 2 through 5, including explicit era 3 and 4 anchors, execute deterministically; era 1 remains blocked by its unsupported vehicle."
                if unsupported_vehicle_ids
                else "Enabled eras 1 through 5, including explicit era 3 and 4 anchors, execute deterministically."
            ),
        },
        {
            "capability": "fuel_mass_observability",
            "verdict": "STRUCTURAL_CHANGE_REQUIRED",
            "reason": "The candidate output does not expose initial, per-lap or final fuel mass, so mass evolution cannot yet be independently audited.",
        },
        {
            "capability": "production_or_opponent_policy_change",
            "verdict": "REFINE",
            "reason": "This campaign validates a frozen offline candidate; it does not authorize production or opponent-policy changes.",
        },
    ]


def build_report(
    runner: pathlib.Path, jobs: int, profile_path: pathlib.Path = BASE_PROFILE
) -> dict[str, Any]:
    if not runner.is_file():
        raise ValidationError(f"missing V3 validation probe: {runner}")
    if jobs < 1 or jobs > MAX_JOBS:
        raise ValueError(f"jobs must be between 1 and {MAX_JOBS}")
    scenario_bytes = BASE_SCENARIO.read_bytes()
    profile_bytes = profile_path.read_bytes()
    plan = build_plan(json.loads(scenario_bytes))
    with concurrent.futures.ThreadPoolExecutor(max_workers=jobs) as executor:
        points = list(
            executor.map(
                lambda point: execute_point(runner, profile_path, point), plan
            )
        )
    points.sort(
        key=lambda point: (
            point["family"],
            point["case_id"],
            point["circuit_id"],
            point["vehicle_id"],
            point["seed"],
        )
    )
    succeeded = [point for point in points if point["execution_status"] == "succeeded"]
    identities = {json.dumps(point["model"], sort_keys=True) for point in succeeded}
    if len(identities) != 1:
        raise ValidationError("validation mixed multiple model identities")
    setup = summarize_setups(points)
    tires = summarize_tires(points)
    progression = progression_summary(points)
    unsupported_vehicle_ids = sorted(
        {
            point["vehicle_id"]
            for point in points
            if point["execution_status"] == "unsupported"
        }
    )
    return {
        "schema_version": SCHEMA_VERSION,
        "campaign": {
            "purpose": "frozen held-out, long-run and active-era validation before fuel/mass work",
            "model": json.loads(next(iter(identities))),
            "runner": {
                "path": str(runner.relative_to(ROOT)),
                "digest": sha256(runner.read_bytes()),
            },
            "base_scenario_path": str(BASE_SCENARIO.relative_to(ROOT)),
            "base_scenario_digest": sha256(scenario_bytes),
            "profile_path": str(profile_path.relative_to(ROOT)),
            "profile_digest": sha256(profile_bytes),
            "execution_count": len(points),
            "succeeded_execution_count": len(succeeded),
            "unsupported_execution_count": len(points) - len(succeeded),
            "unsupported_vehicle_ids": unsupported_vehicle_ids,
            "simulated_lap_count": sum(
                len(point["player_lap_times_ms"]) for point in succeeded
            ),
            "held_out_circuits": [
                {"id": identifier, "slug": slug, "archetype": archetype}
                for identifier, slug, archetype in CIRCUITS
            ],
            "vehicles": [identifier for identifier, _ in VEHICLES],
            "setup_seeds": list(SETUP_SEEDS),
        },
        "verdicts": campaign_verdicts(
            setup, tires, progression, unsupported_vehicle_ids
        ),
        "setup_summary": setup,
        "tire_summary": tires,
        "progression_summary": progression,
        "points": points,
    }


def write_or_check(report: dict[str, Any], output: pathlib.Path, check: bool) -> None:
    report_bytes = canonical_pretty(report)
    digest_bytes = (sha256(report_bytes) + "\n").encode()
    digest_path = output.with_suffix(".sha256")
    if check:
        if output.read_bytes() != report_bytes or digest_path.read_bytes() != digest_bytes:
            raise ValidationError("stored V3 validation artifacts do not match replay")
        return
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_bytes(report_bytes)
    digest_path.write_bytes(digest_bytes)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--runner",
        type=pathlib.Path,
        default=ROOT / "target" / "release" / "examples" / "v3_validation_probe",
    )
    parser.add_argument("--jobs", type=int, default=4)
    parser.add_argument("--profile", type=pathlib.Path, default=BASE_PROFILE)
    parser.add_argument("--output", type=pathlib.Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--check", action="store_true")
    arguments = parser.parse_args()
    try:
        report = build_report(
            arguments.runner.resolve(), arguments.jobs, arguments.profile.resolve()
        )
        write_or_check(report, arguments.output.resolve(), arguments.check)
    except (OSError, ValueError, ValidationError) as error:
        print(f"error: {error}", file=__import__("sys").stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
