#!/usr/bin/env python3
"""Measure isolated Solver response to the experimental SPW Spa relief."""

from __future__ import annotations

import argparse
import concurrent.futures
import hashlib
import json
import os
import pathlib
import statistics
import subprocess
import sys
import tempfile
from typing import Any

import coefficient_screening as screening


ROOT = pathlib.Path(__file__).resolve().parents[2]
TRACK_EXPERIMENT_ROOT = ROOT / "experiments" / "racing_tracks"
sys.path.insert(0, str(TRACK_EXPERIMENT_ROOT))
import build_spa_elevation_prototype as elevation  # noqa: E402
import compare_spa_elevation_sources as sources  # noqa: E402


BASE_SCENARIO = (
    ROOT / "apps" / "pitgun-cli" / "scenarios" / "racing-batch-v1" / "balanced.json"
)
TUNING_RESPONSE = (
    ROOT / "experiments" / "databricks" / "responses" / "racing-aero-candidate-v1.json"
)
DEFAULT_OUTPUT = (
    ROOT
    / "experiments"
    / "racing_response"
    / "results"
    / "racing-spa-relief-impact-v1.json"
)
DEFAULT_SEED = 42
DEFAULT_LEVEL_COUNT = 5
MAX_JOBS = 16
SCHEMA_VERSION = "pitgun.racing-spa-relief-impact/v1"


class ReliefImpactError(RuntimeError):
    """Raised when the isolated relief experiment cannot be trusted."""


def interpolate(target: float, x: list[float], y: list[float]) -> float:
    upper = 1
    while upper < len(x) and x[upper] < target:
        upper += 1
    if upper >= len(x):
        return y[-1]
    lower = upper - 1
    fraction = (target - x[lower]) / (x[upper] - x[lower])
    return y[lower] + fraction * (y[upper] - y[lower])


def build_track_profiles() -> dict[str, dict[str, list[float]]]:
    canonical = json.loads(elevation.TRACK.read_text())["data"]
    spw = json.loads(sources.SPW_RAW_OUTPUT.read_text())
    sample_distance = [float(point["distance_m"]) for point in spw["points"]]
    raw_height = [float(point["elevation_m"]) for point in spw["points"]]
    smoothed_height = elevation.circular_mean(raw_height, radius=1)
    baseline = smoothed_height[0]
    relative_height = [value - baseline for value in smoothed_height]
    relief = [
        interpolate(distance, sample_distance, relative_height)
        for distance in canonical["s_m"]
    ]
    relief[-1] = relief[0]
    shared = {
        "s": canonical["s_m"],
        "x": canonical["x_m"],
        "y": canonical["y_m"],
    }
    return {
        "flat": {**shared, "z": [0.0] * len(canonical["s_m"])},
        "spw_relief": {**shared, "z": relief},
    }


def profile_summary(profile: dict[str, list[float]]) -> dict[str, float | int]:
    z = profile["z"]
    s = profile["s"]
    slope = []
    for index in range(len(s)):
        lower = max(0, index - 1)
        upper = min(len(s) - 1, index + 1)
        slope.append((z[upper] - z[lower]) / (s[upper] - s[lower]))
    return {
        "sample_count": len(s),
        "length_m": s[-1],
        "elevation_range_m": max(z) - min(z),
        "maximum_absolute_slope_ratio": max(abs(value) for value in slope),
        "closure_error_m": abs(z[-1] - z[0]),
    }


def build_scenario(
    base: dict[str, Any],
    profile: dict[str, list[float]],
    downforce: float,
    gearing: float,
) -> bytes:
    scenario = json.loads(json.dumps(base))
    scenario["request"]["track_id"] = "be-1925"
    scenario["request"]["track_profile"] = profile
    tuning = scenario["request"]["competitors"][0]["tuning"]
    tuning["downforce_slider"] = downforce
    tuning["gear_ratio_slider"] = gearing
    return screening.canonical_pretty(scenario)


def run_probe(
    runner: pathlib.Path,
    scenario_path: pathlib.Path,
    seed: int,
) -> dict[str, Any]:
    try:
        completed = subprocess.run(
            [str(runner), str(scenario_path), str(TUNING_RESPONSE), str(seed)],
            cwd=ROOT,
            check=False,
            capture_output=True,
            timeout=120,
        )
    except (OSError, subprocess.SubprocessError) as error:
        raise ReliefImpactError(f"probe failed: {error}") from error
    if completed.returncode != 0:
        raise ReliefImpactError(
            f"probe exited {completed.returncode}: "
            f"{completed.stderr.decode(errors='replace').strip()}"
        )
    if completed.stderr:
        raise ReliefImpactError("successful probe wrote to stderr")
    try:
        result = json.loads(completed.stdout)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ReliefImpactError("probe returned invalid JSON") from error
    if result.get("schema_version") != "pitgun.racing-tuning-response-probe/v1":
        raise ReliefImpactError("probe returned an unsupported result")
    return result


