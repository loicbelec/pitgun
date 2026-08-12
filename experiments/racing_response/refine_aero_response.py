#!/usr/bin/env python3
"""Refine shortlisted Racing aerodynamic response shapes offline."""

from __future__ import annotations

import argparse
import concurrent.futures
import hashlib
import json
import math
import os
import pathlib
import sys
import tempfile
from typing import Any

import coefficient_screening as screening


ROOT = pathlib.Path(__file__).resolve().parents[2]
DEFAULT_OUTPUT = (
    ROOT
    / "experiments"
    / "racing_response"
    / "results"
    / "racing-aero-response-refinement-v1.json"
)
SCHEMA_VERSION = "pitgun.racing-aero-response-refinement/v1"
DEFAULT_SEED = 42
DEFAULT_LEVEL_COUNT = 11
MAX_JOBS = 16
DOWNFORCE_GAINS = (0.225, 0.250, 0.275, 0.300, 0.325, 0.350, 0.375)
DRAG_GAINS = (0.700, 0.750, 0.800, 0.850, 0.900, 0.950)
HISTORICAL_ID = "historical-df-0.550-drag-0.300"


class RefinementError(RuntimeError):
    """Raised when the deterministic refinement result is invalid."""


def refined_candidates(base: dict[str, Any]) -> list[dict[str, Any]]:
    candidates = [
        {
            "id": HISTORICAL_ID,
            "kind": "historical_reference",
            "downforce_slider_gain": base["downforce_slider_gain"],
            "drag_slider_gain": base["drag_slider_gain"],
            "response": dict(base),
        }
    ]
    for downforce_gain in DOWNFORCE_GAINS:
        for drag_gain in DRAG_GAINS:
            response = dict(base)
            response["downforce_slider_gain"] = downforce_gain
            response["drag_slider_gain"] = drag_gain
            candidates.append(
                {
                    "id": f"refined-df-{downforce_gain:.3f}-drag-{drag_gain:.3f}",
                    "kind": "refined_candidate",
                    "downforce_slider_gain": downforce_gain,
                    "drag_slider_gain": drag_gain,
                    "response": response,
                }
            )
    return candidates


def canonical_compact(value: object) -> bytes:
    return json.dumps(
        value, sort_keys=True, separators=(",", ":"), ensure_ascii=False
    ).encode()


def point_set_digest(points: list[dict[str, Any]]) -> str:
    projection = [
        {
            "candidate_id": point["candidate_id"],
            "circuit_id": point["circuit_id"],
            "downforce_slider": point["downforce_slider"],
            "gear_ratio_slider": point["gear_ratio_slider"],
            "experimental_execution_id": point["experimental_execution_id"],
            "total_time_ms": point["total_time_ms"],
            "observed_maximum_speed_kph": point["observed_maximum_speed_kph"],
            "setup_response": point["setup_response"],
        }
        for point in points
    ]
    return screening.sha256(canonical_compact(projection))


def physical_failures(
    indexed: dict[tuple[int, int], dict[str, Any]], level_count: int
) -> list[dict[str, Any]]:
    failures = []
    for gearing_index in range(level_count):
        delta = screening.point_delta(
            indexed[(level_count - 1, gearing_index)], indexed[(0, gearing_index)]
        )
        invariants = {
            "high_downforce_reduces_maximum_speed": delta[
                "observed_maximum_speed_kph"
            ]
            < 0,
            "high_downforce_increases_corner_speed": delta[
                "mean_corner_speed_kph"
            ]
            > 0,
            "high_downforce_increases_drag_work": delta[
                "aerodynamic_drag_work_kj"
            ]
            > 0,
            "high_downforce_increases_mean_downforce": delta["mean_downforce_n"]
            > 0,
        }
        failed = [name for name, passed in invariants.items() if not passed]
        if failed:
            failures.append(
                {
                    "gear_ratio_slider": indexed[(0, gearing_index)][
                        "gear_ratio_slider"
                    ],
                    "failed": failed,
                }
            )
    return failures


