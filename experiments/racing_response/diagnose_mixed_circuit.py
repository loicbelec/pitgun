#!/usr/bin/env python3
"""Diagnose the mixed-circuit review failure before changing physics."""

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
CURRENT_RESPONSE = (
    ROOT / "experiments" / "databricks" / "responses" / "racing-aero-candidate-v1.json"
)
CAMPAIGN = (
    ROOT / "experiments" / "databricks" / "campaigns" / "racing-circuit-sweep-v1.json"
)
DEFAULT_OUTPUT = (
    ROOT
    / "experiments"
    / "racing_response"
    / "results"
    / "racing-mixed-circuit-diagnosis-v1.json"
)
SCHEMA_VERSION = "pitgun.racing-mixed-circuit-diagnosis/v1"
DEFAULT_SEED = 42
DEFAULT_LEVEL_COUNT = 11
MAX_JOBS = 16
CURRENT_DOWNFORCE_GAIN = 0.375
CURRENT_CORNER_SCALE = 1.05
DOWNFORCE_GAINS = (0.375, 0.3875, 0.4000, 0.4125, 0.4250, 0.4375, 0.4500)
CORNER_SCALES = (1.0500, 1.0625, 1.0750, 1.0875, 1.1000)
CALIBRATION_CIRCUITS = (
    ("it-1922", "power", "monza"),
    ("mc-1929", "high-downforce", "monaco"),
    ("hu-1986", "mechanical-grip", "budapest"),
    ("jp-1962", "mixed", "suzuka"),
    ("sg-2008", "street-thermal", "singapore"),
)
HOLDOUT_CIRCUITS = (
    ("gb-1948", "mixed-fast", "silverstone"),
    ("be-1925", "mixed-elevation", "spa"),
)
ALL_CIRCUITS = CALIBRATION_CIRCUITS + HOLDOUT_CIRCUITS
EXPECTED_DOWNFORCE_BOUNDS = {
    "it-1922": (0.0, 0.2),
    "mc-1929": (0.8, 1.0),
    "hu-1986": (0.7, 1.0),
    "jp-1962": (0.25, 0.55),
    "sg-2008": (0.7, 1.0),
}


class MixedCircuitDiagnosisError(RuntimeError):
    """Raised when the deterministic diagnosis cannot be trusted."""


def candidate_responses(base: dict[str, Any]) -> list[dict[str, Any]]:
    candidates = []
    for downforce_gain in DOWNFORCE_GAINS:
        for corner_scale in CORNER_SCALES:
            response = dict(base)
            response["downforce_slider_gain"] = downforce_gain
            response["corner_aero_scale"] = corner_scale
            candidates.append(
                {
                    "id": f"mixed-df-{downforce_gain:.4f}-corner-{corner_scale:.4f}",
                    "kind": "current_reference"
                    if downforce_gain == CURRENT_DOWNFORCE_GAIN
                    and corner_scale == CURRENT_CORNER_SCALE
                    else "bounded_diagnostic",
                    "downforce_slider_gain": downforce_gain,
                    "corner_aero_scale": corner_scale,
                    "coefficient_distance_from_current": round(
                        abs(downforce_gain - CURRENT_DOWNFORCE_GAIN)
                        + abs(corner_scale - CURRENT_CORNER_SCALE),
                        6,
                    ),
                    "response": response,
                }
            )
    return candidates


