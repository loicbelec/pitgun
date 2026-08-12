#!/usr/bin/env python3
"""Measure the bounded Racing setup response with the canonical Pitgun runner."""

from __future__ import annotations

import argparse
import concurrent.futures
import hashlib
import json
import os
import pathlib
import subprocess
import sys
import tempfile
from typing import Any


ROOT = pathlib.Path(__file__).resolve().parents[2]
BASE_SCENARIO = (
    ROOT / "apps" / "pitgun-cli" / "scenarios" / "racing-batch-v1" / "balanced.json"
)
DEFAULT_OUTPUT = (
    ROOT
    / "experiments"
    / "racing_response"
    / "results"
    / "racing-response-surface-v1.json"
)
SCHEMA_VERSION = "pitgun.racing-response-surface/v1"
DEFAULT_SEED = 42
DEFAULT_LEVEL_COUNT = 11
MAX_LEVEL_COUNT = 21
MAX_JOBS = 16

CIRCUITS = (
    ("it-1922", "power", "monza"),
    ("mc-1929", "high-downforce", "monaco"),
    ("hu-1986", "mechanical-grip", "budapest"),
    ("jp-1962", "mixed", "suzuka"),
    ("sg-2008", "street-thermal", "singapore"),
)


class ResponseSurfaceError(RuntimeError):
    """Raised when one bounded response-surface execution is invalid."""


def canonical_pretty(value: object) -> bytes:
    return (json.dumps(value, indent=2, ensure_ascii=False) + "\n").encode()


