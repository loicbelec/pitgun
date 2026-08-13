#!/usr/bin/env python3
"""Fetch SPW LiDAR terrain heights and compare them with the Spa EU-DEM prototype."""

from __future__ import annotations

import argparse
import concurrent.futures
import hashlib
import json
import math
import pathlib
import statistics
import time
import urllib.parse
import urllib.request

import build_spa_elevation_prototype as eudem


ROOT = pathlib.Path(__file__).resolve().parents[2]
RESULTS = ROOT / "experiments" / "racing_tracks" / "results"
SPW_RAW_OUTPUT = RESULTS / "spa-spw-lidar-raw-v1.json"
COMPARISON_OUTPUT = RESULTS / "spa-elevation-source-comparison-v1.json"
SPW_ENDPOINT = (
    "https://geoservices.wallonie.be/arcgis/rest/services/RELIEF/"
    "WALLONIE_MNT_2021_2022/MapServer/identify"
)
SPW_DATASET_ID = "a004e570-99d6-4fe5-b83d-49b774409278"
SPW_DATASET_URL = f"https://geodata.wallonie.be/id/{SPW_DATASET_ID}"
SPW_LAYER_NAME = "Relief Wallonie - MNT 2021-2022 - 50cm"


def identify_url(point: dict[str, float]) -> str:
    longitude = point["longitude"]
    latitude = point["latitude"]
    query = urllib.parse.urlencode(
        {
            "geometry": f"{longitude:.10f},{latitude:.10f}",
            "geometryType": "esriGeometryPoint",
            "sr": "4326",
            "layers": "all:0",
            "tolerance": "1",
            "mapExtent": (
                f"{longitude - 0.001:.10f},{latitude - 0.001:.10f},"
                f"{longitude + 0.001:.10f},{latitude + 0.001:.10f}"
            ),
            "imageDisplay": "1000,1000,96",
            "returnGeometry": "false",
            "f": "json",
        }
    )
    return SPW_ENDPOINT + "?" + query


def stable_metrics(value: object) -> object:
    """Normalize derived floats beyond source accuracy for portable JSON."""
    if isinstance(value, float):
        return round(value, 9)
    if isinstance(value, list):
        return [stable_metrics(item) for item in value]
    if isinstance(value, dict):
        return {key: stable_metrics(item) for key, item in value.items()}
    return value


def fetch_elevation(point: dict[str, float]) -> float:
    last_error = None
    for attempt in range(4):
        try:
            request = urllib.request.Request(
                identify_url(point), headers={"User-Agent": "Pitgun geometry audit/1"}
            )
            with urllib.request.urlopen(request, timeout=30) as response:
                payload = json.load(response)
            results = payload.get("results", [])
            if len(results) != 1 or results[0].get("layerName") != SPW_LAYER_NAME:
                raise RuntimeError("unexpected SPW identify response")
            attributes = results[0].get("attributes", {})
            values = [
                value for key, value in attributes.items() if key.endswith("Pixel Value")
            ]
            if len(values) != 1:
                raise RuntimeError("SPW response contains no unique pixel value")
            return float(values[0])
        except (OSError, ValueError, RuntimeError) as error:
            last_error = error
            time.sleep(0.25 * (2**attempt))
    raise RuntimeError(f"SPW elevation request failed: {last_error}")


