#!/usr/bin/env python3
"""Evaluate the continuous curvature response on the governed seven-circuit grid."""

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
import diagnose_mixed_circuit as mixed
import refine_aero_response as refinement


ROOT = pathlib.Path(__file__).resolve().parents[2]
BASELINE = (
    ROOT
    / "experiments"
    / "racing_response"
    / "results"
    / "racing-mixed-circuit-diagnosis-v1.json"
)
DEFAULT_OUTPUT = (
    ROOT
    / "experiments"
    / "racing_response"
    / "results"
    / "racing-continuous-curvature-response-v1.json"
)
SCHEMA_VERSION = "pitgun.racing-continuous-curvature-response/v1"
DEFAULT_SEED = 42
DEFAULT_LEVEL_COUNT = 11
MAX_JOBS = 16


class ContinuousCurvatureResponseError(RuntimeError):
    """Raised when the governed comparison cannot be trusted."""


def baseline_reference() -> dict[str, Any]:
    report = json.loads(BASELINE.read_text())
    candidates = [
        candidate
        for candidate in report["candidates"]
        if candidate["kind"] == "current_reference"
    ]
    if len(candidates) != 1:
        raise ContinuousCurvatureResponseError(
            "baseline must contain exactly one current reference"
        )
    return candidates[0]


def comparison(
    baseline: dict[str, Any], continuous: dict[str, Any]
) -> list[dict[str, Any]]:
    old = {circuit["circuit_id"]: circuit for circuit in baseline["circuits"]}
    rows = []
    for circuit in continuous["circuits"]:
        previous = old[circuit["circuit_id"]]
        rows.append(
            {
                "circuit_id": circuit["circuit_id"],
                "circuit_slug": circuit["circuit_slug"],
                "baseline_fastest": previous["fastest"],
                "continuous_fastest": circuit["fastest"],
                "optimum_changed": (
                    previous["fastest"]["downforce_slider"],
                    previous["fastest"]["gear_ratio_slider"],
                )
                != (
                    circuit["fastest"]["downforce_slider"],
                    circuit["fastest"]["gear_ratio_slider"],
                ),
                "fastest_time_delta_ms": circuit["fastest"]["total_time_ms"]
                - previous["fastest"]["total_time_ms"],
                "maximum_speed_delta_kph": circuit[
                    "maximum_observed_speed_kph"
                ]
                - previous["maximum_observed_speed_kph"],
                "physical_invariant_failure_delta": circuit[
                    "physical_invariant_failure_count"
                ]
                - previous["physical_invariant_failure_count"],
            }
        )
    return rows


