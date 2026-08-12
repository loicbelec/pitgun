#!/usr/bin/env python3
"""Screen bounded aerodynamic tuning-response coefficients offline."""

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
BASE_RESPONSE = (
    ROOT / "experiments" / "racing_response" / "tuning-response-v1.default.json"
)
DEFAULT_OUTPUT = (
    ROOT
    / "experiments"
    / "racing_response"
    / "results"
    / "racing-coefficient-screening-v1.json"
)
SCHEMA_VERSION = "pitgun.racing-coefficient-screening/v1"
DEFAULT_SEED = 42
DEFAULT_LEVEL_COUNT = 5
MAX_JOBS = 16

CIRCUITS = (
    ("it-1922", "power", "monza"),
    ("mc-1929", "high-downforce", "monaco"),
    ("jp-1962", "mixed", "suzuka"),
)
DOWNFORCE_GAINS = (0.15, 0.25, 0.35, 0.45, 0.55)
DRAG_GAINS = (0.30, 0.45, 0.60, 0.75, 0.90)


class ScreeningError(RuntimeError):
    """Raised when a bounded coefficient-screening run is invalid."""


def canonical_pretty(value: object) -> bytes:
    return (json.dumps(value, indent=2, ensure_ascii=False) + "\n").encode()


def sha256(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


def slider_levels(count: int) -> list[float]:
    if count < 3 or count > 11 or count % 2 == 0:
        raise ValueError("level count must be an odd integer between 3 and 11")
    return [round(index / (count - 1), 6) for index in range(count)]


def candidate_responses(base: dict[str, Any]) -> list[dict[str, Any]]:
    candidates = []
    for downforce_gain in DOWNFORCE_GAINS:
        for drag_gain in DRAG_GAINS:
            response = dict(base)
            response["downforce_slider_gain"] = downforce_gain
            response["drag_slider_gain"] = drag_gain
            candidates.append(
                {
                    "id": f"df-{downforce_gain:.2f}-drag-{drag_gain:.2f}",
                    "downforce_slider_gain": downforce_gain,
                    "drag_slider_gain": drag_gain,
                    "response": response,
                }
            )
    return candidates


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
    if not runner.is_file():
        raise ScreeningError(f"missing probe runner: {runner}")
    return {"path": str(runner.relative_to(ROOT)), "digest": sha256(runner.read_bytes())}


def execute_point(
    runner: pathlib.Path,
    seed: int,
    candidate: dict[str, Any],
    response_path: pathlib.Path,
    circuit_index: int,
    circuit_id: str,
    archetype: str,
    slug: str,
    downforce_index: int,
    downforce: float,
    gearing_index: int,
    gearing: float,
    scenario_path: pathlib.Path,
) -> dict[str, Any]:
    try:
        completed = subprocess.run(
            [str(runner), str(scenario_path), str(response_path), str(seed)],
            cwd=ROOT,
            check=False,
            capture_output=True,
            timeout=120,
        )
    except (OSError, subprocess.SubprocessError) as error:
        raise ScreeningError(
            f"probe failed for {candidate['id']} {slug} "
            f"downforce={downforce} gearing={gearing}: {error}"
        ) from error
    if completed.returncode != 0:
        raise ScreeningError(
            f"probe exited {completed.returncode} for {candidate['id']} {slug}: "
            f"{completed.stderr.decode(errors='replace').strip()}"
        )
    if completed.stderr:
        raise ScreeningError(
            f"successful probe wrote to stderr for {candidate['id']} {slug}"
        )
    try:
        result = json.loads(completed.stdout)
        diagnostics = result["setup_response"]
    except (UnicodeDecodeError, json.JSONDecodeError, KeyError, TypeError) as error:
        raise ScreeningError("probe returned an invalid diagnostic result") from error
    if result.get("schema_version") != "pitgun.racing-tuning-response-probe/v1":
        raise ScreeningError("probe returned an unsupported result")
    return {
        "candidate_id": candidate["id"],
        "circuit_index": circuit_index,
        "circuit_id": circuit_id,
        "circuit_archetype": archetype,
        "circuit_slug": slug,
        "downforce_index": downforce_index,
        "downforce_slider": downforce,
        "gearing_index": gearing_index,
        "gear_ratio_slider": gearing,
        "experimental_execution_id": result["experimental_execution_id"],
        "scenario_digest": result["scenario_digest"],
        "tuning_response_digest": result["tuning_response_digest"],
        "total_time_ms": result["total_time_ms"],
        "observed_maximum_speed_kph": result["observed_maximum_speed_kph"],
        "setup_response": diagnostics,
    }


def point_delta(high: dict[str, Any], low: dict[str, Any]) -> dict[str, float | int]:
    high_response = high["setup_response"]
    low_response = low["setup_response"]
    return {
        "total_time_ms": high["total_time_ms"] - low["total_time_ms"],
        "observed_maximum_speed_kph": high["observed_maximum_speed_kph"]
        - low["observed_maximum_speed_kph"],
        "mean_corner_speed_kph": high_response["mean_corner_speed_kph"]
        - low_response["mean_corner_speed_kph"],
        "aerodynamic_drag_work_kj": high_response["aerodynamic_drag_work_kj"]
        - low_response["aerodynamic_drag_work_kj"],
        "mean_downforce_n": high_response["mean_downforce_n"]
        - low_response["mean_downforce_n"],
    }


def summarize_candidate(
    candidate: dict[str, Any], points: list[dict[str, Any]], level_count: int
) -> dict[str, Any]:
    candidate_points = [point for point in points if point["candidate_id"] == candidate["id"]]
    expected = len(CIRCUITS) * level_count * level_count
    if len(candidate_points) != expected:
        raise ScreeningError(f"incomplete surface for {candidate['id']}")
    midpoint = level_count // 2
    circuits = []
    for circuit_id, archetype, slug in CIRCUITS:
        selected = [point for point in candidate_points if point["circuit_id"] == circuit_id]
        indexed = {
            (point["downforce_index"], point["gearing_index"]): point
            for point in selected
        }
        fastest = min(
            selected,
            key=lambda point: (
                point["total_time_ms"],
                point["downforce_index"],
                point["gearing_index"],
            ),
        )
        downforce_delta = point_delta(
            indexed[(level_count - 1, midpoint)], indexed[(0, midpoint)]
        )
        circuits.append(
            {
                "circuit_id": circuit_id,
                "circuit_archetype": archetype,
                "circuit_slug": slug,
                "fastest": {
                    "downforce_slider": fastest["downforce_slider"],
                    "gear_ratio_slider": fastest["gear_ratio_slider"],
                    "total_time_ms": fastest["total_time_ms"],
                },
                "fastest_downforce_is_on_boundary": fastest["downforce_index"]
                in {0, level_count - 1},
                "isolated_downforce_delta_high_minus_low_at_mid_gearing": downforce_delta,
                "physical_invariants": {
                    "high_downforce_reduces_maximum_speed": downforce_delta[
                        "observed_maximum_speed_kph"
                    ]
                    < 0,
                    "high_downforce_increases_corner_speed": downforce_delta[
                        "mean_corner_speed_kph"
                    ]
                    > 0,
                    "high_downforce_increases_drag_work": downforce_delta[
                        "aerodynamic_drag_work_kj"
                    ]
                    > 0,
                    "high_downforce_increases_mean_downforce": downforce_delta[
                        "mean_downforce_n"
                    ]
                    > 0,
                },
            }
        )
    optima = [circuit["fastest"]["downforce_slider"] for circuit in circuits]
    invariant_failures = sum(
        not passed
        for circuit in circuits
        for passed in circuit["physical_invariants"].values()
    )
    boundary_count = sum(
        circuit["fastest_downforce_is_on_boundary"] for circuit in circuits
    )
    interior_count = len(circuits) - boundary_count
    distinct_optimum_count = len(set(optima))
    optimum_range = max(optima) - min(optima)
    eligible = (
        invariant_failures == 0
        and interior_count >= 1
        and distinct_optimum_count == len(CIRCUITS)
        and optimum_range >= 0.25
    )
    return {
        "candidate_id": candidate["id"],
        "downforce_slider_gain": candidate["downforce_slider_gain"],
        "drag_slider_gain": candidate["drag_slider_gain"],
        "assessment": {
            "eligible_for_deeper_calibration": eligible,
            "physical_invariant_failure_count": invariant_failures,
            "downforce_boundary_optimum_count": boundary_count,
            "downforce_interior_optimum_count": interior_count,
            "distinct_downforce_optimum_count": distinct_optimum_count,
            "downforce_optimum_range": optimum_range,
        },
        "circuits": circuits,
    }


def build_report(
    runner: pathlib.Path, seed: int, level_count: int, jobs: int
) -> dict[str, Any]:
    if seed < 0 or seed > 2**64 - 1:
        raise ValueError("seed must be an unsigned 64-bit integer")
    if jobs < 1 or jobs > MAX_JOBS:
        raise ValueError(f"jobs must be between 1 and {MAX_JOBS}")
    levels = slider_levels(level_count)
    base_scenario_bytes = BASE_SCENARIO.read_bytes()
    base_scenario = json.loads(base_scenario_bytes)
    base_response_bytes = BASE_RESPONSE.read_bytes()
    base_response = json.loads(base_response_bytes)
    candidates = candidate_responses(base_response)
    runner_identity = inspect_runner(runner)

    with tempfile.TemporaryDirectory(prefix="pitgun-coefficient-screening-") as temporary:
        temporary_root = pathlib.Path(temporary)
        response_paths = {}
        for candidate in candidates:
            path = temporary_root / f"{candidate['id']}.json"
            path.write_bytes(canonical_pretty(candidate["response"]))
            response_paths[candidate["id"]] = path
        scenario_paths = {}
        for circuit_index, (circuit_id, _, slug) in enumerate(CIRCUITS):
            for downforce_index, downforce in enumerate(levels):
                for gearing_index, gearing in enumerate(levels):
                    path = temporary_root / (
                        f"scenario-{circuit_index:02d}-{slug}-"
                        f"d{downforce_index:02d}-g{gearing_index:02d}.json"
                    )
                    path.write_bytes(
                        build_scenario(base_scenario, circuit_id, downforce, gearing)
                    )
                    scenario_paths[(circuit_index, downforce_index, gearing_index)] = path

        work = []
        for candidate in candidates:
            for circuit_index, (circuit_id, archetype, slug) in enumerate(CIRCUITS):
                for downforce_index, downforce in enumerate(levels):
                    for gearing_index, gearing in enumerate(levels):
                        work.append(
                            (
                                runner,
                                seed,
                                candidate,
                                response_paths[candidate["id"]],
                                circuit_index,
                                circuit_id,
                                archetype,
                                slug,
                                downforce_index,
                                downforce,
                                gearing_index,
                                gearing,
                                scenario_paths[(circuit_index, downforce_index, gearing_index)],
                            )
                        )
        points = []
        with concurrent.futures.ThreadPoolExecutor(max_workers=jobs) as executor:
            futures = [executor.submit(execute_point, *item) for item in work]
            for completed_count, future in enumerate(
                concurrent.futures.as_completed(futures), start=1
            ):
                points.append(future.result())
                if completed_count % 100 == 0 or completed_count == len(futures):
                    print(
                        f"completed {completed_count}/{len(futures)} simulations",
                        file=sys.stderr,
                    )

    points.sort(
        key=lambda point: (
            point["candidate_id"],
            point["circuit_index"],
            point["downforce_index"],
            point["gearing_index"],
        )
    )
    summaries = [summarize_candidate(candidate, points, level_count) for candidate in candidates]
    summaries.sort(
        key=lambda summary: (
            not summary["assessment"]["eligible_for_deeper_calibration"],
            summary["assessment"]["physical_invariant_failure_count"],
            -summary["assessment"]["distinct_downforce_optimum_count"],
            summary["assessment"]["downforce_boundary_optimum_count"],
            -summary["assessment"]["downforce_optimum_range"],
            summary["candidate_id"],
        )
    )
    for rank, summary in enumerate(summaries, start=1):
        summary["screening_rank"] = rank
    for point in points:
        point.pop("circuit_index")
        point.pop("downforce_index")
        point.pop("gearing_index")

    return {
        "schema_version": SCHEMA_VERSION,
        "question": "Which bounded aerodynamic response families create physically coherent, circuit-dependent setup optima?",
        "scope": {
            "status": "offline screening, not physical calibration",
            "varied_coefficients": ["downforce_slider_gain", "drag_slider_gain"],
            "fixed_coefficients": "historical TuningResponseV1 defaults",
        },
        "runner": runner_identity,
        "base_scenario_digest": sha256(base_scenario_bytes),
        "base_tuning_response_digest": sha256(base_response_bytes),
        "seed": str(seed),
        "slider_levels": levels,
        "candidate_count": len(candidates),
        "planned_run_count": len(points),
        "eligibility_rule": {
            "physical_invariant_failure_count": 0,
            "minimum_downforce_interior_optimum_count": 1,
            "minimum_distinct_downforce_optimum_count": len(CIRCUITS),
            "minimum_downforce_optimum_range": 0.25,
        },
        "candidates": [
            {
                key: value
                for key, value in candidate.items()
                if key != "response"
            }
            for candidate in candidates
        ],
        "summaries": summaries,
        "points": points,
    }


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
        report = build_report(args.runner.resolve(), args.seed, args.levels, args.jobs)
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
                raise ScreeningError(f"coefficient screening is missing or stale: {args.output}")
            print(f"coefficient screening is current: {args.output}")
        else:
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_bytes(report_bytes)
            checksum_path.write_bytes(checksum_bytes)
            print(f"wrote {report['planned_run_count']} deterministic points to {args.output}")
    except (OSError, ValueError, ScreeningError) as error:
        parser.exit(1, f"error: {error}\n")


if __name__ == "__main__":
    main()