def fetch_spw() -> dict[str, object]:
    points = eudem.requested_points()
    # Keep modest concurrency: this is a public administration service, not a
    # Pitgun runtime dependency. Output order remains the centerline order.
    with concurrent.futures.ThreadPoolExecutor(max_workers=4) as executor:
        elevations = list(executor.map(fetch_elevation, points))
    return {
        "schema_version": "pitgun.spa-spw-lidar-fetch/v1",
        "status": "experimental_source_evidence",
        "source_track": {
            "repository": "https://github.com/bacinger/f1-circuits",
            "revision": eudem.SOURCE_REVISION,
            "path": "circuits/be-1925.geojson",
            "digest": eudem.SOURCE_DIGEST,
            "license": "MIT",
        },
        "dem": {
            "provider": "Service public de Wallonie (SPW)",
            "dataset": "Relief de la Wallonie - MNT 2021-2022",
            "dataset_id": SPW_DATASET_ID,
            "dataset_url": SPW_DATASET_URL,
            "service_endpoint": SPW_ENDPOINT,
            "dataset_resolution_m": 0.5,
            "horizontal_reference": "Belgian Lambert 2008 (EPSG:3812)",
            "vertical_reference": "Deuxième Nivellement Général (EPSG:5710)",
            "published_absolute_vertical_accuracy_m": 0.12,
            "license": "CC BY 4.0",
            "source_citation": (
                "Service public de Wallonie (SPW) - Relief de la Wallonie - "
                "Modèle Numérique de Terrain (MNT) 2021-2022 (2024-01-23)"
            ),
        },
        "sampling_spacing_m": eudem.SPACING_M,
        "points": [
            {**point, "elevation_m": elevation}
            for point, elevation in zip(points, elevations)
        ],
    }


def rmse(values: list[float]) -> float:
    return math.sqrt(sum(value * value for value in values) / len(values))


def correlation(left: list[float], right: list[float]) -> float:
    left_mean = statistics.fmean(left)
    right_mean = statistics.fmean(right)
    numerator = sum(
        (left_value - left_mean) * (right_value - right_mean)
        for left_value, right_value in zip(left, right)
    )
    denominator = math.sqrt(
        sum((value - left_mean) ** 2 for value in left)
        * sum((value - right_mean) ** 2 for value in right)
    )
    return numerator / denominator


def slopes(points: list[dict[str, float]], elevation: list[float]) -> list[float]:
    values = [
        (right_z - left_z) / (right_point["distance_m"] - left_point["distance_m"])
        for left_point, right_point, left_z, right_z in zip(
            points, points[1:], elevation, elevation[1:]
        )
    ]
    return values + [values[0]]


def profile_summary(elevation: list[float], slope: list[float]) -> dict[str, float]:
    return {
        "minimum_elevation_m": min(elevation),
        "maximum_elevation_m": max(elevation),
        "elevation_range_m": max(elevation) - min(elevation),
        "cumulative_gain_m": sum(
            max(0.0, right - left) for left, right in zip(elevation, elevation[1:])
        ),
        "cumulative_loss_m": sum(
            max(0.0, left - right) for left, right in zip(elevation, elevation[1:])
        ),
        "maximum_absolute_slope_ratio": max(abs(value) for value in slope),
    }