def build_report(
    runner: pathlib.Path, seed: int, level_count: int, jobs: int
) -> dict[str, Any]:
    if jobs < 1 or jobs > MAX_JOBS:
        raise ValueError(f"jobs must be between 1 and {MAX_JOBS}")
    levels = screening.slider_levels(level_count)
    base_scenario = json.loads(screening.BASE_SCENARIO.read_bytes())
    base_response = json.loads(mixed.CURRENT_RESPONSE.read_bytes())
    candidate = mixed.candidate_responses(base_response)[0]
    if candidate["kind"] != "current_reference":
        raise ContinuousCurvatureResponseError("first candidate is not the reference")
    runner_identity = screening.inspect_runner(runner)

    with tempfile.TemporaryDirectory(prefix="pitgun-continuous-curvature-") as temporary:
        root = pathlib.Path(temporary)
        response_path = root / "tuning-response.json"
        response_path.write_bytes(screening.canonical_pretty(candidate["response"]))
        work = []
        for circuit_index, (circuit_id, archetype, slug) in enumerate(
            mixed.ALL_CIRCUITS
        ):
            for downforce_index, downforce in enumerate(levels):
                for gearing_index, gearing in enumerate(levels):
                    scenario_path = root / (
                        f"scenario-{circuit_index:02d}-{slug}-"
                        f"d{downforce_index:02d}-g{gearing_index:02d}.json"
                    )
                    scenario_path.write_bytes(
                        screening.build_scenario(
                            base_scenario, circuit_id, downforce, gearing
                        )
                    )
                    work.append(
                        (
                            runner,
                            seed,
                            candidate,
                            response_path,
                            circuit_index,
                            circuit_id,
                            archetype,
                            slug,
                            downforce_index,
                            downforce,
                            gearing_index,
                            gearing,
                            scenario_path,
                        )
                    )
        points = []
        with concurrent.futures.ThreadPoolExecutor(max_workers=jobs) as executor:
            futures = [executor.submit(screening.execute_point, *item) for item in work]
            for count, future in enumerate(
                concurrent.futures.as_completed(futures), start=1
            ):
                points.append(future.result())
                if count % 100 == 0 or count == len(futures):
                    print(
                        f"completed {count}/{len(futures)} simulations", file=sys.stderr
                    )

    points.sort(
        key=lambda point: (
            point["circuit_index"],
            point["downforce_index"],
            point["gearing_index"],
        )
    )
    summary = mixed.summarize_candidate(candidate, points, level_count)
    baseline = baseline_reference()
    comparisons = comparison(baseline, summary)
    assessment = summary["assessment"]
    point_projection = [
        {
            "circuit_id": point["circuit_id"],
            "downforce_slider": point["downforce_slider"],
            "gear_ratio_slider": point["gear_ratio_slider"],
            "experimental_execution_id": point["experimental_execution_id"],
            "total_time_ms": point["total_time_ms"],
        }
        for point in points
    ]
    return {
        "schema_version": SCHEMA_VERSION,
        "question": (
            "Does one continuous curvature response remove the Spa speed "
            "discontinuity without invalidating the established circuit optima?"
        ),
        "status": "experimental_not_promoted",
        "runner": runner_identity,
        "seed": str(seed),
        "slider_levels": levels,
        "planned_run_count": len(points),
        "execution_point_set_digest": screening.sha256(
            refinement.canonical_compact(point_projection)
        ),
        "curvature_response": {
            "kind": "cubic_smoothstep",
            "absolute_curvature_input": True,
            "full_straight_at_rad_per_m": 0.0,
            "full_corner_at_rad_per_m": 0.001,
            "shared_by": [
                "corner_speed_limit",
                "backward_braking",
                "forward_acceleration",
                "setup_response_diagnostics",
            ],
        },
        "baseline": {
            "schema_version": json.loads(BASELINE.read_text())["schema_version"],
            "digest": screening.sha256(BASELINE.read_bytes()),
            "runner_digest": json.loads(BASELINE.read_text())["runner"]["digest"],
        },
        "assessment": assessment,
        "comparison": comparisons,
        "optimum_change_count": sum(row["optimum_changed"] for row in comparisons),
        "maximum_speed_guardrail": {
            "limit_kph": 400.0,
            "observed_kph": assessment["global_maximum_observed_speed_kph"],
            "passes": assessment["global_maximum_observed_speed_kph"] < 400.0,
        },
        "decision": {
            "candidate_passes_governed_grid": assessment["eligible"],
            "automatic_game_or_catalog_promotion": False,
        },
        "continuous_candidate": summary,
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
        default=(
            ROOT
            / "target"
            / "release"
            / "examples"
            / "continuous_curvature_response_probe"
        ),
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
                raise ContinuousCurvatureResponseError(
                    "continuous curvature evidence is stale"
                )
            print(f"continuous curvature evidence is current: {args.output}")
        else:
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_bytes(payload)
            checksum_path.write_bytes(checksum)
            print(
                json.dumps(
                    {
                        "assessment": report["assessment"],
                        "optimum_change_count": report["optimum_change_count"],
                        "maximum_speed_guardrail": report[
                            "maximum_speed_guardrail"
                        ],
                        "comparison": report["comparison"],
                    },
                    indent=2,
                )
            )
    except (OSError, ValueError, ContinuousCurvatureResponseError) as error:
        parser.exit(1, f"error: {error}\n")


if __name__ == "__main__":
    main()
