#!/usr/bin/env python3
"""Localize Spa relief response and test smoothing sensitivity by distance."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import pathlib
import subprocess
import tempfile
from typing import Any

import coefficient_screening as screening
import measure_spa_relief_impact as impact


ROOT = pathlib.Path(__file__).resolve().parents[2]
DEFAULT_OUTPUT = (
    ROOT
    / "experiments"
    / "racing_response"
    / "results"
    / "racing-spa-relief-localization-v1.json"
)
DEFAULT_SEED = 42
SETUP = {"downforce_slider": 0.25, "gear_ratio_slider": 0.25}
DISTANCE_SPACING_M = 25.0
SECTOR_LENGTH_M = 250.0
SMOOTHING_RADII = {"spw_75m": 1, "spw_125m": 2, "spw_175m": 3}
SCHEMA_VERSION = "pitgun.racing-spa-relief-localization/v1"


class ReliefLocalizationError(RuntimeError):
    """Raised when telemetry localization evidence cannot be trusted."""


def build_profiles() -> dict[str, dict[str, list[float]]]:
    canonical = json.loads(impact.elevation.TRACK.read_text())["data"]
    spw = json.loads(impact.sources.SPW_RAW_OUTPUT.read_text())
    sample_distance = [float(point["distance_m"]) for point in spw["points"]]
    raw_height = [float(point["elevation_m"]) for point in spw["points"]]
    shared = {
        "s": canonical["s_m"],
        "x": canonical["x_m"],
        "y": canonical["y_m"],
    }
    profiles = {"flat": {**shared, "z": [0.0] * len(canonical["s_m"])}}
    for name, radius in SMOOTHING_RADII.items():
        smoothed = impact.elevation.circular_mean(raw_height, radius=radius)
        relative = [value - smoothed[0] for value in smoothed]
        z = [
            impact.interpolate(distance, sample_distance, relative)
            for distance in canonical["s_m"]
        ]
        z[-1] = z[0]
        profiles[name] = {**shared, "z": z}
    return profiles


def run_probe(
    runner: pathlib.Path, scenario_path: pathlib.Path, seed: int
) -> dict[str, Any]:
    completed = subprocess.run(
        [str(runner), str(scenario_path), str(impact.TUNING_RESPONSE), str(seed)],
        cwd=ROOT,
        check=False,
        capture_output=True,
        timeout=120,
    )
    if completed.returncode != 0:
        raise ReliefLocalizationError(
            f"probe exited {completed.returncode}: "
            f"{completed.stderr.decode(errors='replace').strip()}"
        )
    if completed.stderr:
        raise ReliefLocalizationError("successful probe wrote to stderr")
    try:
        result = json.loads(completed.stdout)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ReliefLocalizationError("probe returned invalid JSON") from error
    if result.get("schema_version") != "pitgun.racing-relief-response-probe/v1":
        raise ReliefLocalizationError("probe returned an unsupported result")
    return result


def unique_telemetry(points: list[dict[str, float]]) -> list[dict[str, float]]:
    by_distance = {}
    for point in points:
        by_distance[float(point["distance_m"])] = point
    return [by_distance[distance] for distance in sorted(by_distance)]


def interpolate_telemetry(
    distance: float, points: list[dict[str, float]], field: str
) -> float:
    positions = [float(point["distance_m"]) for point in points]
    values = [float(point[field]) for point in points]
    return impact.interpolate(distance, positions, values)


def slope(profile: dict[str, list[float]], distance: float) -> float:
    s = profile["s"]
    z = profile["z"]
    lower_distance = max(s[0], distance - 12.5)
    upper_distance = min(s[-1], distance + 12.5)
    return (
        impact.interpolate(upper_distance, s, z)
        - impact.interpolate(lower_distance, s, z)
    ) / (upper_distance - lower_distance)


def align(
    profiles: dict[str, dict[str, list[float]]],
    results: dict[str, dict[str, Any]],
) -> list[dict[str, Any]]:
    track_length = profiles["flat"]["s"][-1]
    distances = [
        index * DISTANCE_SPACING_M
        for index in range(int(track_length // DISTANCE_SPACING_M) + 1)
    ]
    telemetry = {
        name: unique_telemetry(result["telemetry"]) for name, result in results.items()
    }
    rows = []
    for distance in distances:
        variants = {}
        for name, points in telemetry.items():
            variants[name] = {
                field: interpolate_telemetry(distance, points, field)
                for field in ("speed_kph", "throttle_pct", "brake_pct", "g_long", "g_vert")
            }
            variants[name]["slope_ratio"] = slope(profiles[name], distance)
        flat = variants["flat"]
        rows.append(
            {
                "distance_m": distance,
                "variants": variants,
                "spw_75m_minus_flat": {
                    field: variants["spw_75m"][field] - flat[field]
                    for field in (
                        "speed_kph",
                        "throttle_pct",
                        "brake_pct",
                        "g_long",
                        "g_vert",
                    )
                },
            }
        )
    return rows


def extrema(rows: list[dict[str, Any]], field: str) -> dict[str, Any]:
    minimum = min(rows, key=lambda row: row["spw_75m_minus_flat"][field])
    maximum = max(rows, key=lambda row: row["spw_75m_minus_flat"][field])
    return {
        "minimum": {
            "distance_m": minimum["distance_m"],
            "delta": minimum["spw_75m_minus_flat"][field],
            "slope_ratio": minimum["variants"]["spw_75m"]["slope_ratio"],
        },
        "maximum": {
            "distance_m": maximum["distance_m"],
            "delta": maximum["spw_75m_minus_flat"][field],
            "slope_ratio": maximum["variants"]["spw_75m"]["slope_ratio"],
        },
    }


def sector_summary(rows: list[dict[str, Any]]) -> list[dict[str, Any]]:
    sectors = []
    maximum_sector = int(rows[-1]["distance_m"] // SECTOR_LENGTH_M)
    for index in range(maximum_sector + 1):
        selected = [
            row
            for row in rows
            if int(row["distance_m"] // SECTOR_LENGTH_M) == index
        ]
        if not selected:
            continue
        sectors.append(
            {
                "start_distance_m": index * SECTOR_LENGTH_M,
                "end_distance_m": min(
                    (index + 1) * SECTOR_LENGTH_M, rows[-1]["distance_m"]
                ),
                "mean_slope_ratio": sum(
                    row["variants"]["spw_75m"]["slope_ratio"] for row in selected
                )
                / len(selected),
                "mean_speed_delta_kph": sum(
                    row["spw_75m_minus_flat"]["speed_kph"] for row in selected
                )
                / len(selected),
                "maximum_absolute_g_long_delta": max(
                    abs(row["spw_75m_minus_flat"]["g_long"]) for row in selected
                ),
                "mean_brake_delta_pct": sum(
                    row["spw_75m_minus_flat"]["brake_pct"] for row in selected
                )
                / len(selected),
            }
        )
    return sectors


def build_report(runner: pathlib.Path, seed: int) -> dict[str, Any]:
    if not runner.is_file():
        raise ReliefLocalizationError(f"missing probe runner: {runner}")
    profiles = build_profiles()
    base = json.loads(impact.BASE_SCENARIO.read_text())
    results = {}
    with tempfile.TemporaryDirectory(prefix="pitgun-spa-relief-local-") as temporary:
        root = pathlib.Path(temporary)
        for name, profile in profiles.items():
            scenario = impact.build_scenario(
                base,
                profile,
                SETUP["downforce_slider"],
                SETUP["gear_ratio_slider"],
            )
            scenario_path = root / f"{name}.json"
            scenario_path.write_bytes(scenario)
            results[name] = run_probe(runner, scenario_path, seed)
    rows = align(profiles, results)
    sectors = sector_summary(rows)
    time_delta = {
        name: result["total_time_ms"] - results["flat"]["total_time_ms"]
        for name, result in results.items()
        if name != "flat"
    }
    smoothing_spread_ms = max(time_delta.values()) - min(time_delta.values())
    max_g_vert = {
        name: max(abs(point["g_vert"]) for point in result["telemetry"])
        for name, result in results.items()
    }
    return {
        "schema_version": SCHEMA_VERSION,
        "question": (
            "Where does Spa relief change Solver telemetry, and is the result stable "
            "across bounded SPW smoothing windows?"
        ),
        "status": "experimental_not_catalog_eligible",
        "runner": {
            "path": str(runner.relative_to(ROOT)),
            "digest": screening.sha256(runner.read_bytes()),
        },
        "seed": str(seed),
        "setup": SETUP,
        "method": {
            "telemetry_alignment_spacing_m": DISTANCE_SPACING_M,
            "sector_length_m": SECTOR_LENGTH_M,
            "smoothing_variants": {
                name: f"circular {2 * radius + 1}-sample window at 25 m"
                for name, radius in SMOOTHING_RADII.items()
            },
            "profile_source_digest": impact.elevation.digest(
                impact.sources.SPW_RAW_OUTPUT.read_bytes()
            ),
        },
        "profile_summaries": {
            name: impact.profile_summary(profile) for name, profile in profiles.items()
        },
        "lap_time_ms": {name: result["total_time_ms"] for name, result in results.items()},
        "relief_minus_flat_time_ms": time_delta,
        "smoothing_sensitivity": {
            "relief_time_spread_ms": smoothing_spread_ms,
            "bounded": smoothing_spread_ms <= 100,
        },
        "vertical_dynamics_guardrail": {
            "maximum_absolute_g_vert": max_g_vert,
            "limit_g": 3.0,
            "passes": all(value < 3.0 for value in max_g_vert.values()),
        },
        "spw_75m_extrema": {
            "speed_delta_kph": extrema(rows, "speed_kph"),
            "g_long_delta": extrema(rows, "g_long"),
            "brake_delta_pct": extrema(rows, "brake_pct"),
        },
        "most_affected_250m_sectors": sorted(
            sectors,
            key=lambda sector: abs(sector["mean_speed_delta_kph"]),
            reverse=True,
        )[:8],
        "conclusion": {
            "smoothing_response_is_stable": smoothing_spread_ms <= 100,
            "vertical_dynamics_is_bounded": all(value < 3.0 for value in max_g_vert.values()),
            "catalog_promotion_ready": False,
        },
        "aligned_points": rows,
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
        default=ROOT / "target" / "release" / "examples" / "relief_response_probe",
    )
    parser.add_argument("--output", type=pathlib.Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--seed", type=int, default=DEFAULT_SEED)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    try:
        report = stable(build_report(args.runner.resolve(), args.seed))
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
                raise ReliefLocalizationError("Spa relief localization evidence is stale")
            print(f"Spa relief localization evidence is current: {args.output}")
        else:
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_bytes(payload)
            checksum_path.write_bytes(checksum)
            print(
                json.dumps(
                    {
                        "relief_minus_flat_time_ms": report["relief_minus_flat_time_ms"],
                        "smoothing_sensitivity": report["smoothing_sensitivity"],
                        "vertical_dynamics_guardrail": report[
                            "vertical_dynamics_guardrail"
                        ],
                        "spw_75m_extrema": report["spw_75m_extrema"],
                        "most_affected_250m_sectors": report[
                            "most_affected_250m_sectors"
                        ],
                    },
                    indent=2,
                )
            )
    except (OSError, ValueError, ReliefLocalizationError) as error:
        parser.exit(1, f"error: {error}\n")


if __name__ == "__main__":
    main()
