#!/usr/bin/env python3
"""Validate public Pitgun JSON Schemas and documented gateway examples."""

from __future__ import annotations

import json
from pathlib import Path
import sys

import jsonschema


ROOT = Path(__file__).resolve().parents[1]
SCHEMAS_DIR = ROOT / "portal" / "schemas"
GATEWAY_EXAMPLES_DIR = ROOT / "services" / "pitgun-gateway" / "examples"


def load_json(path: Path) -> object:
    with path.open("r", encoding="utf-8") as file:
        return json.load(file)


def validate_schema_files() -> None:
    for schema_path in sorted(SCHEMAS_DIR.glob("**/*.json")):
        schema = load_json(schema_path)
        jsonschema.Draft202012Validator.check_schema(schema)
        print(f"OK schema {schema_path.relative_to(ROOT)}")


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
    validate_gateway_examples()
    return 0


if __name__ == "__main__":
    sys.exit(main())
