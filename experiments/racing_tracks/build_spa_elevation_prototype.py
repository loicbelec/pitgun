#!/usr/bin/env python3
"""Fetch and analyze a non-canonical Spa EU-DEM elevation prototype."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import pathlib
import time
import urllib.parse
import urllib.request


ROOT = pathlib.Path(__file__).resolve().parents[2]
TRACK = ROOT / "catalogs" / "racing" / "v1.1.0" / "simulation" / "circuits" / "spa.json"
RAW_OUTPUT = ROOT / "experiments" / "racing_tracks" / "results" / "spa-eudem25m-raw-v1.json"
PROFILE_OUTPUT = ROOT / "experiments" / "racing_tracks" / "results" / "spa-elevation-prototype-v1.json"
API_ENDPOINT = "https://api.opentopodata.org/v1/eudem25m"
SOURCE_REVISION = "b37cc90ccdc41553fda91fc2a1f2c186b28fdc1a"
SOURCE_DIGEST = "sha256:b867de9cc8364cc9b545a044377a56a697b8db77823c849c1a1c03760a943381"
REFERENCE_LONGITUDE_DEG = 5.968647300653595
REFERENCE_LATITUDE_DEG = 50.43471801960784
EARTH_RADIUS_M = 6_371_000.0
SPACING_M = 25.0


def pretty(value: object) -> bytes:
    return (json.dumps(value, indent=2, ensure_ascii=False) + "\n").encode()


def digest(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


def interpolate(target: float, x: list[float], y: list[float]) -> float:
    upper = 1
    while upper < len(x) and x[upper] < target:
        upper += 1
    if upper >= len(x):
        return y[-1]
    lower = upper - 1
    fraction = (target - x[lower]) / (x[upper] - x[lower])
    return y[lower] + fraction * (y[upper] - y[lower])


def local_to_wgs84(x_m: float, y_m: float) -> tuple[float, float]:
    latitude = REFERENCE_LATITUDE_DEG + math.degrees(y_m / EARTH_RADIUS_M)
    longitude = REFERENCE_LONGITUDE_DEG + math.degrees(
        x_m / (EARTH_RADIUS_M * math.cos(math.radians(REFERENCE_LATITUDE_DEG)))
    )
    return latitude, longitude


def requested_points() -> list[dict[str, float]]:
    track = json.loads(TRACK.read_text())["data"]
    distance = track["s_m"]
    sample_distance = [index * SPACING_M for index in range(int(distance[-1] // SPACING_M) + 1)]
    if sample_distance[-1] != distance[-1]:
        sample_distance.append(distance[-1])
    points = []
    for value in sample_distance:
        x_m = interpolate(value, distance, track["x_m"])
        y_m = interpolate(value, distance, track["y_m"])
        latitude, longitude = local_to_wgs84(x_m, y_m)
        points.append({"distance_m": value, "latitude": latitude, "longitude": longitude})
    points[-1]["latitude"] = points[0]["latitude"]
    points[-1]["longitude"] = points[0]["longitude"]
    return points


def fetch() -> dict[str, object]:
    points = requested_points()
    elevations = []
    for offset in range(0, len(points), 100):
        chunk = points[offset : offset + 100]
        locations = "|".join(
            f"{point['latitude']:.10f},{point['longitude']:.10f}" for point in chunk
        )
        url = API_ENDPOINT + "?" + urllib.parse.urlencode(
            {"locations": locations, "interpolation": "bilinear"}
        )
        with urllib.request.urlopen(url, timeout=30) as response:
            payload = json.load(response)
        if payload.get("status") != "OK" or len(payload.get("results", [])) != len(chunk):
            raise RuntimeError(f"unexpected OpenTopoData response for chunk {offset // 100}")
        elevations.extend(result["elevation"] for result in payload["results"])
        if offset + 100 < len(points):
            time.sleep(1.1)
    return {
        "schema_version": "pitgun.spa-elevation-fetch/v1",
        "source_track": {
            "repository": "https://github.com/bacinger/f1-circuits",
            "revision": SOURCE_REVISION,
            "path": "circuits/be-1925.geojson",
            "digest": SOURCE_DIGEST,
            "license": "MIT",
        },
        "dem": {
            "provider": "OpenTopoData",
            "dataset": "EU-DEM v1.1",
            "dataset_resolution_m": 25,
            "api_endpoint": API_ENDPOINT,
            "interpolation": "bilinear",
            "license": "Copernicus full, open and free access with attribution",
        },
        "sampling_spacing_m": SPACING_M,
        "points": [
            {**point, "elevation_m": elevation}
            for point, elevation in zip(points, elevations)
        ],
    }


def circular_mean(values: list[float], radius: int) -> list[float]:
    core = values[:-1]
    size = len(core)
    result = [
        sum(core[(index + offset) % size] for offset in range(-radius, radius + 1))
        / (2 * radius + 1)
        for index in range(size)
    ]
    return result + [result[0]]


def build_profile(raw: dict[str, object]) -> dict[str, object]:
    points = raw["points"]
    elevation = [float(point["elevation_m"]) for point in points]
    smoothed = circular_mean(elevation, radius=1)
    relative = [value - smoothed[0] for value in smoothed]
    slope = []
    for index in range(len(points) - 1):
        delta_s = points[index + 1]["distance_m"] - points[index]["distance_m"]
        slope.append((relative[index + 1] - relative[index]) / delta_s)
    slope.append(slope[0])
    gain = sum(max(0.0, right - left) for left, right in zip(smoothed, smoothed[1:]))
    loss = sum(max(0.0, left - right) for left, right in zip(smoothed, smoothed[1:]))
    return {
        "schema_version": "pitgun.spa-elevation-prototype/v1",
        "status": "experimental_not_catalog_eligible",
        "raw_fetch_digest": digest(pretty(raw)),
        "method": {
            "sampling_spacing_m": SPACING_M,
            "smoothing": "circular three-sample moving average",
            "vertical_reference": "relative to smoothed start/finish elevation",
            "slope_unit": "rise over run",
        },
        "summary": {
            "source_elevation_minimum_m": min(elevation),
            "source_elevation_maximum_m": max(elevation),
            "smoothed_elevation_range_m": max(smoothed) - min(smoothed),
            "smoothed_elevation_gain_m": gain,
            "smoothed_elevation_loss_m": loss,
            "maximum_absolute_slope": max(abs(value) for value in slope),
            "closure_error_m": abs(relative[-1] - relative[0]),
        },
        "promotion_blockers": [
            "validate the recovered profile against an independent or official reference",
            "review smoothing and slope bounds before one-metre resampling",
            "define unambiguous elevation and slope semantics in the next schema",
            "publish corrected data only in a new immutable catalog version",
        ],
        "points": [
            {
                **point,
                "smoothed_elevation_m": smoothed[index],
                "relative_elevation_m": relative[index],
                "slope": slope[index],
            }
            for index, point in enumerate(points)
        ],
    }


def write_artifact(path: pathlib.Path, payload: dict[str, object]) -> None:
    data = pretty(payload)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)
    path.with_suffix(".sha256").write_text(
        hashlib.sha256(data).hexdigest() + "  " + path.name + "\n"
    )


def verify_artifact(path: pathlib.Path) -> bool:
    checksum_path = path.with_suffix(".sha256")
    if not path.is_file() or not checksum_path.is_file():
        return False
    expected = hashlib.sha256(path.read_bytes()).hexdigest() + "  " + path.name + "\n"
    return checksum_path.read_text() == expected


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--fetch", action="store_true")
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    if args.fetch:
        write_artifact(RAW_OUTPUT, fetch())
    if not verify_artifact(RAW_OUTPUT):
        parser.exit(1, f"error: raw elevation artifact is missing or corrupt: {RAW_OUTPUT}\n")
    raw = json.loads(RAW_OUTPUT.read_text())
    profile = build_profile(raw)
    data = pretty(profile)
    checksum = hashlib.sha256(data).hexdigest() + "  " + PROFILE_OUTPUT.name + "\n"
    if args.check:
        if (
            not PROFILE_OUTPUT.is_file()
            or PROFILE_OUTPUT.read_bytes() != data
            or not PROFILE_OUTPUT.with_suffix(".sha256").is_file()
            or PROFILE_OUTPUT.with_suffix(".sha256").read_text() != checksum
        ):
            parser.exit(1, "error: Spa elevation prototype is missing or stale\n")
        print(f"Spa elevation prototype is current: {PROFILE_OUTPUT}")
        return
    write_artifact(PROFILE_OUTPUT, profile)
    print(f"wrote Spa elevation prototype with {len(profile['points'])} points")


if __name__ == "__main__":
    main()