def execute_point(
    runner: pathlib.Path,
    scenario_path: pathlib.Path,
    seed: int,
    variant: str,
    downforce_index: int,
    downforce: float,
    gearing_index: int,
    gearing: float,
) -> dict[str, Any]:
    result = run_probe(runner, scenario_path, seed)
    return {
        "variant": variant,
        "downforce_index": downforce_index,
        "downforce_slider": downforce,
        "gearing_index": gearing_index,
        "gear_ratio_slider": gearing,
        "experimental_execution_id": result["experimental_execution_id"],
        "scenario_digest": result["scenario_digest"],
        "tuning_response_digest": result["tuning_response_digest"],
        "total_time_ms": result["total_time_ms"],
        "observed_maximum_speed_kph": result["observed_maximum_speed_kph"],
        "setup_response": result["setup_response"],
    }


def delta(relief: dict[str, Any], flat: dict[str, Any]) -> dict[str, float | int]:
    return {
        "total_time_ms": relief["total_time_ms"] - flat["total_time_ms"],
        "observed_maximum_speed_kph": (
            relief["observed_maximum_speed_kph"] - flat["observed_maximum_speed_kph"]
        ),
        "mean_corner_speed_kph": (
            relief["setup_response"]["mean_corner_speed_kph"]
            - flat["setup_response"]["mean_corner_speed_kph"]
        ),
        "aerodynamic_drag_work_kj": (
            relief["setup_response"]["aerodynamic_drag_work_kj"]
            - flat["setup_response"]["aerodynamic_drag_work_kj"]
        ),
        "mean_downforce_n": (
            relief["setup_response"]["mean_downforce_n"]
            - flat["setup_response"]["mean_downforce_n"]
        ),
    }


def fastest(points: list[dict[str, Any]], variant: str) -> dict[str, Any]:
    point = min(
        (item for item in points if item["variant"] == variant),
        key=lambda item: (
            item["total_time_ms"],
            item["downforce_index"],
            item["gearing_index"],
        ),
    )
    return {
        "downforce_slider": point["downforce_slider"],
        "gear_ratio_slider": point["gear_ratio_slider"],
        "total_time_ms": point["total_time_ms"],
        "observed_maximum_speed_kph": point["observed_maximum_speed_kph"],
        "on_setup_boundary": point["downforce_index"] in {0, 4}
        or point["gearing_index"] in {0, 4},
    }


