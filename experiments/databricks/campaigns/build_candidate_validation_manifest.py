#!/usr/bin/env python3
"""Build the immutable governed candidate-validation campaign."""

from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import subprocess


ROOT = pathlib.Path(__file__).resolve().parents[3]
CAMPAIGN_ROOT = ROOT / "experiments" / "databricks" / "campaigns"
SCENARIO_ROOT = ROOT / "apps" / "pitgun-cli" / "scenarios" / "racing-circuit-sweep-v1"
SOURCE = CAMPAIGN_ROOT / "racing-circuit-sweep-v1.json"
NAME = "racing-aero-candidate-validation-v1"
RESPONSES = (
    ("historical-v1", "racing-historical-v1"),
    ("aero-candidate-v1", "racing-aero-candidate-v1"),
)


def sha256_json(value: object) -> str:
    payload = json.dumps(value, sort_keys=True, separators=(",", ":")).encode()
    return "sha256:" + hashlib.sha256(payload).hexdigest()


def probe_identity(
    probe: pathlib.Path, scenario: pathlib.Path, response: pathlib.Path
) -> dict:
    completed = subprocess.run(
        [str(probe), str(scenario), str(response), "42"],
        check=True,
        capture_output=True,
        text=True,
    )
    result = json.loads(completed.stdout)
    if result.get("schema_version") != "pitgun.racing-tuning-response-probe/v1":
        raise RuntimeError("probe returned an unsupported contract")
    return result


def build(probe: pathlib.Path) -> bytes:
    source = json.loads(SOURCE.read_text())
    response_rows = []
    configurations = []
    for response_id, response_resource in RESPONSES:
        response_path = (
            ROOT
            / "experiments"
            / "databricks"
            / "responses"
            / f"{response_resource}.json"
        )
        first = probe_identity(
            probe,
            SCENARIO_ROOT / f"{source['configurations'][0]['scenario_resource']}.json",
            response_path,
        )
        response_digest = first["tuning_response_digest"]
        response_rows.append(
            {
                "id": response_id,
                "resource": response_resource,
                "expected_tuning_response_digest": response_digest,
            }
        )
        for configuration in source["configurations"]:
            scenario_path = SCENARIO_ROOT / f"{configuration['scenario_resource']}.json"
            identity = probe_identity(probe, scenario_path, response_path)
            experimental_configuration_id = sha256_json(
                {
                    "response_digest": response_digest,
                    "scenario_digest": identity["scenario_digest"],
                }
            )
            configurations.append(
                {
                    **configuration,
                    "id": f"{response_id}--{configuration['id']}",
                    "response_id": response_id,
                    "response_resource": response_resource,
                    "expected_tuning_response_digest": response_digest,
                    "expected_scenario_digest": identity["scenario_digest"],
                    "expected_experimental_configuration_id": experimental_configuration_id,
                }
            )
            configurations[-1].pop("expected_configuration_id", None)

    manifest = {
        "schema_version": "pitgun.calibration-campaign/v3",
        "campaign_id": "racing-aero-candidate-validation-2026-v1",
        "question": "Does the calibrated aerodynamic response improve setup sensitivity while preserving credible circuit behavior across seeds?",
        "parameter_space_version": "3.0.0",
        "execution_class": "experimental-tuning-response",
        "promotion_policy": "human-review-required",
        "scenario": source["scenario"],
        "model": source["model"],
        "data_pack": source["data_pack"],
        "vehicle_id": source["vehicle_id"],
        "era": source["era"],
        "seeds": source["seeds"],
        "circuits": source["circuits"],
        "responses": response_rows,
        "configurations": configurations,
        "acceptance_criteria": {
            "execution_success_rate_min": 1.0,
            "maximum_speed_kph_max": 400.0,
            "candidate_must_improve_setup_discrimination": True,
            "automatic_release": False,
        },
        "planned_run_count": len(configurations) * len(source["seeds"]),
    }
    return (json.dumps(manifest, indent=2) + "\n").encode()


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--probe", type=pathlib.Path, required=True)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    payload = build(args.probe.resolve())
    manifest_path = CAMPAIGN_ROOT / f"{NAME}.json"
    checksum_path = CAMPAIGN_ROOT / f"{NAME}.sha256"
    checksum = f"{hashlib.sha256(payload).hexdigest()}  {manifest_path.name}\n"
    if args.check:
        if (
            manifest_path.read_bytes() != payload
            or checksum_path.read_text() != checksum
        ):
            raise SystemExit("candidate-validation manifest is stale")
        return
    manifest_path.write_bytes(payload)
    checksum_path.write_text(checksum)


if __name__ == "__main__":
    main()
