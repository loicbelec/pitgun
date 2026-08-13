#!/usr/bin/env python3
"""Validate the continuous curvature candidate with band-aware guardrails."""

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
import diagnose_spa_high_speed as high_speed


ROOT = pathlib.Path(__file__).resolve().parents[2]
CANDIDATE_EVIDENCE = (
    ROOT
    / "experiments"
    / "racing_response"
    / "results"
    / "racing-continuous-curvature-response-v1.json"
)
DEFAULT_OUTPUT = (
    ROOT
    / "experiments"
    / "racing_response"
    / "results"
    / "racing-curvature-band-guardrails-v1.json"
)
SCHEMA_VERSION = "pitgun.racing-curvature-band-guardrails/v1"
DEFAULT_SEED = 42
DEFAULT_LEVEL_COUNT = 11
MAX_JOBS = 16


class CurvatureBandGuardrailError(RuntimeError):
    """Raised when the band-aware evidence cannot be trusted."""


def summarize(
    circuit: tuple[str, str, str], points: list[dict[str, Any]], level_count: int
) -> dict[str, Any]:
    circuit_id, archetype, slug = circuit
    selected = [point for point in points if point["circuit_id"] == circuit_id]
    if len(selected) != 2 * level_count:
        raise CurvatureBandGuardrailError(f"incomplete response for {slug}")
    indexed = {
        (point["downforce_slider"], point["gearing_index"]): point
        for point in selected
    }
    deltas = [
        high_speed.response_delta(indexed[(1.0, index)], indexed[(0.0, index)])
        for index in range(level_count)
    ]
    failures = []
    for delta in deltas:
        bands = delta["mean_speed_by_curvature_band_kph"]
        invariants = {
            "high_downforce_reduces_peak_speed": delta["peak_speed_kph"] < 0.0,
            "high_downforce_increases_drag_work": delta[
                "aerodynamic_drag_work_kj"
            ]
            > 0.0,
            "high_downforce_increases_mean_downforce": delta["mean_downforce_n"]
            > 0.0,
            "high_downforce_increases_high_curvature_speed": bands[
                "high_curvature"
            ]
            > 0.0,
        }
        failed = [name for name, passed in invariants.items() if not passed]
        if failed:
            failures.append(
                {
                    "gear_ratio_slider": delta["gear_ratio_slider"],
                    "failed": failed,
                }
            )
    peak = max(selected, key=lambda point: point["peak_speed"]["speed_kph"])
    return {
        "circuit_id": circuit_id,
        "circuit_archetype": archetype,
        "circuit_slug": slug,
        "maximum_observed_speed_kph": peak["peak_speed"]["speed_kph"],
        "maximum_speed_passes": peak["peak_speed"]["speed_kph"] < 400.0,
        "curvature_band_invariant_failures": failures,
        "curvature_band_invariants_pass": not failures,
        "legacy_corner_failure_count": sum(
            delta["legacy_mean_corner_speed_kph"] <= 0.0 for delta in deltas
        ),
        "high_minus_low_downforce": deltas,
    }