def summarize_circuit(
    circuit: tuple[str, str, str],
    points: list[dict[str, Any]],
    level_count: int,
) -> dict[str, Any]:
    circuit_id, archetype, slug = circuit
    selected = [point for point in points if point["circuit_id"] == circuit_id]
    if len(selected) != level_count * level_count:
        raise RefinementError(f"incomplete refined surface for {slug}")
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
    best_other_downforce = min(
        point["total_time_ms"]
        for point in selected
        if point["downforce_index"] != fastest["downforce_index"]
    )
    midpoint = level_count // 2
    failures = physical_failures(indexed, level_count)
    return {
        "circuit_id": circuit_id,
        "circuit_archetype": archetype,
        "circuit_slug": slug,
        "fastest": {
            "downforce_slider": fastest["downforce_slider"],
            "gear_ratio_slider": fastest["gear_ratio_slider"],
            "total_time_ms": fastest["total_time_ms"],
        },
        "downforce_decision_margin_ms": best_other_downforce
        - fastest["total_time_ms"],
        "physical_invariant_failures": failures,
        "isolated_downforce_delta_high_minus_low_at_mid_gearing": screening.point_delta(
            indexed[(level_count - 1, midpoint)], indexed[(0, midpoint)]
        ),
        "total_time_grid_ms": [
            [indexed[(downforce, gearing)]["total_time_ms"] for gearing in range(level_count)]
            for downforce in range(level_count)
        ],
        "maximum_speed_grid_kph": [
            [
                indexed[(downforce, gearing)]["observed_maximum_speed_kph"]
                for gearing in range(level_count)
            ]
            for downforce in range(level_count)
        ],
    }


def summarize_candidate(
    candidate: dict[str, Any],
    all_points: list[dict[str, Any]],
    level_count: int,
) -> dict[str, Any]:
    points = [point for point in all_points if point["candidate_id"] == candidate["id"]]
    expected = len(screening.CIRCUITS) * level_count * level_count
    if len(points) != expected:
        raise RefinementError(f"incomplete candidate {candidate['id']}")
    response_digests = {point["tuning_response_digest"] for point in points}
    if len(response_digests) != 1:
        raise RefinementError(f"response identity changed for {candidate['id']}")
    circuits = [
        summarize_circuit(circuit, points, level_count)
        for circuit in screening.CIRCUITS
    ]
    optima = {
        circuit["circuit_slug"]: circuit["fastest"]["downforce_slider"]
        for circuit in circuits
    }
    minimum_optimum_separation = min(
        optima["suzuka"] - optima["monza"],
        optima["monaco"] - optima["suzuka"],
    )
    invariant_failure_count = sum(
        len(circuit["physical_invariant_failures"]) for circuit in circuits
    )
    shape_eligible = (
        invariant_failure_count == 0
        and optima["monza"] < optima["suzuka"] < optima["monaco"]
        and 0.0 < optima["suzuka"] < 1.0
        and minimum_optimum_separation + 1e-9 >= 0.2
    )
    return {
        "candidate_id": candidate["id"],
        "kind": candidate["kind"],
        "tuning_response_digest": next(iter(response_digests)),
        "downforce_slider_gain": candidate["downforce_slider_gain"],
        "drag_slider_gain": candidate["drag_slider_gain"],
        "shape_assessment": {
            "eligible": shape_eligible,
            "physical_invariant_failure_count": invariant_failure_count,
            "strict_monza_suzuka_monaco_ordering": optima["monza"]
            < optima["suzuka"]
            < optima["monaco"],
            "suzuka_optimum_is_interior": 0.0 < optima["suzuka"] < 1.0,
            "minimum_adjacent_optimum_separation": minimum_optimum_separation,
            "optima_are_materially_separated": minimum_optimum_separation + 1e-9
            >= 0.2,
        },
        "circuits": circuits,
    }


def attach_compatibility(
    summary: dict[str, Any], historical: dict[str, Any]
) -> None:
    historical_times = {
        circuit["circuit_slug"]: circuit["fastest"]["total_time_ms"]
        for circuit in historical["circuits"]
    }
    gaps = {
        circuit["circuit_slug"]: circuit["fastest"]["total_time_ms"]
        - historical_times[circuit["circuit_slug"]]
        for circuit in summary["circuits"]
    }
    summary["compatibility_pace"] = {
        "reference": HISTORICAL_ID,
        "gap_to_historical_fastest_ms": gaps,
        "root_mean_square_gap_ms": round(
            math.sqrt(sum(gap * gap for gap in gaps.values()) / len(gaps)), 3
        ),
        "meaning": "gameplay compatibility only; not physical calibration",
    }


