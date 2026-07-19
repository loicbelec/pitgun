#!/usr/bin/env python3
"""Validate the public Pitgun contract catalog and executable examples."""

from __future__ import annotations

import json
from pathlib import Path
import sys

import jsonschema


ROOT = Path(__file__).resolve().parents[1]
SCHEMAS_DIR = ROOT / "schemas"
GATEWAY_EXAMPLES_DIR = ROOT / "services" / "pitgun-gateway" / "examples"
PUBLIC_BASE_URL = "https://schemas.pitgun.io"
ALLOWED_KINDS = {"json-schema", "data-dictionary"}
ALLOWED_LIFECYCLES = {"active", "experimental", "legacy"}


def load_json(path: Path) -> object:
    with path.open("r", encoding="utf-8") as file:
        return json.load(file)


def artifact_files() -> list[Path]:
    return sorted(
        path for path in SCHEMAS_DIR.glob("**/*.json") if path.name != "index.json"
    )


def validate_catalog() -> list[dict[str, object]]:
    catalog_path = SCHEMAS_DIR / "index.json"
    catalog = load_json(catalog_path)
    if catalog.get("base_url") != PUBLIC_BASE_URL:
        raise SystemExit("FAIL catalog: unexpected base_url")

    if catalog.get("catalog_version") != 2:
        raise SystemExit("FAIL catalog: expected catalog_version 2")

    entries = catalog.get("artifacts")
    if not isinstance(entries, list):
        raise SystemExit("FAIL catalog: artifacts must be an array")

    if not all(isinstance(entry, dict) for entry in entries):
        raise SystemExit("FAIL catalog: every artifact must be an object")

    typed_entries: list[dict[str, object]] = entries
    for entry in typed_entries:
        path = entry.get("path")
        artifact_id = entry.get("id")
        kind = entry.get("kind")
        lifecycle = entry.get("lifecycle")
        owner = entry.get("owner")
        evidence = entry.get("evidence")

        if not isinstance(path, str) or not path:
            raise SystemExit("FAIL catalog: every artifact needs a path")
        expected_id = f"{PUBLIC_BASE_URL}/{path}"
        if artifact_id != expected_id:
            raise SystemExit(f"FAIL catalog {path}: unexpected id")
        if kind not in ALLOWED_KINDS:
            raise SystemExit(f"FAIL catalog {path}: unsupported kind {kind!r}")
        if lifecycle not in ALLOWED_LIFECYCLES:
            raise SystemExit(f"FAIL catalog {path}: unsupported lifecycle {lifecycle!r}")
        if not isinstance(owner, str) or not owner:
            raise SystemExit(f"FAIL catalog {path}: owner must be a non-empty string")
        if not isinstance(evidence, list) or not all(
            isinstance(item, str) and item for item in evidence
        ):
            raise SystemExit(f"FAIL catalog {path}: evidence must be an array of paths")
        if lifecycle == "active" and not evidence:
            raise SystemExit(f"FAIL catalog {path}: active artifacts need evidence")
        for evidence_path in evidence:
            if not (ROOT / evidence_path).exists():
                raise SystemExit(
                    f"FAIL catalog {path}: evidence path does not exist: {evidence_path}"
                )

    catalog_entries = {
        (entry.get("path"), entry.get("id"))
        for entry in typed_entries
    }
    expected_entries = {
        (
            path.relative_to(SCHEMAS_DIR).as_posix(),
            f"{PUBLIC_BASE_URL}/{path.relative_to(SCHEMAS_DIR).as_posix()}",
        )
        for path in artifact_files()
    }
    if catalog_entries != expected_entries:
        raise SystemExit("FAIL catalog: entries do not match the published artifacts")
    print(f"OK catalog {catalog_path.relative_to(ROOT)}")
    return typed_entries


def validate_artifacts(entries: list[dict[str, object]]) -> None:
    for entry in entries:
        relative_path = str(entry["path"])
        artifact_path = SCHEMAS_DIR / relative_path
        artifact = load_json(artifact_path)
        expected_id = f"{PUBLIC_BASE_URL}/{relative_path}"

        if entry["kind"] == "json-schema":
            jsonschema.Draft202012Validator.check_schema(artifact)
            identifier = artifact.get("$id")
        else:
            identifier = artifact.get("id")
            if not isinstance(artifact.get("document_type"), str):
                raise SystemExit(
                    f"FAIL dictionary {artifact_path.relative_to(ROOT)}: "
                    "missing document_type"
                )

        if identifier != expected_id:
            raise SystemExit(
                f"FAIL artifact {artifact_path.relative_to(ROOT)}: "
                f"expected identifier {expected_id!r}, got {identifier!r}"
            )
        print(f"OK {entry['kind']} {artifact_path.relative_to(ROOT)}")


def validate_gateway_examples() -> None:
    schema_path = SCHEMAS_DIR / "pitgun-envelope" / "v1.json"
    schema = load_json(schema_path)
    validator = jsonschema.Draft202012Validator(
        schema, format_checker=jsonschema.FormatChecker()
    )

    for example_path in sorted(GATEWAY_EXAMPLES_DIR.glob("*.json")):
        payload = load_json(example_path)
        errors = sorted(validator.iter_errors(payload), key=lambda error: error.path)
        if errors:
            print(f"FAIL example {example_path.relative_to(ROOT)}")
            for error in errors:
                location = ".".join(str(part) for part in error.path) or "<root>"
                print(f"  {location}: {error.message}")
            raise SystemExit(1)
        print(f"OK example {example_path.relative_to(ROOT)}")


def main() -> int:
    entries = validate_catalog()
    validate_artifacts(entries)
    validate_gateway_examples()
    return 0


if __name__ == "__main__":
    sys.exit(main())
