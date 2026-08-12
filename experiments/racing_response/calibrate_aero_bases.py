#!/usr/bin/env python3
"""Calibrate Racing aerodynamic bases around neutral-slider preservation."""

from __future__ import annotations

import argparse
import concurrent.futures
import hashlib
import json
import os
import pathlib
import sys
import tempfile
from typing import Any

import coefficient_screening as screening
import refine_aero_response as refinement


ROOT = pathlib.Path(__file__).resolve().parents[2]
DEFAULT_OUTPUT = (
    ROOT
    / "experiments"
    / "racing_response"
    / "results"
    / "racing-aero-base-calibration-v1.json"
)
SCHEMA_VERSION = "pitgun.racing-aero-base-calibration/v1"
DEFAULT_SEED = 42
DEFAULT_LEVEL_COUNT = 11
MAX_JOBS = 16
SELECTED_DOWNFORCE_GAIN = 0.375
SELECTED_DRAG_GAIN = 0.950
DRAG_BASES = (0.425, 0.475, 0.525, 0.575, 0.625)
DOWNFORCE_BASES = (0.7500, 0.79375, 0.8375, 0.88125, 0.9250)
ANCHOR_ID = "shape-anchor-base-0.8500-0.7500"


class BaseCalibrationError(RuntimeError):
    """Raised when the deterministic aerodynamic base calibration is invalid."""


def neutral_preserving_bases(
    historical: dict[str, Any], downforce_gain: float, drag_gain: float
) -> dict[str, float]:
    midpoint = 0.5
    historical_drag = historical["drag_base"] + historical["drag_slider_gain"] * midpoint
    historical_downforce = (
        historical["downforce_base"]
        + historical["downforce_slider_gain"] * midpoint
    )
    return {
        "drag_base": historical_drag - drag_gain * midpoint,
        "downforce_base": historical_downforce - downforce_gain * midpoint,
    }


def calibration_candidates(base: dict[str, Any]) -> list[dict[str, Any]]:
    anchor = dict(base)
    anchor["downforce_slider_gain"] = SELECTED_DOWNFORCE_GAIN
    anchor["drag_slider_gain"] = SELECTED_DRAG_GAIN
    candidates = [
        {
            "id": refinement.HISTORICAL_ID,
            "kind": "historical_reference",
            "response": dict(base),
        },
        {
            "id": ANCHOR_ID,
            "kind": "shape_anchor_reference",
            "response": anchor,
        },
    ]
    for drag_base in DRAG_BASES:
        for downforce_base in DOWNFORCE_BASES:
            response = dict(anchor)
            response["drag_base"] = drag_base
            response["downforce_base"] = downforce_base
            candidates.append(
                {
                    "id": f"base-drag-{drag_base:.3f}-df-{downforce_base:.5f}",
                    "kind": "base_candidate",
                    "response": response,
                }
            )
    for candidate in candidates:
        response = candidate["response"]
        candidate.update(
            {
                "drag_base": response["drag_base"],
                "downforce_base": response["downforce_base"],
                "drag_slider_gain": response["drag_slider_gain"],
                "downforce_slider_gain": response["downforce_slider_gain"],
            }
        )
    return candidates


