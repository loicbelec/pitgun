#!/usr/bin/env python3
"""Refine the bounded Racing aerodynamic base calibration neighborhood."""

from __future__ import annotations

import argparse
import hashlib
import os
import pathlib

import calibrate_aero_bases as calibration
import coefficient_screening as screening


ROOT = pathlib.Path(__file__).resolve().parents[2]
DEFAULT_OUTPUT = (
    ROOT
    / "experiments"
    / "racing_response"
    / "results"
    / "racing-aero-base-refinement-v1.json"
)
SCHEMA_VERSION = "pitgun.racing-aero-base-refinement/v1"
COARSE_REPORT = (
    ROOT
    / "experiments"
    / "racing_response"
    / "results"
    / "racing-aero-base-calibration-v1.json"
)
DRAG_BASES = (0.575, 0.600, 0.625, 0.650, 0.675)
DOWNFORCE_BASES = (1.01250, 1.034375, 1.05625, 1.078125, 1.10000)


class BaseRefinementError(RuntimeError):
    """Raised when the aerodynamic base refinement is invalid."""


def build_report(
    runner: pathlib.Path, seed: int, level_count: int, jobs: int
) -> dict:
    if not COARSE_REPORT.is_file():
        raise BaseRefinementError(f"missing coarse calibration report: {COARSE_REPORT}")
    original_drag_bases = calibration.DRAG_BASES
    original_downforce_bases = calibration.DOWNFORCE_BASES
    try:
        calibration.DRAG_BASES = DRAG_BASES
        calibration.DOWNFORCE_BASES = DOWNFORCE_BASES
        report = calibration.build_report(runner, seed, level_count, jobs)
    finally:
        calibration.DRAG_BASES = original_drag_bases
        calibration.DOWNFORCE_BASES = original_downforce_bases
    report.update(
        {
            "schema_version": SCHEMA_VERSION,
            "question": "Which aerodynamic bases around the coarse optimum minimize historical gameplay-pace disruption while preserving the reviewed circuit response?",
            "coarse_calibration_report_digest": screening.sha256(
                COARSE_REPORT.read_bytes()
            ),
            "refinement_space": {
                "drag_bases": list(DRAG_BASES),
                "downforce_bases": list(DOWNFORCE_BASES),
            },
        }
    )
    for candidate in report["candidates"]:
        parameter_space_interior = (
            candidate["drag_base"] not in {min(DRAG_BASES), max(DRAG_BASES)}
            and candidate["downforce_base"]
            not in {min(DOWNFORCE_BASES), max(DOWNFORCE_BASES)}
        )
        candidate["refinement_assessment"] = {
            "eligible": candidate["calibration_assessment"]["eligible"]
            and parameter_space_interior,
            "parameter_space_interior": parameter_space_interior,
        }
    report["candidates"].sort(
        key=lambda candidate: (
            not candidate["refinement_assessment"]["eligible"],
            candidate["compatibility_pace"]["root_mean_square_gap_ms"],
            candidate["candidate_id"],
        )
    )
    for rank, candidate in enumerate(report["candidates"], start=1):
        candidate["refinement_rank"] = rank
    report["eligibility_rule"]["parameter_space"] = (
        "candidate must be interior to both refined base axes"
    )
    return report


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--runner",
        type=pathlib.Path,
        default=ROOT / "target" / "release" / "examples" / "tuning_response_probe",
    )
    parser.add_argument("--output", type=pathlib.Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--seed", type=int, default=calibration.DEFAULT_SEED)
    parser.add_argument("--levels", type=int, default=calibration.DEFAULT_LEVEL_COUNT)
    parser.add_argument(
        "--jobs",
        type=int,
        default=min(4, os.cpu_count() or 1, calibration.MAX_JOBS),
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
                raise BaseRefinementError(
                    f"aero base refinement is missing or stale: {args.output}"
                )
            print(f"aero base refinement is current: {args.output}")
        else:
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_bytes(report_bytes)
            checksum_path.write_bytes(checksum_bytes)
            print(f"wrote compact evidence for {report['planned_run_count']} points")
    except (
        OSError,
        ValueError,
        BaseRefinementError,
        calibration.BaseCalibrationError,
        calibration.refinement.RefinementError,
        screening.ScreeningError,
    ) as error:
        parser.exit(1, f"error: {error}\n")


if __name__ == "__main__":
    main()
