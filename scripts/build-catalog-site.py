#!/usr/bin/env python3
"""Build and validate the static catalog.pitgun.io publication tree."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import sys
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
CATALOGS_ROOT = ROOT / "catalogs"
HTACCESS_SOURCE = CATALOGS_ROOT / ".htaccess"
VERSION_PATTERN = re.compile(r"^v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$")
DIGEST_PREFIX = "sha256:"
POINTER_SCHEMA = "pitgun.catalog-pointer/v1"
IMMUTABLE_HTACCESS = """<IfModule mod_headers.c>
  Header always set Cache-Control "public, max-age=31536000, immutable"
</IfModule>
"""


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


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def validate_release(
    domain: str, release_root: Path
) -> tuple[tuple[int, int, int], dict[str, Any]]:
    version_match = VERSION_PATTERN.fullmatch(release_root.name)
    require(version_match is not None, f"invalid release directory: {release_root}")
    version_key = tuple(int(part) for part in version_match.groups())
    version = release_root.name.removeprefix("v")

    manifest_path = release_root / "catalog.json"
    identity_path = release_root / "release.json"
    require(manifest_path.is_file(), f"missing {manifest_path}")
    require(identity_path.is_file(), f"missing {identity_path}")

    manifest = load_json(manifest_path)
    identity = load_json(identity_path)
    catalog = manifest.get("catalog", {})
    expected_id = f"pitgun.{domain}"
    require(
        catalog.get("id") == expected_id,
        f"{manifest_path}: expected catalog id {expected_id}",
    )
    require(catalog.get("version") == version, f"{manifest_path}: version mismatch")
    require(identity.get("id") == expected_id, f"{identity_path}: catalog id mismatch")
    require(identity.get("version") == version, f"{identity_path}: version mismatch")

    manifest_digest = digest_bytes(canonical_json(manifest))
    require(
        identity.get("manifest_digest") == manifest_digest,
        f"{identity_path}: manifest digest mismatch",
    )

    for pack_name in ("simulation_pack", "presentation_pack"):
        pack = manifest.get(pack_name, {})
        pack_identity = pack.get("identity", {})
        index = pack.get("index", {})
        index_path = release_root / str(index.get("path", ""))
        require(index_path.is_file(), f"{manifest_path}: missing {pack_name} index")
        index_digest = digest_bytes(canonical_json(load_json(index_path)))
        require(index.get("digest") == index_digest, f"{index_path}: index digest mismatch")
        require(
            pack_identity.get("digest") == index_digest,
            f"{manifest_path}: {pack_name} identity digest mismatch",
        )

    simulation_index = load_json(
        release_root / manifest["simulation_pack"]["index"]["path"]
    )
    resources = simulation_index.get("resources")
    require(isinstance(resources, list) and resources, "simulation index has no resources")
    for resource in resources:
        resource_path = release_root / str(resource.get("path", ""))
        require(resource_path.is_file(), f"missing catalog resource: {resource_path}")
        actual = digest_bytes(resource_path.read_bytes())
        require(
            resource.get("digest") == actual,
            f"{resource_path}: exact byte digest mismatch",
        )

    pointer = {
        "schema_version": POINTER_SCHEMA,
        "catalog": {
            "id": expected_id,
            "version": version,
        },
        "manifest": {
            "path": f"{release_root.name}/catalog.json",
            "digest": manifest_digest,
        },
        "release_identity": {
            "path": f"{release_root.name}/release.json",
        },
    }
    return version_key, pointer


def build(output_root: Path) -> int:
    require(output_root.resolve() != CATALOGS_ROOT.resolve(), "output must not replace catalogs/")
    if output_root.exists():
        shutil.rmtree(output_root)
    output_root.mkdir(parents=True)
    shutil.copy2(HTACCESS_SOURCE, output_root / ".htaccess")

    domains = 0
    releases = 0
    for domain_root in sorted(path for path in CATALOGS_ROOT.iterdir() if path.is_dir()):
        version_roots = sorted(
            path
            for path in domain_root.iterdir()
            if path.is_dir() and VERSION_PATTERN.fullmatch(path.name)
        )
        if not version_roots:
            continue

        validated = [
            (*validate_release(domain_root.name, release_root), release_root)
            for release_root in version_roots
        ]
        selected_version = (
            (domain_root / "LATEST").read_text(encoding="utf-8").strip()
        )
        require(
            VERSION_PATTERN.fullmatch(f"v{selected_version}") is not None,
            f"{domain_root / 'LATEST'}: expected a stable semantic version",
        )
        selected = next(
            (
                item
                for item in validated
                if item[2].name == f"v{selected_version}"
            ),
            None,
        )
        require(
            selected is not None,
            f"{domain_root / 'LATEST'}: selected release v{selected_version} does not exist",
        )
        target_domain = output_root / domain_root.name
        target_domain.mkdir()
        for _, _, release_root in validated:
            target_release = target_domain / release_root.name
            shutil.copytree(release_root, target_release)
            (target_release / ".htaccess").write_text(
                IMMUTABLE_HTACCESS, encoding="utf-8"
            )
            releases += 1

        _, latest_pointer, _ = selected
        (target_domain / "latest.json").write_bytes(pretty_json(latest_pointer))
        domains += 1

    require(domains > 0, "no catalog domains found")
    print(f"OK catalog site: {domains} domain(s), {releases} immutable release(s)")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        return build(args.output)
    except (OSError, ValueError, KeyError, TypeError, json.JSONDecodeError) as error:
        print(f"FAIL catalog site: {error}")
        return 1


if __name__ == "__main__":
    sys.exit(main())