def sha256(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


def slider_levels(count: int) -> list[float]:
    if count < 3 or count > MAX_LEVEL_COUNT or count % 2 == 0:
        raise ValueError(
            f"level count must be an odd integer between 3 and {MAX_LEVEL_COUNT}"
        )
    return [round(index / (count - 1), 6) for index in range(count)]


def build_scenario(
    base: dict[str, Any], circuit_id: str, downforce: float, gearing: float
) -> bytes:
    scenario = json.loads(json.dumps(base))
    scenario["request"]["track_id"] = circuit_id
    tuning = scenario["request"]["competitors"][0]["tuning"]
    tuning["downforce_slider"] = downforce
    tuning["gear_ratio_slider"] = gearing
    return canonical_pretty(scenario)


def inspect_runner(runner: pathlib.Path) -> dict[str, str]:
    try:
        completed = subprocess.run(
            [str(runner), "--version"],
            cwd=ROOT,
            check=True,
            capture_output=True,
            timeout=30,
        )
    except (OSError, subprocess.SubprocessError) as error:
        raise ResponseSurfaceError(f"runner identity probe failed: {error}") from error
    if completed.stderr:
        raise ResponseSurfaceError("successful runner identity probe wrote to stderr")
    return {
        "version": completed.stdout.decode().strip(),
        "digest": sha256(runner.read_bytes()),
    }


def execute_point(
    runner: pathlib.Path,
    scenario_root: pathlib.Path,
    seed: int,
    circuit_index: int,
    circuit_id: str,
    archetype: str,
    slug: str,
    downforce_index: int,
    downforce: float,
    gearing_index: int,
    gearing: float,
    scenario_bytes: bytes,
) -> dict[str, Any]:
    scenario_path = scenario_root / (
        f"{circuit_index:02d}-{slug}-d{downforce_index:02d}-g{gearing_index:02d}.json"
    )
    scenario_path.write_bytes(scenario_bytes)
    try:
        completed = subprocess.run(
            [
                str(runner),
                "run",
                "racing",
                "--scenario",
                str(scenario_path),
                "--seed",
                str(seed),
            ],
            cwd=ROOT,
            check=False,
            capture_output=True,
            timeout=120,
        )
    except (OSError, subprocess.SubprocessError) as error:
        raise ResponseSurfaceError(
            f"runner failed for {slug} downforce={downforce} gearing={gearing}: {error}"
        ) from error
    if completed.returncode != 0:
        diagnostic = completed.stderr.decode(errors="replace").strip()
        raise ResponseSurfaceError(
            f"runner exited {completed.returncode} for {slug} "
            f"downforce={downforce} gearing={gearing}: {diagnostic}"
        )
    if completed.stderr:
        raise ResponseSurfaceError(
            f"successful runner wrote to stderr for {slug} "
            f"downforce={downforce} gearing={gearing}"
        )
    try:
        result = json.loads(completed.stdout)
        diagnostics = result["summary"]["setup_response"]
        metrics = result["summary"]["metrics"]["metrics"]
    except (UnicodeDecodeError, json.JSONDecodeError, KeyError, TypeError) as error:
        raise ResponseSurfaceError(
            f"runner returned an invalid diagnostic result for {slug}"
        ) from error
    if result.get("schema_version") != "pitgun.batch-run-result/v1":
        raise ResponseSurfaceError("runner returned an unsupported batch result")
    if diagnostics.get("schema_version") != "pitgun.racing-setup-response/v1":
        raise ResponseSurfaceError("runner returned unsupported setup diagnostics")
    maximum_speed = [
        metric["value"]
        for metric in metrics
        if metric.get("id") == "racing.observed-maximum-speed"
        and metric.get("unit") == "km/h"
    ]
    if len(maximum_speed) != 1:
        raise ResponseSurfaceError("runner did not return one maximum-speed metric")

    return {
        "circuit_index": circuit_index,
        "circuit_id": circuit_id,
        "circuit_archetype": archetype,
        "circuit_slug": slug,
        "downforce_index": downforce_index,
        "downforce_slider": downforce,
        "gearing_index": gearing_index,
        "gear_ratio_slider": gearing,
        "configuration_id": result["configuration_id"],
        "scenario_digest": result["scenario_digest"],
        "run_id": result["run_id"],
        "total_time_ms": result["summary"]["total_time_ms"],
        "observed_maximum_speed_kph": maximum_speed[0],
        "setup_response": diagnostics,
    }


def point_key(point: dict[str, Any]) -> tuple[int, int, int]:
    return (
        point["circuit_index"],
        point["downforce_index"],
        point["gearing_index"],
    )


def delta(high: dict[str, Any], low: dict[str, Any]) -> dict[str, float | int]:
    high_response = high["setup_response"]
    low_response = low["setup_response"]
    return {
        "total_time_ms": high["total_time_ms"] - low["total_time_ms"],
        "observed_maximum_speed_kph": high["observed_maximum_speed_kph"]
        - low["observed_maximum_speed_kph"],
        "mean_straight_speed_kph": high_response["mean_straight_speed_kph"]
        - low_response["mean_straight_speed_kph"],
        "mean_corner_speed_kph": high_response["mean_corner_speed_kph"]
        - low_response["mean_corner_speed_kph"],
        "maximum_rpm_utilization": high_response["maximum_rpm_utilization"]
        - low_response["maximum_rpm_utilization"],
        "aerodynamic_drag_work_kj": high_response["aerodynamic_drag_work_kj"]
        - low_response["aerodynamic_drag_work_kj"],
        "mean_downforce_n": high_response["mean_downforce_n"]
        - low_response["mean_downforce_n"],
    }


def summarize_circuit(
    circuit: tuple[str, str, str], points: list[dict[str, Any]], level_count: int
) -> dict[str, Any]:
    circuit_id, archetype, slug = circuit
    circuit_points = [point for point in points if point["circuit_id"] == circuit_id]
    if len(circuit_points) != level_count * level_count:
        raise ResponseSurfaceError(f"incomplete response surface for {slug}")
    descriptor = circuit_points[0]["setup_response"]["circuit"]
    if any(point["setup_response"]["circuit"] != descriptor for point in circuit_points):
        raise ResponseSurfaceError(f"physical circuit descriptors changed across {slug}")

    midpoint = level_count // 2
    indexed = {
        (point["downforce_index"], point["gearing_index"]): point
        for point in circuit_points
    }
    fastest = min(
        circuit_points,
        key=lambda point: (
            point["total_time_ms"],
            point["downforce_index"],
            point["gearing_index"],
        ),
    )
    slowest = max(point["total_time_ms"] for point in circuit_points)
    return {
        "circuit_id": circuit_id,
        "circuit_archetype": archetype,
        "circuit_slug": slug,
        "circuit": descriptor,
        "fastest": {
            "downforce_slider": fastest["downforce_slider"],
            "gear_ratio_slider": fastest["gear_ratio_slider"],
            "total_time_ms": fastest["total_time_ms"],
            "configuration_id": fastest["configuration_id"],
        },
        "fastest_is_on_boundary": fastest["downforce_index"] in {0, level_count - 1}
        or fastest["gearing_index"] in {0, level_count - 1},
        "total_time_range_ms": slowest - fastest["total_time_ms"],
        "isolated_downforce_delta_high_minus_low_at_mid_gearing": delta(
            indexed[(level_count - 1, midpoint)], indexed[(0, midpoint)]
        ),
        "isolated_gearing_delta_long_minus_short_at_mid_downforce": delta(
            indexed[(midpoint, level_count - 1)], indexed[(midpoint, 0)]
        ),
    }


def build_report(
    runner: pathlib.Path,
    seed: int,
    level_count: int,
    jobs: int,
) -> dict[str, Any]:
    if not runner.is_file():
        raise ResponseSurfaceError(f"missing runner: {runner}")
    if seed < 0 or seed > 2**64 - 1:
        raise ValueError("seed must be an unsigned 64-bit integer")
    if jobs < 1 or jobs > MAX_JOBS:
        raise ValueError(f"jobs must be between 1 and {MAX_JOBS}")

    levels = slider_levels(level_count)
    base_bytes = BASE_SCENARIO.read_bytes()
    base = json.loads(base_bytes)
    runner_identity = inspect_runner(runner)
    work = []
    with tempfile.TemporaryDirectory(prefix="pitgun-racing-response-") as temporary:
        scenario_root = pathlib.Path(temporary)
        for circuit_index, (circuit_id, archetype, slug) in enumerate(CIRCUITS):
            for downforce_index, downforce in enumerate(levels):
                for gearing_index, gearing in enumerate(levels):
                    scenario_bytes = build_scenario(base, circuit_id, downforce, gearing)
                    work.append(
                        (
                            runner,
                            scenario_root,
                            seed,
                            circuit_index,
                            circuit_id,
                            archetype,
                            slug,
                            downforce_index,
                            downforce,
                            gearing_index,
                            gearing,
                            scenario_bytes,
                        )
                    )

        points = []
        with concurrent.futures.ThreadPoolExecutor(max_workers=jobs) as executor:
            futures = [executor.submit(execute_point, *item) for item in work]
            for completed_count, future in enumerate(
                concurrent.futures.as_completed(futures), start=1
            ):
                points.append(future.result())
                if completed_count % 25 == 0 or completed_count == len(futures):
                    print(
                        f"completed {completed_count}/{len(futures)} simulations",
                        file=sys.stderr,
                    )

    points.sort(key=point_key)
    summaries = [
        summarize_circuit(circuit, points, level_count) for circuit in CIRCUITS
    ]
    for point in points:
        point.pop("circuit_index")
        point.pop("downforce_index")
        point.pop("gearing_index")

    return {
        "schema_version": SCHEMA_VERSION,
        "question": "How do bounded downforce and gearing inputs affect five representative physical circuits?",
        "runner": runner_identity,
        "base_scenario_digest": sha256(base_bytes),
        "seed": str(seed),
        "slider_levels": levels,
        "planned_run_count": len(points),
        "circuits": [
            {"id": circuit_id, "archetype": archetype, "slug": slug}
            for circuit_id, archetype, slug in CIRCUITS
        ],
        "summaries": summaries,
        "points": points,
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--runner",
        type=pathlib.Path,
        default=ROOT / "target" / "release" / "pitgun",
    )
    parser.add_argument("--output", type=pathlib.Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--seed", type=int, default=DEFAULT_SEED)
    parser.add_argument("--levels", type=int, default=DEFAULT_LEVEL_COUNT)
    parser.add_argument(
        "--jobs", type=int, default=min(4, os.cpu_count() or 1, MAX_JOBS)
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="fail when the existing output differs instead of writing it",
    )
    args = parser.parse_args()

    try:
        report = build_report(
            args.runner.resolve(), args.seed, args.levels, args.jobs
        )
        report_bytes = canonical_pretty(report)
        checksum_path = args.output.with_suffix(".sha256")
        checksum_bytes = (
            hashlib.sha256(report_bytes).hexdigest()
            + "  "
            + args.output.name
            + "\n"
        ).encode()
        if args.check:
            if (
                not args.output.is_file()
                or args.output.read_bytes() != report_bytes
                or not checksum_path.is_file()
                or checksum_path.read_bytes() != checksum_bytes
            ):
                raise ResponseSurfaceError(
                    f"response surface is missing or stale: {args.output}"
                )
            print(f"response surface is current: {args.output}")
        else:
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_bytes(report_bytes)
            checksum_path.write_bytes(checksum_bytes)
            print(
                f"wrote {report['planned_run_count']} deterministic points to {args.output}"
            )
    except (OSError, ValueError, ResponseSurfaceError) as error:
        parser.exit(1, f"error: {error}\n")


if __name__ == "__main__":
    main()
