#!/usr/bin/env python3
"""Generate and verify the immutable Racing Catalog V1 indexes and identities."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import re
import sys
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
CATALOG_ROOT = ROOT / "catalogs" / "racing"
VERSION_PATTERN = re.compile(r"^v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$")
EMBEDDED_RUST = ROOT / "generated" / "racing_catalog_v1.rs"
MODEL_V2_EMBEDDED_RUST = ROOT / "generated" / "racing_catalog_model_v2.rs"
MODEL_V3_THERMAL_EMBEDDED_RUST = (
    ROOT / "generated" / "racing_catalog_model_v3_thermal.rs"
)
MODEL_V3_COMPONENT_EMBEDDED_RUST = (
    ROOT / "generated" / "racing_catalog_model_v3_component.rs"
)

DIGEST_PREFIX = "sha256:"
MODEL_COMPATIBILITY_BY_RELEASE = {
    "1.0.0": ("pitgun.racing", ["1.0.0"]),
    "1.1.0": ("pitgun.racing", ["1.0.0"]),
    "1.2.0": ("pitgun.racing", ["2.0.0"]),
    "1.3.0": ("pitgun.racing", ["2.0.0"]),
    "1.4.0": ("pitgun.racing", ["2.0.0"]),
    "1.5.0": ("pitgun.racing-v3-candidate", ["0.10.0"]),
    "1.6.0": ("pitgun.racing-v3-candidate", ["0.11.0"]),
}


def load_json(path: Path) -> Any:
    with path.open("r", encoding="utf-8") as file:
        return json.load(file)


def canonical_json(value: Any) -> bytes:
    return json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
        allow_nan=False,
    ).encode("utf-8")


def pretty_json(value: Any) -> bytes:
    return (
        json.dumps(value, ensure_ascii=False, indent=2, allow_nan=False) + "\n"
    ).encode("utf-8")


def digest_bytes(value: bytes) -> str:
    return DIGEST_PREFIX + hashlib.sha256(value).hexdigest()


def release_roots() -> list[Path]:
    roots = sorted(
        path
        for path in CATALOG_ROOT.iterdir()
        if path.is_dir() and VERSION_PATTERN.fullmatch(path.name)
    )
    if not roots:
        raise SystemExit("FAIL Racing catalog: no immutable release")
    return roots


def selected_release_root() -> Path:
    version = (CATALOG_ROOT / "LATEST").read_text(encoding="utf-8").strip()
    root = CATALOG_ROOT / f"v{version}"
    if root not in release_roots():
        raise SystemExit(f"FAIL Racing catalog: selected release v{version} does not exist")
    return root


def simulation_resources(release_root: Path) -> list[dict[str, str]]:
    simulation_root = release_root / "simulation"
    resources: list[dict[str, str]] = []
    for path in sorted(simulation_root.glob("*/*.json")):
        relative = path.relative_to(simulation_root).as_posix()
        category, filename = relative.split("/", maxsplit=1)
        stem = Path(filename).stem
        resources.append(
            {
                "id": f"pitgun.racing.{category}.{stem}",
                "path": f"simulation/{relative}",
                "media_type": "application/json",
                "digest": digest_bytes(path.read_bytes()),
            }
        )
    resources.sort(key=lambda resource: (resource["id"], resource["path"]))
    if not resources:
        raise SystemExit("FAIL Racing catalog: no simulation resources")
    ids = [resource["id"] for resource in resources]
    paths = [resource["path"] for resource in resources]
    if len(ids) != len(set(ids)) or len(paths) != len(set(paths)):
        raise SystemExit("FAIL Racing catalog: duplicate resource ID or path")
    return resources


def validate_pack_boundary(
    release_root: Path, presentation_index: dict[str, Any]
) -> None:
    simulation_root = release_root / "simulation"
    circuit_ids: list[str] = []
    for path in sorted((simulation_root / "circuits").glob("*.json")):
        value = load_json(path)
        meta = value.get("meta", {})
        if not isinstance(meta, dict) or set(meta) - {"id"}:
            raise SystemExit(
                f"FAIL Racing catalog: presentation metadata leaked into {path}"
            )
        circuit_ids.append(path.stem)

    driver_ids: list[str] = []
    for path in sorted((simulation_root / "drivers").glob("*.json")):
        value = load_json(path)
        if "aggressiveness" not in value:
            continue
        if "display_name" in value:
            raise SystemExit(
                f"FAIL Racing catalog: presentation metadata leaked into {path}"
            )
        driver_ids.append(path.stem)

    presented_circuits = [
        entry.get("source_id") for entry in presentation_index.get("circuits", [])
    ]
    presented_drivers = [
        entry.get("id") for entry in presentation_index.get("drivers", [])
    ]
    if presented_circuits != circuit_ids:
        raise SystemExit(
            "FAIL Racing catalog: presentation circuits do not exactly match "
            "simulation circuit resources"
        )
    if presented_drivers != driver_ids:
        raise SystemExit(
            "FAIL Racing catalog: presentation drivers do not exactly match "
            "simulation driver resources"
        )


def validate_opponent_policies(release_root: Path) -> None:
    for path in sorted((release_root / "simulation" / "policies").glob("*.json")):
        policy = load_json(path)
        if policy.get("schema_version") not in {
            "pitgun.racing-opponent-policy/v1",
            "pitgun.racing-opponent-policy/v2",
        }:
            raise SystemExit(f"FAIL Racing opponent policy {path}: unsupported schema")

        profiles = policy.get("profiles", policy.get("development_profiles", []))
        profile_ids = {profile["id"] for profile in profiles}
        if len(profile_ids) != len(profiles):
            raise SystemExit(f"FAIL Racing opponent policy {path}: duplicate profile IDs")
        role_ids = [role["id"] for role in policy["composition"]["roles"]]
        if role_ids != ["front-runner", "midfield", "challenger"]:
            raise SystemExit(f"FAIL Racing opponent policy {path}: roles are not canonical")
        for role in policy["composition"]["roles"]:
            if not set(role["eligible_profile_ids"]).issubset(profile_ids):
                raise SystemExit(
                    f"FAIL Racing opponent policy {path}: role references unknown profile"
                )
        if policy["schema_version"] == "pitgun.racing-opponent-policy/v1":
            for profile in profiles:
                for slider in profile["setup"].values():
                    if not slider["min"] <= slider["center"] <= slider["max"]:
                        raise SystemExit(
                            f"FAIL Racing opponent policy {path}: invalid setup bounds"
                        )
        else:
            if sum(role["count"] for role in policy["composition"]["roles"]) != 9:
                raise SystemExit(
                    f"FAIL Racing opponent policy {path}: field size does not reconcile"
                )
            if policy["scope"]["supported_game_eras"] != [1, 2, 3, 4, 5]:
                raise SystemExit(
                    f"FAIL Racing opponent policy {path}: supported eras changed"
                )
            if policy["scope"]["unsupported_game_eras"] != [6, 7]:
                raise SystemExit(
                    f"FAIL Racing opponent policy {path}: late eras must remain disabled"
                )


def generated_release_artifacts(release_root: Path) -> dict[Path, bytes]:
    version = release_root.name.removeprefix("v")
    try:
        compatible_model_id, compatible_model_versions = (
            MODEL_COMPATIBILITY_BY_RELEASE[version]
        )
    except KeyError as error:
        raise SystemExit(
            f"FAIL Racing catalog: model compatibility is not governed for {version}"
        ) from error
    simulation_index_path = release_root / "simulation" / "index.json"
    presentation_index_path = release_root / "presentation" / "index.json"
    catalog_manifest_path = release_root / "catalog.json"
    release_identity_path = release_root / "release.json"
    resources = simulation_resources(release_root)
    simulation_index = {
        "schema_version": "pitgun.racing-simulation-index/v1",
        "resources": resources,
    }
    presentation_index = load_json(presentation_index_path)
    if (
        presentation_index.get("schema_version")
        != "pitgun.racing-presentation-index/v1"
    ):
        raise SystemExit("FAIL Racing catalog: unsupported presentation index")
    validate_pack_boundary(release_root, presentation_index)
    validate_opponent_policies(release_root)

    simulation_digest = digest_bytes(canonical_json(simulation_index))
    presentation_digest = digest_bytes(canonical_json(presentation_index))
    manifest = {
        "schema_version": "pitgun.resource-catalog/v1",
        "catalog": {
            "id": "pitgun.racing",
            "version": version,
        },
        "simulation_pack": {
            "identity": {
                "id": "pitgun.racing.simulation",
                "version": version,
                "digest": simulation_digest,
            },
            "index": {
                "path": "simulation/index.json",
                "media_type": "application/json",
                "digest": simulation_digest,
            },
        },
        "presentation_pack": {
            "identity": {
                "id": "pitgun.racing.presentation",
                "version": version,
                "digest": presentation_digest,
            },
            "index": {
                "path": "presentation/index.json",
                "media_type": "application/json",
                "digest": presentation_digest,
            },
        },
        "compatibility": {
            "schema_version": "pitgun.catalog-compatibility/v1",
            "contract_versions": ["pitgun.deterministic-run/v1"],
            "models": [
                {
                    "id": compatible_model_id,
                    "versions": compatible_model_versions,
                }
            ],
        },
    }
    identity = {
        "schema_version": "pitgun.catalog-release-identity/v1",
        "id": "pitgun.racing",
        "version": version,
        "manifest_digest": digest_bytes(canonical_json(manifest)),
    }

    return {
        simulation_index_path: pretty_json(simulation_index),
        catalog_manifest_path: pretty_json(manifest),
        release_identity_path: pretty_json(identity),
    }


def generated_embedded_artifact(
    release_root: Path, constant_name: str = "EMBEDDED_FILES"
) -> bytes:
    resources = simulation_resources(release_root)
    embedded_lines = [
        "// Generated by scripts/generate-racing-catalog.py; do not edit.",
        f"const {constant_name}: &[(&str, &[u8])] = &[",
    ]
    for resource in resources:
        path = resource["path"].removeprefix("simulation/")
        embedded_lines.extend(
            [
                "    (",
                f'        "{path}",',
                "        include_bytes!(concat!(",
                '            env!("CARGO_MANIFEST_DIR"),',
                f'            "/../../catalogs/racing/{release_root.name}/simulation/{path}"',
                "        )),",
                "    ),",
            ]
        )
    embedded_lines.extend(["];", ""])

    return "\n".join(embedded_lines).encode("utf-8")


def generated_artifacts() -> dict[Path, bytes]:
    artifacts: dict[Path, bytes] = {}
    for release_root in release_roots():
        artifacts.update(generated_release_artifacts(release_root))
    selected = selected_release_root()
    artifacts[EMBEDDED_RUST] = generated_embedded_artifact(selected)
    artifacts[MODEL_V2_EMBEDDED_RUST] = generated_embedded_artifact(
        CATALOG_ROOT / "v1.2.0", "MODEL_V2_EMBEDDED_FILES"
    )
    artifacts[MODEL_V3_THERMAL_EMBEDDED_RUST] = generated_embedded_artifact(
        CATALOG_ROOT / "v1.5.0", "MODEL_V3_THERMAL_EMBEDDED_FILES"
    )
    artifacts[MODEL_V3_COMPONENT_EMBEDDED_RUST] = generated_embedded_artifact(
        CATALOG_ROOT / "v1.6.0", "MODEL_V3_COMPONENT_EMBEDDED_FILES"
    )
    return artifacts


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--check",
        action="store_true",
        help="fail if checked-in generated artifacts are stale",
    )
    args = parser.parse_args()

    artifacts = generated_artifacts()
    stale: list[Path] = []
    for path, expected in artifacts.items():
        if args.check:
            if not path.exists() or path.read_bytes() != expected:
                stale.append(path)
            continue
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(expected)
        print(f"WROTE {path.relative_to(ROOT)}")

    if stale:
        for path in stale:
            print(f"STALE {path.relative_to(ROOT)}")
        print("Run: python3 scripts/generate-racing-catalog.py")
        return 1
    if args.check:
        resource_count = sum(len(simulation_resources(root)) for root in release_roots())
        print(
            f"OK Racing Catalog ({len(release_roots())} releases, "
            f"{resource_count} indexed resources)"
        )
    return 0


if __name__ == "__main__":
    sys.exit(main())