def build_report(
    runner: pathlib.Path, seed: int, level_count: int, jobs: int
) -> dict[str, Any]:
    if jobs < 1 or jobs > MAX_JOBS:
        raise ValueError(f"jobs must be between 1 and {MAX_JOBS}")
    if not runner.is_file():
        raise CurvatureBandGuardrailError(f"missing probe runner: {runner}")
    levels = screening.slider_levels(level_count)
    base = json.loads(high_speed.BASE_SCENARIO.read_text())
    points = []
    with tempfile.TemporaryDirectory(prefix="pitgun-curvature-bands-") as temporary:
        root = pathlib.Path(temporary)
        work = []
        for circuit_id, archetype, slug in mixed.ALL_CIRCUITS:
            for downforce in (0.0, 1.0):
                for gearing_index, gearing in enumerate(levels):
                    path = root / (
                        f"{slug}-df-{downforce:.1f}-gear-{gearing:.1f}.json"
                    )
                    path.write_bytes(
                        high_speed.build_scenario(base, circuit_id, downforce, gearing)
                    )
                    work.append(
                        (
                            runner,
                            high_speed.CURRENT_RESPONSE,
                            path,
                            seed,
                            circuit_id,
                            archetype,
                            slug,
                            downforce,
                            gearing_index,
                            gearing,
                            "continuous-v1",
                        )
                    )
        with concurrent.futures.ThreadPoolExecutor(max_workers=jobs) as executor:
            futures = [executor.submit(high_speed.execute_point, *item) for item in work]
            for count, future in enumerate(
                concurrent.futures.as_completed(futures), start=1
            ):
                points.append(future.result())
                if count % 50 == 0 or count == len(futures):
                    print(
                        f"completed {count}/{len(futures)} simulations", file=sys.stderr
                    )
    points.sort(
        key=lambda point: (
            point["circuit_id"],
            point["downforce_slider"],
            point["gearing_index"],
        )
    )
    circuits = [summarize(circuit, points, level_count) for circuit in mixed.ALL_CIRCUITS]
    candidate = json.loads(CANDIDATE_EVIDENCE.read_text())
    candidate_comparison = {
        row["circuit_id"]: row for row in candidate["comparison"]
    }
    suzuka = candidate_comparison["jp-1962"]
    projection = [
        {
            "circuit_id": point["circuit_id"],
            "downforce_slider": point["downforce_slider"],
            "gear_ratio_slider": point["gear_ratio_slider"],
            "experimental_execution_id": point["experimental_execution_id"],
        }
        for point in points
    ]
    return {
        "schema_version": SCHEMA_VERSION,
        "question": (
            "Does the continuous candidate satisfy physical trade-offs when "
            "response is measured by curvature band rather than one binary label?"
        ),
        "status": "experimental_promotion_review",
        "runner": {
            "path": str(runner.relative_to(ROOT)),
            "digest": screening.sha256(runner.read_bytes()),
        },
        "seed": str(seed),
        "gearing_level_count": level_count,
        "planned_run_count": len(points),
        "execution_point_set_digest": screening.sha256(
            screening.canonical_pretty(projection)
        ),
        "guardrail_policy": {
            "per_gearing_required": [
                "high downforce reduces peak speed",
                "high downforce increases aerodynamic drag work",
                "high downforce increases mean downforce",
                "high downforce increases mean speed in the high-curvature band",
            ],
            "informational_only": [
                "near-straight mean speed",
                "low-curvature mean speed",
                "medium-curvature mean speed",
                "legacy binary mean-corner speed",
            ],
            "reason": (
                "Downforce is expected to cost speed outside genuinely tight "
                "corners; requiring every non-straight aggregate to improve "
                "would preserve the obsolete binary abstraction."
            ),
        },
        "source_candidate": {
            "path": str(CANDIDATE_EVIDENCE.relative_to(ROOT)),
            "digest": screening.sha256(CANDIDATE_EVIDENCE.read_bytes()),
        },
        "all_circuit_band_guardrails_pass": all(
            circuit["curvature_band_invariants_pass"] for circuit in circuits
        ),
        "all_circuit_speed_guardrails_pass": all(
            circuit["maximum_speed_passes"] for circuit in circuits
        ),
        "suzuka_optimum_review": {
            "baseline": suzuka["baseline_fastest"],
            "candidate": suzuka["continuous_fastest"],
            "downforce_grid_step": abs(
                suzuka["continuous_fastest"]["downforce_slider"]
                - suzuka["baseline_fastest"]["downforce_slider"]
            ),
            "gearing_grid_step": abs(
                suzuka["continuous_fastest"]["gear_ratio_slider"]
                - suzuka["baseline_fastest"]["gear_ratio_slider"]
            ),
            "classification": "one_step_bounded_movement",
        },
        "decision": {
            "candidate_physically_eligible_for_versioned_promotion": all(
                circuit["curvature_band_invariants_pass"]
                and circuit["maximum_speed_passes"]
                for circuit in circuits
            ),
            "automatic_game_or_catalog_promotion": False,
        },
        "circuits": circuits,
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
        default=ROOT / "target" / "release" / "examples" / "high_speed_response_probe",
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
                raise CurvatureBandGuardrailError("curvature-band evidence is stale")
            print(f"curvature-band evidence is current: {args.output}")
        else:
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_bytes(payload)
            checksum_path.write_bytes(checksum)
            print(
                json.dumps(
                    {
                        "all_circuit_band_guardrails_pass": report[
                            "all_circuit_band_guardrails_pass"
                        ],
                        "all_circuit_speed_guardrails_pass": report[
                            "all_circuit_speed_guardrails_pass"
                        ],
                        "suzuka_optimum_review": report["suzuka_optimum_review"],
                        "decision": report["decision"],
                        "failures": {
                            circuit["circuit_slug"]: circuit[
                                "curvature_band_invariant_failures"
                            ]
                            for circuit in report["circuits"]
                            if circuit["curvature_band_invariant_failures"]
                        },
                    },
                    indent=2,
                )
            )
    except (OSError, ValueError, CurvatureBandGuardrailError) as error:
        parser.exit(1, f"error: {error}\n")


if __name__ == "__main__":
    main()
