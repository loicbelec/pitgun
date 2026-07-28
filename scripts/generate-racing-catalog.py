#!/usr/bin/env python3
"""Generate and verify the immutable Racing Catalog V1 indexes and identities."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import sys
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
RELEASE_ROOT = ROOT / "catalogs" / "racing" / "v1.0.0"
SIMULATION_ROOT = RELEASE_ROOT / "simulation"
PRESENTATION_INDEX = RELEASE_ROOT / "presentation" / "index.json"
SIMULATION_INDEX = SIMULATION_ROOT / "index.json"
CATALOG_MANIFEST = RELEASE_ROOT / "catalog.json"
RELEASE_IDENTITY = RELEASE_ROOT / "release.json"
EMBEDDED_RUST = ROOT / "generated" / "racing_catalog_v1.rs"

DIGEST_PREFIX = "sha256:"


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


def simulation_resources() -> list[dict[str, str]]:
    resources: list[dict[str, str]] = []
    for path in sorted(SIMULATION_ROOT.glob("*/*.json")):
        relative = path.relative_to(SIMULATION_ROOT).as_posix()
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


def validate_pack_boundary(presentation_index: dict[str, Any]) -> None:
    circuit_ids: list[str] = []
    for path in sorted((SIMULATION_ROOT / "circuits").glob("*.json")):
        value = load_json(path)
        meta = value.get("meta", {})
        if not isinstance(meta, dict) or set(meta) - {"id"}:
            raise SystemExit(
                f"FAIL Racing catalog: presentation metadata leaked into {path}"
            )
        circuit_ids.append(path.stem)

    driver_ids: list[str] = []
    for path in sorted((SIMULATION_ROOT / "drivers").glob("*.json")):
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


def generated_artifacts() -> dict[Path, bytes]:
    resources = simulation_resources()
    simulation_index = {
        "schema_version": "pitgun.racing-simulation-index/v1",
        "resources": resources,
    }
    presentation_index = load_json(PRESENTATION_INDEX)
    if (
        presentation_index.get("schema_version")
        != "pitgun.racing-presentation-index/v1"
    ):
        raise SystemExit("FAIL Racing catalog: unsupported presentation index")
    validate_pack_boundary(presentation_index)

    simulation_digest = digest_bytes(canonical_json(simulation_index))
    presentation_digest = digest_bytes(canonical_json(presentation_index))
    manifest = {
        "schema_version": "pitgun.resource-catalog/v1",
        "catalog": {
            "id": "pitgun.racing",
            "version": "1.0.0",
        },
        "simulation_pack": {
            "identity": {
                "id": "pitgun.racing.simulation",
                "version": "1.0.0",
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
                "version": "1.0.0",
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
                    "id": "pitgun.racing",
                    "versions": ["1.0.0"],
                }
            ],
        },
    }
    identity = {
        "schema_version": "pitgun.catalog-release-identity/v1",
        "id": "pitgun.racing",
        "version": "1.0.0",
        "manifest_digest": digest_bytes(canonical_json(manifest)),
    }

    embedded_lines = [
        "// Generated by scripts/generate-racing-catalog.py; do not edit.",
        "const EMBEDDED_FILES: &[(&str, &[u8])] = &[",
    ]
    for resource in resources:
        path = resource["path"].removeprefix("simulation/")
        embedded_lines.extend(
            [
                "    (",
                f'        "{path}",',
                "        include_bytes!(concat!(",
                '            env!("CARGO_MANIFEST_DIR"),',
                f'            "/../../catalogs/racing/v1.0.0/simulation/{path}"',
                "        )),",
                "    ),",
            ]
        )
    embedded_lines.extend(["];", ""])

    return {
        SIMULATION_INDEX: pretty_json(simulation_index),
        CATALOG_MANIFEST: pretty_json(manifest),
        RELEASE_IDENTITY: pretty_json(identity),
        EMBEDDED_RUST: "\n".join(embedded_lines).encode("utf-8"),
    }


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
        print(f"OK Racing Catalog V1 ({len(simulation_resources())} resources)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