def build_report(
    runner: pathlib.Path, seed: int, level_count: int, jobs: int
) -> dict[str, Any]:
    if level_count != DEFAULT_LEVEL_COUNT:
        raise ValueError("V1 evidence requires the fixed five-level setup grid")
    if jobs < 1 or jobs > MAX_JOBS:
        raise ValueError(f"jobs must be between 1 and {MAX_JOBS}")
    if not runner.is_file():
        raise ReliefImpactError(f"missing probe runner: {runner}")
    if not elevation.verify_artifact(sources.SPW_RAW_OUTPUT):
        raise ReliefImpactError("SPW source evidence is missing or corrupt")

    base = json.loads(BASE_SCENARIO.read_text())
    profiles = build_track_profiles()
    levels = screening.slider_levels(level_count)
    planned = []
    with tempfile.TemporaryDirectory(prefix="pitgun-spa-relief-") as temporary:
        root = pathlib.Path(temporary)
        for variant, profile in profiles.items():
            for downforce_index, downforce in enumerate(levels):
                for gearing_index, gearing in enumerate(levels):
                    scenario_path = root / (
                        f"{variant}-df-{downforce_index}-gear-{gearing_index}.json"
                    )
                    scenario_path.write_bytes(
                        build_scenario(base, profile, downforce, gearing)
                    )
                    planned.append(
                        (
                            scenario_path,
                            variant,
                            downforce_index,
                            downforce,
                            gearing_index,
                            gearing,
                        )
                    )
        with concurrent.futures.ThreadPoolExecutor(max_workers=jobs) as executor:
            futures = [
                executor.submit(
                    execute_point,
                    runner,
                    scenario_path,
                    seed,
                    variant,
                    downforce_index,
                    downforce,
                    gearing_index,
                    gearing,
                )
                for (
                    scenario_path,
                    variant,
                    downforce_index,
                    downforce,
                    gearing_index,
                    gearing,
                ) in planned
            ]
            points = [future.result() for future in futures]

        midpoint_path = root / "spw_relief-df-2-gear-2.json"
        first_repeat = run_probe(runner, midpoint_path, seed)
        second_repeat = run_probe(runner, midpoint_path, seed)

    indexed = {
        (point["variant"], point["downforce_index"], point["gearing_index"]): point
        for point in points
    }
    matched_deltas = [
        delta(
            indexed[("spw_relief", downforce_index, gearing_index)],
            indexed[("flat", downforce_index, gearing_index)],
        )
        for downforce_index in range(level_count)
        for gearing_index in range(level_count)
    ]
    midpoint_delta = delta(indexed[("spw_relief", 2, 2)], indexed[("flat", 2, 2)])
    flat_fastest = fastest(points, "flat")
    relief_fastest = fastest(points, "spw_relief")
    profile_digest = screening.sha256(
        screening.canonical_pretty(profiles["spw_relief"])
    )
    return {
        "schema_version": SCHEMA_VERSION,
        "question": (
            "How does the validated SPW Spa relief change deterministic Solver response "
            "when horizontal geometry, vehicle, tuning response, seed, and setup grid are fixed?"
        ),
        "status": "experimental_not_catalog_eligible",
        "runner": {
            "path": str(runner.relative_to(ROOT)),
            "digest": screening.sha256(runner.read_bytes()),
        },
        "seed": str(seed),
        "setup_levels": levels,
        "planned_run_count": len(points),
        "isolation": {
            "fixed": [
                "s_m",
                "x_m",
                "y_m",
                "vehicle",
                "tuning response",
                "seed",
                "setup grid",
            ],
            "changed": ["z_m", "slope derived deterministically from z_m"],
            "horizontal_geometry_note": (
                "both variants use track_profile, so curvature is derived identically "
                "from the same x/y arrays"
            ),
        },
        "relief_evidence": {
            "spw_raw_digest": elevation.digest(sources.SPW_RAW_OUTPUT.read_bytes()),
            "experimental_profile_digest": profile_digest,
            "method": (
                "SPW 25 m centerline samples, circular three-sample smoothing, "
                "linear interpolation to the canonical one-metre s grid"
            ),
            "profiles": {
                variant: profile_summary(profile) for variant, profile in profiles.items()
            },
        },
        "determinism": {
            "midpoint_repeated_output_equal": first_repeat == second_repeat,
            "experimental_execution_id": first_repeat["experimental_execution_id"],
        },
        "response": {
            "flat_fastest": flat_fastest,
            "spw_relief_fastest": relief_fastest,
            "setup_optimum_changed": (
                flat_fastest["downforce_slider"] != relief_fastest["downforce_slider"]
                or flat_fastest["gear_ratio_slider"]
                != relief_fastest["gear_ratio_slider"]
            ),
            "midpoint_relief_minus_flat": midpoint_delta,
            "matched_grid_relief_minus_flat": {
                "minimum_total_time_ms": min(
                    item["total_time_ms"] for item in matched_deltas
                ),
                "maximum_total_time_ms": max(
                    item["total_time_ms"] for item in matched_deltas
                ),
                "mean_total_time_ms": statistics.fmean(
                    item["total_time_ms"] for item in matched_deltas
                ),
                "mean_maximum_speed_kph": statistics.fmean(
                    item["observed_maximum_speed_kph"] for item in matched_deltas
                ),
            },
        },
        "automatic_model_change": False,
        "next_review": [
            "inspect speed and longitudinal-force changes by distance",
            "review SPW smoothing and slope bounds",
            "repeat the governed setup campaign only if the isolated response is credible",
        ],
        "points": sorted(
            points,
            key=lambda point: (
                point["variant"],
                point["downforce_index"],
                point["gearing_index"],
            ),
        ),
    }


def stable(value: object) -> object:
    if isinstance(value, float):
        return round(value, 9)
    if isinstance(value, list):
        return [stable(item) for item in value]
    if isinstance(value, dict):
        return {key: stable(item) for key, item in value.items()}
    return value


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--runner",
        type=pathlib.Path,
        default=ROOT / "target" / "release" / "examples" / "tuning_response_probe",
    )
    parser.add_argument("--output", type=pathlib.Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--seed", type=int, default=DEFAULT_SEED)
    parser.add_argument("--levels", type=int, default=DEFAULT_LEVEL_COUNT)
    parser.add_argument(
        "--jobs", type=int, default=min(4, os.cpu_count() or 1, MAX_JOBS)
    )
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    try:
        report = stable(
            build_report(args.runner.resolve(), args.seed, args.levels, args.jobs)
        )
        payload = screening.canonical_pretty(report)
        checksum_path = args.output.with_suffix(".sha256")
        checksum = (
            hashlib.sha256(payload).hexdigest() + "  " + args.output.name + "\n"
        ).encode()
        if args.check:
            if (
                not args.output.is_file()
                or args.output.read_bytes() != payload
                or not checksum_path.is_file()
                or checksum_path.read_bytes() != checksum
            ):
                raise ReliefImpactError("Spa relief impact evidence is stale")
            print(f"Spa relief impact evidence is current: {args.output}")
        else:
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_bytes(payload)
            checksum_path.write_bytes(checksum)
            print(json.dumps(report["response"], indent=2))
    except (OSError, ValueError, ReliefImpactError) as error:
        parser.exit(1, f"error: {error}\n")


if __name__ == "__main__":
    main()