def summarize_candidate(
    candidate: dict[str, Any], points: list[dict[str, Any]], level_count: int
) -> dict[str, Any]:
    summary = refinement.summarize_candidate(candidate, points, level_count)
    candidate_points = [
        point for point in points if point["candidate_id"] == candidate["id"]
    ]
    maximum_speed = max(
        point["observed_maximum_speed_kph"] for point in candidate_points
    )
    summary.update(
        {
            "drag_base": candidate["drag_base"],
            "downforce_base": candidate["downforce_base"],
            "maximum_observed_speed_kph": maximum_speed,
            "speed_cap_headroom_kph": 400.0 - maximum_speed,
        }
    )
    return summary


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
    candidates = calibration_candidates(base_response)
    runner_identity = screening.inspect_runner(runner)

    with tempfile.TemporaryDirectory(prefix="pitgun-aero-base-calibration-") as temporary:
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
    summaries = [summarize_candidate(candidate, points, level_count) for candidate in candidates]
    historical = next(
        summary for summary in summaries if summary["candidate_id"] == refinement.HISTORICAL_ID
    )
    anchor = next(summary for summary in summaries if summary["candidate_id"] == ANCHOR_ID)
    refinement.attach_compatibility(anchor, historical)
    candidates_summaries = [
        summary
        for summary in summaries
        if summary["candidate_id"] not in {refinement.HISTORICAL_ID, ANCHOR_ID}
    ]
    anchor_gap = anchor["compatibility_pace"]["root_mean_square_gap_ms"]
    for summary in candidates_summaries:
        refinement.attach_compatibility(summary, historical)
        pace_gap = summary["compatibility_pace"]["root_mean_square_gap_ms"]
        summary["calibration_assessment"] = {
            "eligible": summary["shape_assessment"]["eligible"]
            and summary["speed_cap_headroom_kph"] >= 5.0
            and pace_gap < anchor_gap,
            "shape_eligible": summary["shape_assessment"]["eligible"],
            "speed_cap_headroom_at_least_5_kph": summary[
                "speed_cap_headroom_kph"
            ]
            >= 5.0,
            "improves_shape_anchor_pace_gap": pace_gap < anchor_gap,
            "pace_gap_improvement_vs_anchor_ms": round(anchor_gap - pace_gap, 3),
        }
    candidates_summaries.sort(
        key=lambda summary: (
            not summary["calibration_assessment"]["eligible"],
            summary["compatibility_pace"]["root_mean_square_gap_ms"],
            summary["candidate_id"],
        )
    )
    for rank, summary in enumerate(candidates_summaries, start=1):
        summary["calibration_rank"] = rank

    neutral = neutral_preserving_bases(
        base_response, SELECTED_DOWNFORCE_GAIN, SELECTED_DRAG_GAIN
    )
    return {
        "schema_version": SCHEMA_VERSION,
        "question": "Which bounded aerodynamic bases recover historical gameplay pace without losing the reviewed circuit-dependent response shape?",
        "scope": {
            "status": "offline compatibility calibration, not physical calibration",
            "varied_coefficients": ["drag_base", "downforce_base"],
            "fixed_downforce_slider_gain": SELECTED_DOWNFORCE_GAIN,
            "fixed_drag_slider_gain": SELECTED_DRAG_GAIN,
        },
        "neutral_slider_preservation": {
            "slider": 0.5,
            "historical_drag_blend": 1.0,
            "historical_downforce_blend": 1.025,
            "derived_drag_base": neutral["drag_base"],
            "derived_downforce_base": neutral["downforce_base"],
        },
        "runner": runner_identity,
        "base_scenario_digest": screening.sha256(base_scenario_bytes),
        "base_tuning_response_digest": screening.sha256(base_response_bytes),
        "seed": str(seed),
        "slider_levels": levels,
        "candidate_count": len(candidates_summaries),
        "planned_run_count": len(points),
        "execution_point_set_digest": refinement.point_set_digest(points),
        "eligibility_rule": {
            "response_shape": "same material Monza < Suzuka < Monaco rule as #179",
            "minimum_speed_cap_headroom_kph": 5.0,
            "pace_gap": "strictly below the #179 shape anchor",
        },
        "historical_reference": historical,
        "shape_anchor_reference": anchor,
        "candidates": candidates_summaries,
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
                raise BaseCalibrationError(
                    f"aero base calibration is missing or stale: {args.output}"
                )
            print(f"aero base calibration is current: {args.output}")
        else:
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_bytes(report_bytes)
            checksum_path.write_bytes(checksum_bytes)
            print(f"wrote compact evidence for {report['planned_run_count']} points")
    except (
        OSError,
        ValueError,
        BaseCalibrationError,
        refinement.RefinementError,
        screening.ScreeningError,
    ) as error:
        parser.exit(1, f"error: {error}\n")


if __name__ == "__main__":
    main()
