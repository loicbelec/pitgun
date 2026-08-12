#!/usr/bin/env python3
"""Diagnose the Spa high-speed holdout without changing Racing physics."""

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

import coefficient_screening as screening


ROOT = pathlib.Path(__file__).resolve().parents[2]
BASE_SCENARIO = (
    ROOT / "apps" / "pitgun-cli" / "scenarios" / "racing-batch-v1" / "balanced.json"
)
CURRENT_RESPONSE = (
    ROOT / "experiments" / "databricks" / "responses" / "racing-aero-candidate-v1.json"
)
MIXED_CIRCUIT_DIAGNOSIS = (
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
    / "racing-spa-high-speed-diagnosis-v1.json"
)
SCHEMA_VERSION = "pitgun.racing-spa-high-speed-diagnosis/v1"
DEFAULT_SEED = 42
DEFAULT_LEVEL_COUNT = 11
MAX_JOBS = 16
SPEED_GUARDRAIL_KPH = 400.0
CIRCUITS = (
    ("jp-1962", "mixed", "suzuka"),
    ("gb-1948", "mixed-fast", "silverstone"),
    ("be-1925", "mixed-elevation", "spa"),
)


class SpaDiagnosisError(RuntimeError):
    """Raised when the deterministic Spa diagnosis cannot be trusted."""


def build_scenario(
    base: dict[str, Any], circuit_id: str, downforce: float, gearing: float
) -> bytes:
    scenario = json.loads(json.dumps(base))
    scenario["request"]["track_id"] = circuit_id
    tuning = scenario["request"]["competitors"][0]["tuning"]
    tuning["downforce_slider"] = downforce
    tuning["gear_ratio_slider"] = gearing
    return screening.canonical_pretty(scenario)


