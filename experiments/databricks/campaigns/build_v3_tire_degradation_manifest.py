#!/usr/bin/env python3
"""Build the immutable Databricks manifest for Racing V3 tire degradation."""

from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import re
import subprocess
import sys
import tempfile
from typing import Any


ROOT = pathlib.Path(__file__).resolve().parents[3]
CAMPAIGN_ROOT = ROOT / "experiments" / "databricks" / "campaigns"
NAME = "racing-v3-tire-degradation-v1"
SCHEMA_VERSION = "pitgun.racing-v3-physics-campaign/v1"

sys.path.insert(0, str(ROOT))
from experiments.racing_v3_tire_degradation import validate_local as campaign  # noqa: E402


def canonical_pretty(value: object) -> bytes:
    return (json.dumps(value, indent=2, ensure_ascii=False) + "\n").encode()


def canonical_digest(value: object) -> str:
    payload = json.dumps(value, sort_keys=True, separators=(",", ":")).encode()
    return "sha256:" + hashlib.sha256(payload).hexdigest()


def configuration_id(point: dict[str, Any]) -> str:
    raw = "-".join(
        (point["circuit_slug"], point["vehicle_id"], point["case_id"])
    ).lower()
    identifier = re.sub(r"[^a-z0-9]+", "-", raw).strip("-")
    if not identifier:
        raise RuntimeError("campaign point produced an empty identifier")
    return identifier


def probe_identity(
    probe: pathlib.Path,
    scenario: dict[str, Any],
    profile: dict[str, Any],
    seed: int,
) -> dict[str, Any]:
    with tempfile.TemporaryDirectory(prefix="pitgun-v3-manifest-") as directory:
        root = pathlib.Path(directory)
        scenario_path = root / "scenario.json"
        profile_path = root / "profile.json"
        scenario_path.write_bytes(canonical_pretty(scenario))
        profile_path.write_bytes(canonical_pretty(profile))
        completed = subprocess.run(
            [str(probe), str(scenario_path), str(profile_path), str(seed)],
            cwd=ROOT,
            check=True,
            capture_output=True,
        )
    if completed.stderr:
        raise RuntimeError("successful V3 manifest probe wrote to stderr")
    result = json.loads(completed.stdout)
    if result.get("schema_version") != "pitgun.racing-v3-validation-probe/v1":
        raise RuntimeError("V3 manifest probe returned an unsupported contract")
    return result


def build(probe: pathlib.Path) -> bytes:
    base_scenario = json.loads(campaign.shared.BASE_SCENARIO.read_bytes())
    base_profile = json.loads(campaign.PROFILE.read_bytes())
    plan = campaign.build_plan(base_scenario, base_profile)
    configurations = []
    model = None
    for point in plan:
        identity = probe_identity(
            probe, point["scenario"], point["profile"], int(point["seed"])
        )
        if model is None:
            model = identity["model"]
        elif identity["model"] != model:
            raise RuntimeError("campaign mixes V3 model identities")
        metadata = {
            key: value
            for key, value in point.items()
            if key not in {"scenario", "profile", "seed"}
        }
        point_id = configuration_id(point)
        experimental_configuration_id = canonical_digest(
            {
                "analysis_role": point_id,
                "profile_digest": identity["profile_digest"],
                "scenario_digest": identity["scenario_digest"],
            }
        )
        configurations.append(
            {
                "id": point_id,
                "family": point["family"],
                "circuit_id": point["circuit_id"],
                "circuit_archetype": point["circuit_archetype"],
                "vehicle_id": point["vehicle_id"],
                "era": point["era"],
                "seed": str(point["seed"]),
                "response_id": "v7-" + identity["profile_digest"].split(":", 1)[1][0:12],
                "expected_scenario_digest": identity["scenario_digest"],
                "expected_profile_digest": identity["profile_digest"],
                "expected_experimental_configuration_id": experimental_configuration_id,
                "expected_experimental_execution_id": identity[
                    "experimental_execution_id"
                ],
                "metadata": metadata,
                "scenario": point["scenario"],
                "profile": point["profile"],
            }
        )

    if model is None:
        raise RuntimeError("campaign plan is empty")
    if len({row["id"] for row in configurations}) != len(configurations):
        raise RuntimeError("generated configuration identifiers are not unique")

    source = base_scenario
    manifest = {
        "schema_version": SCHEMA_VERSION,
        "campaign_id": "racing-v3-tire-degradation-2026-v1",
        "question": "Does compound-dependent wear create observable, explainable tire and strategy trade-offs across representative circuits and vehicle generations?",
        "parameter_space_version": "7.0.0",
        "execution_class": "experimental-v3-physics",
        "promotion_policy": "human-review-required",
        "automatic_catalog_promotion": False,
        "scenario": {
            "id": "racing.v3-tire-degradation-campaign",
            "version": "1.0.0",
        },
        "model": model,
        "data_pack": source["data_pack"],
        "circuits": [
            {"id": identifier, "slug": slug, "archetype": archetype}
            for identifier, slug, archetype in campaign.shared.CIRCUITS
        ],
        "vehicles": [
            {"id": identifier, "era": era}
            for identifier, era in campaign.shared.VEHICLES
        ],
        "dimensions": {
            "compounds": list(campaign.COMPOUNDS),
            "fuel_loads_kg": list(campaign.FUEL_LOADS_KG),
            "stop_windows": list(campaign.STOP_WINDOWS),
            "driver_levels": sorted(campaign.DRIVER_LEVELS),
            "thermal_gains": list(campaign.THERMAL_GAINS),
            "workload_energies_j": list(campaign.WORKLOAD_ENERGIES_J),
        },
        "configurations": configurations,
        "acceptance_criteria": {
            "execution_success_rate_min": 1.0,
            "compound_wear_order": ["soft", "medium", "hard"],
            "runtime_parameter_signal_required": True,
            "automatic_release": False,
        },
        "planned_run_count": len(configurations),
        "unique_physical_execution_count": len(
            {
                configuration["expected_experimental_execution_id"]
                for configuration in configurations
            }
        ),
    }
    return canonical_pretty(manifest)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--probe", type=pathlib.Path, required=True)
    parser.add_argument("--check", action="store_true")
    arguments = parser.parse_args()
    payload = build(arguments.probe.resolve())
    manifest_path = CAMPAIGN_ROOT / f"{NAME}.json"
    checksum_path = CAMPAIGN_ROOT / f"{NAME}.sha256"
    checksum = f"{hashlib.sha256(payload).hexdigest()}  {manifest_path.name}\n"
    if arguments.check:
        if (
            manifest_path.read_bytes() != payload
            or checksum_path.read_text() != checksum
        ):
            raise SystemExit("V3 tire-degradation manifest is stale")
        return
    manifest_path.write_bytes(payload)
    checksum_path.write_text(checksum)


if __name__ == "__main__":
    main()