def build_comparison(
    spw: dict[str, object], eudem_raw: dict[str, object]
) -> dict[str, object]:
    spw_points = spw["points"]
    eudem_points = eudem_raw["points"]
    if len(spw_points) != len(eudem_points) or any(
        left["distance_m"] != right["distance_m"]
        for left, right in zip(spw_points, eudem_points)
    ):
        raise ValueError("SPW and EU-DEM samples do not share the same centerline grid")

    spw_raw = [float(point["elevation_m"]) for point in spw_points]
    eudem_values = [float(point["elevation_m"]) for point in eudem_points]
    spw_smoothed = eudem.circular_mean(spw_raw, radius=1)
    eudem_smoothed = eudem.circular_mean(eudem_values, radius=1)
    delta = [left - right for left, right in zip(spw_smoothed, eudem_smoothed)]
    mean_bias = statistics.fmean(delta)
    centered_delta = [value - mean_bias for value in delta]
    spw_slope = slopes(spw_points, spw_smoothed)
    eudem_slope = slopes(eudem_points, eudem_smoothed)
    slope_delta = [left - right for left, right in zip(spw_slope, eudem_slope)]

    agreement = {
        "mean_spw_minus_eudem_m": mean_bias,
        "absolute_elevation_rmse_m": rmse(delta),
        "bias_removed_profile_rmse_m": rmse(centered_delta),
        "maximum_absolute_bias_removed_difference_m": max(
            abs(value) for value in centered_delta
        ),
        "profile_pearson_correlation": correlation(spw_smoothed, eudem_smoothed),
        "elevation_range_difference_m": (
            max(spw_smoothed)
            - min(spw_smoothed)
            - (max(eudem_smoothed) - min(eudem_smoothed))
        ),
        "slope_ratio_rmse": rmse(slope_delta),
        "spw_maximum_absolute_slope_ratio": max(abs(value) for value in spw_slope),
        "eudem_maximum_absolute_slope_ratio": max(abs(value) for value in eudem_slope),
    }
    corroborated = (
        agreement["profile_pearson_correlation"] >= 0.98
        and agreement["bias_removed_profile_rmse_m"] <= 5.0
        and abs(agreement["elevation_range_difference_m"]) <= 10.0
    )
    return {
        "schema_version": "pitgun.spa-elevation-source-comparison/v1",
        "status": "experimental_source_evidence",
        "inputs": {
            "spw_raw_digest": eudem.digest(eudem.pretty(spw)),
            "eudem_raw_digest": eudem.digest(eudem.pretty(eudem_raw)),
        },
        "method": {
            "shared_sampling_spacing_m": eudem.SPACING_M,
            "shared_smoothing": "circular three-sample moving average",
            "slope_unit": "rise over run",
            "absolute_bias_note": (
                "absolute bias may include vertical-datum differences; centered profile "
                "metrics assess relief shape"
            ),
        },
        "sample_count": len(spw_points),
        "profiles": {
            "spw_lidar": profile_summary(spw_smoothed, spw_slope),
            "eudem": profile_summary(eudem_smoothed, eudem_slope),
        },
        "agreement": agreement,
        "conclusion": {
            "relief_shape_corroborated": corroborated,
            "catalog_promotion_ready": False,
            "remaining_review": [
                "inspect local outliers and verify centerline placement on the 0.5 m terrain",
                "define the smoothing policy used for Solver-scale elevation and slope",
                "measure deterministic Solver and calibration impact before a new catalog release",
            ],
        },
        "points": [
            {
                "distance_m": point["distance_m"],
                "spw_smoothed_elevation_m": spw_smoothed[index],
                "eudem_smoothed_elevation_m": eudem_smoothed[index],
                "spw_minus_eudem_m": delta[index],
                "bias_removed_difference_m": centered_delta[index],
                "spw_slope_ratio": spw_slope[index],
                "eudem_slope_ratio": eudem_slope[index],
            }
            for index, point in enumerate(spw_points)
        ],
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--fetch", action="store_true")
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    if args.fetch:
        eudem.write_artifact(SPW_RAW_OUTPUT, fetch_spw())
    if not eudem.verify_artifact(SPW_RAW_OUTPUT):
        parser.exit(1, f"error: SPW artifact is missing or corrupt: {SPW_RAW_OUTPUT}\n")
    if not eudem.verify_artifact(eudem.RAW_OUTPUT):
        parser.exit(1, f"error: EU-DEM artifact is missing or corrupt: {eudem.RAW_OUTPUT}\n")

    spw = json.loads(SPW_RAW_OUTPUT.read_text())
    eudem_raw = json.loads(eudem.RAW_OUTPUT.read_text())
    comparison = stable_metrics(build_comparison(spw, eudem_raw))
    data = eudem.pretty(comparison)
    checksum = hashlib.sha256(data).hexdigest() + "  " + COMPARISON_OUTPUT.name + "\n"
    if args.check:
        if (
            not COMPARISON_OUTPUT.is_file()
            or COMPARISON_OUTPUT.read_bytes() != data
            or not COMPARISON_OUTPUT.with_suffix(".sha256").is_file()
            or COMPARISON_OUTPUT.with_suffix(".sha256").read_text() != checksum
        ):
            parser.exit(1, "error: Spa elevation comparison is missing or stale\n")
        print(f"Spa elevation comparison is current: {COMPARISON_OUTPUT}")
        return
    eudem.write_artifact(COMPARISON_OUTPUT, comparison)
    print(json.dumps(comparison["agreement"], indent=2))


if __name__ == "__main__":
    main()