def execute_point(
    runner: pathlib.Path,
    response_path: pathlib.Path,
    scenario_path: pathlib.Path,
    seed: int,
    circuit_id: str,
    archetype: str,
    slug: str,
    downforce: float,
    gearing_index: int,
    gearing: float,
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
        raise SpaDiagnosisError(
            f"probe failed for {slug} downforce={downforce} gearing={gearing}: {error}"
        ) from error
    if completed.returncode != 0:
        raise SpaDiagnosisError(
            f"probe exited {completed.returncode} for {slug}: "
            f"{completed.stderr.decode(errors='replace').strip()}"
        )
    if completed.stderr:
        raise SpaDiagnosisError(f"successful probe wrote to stderr for {slug}")
    try:
        result = json.loads(completed.stdout)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise SpaDiagnosisError("probe returned invalid JSON") from error
    if result.get("schema_version") != "pitgun.racing-high-speed-response-probe/v1":
        raise SpaDiagnosisError("probe returned an unsupported result")
    return {
        "circuit_id": circuit_id,
        "circuit_archetype": archetype,
        "circuit_slug": slug,
        "downforce_slider": downforce,
        "gearing_index": gearing_index,
        "gear_ratio_slider": gearing,
        "experimental_execution_id": result["experimental_execution_id"],
        "tuning_response_digest": result["tuning_response_digest"],
        "total_time_ms": result["total_time_ms"],
        "track": result["track"],
        "peak_speed": result["peak_speed"],
        "curvature_bands": result["curvature_bands"],
        "setup_response": result["setup_response"],
    }


def response_delta(high: dict[str, Any], low: dict[str, Any]) -> dict[str, Any]:
    low_bands = {band["id"]: band for band in low["curvature_bands"]}
    high_bands = {band["id"]: band for band in high["curvature_bands"]}
    return {
        "gear_ratio_slider": low["gear_ratio_slider"],
        "total_time_ms": high["total_time_ms"] - low["total_time_ms"],
        "peak_speed_kph": high["peak_speed"]["speed_kph"]
        - low["peak_speed"]["speed_kph"],
        "legacy_mean_corner_speed_kph": high["setup_response"][
            "mean_corner_speed_kph"
        ]
        - low["setup_response"]["mean_corner_speed_kph"],
        "aerodynamic_drag_work_kj": high["setup_response"][
            "aerodynamic_drag_work_kj"
        ]
        - low["setup_response"]["aerodynamic_drag_work_kj"],
        "mean_downforce_n": high["setup_response"]["mean_downforce_n"]
        - low["setup_response"]["mean_downforce_n"],
        "mean_speed_by_curvature_band_kph": {
            band_id: high_bands[band_id]["mean_speed_kph"]
            - low_bands[band_id]["mean_speed_kph"]
            for band_id in low_bands
        },
    }


def summarize_circuit(
    circuit: tuple[str, str, str], points: list[dict[str, Any]], level_count: int
) -> dict[str, Any]:
    circuit_id, archetype, slug = circuit
    selected = [point for point in points if point["circuit_id"] == circuit_id]
    if len(selected) != 2 * level_count:
        raise SpaDiagnosisError(f"incomplete response for {slug}")
    indexed = {
        (point["downforce_slider"], point["gearing_index"]): point
        for point in selected
    }
    deltas = [
        response_delta(indexed[(1.0, index)], indexed[(0.0, index)])
        for index in range(level_count)
    ]
    peak = max(
        selected,
        key=lambda point: (
            point["peak_speed"]["speed_kph"],
            -point["downforce_slider"],
            point["gearing_index"],
        ),
    )
    fastest = min(
        selected,
        key=lambda point: (
            point["total_time_ms"],
            point["downforce_slider"],
            point["gearing_index"],
        ),
    )
    band_ids = [band["id"] for band in selected[0]["curvature_bands"]]
    return {
        "circuit_id": circuit_id,
        "circuit_archetype": archetype,
        "circuit_slug": slug,
        "track": selected[0]["track"],
        "peak": {
            "downforce_slider": peak["downforce_slider"],
            "gear_ratio_slider": peak["gear_ratio_slider"],
            **peak["peak_speed"],
        },
        "speed_guardrail": {
            "maximum_kph": SPEED_GUARDRAIL_KPH,
            "headroom_kph": SPEED_GUARDRAIL_KPH - peak["peak_speed"]["speed_kph"],
            "passes": peak["peak_speed"]["speed_kph"] < SPEED_GUARDRAIL_KPH,
        },
        "fastest_extreme_setup": {
            "downforce_slider": fastest["downforce_slider"],
            "gear_ratio_slider": fastest["gear_ratio_slider"],
            "total_time_ms": fastest["total_time_ms"],
        },
        "high_minus_low_downforce": {
            "gearing_level_count": level_count,
            "legacy_corner_speed_failure_count": sum(
                delta["legacy_mean_corner_speed_kph"] <= 0.0 for delta in deltas
            ),
            "curvature_band_speed_failure_count": {
                band_id: sum(
                    delta["mean_speed_by_curvature_band_kph"][band_id] <= 0.0
                    for delta in deltas
                )
                for band_id in band_ids
            },
            "deltas": deltas,
        },
    }


def build_report(
    runner: pathlib.Path, seed: int, level_count: int, jobs: int
) -> dict[str, Any]:
    if seed < 0 or seed > 2**64 - 1:
        raise ValueError("seed must be an unsigned 64-bit integer")
    if level_count < 3 or level_count > 11 or level_count % 2 == 0:
        raise ValueError("level count must be an odd integer between 3 and 11")
    if jobs < 1 or jobs > MAX_JOBS:
        raise ValueError(f"jobs must be between 1 and {MAX_JOBS}")
    if not runner.is_file():
        raise SpaDiagnosisError(f"missing probe runner: {runner}")
    base = json.loads(BASE_SCENARIO.read_text())
    response_bytes = CURRENT_RESPONSE.read_bytes()
    response = json.loads(response_bytes)
    mixed_diagnosis_bytes = MIXED_CIRCUIT_DIAGNOSIS.read_bytes()
    mixed_diagnosis = json.loads(mixed_diagnosis_bytes)
    selected_calibration = mixed_diagnosis["selected_calibration_candidate"]
    if not selected_calibration["assessment"]["calibration_eligible"]:
        raise SpaDiagnosisError(
            "the governed calibration baseline is no longer eligible"
        )
    levels = screening.slider_levels(level_count)
    points = []
    with tempfile.TemporaryDirectory(prefix="pitgun-spa-diagnosis-") as temporary:
        temporary_root = pathlib.Path(temporary)
        planned = []
        for circuit in CIRCUITS:
            circuit_id, archetype, slug = circuit
            for downforce in (0.0, 1.0):
                for gearing_index, gearing in enumerate(levels):
                    scenario_path = temporary_root / (
                        f"{slug}-df-{downforce:.1f}-gear-{gearing:.1f}.json"
                    )
                    scenario_path.write_bytes(
                        build_scenario(base, circuit_id, downforce, gearing)
                    )
                    planned.append(
                        (
                            circuit_id,
                            archetype,
                            slug,
                            downforce,
                            gearing_index,
                            gearing,
                            scenario_path,
                        )
                    )
        with concurrent.futures.ThreadPoolExecutor(max_workers=jobs) as executor:
            futures = [
                executor.submit(
                    execute_point,
                    runner,
                    CURRENT_RESPONSE,
                    scenario_path,
                    seed,
                    circuit_id,
                    archetype,
                    slug,
                    downforce,
                    gearing_index,
                    gearing,
                )
                for (
                    circuit_id,
                    archetype,
                    slug,
                    downforce,
                    gearing_index,
                    gearing,
                    scenario_path,
                ) in planned
            ]
            points = [future.result() for future in futures]

    observed_response_digests = {
        point["tuning_response_digest"] for point in points
    }
    if observed_response_digests != {selected_calibration["tuning_response_digest"]}:
        raise SpaDiagnosisError(
            "the governed calibration baseline does not match the probed response"
        )

    circuits = [summarize_circuit(circuit, points, level_count) for circuit in CIRCUITS]
    spa = next(circuit for circuit in circuits if circuit["circuit_slug"] == "spa")
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
        "question": "Is Spa a track-data defect or evidence of a general solver segmentation weakness?",
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
        "tuning_response": response,
        "tuning_response_digest": screening.sha256(response_bytes),
        "unchanged_calibration_baseline": {
            "source_path": str(MIXED_CIRCUIT_DIAGNOSIS.relative_to(ROOT)),
            "source_digest": screening.sha256(mixed_diagnosis_bytes),
            "execution_point_set_digest": mixed_diagnosis[
                "execution_point_set_digest"
            ],
            "calibration_eligible": True,
            "optima": [
                {
                    "circuit_id": circuit["circuit_id"],
                    "circuit_slug": circuit["circuit_slug"],
                    **circuit["fastest"],
                }
                for circuit in selected_calibration["circuits"][:5]
            ],
        },
        "diagnosis": {
            "classification": "GENERAL_SEGMENTATION_WEAKNESS_EXPOSED_BY_SPA",
            "spa_speed_reproduced_kph": spa["peak"]["speed_kph"],
            "spa_legacy_corner_response_failure_count": spa[
                "high_minus_low_downforce"
            ]["legacy_corner_speed_failure_count"],
            "track_data_finding": "Spa is labelled mixed-elevation but its elevation and slope channels are flat, as are the comparison circuits.",
            "metric_finding": "The legacy mean-corner metric merges every sample above one binary curvature threshold.",
            "solver_finding": "The same binary threshold selects straight or corner aero during forward acceleration, while corner limits and backward braking always use corner aero.",
            "coefficient_finding": "Changing calibrated coefficients is not justified until segmentation and track representation are corrected and remeasured.",
        },
        "guardrail": {
            "id": "racing-holdout-maximum-speed-v1",
            "scope": ["silverstone", "spa"],
            "rule": "maximum observed speed must remain strictly below 400 km/h over the bounded extreme setup grid",
            "passes": all(
                circuit["speed_guardrail"]["passes"]
                for circuit in circuits
                if circuit["circuit_slug"] in {"silverstone", "spa"}
            ),
        },
        "recommendation": "DESIGN_CONTINUOUS_CURVATURE_RESPONSE_AND_AUDIT_TRACK_PACK",
        "automatic_model_change": False,
        "circuits": circuits,
    }


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
                raise SpaDiagnosisError("Spa high-speed diagnosis is stale")
            print(f"Spa high-speed diagnosis is current: {args.output}")
        else:
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_bytes(payload)
            checksum_path.write_bytes(checksum)
            print(f"wrote Spa diagnosis for {report['planned_run_count']} points")
    except (OSError, ValueError, SpaDiagnosisError) as error:
        parser.exit(1, f"error: {error}\n")


if __name__ == "__main__":
    main()