def summarize_circuit(
    circuit: tuple[str, str, str], points: list[dict[str, Any]], level_count: int
) -> dict[str, Any]:
    circuit_id, archetype, slug = circuit
    selected = [point for point in points if point["circuit_id"] == circuit_id]
    if len(selected) != level_count * level_count:
        raise MixedCircuitDiagnosisError(f"incomplete surface for {slug}")
    indexed = {
        (point["downforce_index"], point["gearing_index"]): point for point in selected
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
    return {
        "circuit_id": circuit_id,
        "circuit_archetype": archetype,
        "circuit_slug": slug,
        "fastest": {
            "downforce_slider": fastest["downforce_slider"],
            "gear_ratio_slider": fastest["gear_ratio_slider"],
            "total_time_ms": fastest["total_time_ms"],
        },
        "downforce_decision_margin_ms": best_other_downforce - fastest["total_time_ms"],
        "maximum_observed_speed_kph": max(
            point["observed_maximum_speed_kph"] for point in selected
        ),
        "physical_invariant_failure_count": len(
            refinement.physical_failures(indexed, level_count)
        ),
    }


def summarize_candidate(
    candidate: dict[str, Any], points: list[dict[str, Any]], level_count: int
) -> dict[str, Any]:
    selected = [point for point in points if point["candidate_id"] == candidate["id"]]
    expected = len(ALL_CIRCUITS) * level_count * level_count
    if len(selected) != expected:
        raise MixedCircuitDiagnosisError(f"incomplete candidate {candidate['id']}")
    circuits = [
        summarize_circuit(circuit, selected, level_count) for circuit in ALL_CIRCUITS
    ]
    assessment = assess_circuits(circuits)
    return {
        "candidate_id": candidate["id"],
        "kind": candidate["kind"],
        "tuning_response_digest": selected[0]["tuning_response_digest"],
        "downforce_slider_gain": candidate["downforce_slider_gain"],
        "corner_aero_scale": candidate["corner_aero_scale"],
        "coefficient_distance_from_current": candidate[
            "coefficient_distance_from_current"
        ],
        "assessment": assessment,
        "circuits": circuits,
    }


def assess_circuits(circuits: list[dict[str, Any]]) -> dict[str, Any]:
    """Separate calibration eligibility from post-selection holdout findings."""

    by_id = {circuit["circuit_id"]: circuit for circuit in circuits}
    bound_checks = {
        circuit_id: lower - 1e-9
        <= by_id[circuit_id]["fastest"]["downforce_slider"]
        <= upper + 1e-9
        for circuit_id, (lower, upper) in EXPECTED_DOWNFORCE_BOUNDS.items()
    }
    calibration_ids = {circuit[0] for circuit in CALIBRATION_CIRCUITS}
    calibration_circuits = [
        circuit for circuit in circuits if circuit["circuit_id"] in calibration_ids
    ]
    calibration_invariant_failures = sum(
        circuit["physical_invariant_failure_count"] for circuit in calibration_circuits
    )
    calibration_maximum_speed = max(
        circuit["maximum_observed_speed_kph"] for circuit in calibration_circuits
    )
    global_invariant_failures = sum(
        circuit["physical_invariant_failure_count"] for circuit in circuits
    )
    global_maximum_speed = max(
        circuit["maximum_observed_speed_kph"] for circuit in circuits
    )
    calibration_eligible = (
        all(bound_checks.values())
        and calibration_invariant_failures == 0
        and calibration_maximum_speed < 400.0
    )
    global_guardrails_pass = (
        global_invariant_failures == 0 and global_maximum_speed < 400.0
    )
    return {
        "calibration_eligible": calibration_eligible,
        "global_guardrails_pass": global_guardrails_pass,
        "eligible": calibration_eligible and global_guardrails_pass,
        "continuous_optimum_within_expected_bounds": bound_checks,
        "calibration_physical_invariant_failure_count": calibration_invariant_failures,
        "global_physical_invariant_failure_count": global_invariant_failures,
        "calibration_maximum_observed_speed_kph": calibration_maximum_speed,
        "global_maximum_observed_speed_kph": global_maximum_speed,
        "global_speed_cap_headroom_kph": 400.0 - global_maximum_speed,
    }


def diagnose_review_grid(
    current_points: list[dict[str, Any]], level_count: int
) -> dict[str, Any]:
    campaign = json.loads(CAMPAIGN.read_text())
    configurations = [
        configuration
        for configuration in campaign["configurations"]
        if configuration["circuit_id"] == "jp-1962"
    ]
    indexed = {
        (point["downforce_slider"], point["gear_ratio_slider"]): point
        for point in current_points
        if point["circuit_id"] == "jp-1962"
    }
    discrete = []
    for configuration in configurations:
        setup = configuration["setup"]
        point = indexed[(setup["downforce_slider"], setup["gear_ratio_slider"])]
        discrete.append(
            {
                "configuration_family": configuration["configuration_family"],
                "downforce_slider": setup["downforce_slider"],
                "gear_ratio_slider": setup["gear_ratio_slider"],
                "total_time_ms": point["total_time_ms"],
            }
        )
    discrete.sort(key=lambda row: (row["total_time_ms"], row["configuration_family"]))
    continuous = min(
        (point for point in current_points if point["circuit_id"] == "jp-1962"),
        key=lambda point: (
            point["total_time_ms"],
            point["downforce_index"],
            point["gearing_index"],
        ),
    )
    low = next(
        row for row in discrete if row["configuration_family"] == "low-downforce"
    )
    balanced = next(
        row for row in discrete if row["configuration_family"] == "balanced"
    )
    optimum = continuous["downforce_slider"]
    classification = (
        "review-grid-aliasing"
        if discrete[0]["configuration_family"].startswith("low-downforce")
        and abs(low["downforce_slider"] - optimum)
        < abs(balanced["downforce_slider"] - optimum)
        and EXPECTED_DOWNFORCE_BOUNDS["jp-1962"][0]
        <= optimum
        <= EXPECTED_DOWNFORCE_BOUNDS["jp-1962"][1]
        else "physics-response-mismatch"
    )
    return {
        "classification": classification,
        "continuous_optimum": {
            "downforce_slider": optimum,
            "gear_ratio_slider": continuous["gear_ratio_slider"],
            "total_time_ms": continuous["total_time_ms"],
        },
        "reviewed_configuration_ranking": discrete,
        "distance_to_continuous_downforce_optimum": {
            "low-downforce": abs(low["downforce_slider"] - optimum),
            "balanced": abs(balanced["downforce_slider"] - optimum),
        },
        "meaning": "A coarse named setup family must not override the continuous physical optimum.",
    }


def build_report(
    runner: pathlib.Path, seed: int, level_count: int, jobs: int
) -> dict[str, Any]:
    if jobs < 1 or jobs > MAX_JOBS:
        raise ValueError(f"jobs must be between 1 and {MAX_JOBS}")
    levels = screening.slider_levels(level_count)
    base_scenario = json.loads(screening.BASE_SCENARIO.read_bytes())
    base_response_bytes = CURRENT_RESPONSE.read_bytes()
    base_response = json.loads(base_response_bytes)
    candidates = candidate_responses(base_response)
    runner_identity = screening.inspect_runner(runner)

    with tempfile.TemporaryDirectory(prefix="pitgun-mixed-diagnosis-") as temporary:
        root = pathlib.Path(temporary)
        response_paths = {}
        for candidate in candidates:
            path = root / f"{candidate['id']}.json"
            path.write_bytes(screening.canonical_pretty(candidate["response"]))
            response_paths[candidate["id"]] = path
        scenario_paths = {}
        for circuit_index, (circuit_id, _, slug) in enumerate(ALL_CIRCUITS):
            for downforce_index, downforce in enumerate(levels):
                for gearing_index, gearing in enumerate(levels):
                    path = root / (
                        f"scenario-{circuit_index:02d}-{slug}-"
                        f"d{downforce_index:02d}-g{gearing_index:02d}.json"
                    )
                    path.write_bytes(
                        screening.build_scenario(
                            base_scenario, circuit_id, downforce, gearing
                        )
                    )
                    scenario_paths[(circuit_index, downforce_index, gearing_index)] = (
                        path
                    )
        work = []
        for candidate in candidates:
            for circuit_index, (circuit_id, archetype, slug) in enumerate(ALL_CIRCUITS):
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
                                scenario_paths[
                                    (circuit_index, downforce_index, gearing_index)
                                ],
                            )
                        )
        points = []
        with concurrent.futures.ThreadPoolExecutor(max_workers=jobs) as executor:
            futures = [executor.submit(screening.execute_point, *item) for item in work]
            for count, future in enumerate(
                concurrent.futures.as_completed(futures), start=1
            ):
                points.append(future.result())
                if count % 1000 == 0 or count == len(futures):
                    print(
                        f"completed {count}/{len(futures)} simulations", file=sys.stderr
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
    calibration_eligible = [
        summary
        for summary in summaries
        if summary["assessment"]["calibration_eligible"]
    ]
    calibration_eligible.sort(
        key=lambda summary: (
            summary["coefficient_distance_from_current"],
            summary["candidate_id"],
        )
    )
    selected = calibration_eligible[0] if calibration_eligible else None
    current_id = candidate_responses(base_response)[0]["id"]
    current_points = [point for point in points if point["candidate_id"] == current_id]
    diagnosis = diagnose_review_grid(current_points, level_count)
    if (
        selected
        and selected["kind"] == "current_reference"
        and diagnosis["classification"] == "review-grid-aliasing"
        and selected["assessment"]["global_guardrails_pass"]
    ):
        recommendation = "KEEP_COEFFICIENTS_REFINE_REVIEW_GRID"
    elif (
        selected
        and selected["kind"] == "current_reference"
        and diagnosis["classification"] == "review-grid-aliasing"
    ):
        recommendation = "KEEP_MIXED_RESPONSE_OPEN_GLOBAL_GUARDRAIL_REFINEMENT"
    else:
        recommendation = "REFINE_MIXED_RESPONSE"
    point_projection = [
        {
            "candidate_id": point["candidate_id"],
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
        "question": "Does the Suzuka review failure require a physics change or a finer review grid?",
        "runner": runner_identity,
        "seed": str(seed),
        "slider_levels": levels,
        "candidate_count": len(candidates),
        "calibration_circuit_count": len(CALIBRATION_CIRCUITS),
        "holdout_circuit_count": len(HOLDOUT_CIRCUITS),
        "planned_run_count": len(points),
        "execution_point_set_digest": screening.sha256(
            refinement.canonical_compact(point_projection)
        ),
        "parameter_space": {
            "downforce_slider_gain": list(DOWNFORCE_GAINS),
            "corner_aero_scale": list(CORNER_SCALES),
            "fixed_drag_base": base_response["drag_base"],
            "fixed_drag_slider_gain": base_response["drag_slider_gain"],
            "fixed_downforce_base": base_response["downforce_base"],
        },
        "expected_continuous_downforce_bounds": EXPECTED_DOWNFORCE_BOUNDS,
        "review_grid_diagnosis": diagnosis,
        "calibration_eligible_candidate_count": len(calibration_eligible),
        "globally_eligible_candidate_count": sum(
            summary["assessment"]["eligible"] for summary in summaries
        ),
        "selected_calibration_candidate": selected,
        "holdout_findings": {
            "meaning": "Observed only after calibration selection; these findings cannot improve a candidate rank.",
            "selected_candidate_global_guardrails_pass": selected["assessment"][
                "global_guardrails_pass"
            ]
            if selected
            else False,
            "selected_candidate_global_speed_cap_headroom_kph": selected["assessment"][
                "global_speed_cap_headroom_kph"
            ]
            if selected
            else None,
        },
        "recommendation": recommendation,
        "automatic_model_change": False,
        "candidates": summaries,
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
                raise MixedCircuitDiagnosisError("mixed-circuit diagnosis is stale")
            print(f"mixed-circuit diagnosis is current: {args.output}")
        else:
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_bytes(payload)
            checksum_path.write_bytes(checksum)
            print(
                f"wrote diagnosis for {report['planned_run_count']} deterministic points"
            )
    except (
        OSError,
        ValueError,
        MixedCircuitDiagnosisError,
        screening.ScreeningError,
    ) as error:
        parser.exit(1, f"error: {error}\n")


if __name__ == "__main__":
    main()