def build_report(
    runner: pathlib.Path, seed: int, level_count: int, jobs: int
) -> dict[str, Any]:
    if seed < 0 or seed > 2**64 - 1:
        raise ValueError("seed must be an unsigned 64-bit integer")
    if jobs < 1 or jobs > MAX_JOBS:
        raise ValueError(f"jobs must be between 1 and {MAX_JOBS}")
    levels = screening.slider_levels(level_count)
    base_scenario_bytes = screening.BASE_SCENARIO.read_bytes()
    base_scenario = json.loads(base_scenario_bytes)
    base_response_bytes = screening.BASE_RESPONSE.read_bytes()
    base_response = json.loads(base_response_bytes)
    candidates = refined_candidates(base_response)
    runner_identity = screening.inspect_runner(runner)

    with tempfile.TemporaryDirectory(prefix="pitgun-aero-refinement-") as temporary:
        temporary_root = pathlib.Path(temporary)
        response_paths = {}
        for candidate in candidates:
            path = temporary_root / f"{candidate['id']}.json"
            path.write_bytes(screening.canonical_pretty(candidate["response"]))
            response_paths[candidate["id"]] = path
        scenario_paths = {}
        for circuit_index, (circuit_id, _, slug) in enumerate(screening.CIRCUITS):
            for downforce_index, downforce in enumerate(levels):
                for gearing_index, gearing in enumerate(levels):
                    path = temporary_root / (
                        f"scenario-{circuit_index:02d}-{slug}-"
                        f"d{downforce_index:02d}-g{gearing_index:02d}.json"
                    )
                    path.write_bytes(
                        screening.build_scenario(
                            base_scenario, circuit_id, downforce, gearing
                        )
                    )
                    scenario_paths[(circuit_index, downforce_index, gearing_index)] = path

        work = []
        for candidate in candidates:
            for circuit_index, (circuit_id, archetype, slug) in enumerate(
                screening.CIRCUITS
            ):
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
            futures = [executor.submit(screening.execute_point, *item) for item in work]
            for completed_count, future in enumerate(
                concurrent.futures.as_completed(futures), start=1
            ):
                points.append(future.result())
                if completed_count % 500 == 0 or completed_count == len(futures):
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
    summaries = [
        summarize_candidate(candidate, points, level_count) for candidate in candidates
    ]
    historical = next(
        summary for summary in summaries if summary["candidate_id"] == HISTORICAL_ID
    )
    refined = [summary for summary in summaries if summary is not historical]
    for summary in refined:
        attach_compatibility(summary, historical)
    refined.sort(
        key=lambda summary: (
            not summary["shape_assessment"]["eligible"],
            summary["shape_assessment"]["physical_invariant_failure_count"],
            summary["compatibility_pace"]["root_mean_square_gap_ms"],
            summary["candidate_id"],
        )
    )
    for rank, summary in enumerate(refined, start=1):
        summary["refinement_rank"] = rank

    return {
        "schema_version": SCHEMA_VERSION,
        "question": "Which shortlisted aerodynamic response shape remains circuit-dependent on a finer grid with the smallest historical pace disruption?",
        "scope": {
            "status": "offline shape refinement, not physical calibration",
            "varied_coefficients": ["downforce_slider_gain", "drag_slider_gain"],
            "fixed_coefficients": "historical TuningResponseV1 defaults",
        },
        "runner": runner_identity,
        "base_scenario_digest": screening.sha256(base_scenario_bytes),
        "base_tuning_response_digest": screening.sha256(base_response_bytes),
        "seed": str(seed),
        "slider_levels": levels,
        "refined_candidate_count": len(refined),
        "planned_run_count": len(points),
        "execution_point_set_digest": point_set_digest(points),
        "eligibility_rule": {
            "physical_invariant_failure_count": 0,
            "strict_downforce_ordering": "monza < suzuka < monaco",
            "suzuka_optimum": "strictly between 0.0 and 1.0",
            "minimum_adjacent_downforce_optimum_separation": 0.2,
        },
        "randomness_control": {
            "property": "lap noise is common across setup configurations for a fixed seed, driver, and lap",
            "consequence": "setup ranking is unaffected by the additive driver-noise draw",
        },
        "historical_reference": historical,
        "candidates": refined,
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
        report_bytes = screening.canonical_pretty(report)
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
                raise RefinementError(f"aero refinement is missing or stale: {args.output}")
            print(f"aero refinement is current: {args.output}")
        else:
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_bytes(report_bytes)
            checksum_path.write_bytes(checksum_bytes)
            print(f"wrote compact evidence for {report['planned_run_count']} points")
    except (OSError, ValueError, RefinementError, screening.ScreeningError) as error:
        parser.exit(1, f"error: {error}\n")


if __name__ == "__main__":
    main()
