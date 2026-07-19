#!/usr/bin/env python3
"""Validate public Pitgun JSON Schemas and documented gateway examples."""

from __future__ import annotations

import json
from pathlib import Path
import sys

import jsonschema


ROOT = Path(__file__).resolve().parents[1]
SCHEMAS_DIR = ROOT / "schemas"
GATEWAY_EXAMPLES_DIR = ROOT / "services" / "pitgun-gateway" / "examples"
PUBLIC_BASE_URL = "https://schemas.pitgun.io"


def load_json(path: Path) -> object:
    with path.open("r", encoding="utf-8") as file:
        return json.load(file)


def schema_files() -> list[Path]:
    return sorted(
        path for path in SCHEMAS_DIR.glob("**/*.json") if path.name != "index.json"
    )


def validate_schema_files() -> None:
    for schema_path in schema_files():
        schema = load_json(schema_path)
        jsonschema.Draft202012Validator.check_schema(schema)
        relative_path = schema_path.relative_to(SCHEMAS_DIR).as_posix()
        expected_id = f"{PUBLIC_BASE_URL}/{relative_path}"
        if schema.get("$id") != expected_id:
            raise SystemExit(
                f"FAIL schema {schema_path.relative_to(ROOT)}: "
                f"expected $id {expected_id!r}, got {schema.get('$id')!r}"
            )
        print(f"OK schema {schema_path.relative_to(ROOT)}")


def validate_catalog() -> None:
    catalog_path = SCHEMAS_DIR / "index.json"
    catalog = load_json(catalog_path)
    if catalog.get("base_url") != PUBLIC_BASE_URL:
        raise SystemExit("FAIL catalog: unexpected base_url")

    entries = catalog.get("schemas")
    if not isinstance(entries, list):
        raise SystemExit("FAIL catalog: schemas must be an array")

    catalog_entries = {
        (entry.get("path"), entry.get("id"))
        for entry in entries
        if isinstance(entry, dict)
    }
    expected_entries = {
        (
            path.relative_to(SCHEMAS_DIR).as_posix(),
            f"{PUBLIC_BASE_URL}/{path.relative_to(SCHEMAS_DIR).as_posix()}",
        )
        for path in schema_files()
    }
    if catalog_entries != expected_entries:
        raise SystemExit("FAIL catalog: entries do not match the published schema files")
    print(f"OK catalog {catalog_path.relative_to(ROOT)}")


def validate_gateway_examples() -> None:
    schema_path = SCHEMAS_DIR / "pitgun-envelope" / "v1.json"
    schema = load_json(schema_path)
    validator = jsonschema.Draft202012Validator(schema)

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
    validate_schema_files()
    validate_catalog()
    validate_gateway_examples()
    return 0


if __name__ == "__main__":
    sys.exit(main())
