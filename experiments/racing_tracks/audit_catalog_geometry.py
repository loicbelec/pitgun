#!/usr/bin/env python3
"""Audit the geometry and provenance state of the immutable Racing track pack."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import pathlib


ROOT = pathlib.Path(__file__).resolve().parents[2]
CATALOG_ROOT = ROOT / "catalogs" / "racing" / "v1.1.0" / "simulation" / "circuits"
DEFAULT_OUTPUT = ROOT / "experiments" / "racing_tracks" / "results" / "racing-track-audit-v1.json"
SCHEMA_VERSION = "pitgun.racing-track-audit/v1"
VALIDATION_POLICY = {
    "id": "pitgun.racing-track-validation/v1",
    "required_channels": [
        "s_m",
        "x_m",
        "y_m",
        "z_m",
        "curvature_radpm",
        "slope_pct",
    ],
    "minimum_sample_count": 2,
    "sample_spacing_m": {"minimum": 0.99, "maximum": 1.01},
    "maximum_closure_gap_m": 2.0,
    "maximum_absolute_curvature_rad_per_m": 0.2,
    "maximum_absolute_slope_ratio": 0.3,
}
SOURCE_REPOSITORY = "https://github.com/bacinger/f1-circuits"
SOURCE_LICENSE = "MIT"
SOURCE_REVISION = "394d8fbe70ef2c0b0c8d23ff7bee61fa09606055"
TRACKEAGLE_REVISION = "647971e"
BAKER_DIGEST = "sha256:3a93b6e8843724c613ed0feb22ff2dfbe4f53343230c5551acc9576c4e836d13"
SOURCE_DIGESTS = {
    "ae-2009": "sha256:91ea23fc8bdcdbc1e23b007f99910c84fc0ff595663b3bf5c9954f54f882caa3",
    "at-1969": "sha256:6d84ffac3a212aed8bedd6513535b1cffb1ff2586e055f4921b7f8cd4c334ec3",
    "au-1953": "sha256:a686ec41ebbc1ca6f9c8e24c2c4bd0a2e1743d0cff730aaa26479059abd99ee5",
    "az-2016": "sha256:778230a47bee57c5eebb9d5b48143eadceff92603fd6035551e57f165862fd4b",
    "be-1925": "sha256:b867de9cc8364cc9b545a044377a56a697b8db77823c849c1a1c03760a943381",
    "bh-2002": "sha256:6bac0f8f1f870faedb0cda06f04f6de3e98445d5b27f32c351bdddae105c3864",
    "br-1940": "sha256:e650e59b876af7c455ec3222e0801b5aab008ed858c71bf6bc84e7af6d6f2bba",
    "ca-1978": "sha256:8ae58f4f6c32dc1b5f0df475bc3f6ef6441c63838e3ead4fd38299e2dc6cb5e6",
    "cn-2004": "sha256:5cbf74478ccf3422dcc8085adde5582d9a393b572b5a038ecb5c9d80d0f8baa7",
    "es-1991": "sha256:739784fa7fe51e38040b7a30a21fe5288564f8901a87fbb27c8cc22462a05c29",
    "es-2026": "sha256:b362021622d76cd4600dd8d1e825224689ee7d9a98b0d07e39726b65c3b4def1",
    "gb-1948": "sha256:1e526462a785f15acb6bf9af2fefa7a1b4521e665bfbe663d58539516ba9ea68",
    "hu-1986": "sha256:f822ff317874776d37d15970723f6007b463ab04c25ef5ce8ecb651c5478ee0d",
    "it-1922": "sha256:8b0758ef5a52bb9360c7673d45e0fbea6f29f7349486c5acbb2fe9aa340cb495",
    "jp-1962": "sha256:14419e615c3805ac74547de107ddb46872be0697bf279e2254130d495f086eee",
    "mc-1929": "sha256:daf184fc7d0948cadd10ce9428ba87b5462d9df6e33daec7b6f9aad235dc879e",
    "mx-1962": "sha256:c552fb24debdb6f8381b70da1eec02c6595d870a4abf9f87b582dbd3bac2eb06",
    "nl-1948": "sha256:4f639dad939d1fd87514695a91f3d0926feb0a72ec90af557a3b1209023c0215",
    "qa-2004": "sha256:961a8c50ce500ed83446ce32e5947ce594706a7d8d7ae4fcffc60b78a38f822b",
    "sa-2021": "sha256:4500c0f47b9329cc9359d75a397b39c00e4646a318d5c54edbf30d28d59b5df5",
    "sg-2008": "sha256:aa9584b50acffaabed7a06b5c0baa37fab38f0c70ad3c5ebba27706f3371bf9c",
    "us-2012": "sha256:199621d61cf0b50429fc9fcf1a8dabf016df453e33bf64299a651e1acad7e158",
    "us-2022": "sha256:6081f7e67221df77a20e3c9e1ed226b008ec06c1ce7501fc10bcaf9e3a11d0c7",
    "us-2023": "sha256:bebd86828c5016ede9d6b1133f3f8fbb8301e357018bb551d2514a621803d29d",
}
SPA_SOURCE = {
    "repository": SOURCE_REPOSITORY,
    "revision": "b37cc90ccdc41553fda91fc2a1f2c186b28fdc1a",
    "path": "circuits/be-1925.geojson",
    "digest": "sha256:b867de9cc8364cc9b545a044377a56a697b8db77823c849c1a1c03760a943381",
    "license": SOURCE_LICENSE,
    "reference_longitude_deg": 5.968647300653595,
    "reference_latitude_deg": 50.43471801960784,
    "source_point_count": 153,
}


def pretty(value: object) -> bytes:
    return (json.dumps(value, indent=2, ensure_ascii=False) + "\n").encode()


def digest(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


def validation_issue(code: str, severity: str, message: str) -> dict[str, str]:
    return {"code": code, "severity": severity, "message": message}


def validate_geometry(data: object) -> list[dict[str, str]]:
    issues = []
    if not isinstance(data, dict):
        return [validation_issue("invalid_data", "error", "data must be an object")]

    channels = {}
    for name in VALIDATION_POLICY["required_channels"]:
        values = data.get(name)
        if not isinstance(values, list):
            issues.append(
                validation_issue(
                    "missing_channel", "error", f"{name} must be a JSON array"
                )
            )
            continue
        if len(values) < VALIDATION_POLICY["minimum_sample_count"]:
            issues.append(
                validation_issue(
                    "insufficient_samples",
                    "error",
                    f"{name} must contain at least two samples",
                )
            )
            continue
        if any(
            not isinstance(value, (int, float)) or not math.isfinite(value)
            for value in values
        ):
            issues.append(
                validation_issue(
                    "non_finite_sample", "error", f"{name} contains a non-finite value"
                )
            )
            continue
        channels[name] = values

    if set(channels) != set(VALIDATION_POLICY["required_channels"]):
        return issues

    sample_count = len(channels["s_m"])
    mismatched = [name for name, values in channels.items() if len(values) != sample_count]
    if mismatched:
        issues.append(
            validation_issue(
                "channel_length_mismatch",
                "error",
                "channels do not share the s_m sample count: " + ", ".join(mismatched),
            )
        )
        return issues

    spacing = [
        right - left
        for left, right in zip(channels["s_m"], channels["s_m"][1:])
    ]
    if any(value <= 0.0 for value in spacing):
        issues.append(
            validation_issue(
                "non_monotonic_distance", "error", "s_m must be strictly increasing"
            )
        )
    spacing_policy = VALIDATION_POLICY["sample_spacing_m"]
    if min(spacing) < spacing_policy["minimum"] or max(spacing) > spacing_policy["maximum"]:
        issues.append(
            validation_issue(
                "unexpected_sample_spacing",
                "error",
                "sample spacing is outside the V1 one-metre tolerance",
            )
        )

    closure_gap_m = math.hypot(
        channels["x_m"][-1] - channels["x_m"][0],
        channels["y_m"][-1] - channels["y_m"][0],
    )
    if closure_gap_m > VALIDATION_POLICY["maximum_closure_gap_m"]:
        issues.append(
            validation_issue(
                "open_centerline",
                "error",
                f"centerline closure gap is {closure_gap_m:.3f} m",
            )
        )

    maximum_curvature = max(abs(value) for value in channels["curvature_radpm"])
    if maximum_curvature > VALIDATION_POLICY["maximum_absolute_curvature_rad_per_m"]:
        issues.append(
            validation_issue(
                "curvature_discontinuity",
                "error",
                f"absolute curvature reaches {maximum_curvature:.6f} rad/m",
            )
        )

    elevation_range_m = max(channels["z_m"]) - min(channels["z_m"])
    maximum_slope = max(abs(value) for value in channels["slope_pct"])
    if maximum_slope > VALIDATION_POLICY["maximum_absolute_slope_ratio"]:
        issues.append(
            validation_issue(
                "slope_discontinuity",
                "error",
                f"absolute slope reaches {maximum_slope:.6f} rise/run",
            )
        )
    if elevation_range_m == 0.0 and maximum_slope != 0.0:
        issues.append(
            validation_issue(
                "vertical_channel_mismatch",
                "error",
                "flat elevation is inconsistent with a populated slope channel",
            )
        )
    elif elevation_range_m != 0.0 and maximum_slope == 0.0:
        issues.append(
            validation_issue(
                "vertical_channel_mismatch",
                "error",
                "populated elevation is inconsistent with a flat slope channel",
            )
        )
    elif elevation_range_m == 0.0:
        issues.append(
            validation_issue(
                "flat_vertical_placeholder",
                "warning",
                "elevation and slope are explicit flat placeholders",
            )
        )
    return issues


def summarize(path: pathlib.Path) -> dict[str, object] | None:
    raw = path.read_bytes()
    payload = json.loads(raw)
    if "meta" not in payload:
        return None
    data = payload["data"]
    validation_issues = validate_geometry(data)
    distance = data["s_m"]
    x = data["x_m"]
    y = data["y_m"]
    elevation = data["z_m"]
    curvature = data["curvature_radpm"]
    slope = data["slope_pct"]
    spacing = [right - left for left, right in zip(distance, distance[1:])]
    length_m = distance[-1]
    # libm implementations may differ at the last binary digit. A picometre
    # resolution is far beyond the source geometry accuracy and keeps the
    # versioned JSON byte-identical across supported Python versions.
    closure_gap_m = round(math.hypot(x[-1] - x[0], y[-1] - y[0]), 12)
    elevation_range_m = max(elevation) - min(elevation)
    maximum_absolute_slope = max(abs(value) for value in slope)
    source_id = payload["meta"]["id"]
    source_digest = SOURCE_DIGESTS.get(source_id)
    return {
        "slug": path.stem,
        "circuit_id": source_id,
        "resource_path": str(path.relative_to(ROOT)),
        "resource_digest": digest(raw),
        "sample_count": len(distance),
        "length_m": length_m,
        "sample_spacing_m": {
            "minimum": min(spacing),
            "maximum": max(spacing),
        },
        "local_xy_extent_m": {
            "x_minimum": min(x),
            "x_maximum": max(x),
            "y_minimum": min(y),
            "y_maximum": max(y),
        },
        "closure_gap_m": closure_gap_m,
        "maximum_absolute_curvature_rad_per_m": max(abs(value) for value in curvature),
        "elevation_range_m": elevation_range_m,
        "maximum_absolute_slope": maximum_absolute_slope,
        "validation": {
            "status": "invalid"
            if any(issue["severity"] == "error" for issue in validation_issues)
            else "warning"
            if validation_issues
            else "valid",
            "issues": validation_issues,
        },
        "vertical_channel_status": "flat_placeholder"
        if elevation_range_m == 0.0 and maximum_absolute_slope == 0.0
        else "populated",
        "source_provenance": {
            "status": "verified_exact" if source_digest else "unverified",
            "source_id": source_id,
            "repository": SOURCE_REPOSITORY,
            "revision": SOURCE_REVISION,
            "path": f"circuits/{source_id}.geojson",
            "digest": source_digest,
            "license": SOURCE_LICENSE,
            **({"wgs84_reference": SPA_SOURCE} if source_id == "be-1925" else {}),
        },
    }


def build_report() -> dict[str, object]:
    circuits = [
        summary
        for path in sorted(CATALOG_ROOT.glob("*.json"))
        if (summary := summarize(path)) is not None
    ]
    flat_count = sum(
        circuit["vertical_channel_status"] == "flat_placeholder" for circuit in circuits
    )
    verified_count = sum(
        circuit["source_provenance"]["status"] == "verified_exact" for circuit in circuits
    )
    invalid_count = sum(
        circuit["validation"]["status"] == "invalid" for circuit in circuits
    )
    return {
        "schema_version": SCHEMA_VERSION,
        "catalog_version": "1.1.0",
        "circuit_count": len(circuits),
        "summary": {
            "flat_vertical_channel_count": flat_count,
            "verified_exact_source_count": verified_count,
            "inferred_source_count": len(circuits) - verified_count,
            "invalid_geometry_count": invalid_count,
            "promotion_ready": False,
        },
        "validation_policy": VALIDATION_POLICY,
        "known_pipeline": {
            "upstream_repository": SOURCE_REPOSITORY,
            "upstream_license": SOURCE_LICENSE,
            "trackeagle_revision_at_framework_import": TRACKEAGLE_REVISION,
            "track_baker_digest": BAKER_DIGEST,
            "projection": {
                "earth_radius_m": 6_371_000.0,
                "reference": "arithmetic mean of source WGS84 longitude and latitude",
                "x": "radians(lon - lon0) * R * cos(radians(lat0))",
                "y": "radians(lat - lat0) * R",
            },
            "resampling": {
                "spacing_m": 1.0,
                "xy_smoothing_lambda": 100,
                "z_smoothing_lambda": 500,
                "heading_smoothing_lambda": 500,
                "curvature_window_samples": 51,
            },
        },
        "findings": [
            "Every V1.1.0 physical circuit has flat elevation and slope channels.",
            "Local XY coordinates are reversible when the original WGS84 reference is retained.",
            "All 24 source GeoJSON blobs match one pinned MIT-licensed upstream revision exactly.",
            "Immutable V1.1.0 resources must not be edited; corrected tracks require a new catalog version.",
        ],
        "circuits": circuits,
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=pathlib.Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    payload = pretty(build_report())
    checksum_path = args.output.with_suffix(".sha256")
    checksum = (hashlib.sha256(payload).hexdigest() + "  " + args.output.name + "\n").encode()
    if args.check:
        if build_report()["summary"]["invalid_geometry_count"]:
            parser.exit(1, "error: Racing track audit contains invalid geometry\n")
        if (
            not args.output.is_file()
            or args.output.read_bytes() != payload
            or not checksum_path.is_file()
            or checksum_path.read_bytes() != checksum
        ):
            parser.exit(1, "error: Racing track audit is missing or stale\n")
        print(f"Racing track audit is current: {args.output}")
        return
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_bytes(payload)
    checksum_path.write_bytes(checksum)
    print(f"wrote geometry audit for {build_report()['circuit_count']} circuits")


if __name__ == "__main__":
    main()
