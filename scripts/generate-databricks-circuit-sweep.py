#!/usr/bin/env python3
"""Generate the immutable, allowlisted Racing circuit-sweep inputs."""

from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import subprocess


ROOT = pathlib.Path(__file__).resolve().parents[1]
BASE_SCENARIO = (
    ROOT / "apps" / "pitgun-cli" / "scenarios" / "racing-batch-v1" / "balanced.json"
)
SCENARIO_ROOT = (
    ROOT / "apps" / "pitgun-cli" / "scenarios" / "racing-circuit-sweep-v1"
)
CAMPAIGN_ROOT = ROOT / "experiments" / "databricks" / "campaigns"
CAMPAIGN_NAME = "racing-circuit-sweep-v1"
SEEDS = ["42", "2026", "20260810"]

CIRCUITS = (
    ("it-1922", "power", "monza"),
    ("mc-1929", "high-downforce", "monaco"),
    ("hu-1986", "mechanical-grip", "budapest"),
    ("jp-1962", "mixed", "suzuka"),
    ("sg-2008", "street-thermal", "singapore"),
)

SETUPS = (
    ("low-downforce", 0.2, 0.3),
    ("balanced", 0.5, 0.5),
    ("high-downforce", 0.8, 0.7),
    ("low-downforce-long-gearing", 0.2, 0.7),
    ("high-downforce-short-gearing", 0.8, 0.3),
    ("balanced-short-gearing", 0.5, 0.3),
    ("balanced-long-gearing", 0.5, 0.7),
)


def canonical_pretty(value: object) -> bytes:
    return (json.dumps(value, indent=2, ensure_ascii=False) + "\n").encode()


def inspect_scenario(runner: pathlib.Path, scenario: pathlib.Path) -> dict[str, object]:
    completed = subprocess.run(
        [str(runner), "run", "racing", "--scenario", str(scenario), "--seed", "42"],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    if completed.stderr:
        raise RuntimeError(f"runner wrote unexpected stderr for {scenario.name}")
    result = json.loads(completed.stdout)
    return {
        "configuration_id": result["configuration_id"],
        "scenario_digest": result["scenario_digest"],
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--runner",
        type=pathlib.Path,
        default=ROOT / "target" / "debug" / "pitgun",
    )
    args = parser.parse_args()
    runner = args.runner.resolve()
    if not runner.is_file():
        raise SystemExit(f"missing runner: {runner}")

    base = json.loads(BASE_SCENARIO.read_text())
    SCENARIO_ROOT.mkdir(parents=True, exist_ok=True)
    expected_names: set[str] = set()
    configurations = []

    for circuit_id, archetype, circuit_slug in CIRCUITS:
        for family, downforce, gearing in SETUPS:
            resource_id = f"{circuit_slug}--{family}"
            resource_name = resource_id + ".json"
            expected_names.add(resource_name)
            scenario = json.loads(json.dumps(base))
            scenario["request"]["track_id"] = circuit_id
            tuning = scenario["request"]["competitors"][0]["tuning"]
            tuning["downforce_slider"] = downforce
            tuning["gear_ratio_slider"] = gearing
            scenario_path = SCENARIO_ROOT / resource_name
            scenario_path.write_bytes(canonical_pretty(scenario))
            identity = inspect_scenario(runner, scenario_path)
            configurations.append(
                {
                    "id": resource_id,
                    "configuration_family": family,
                    "circuit_id": circuit_id,
                    "circuit_archetype": archetype,
                    "scenario_resource": resource_id,
                    "expected_configuration_id": identity["configuration_id"],
                    "expected_scenario_digest": identity["scenario_digest"],
                    "setup": {
                        "downforce_slider": downforce,
                        "gear_ratio_slider": gearing,
                    },
                    "strategy": {"id": "single-lap-no-stop"},
                }
            )

    unexpected = {path.name for path in SCENARIO_ROOT.glob("*.json")} - expected_names
    if unexpected:
        raise SystemExit(
            "refusing to retain unplanned scenarios: "
            + ", ".join(sorted(unexpected))
        )

    manifest = {
        "schema_version": "pitgun.calibration-campaign/v2",
        "campaign_id": "racing-circuit-sweep-2026-v1",
        "question": "Which bounded setup families are fast and seed-robust across five representative 2026 circuit archetypes?",
        "parameter_space_version": "2.0.0",
        "scenario": {"id": "racing.single-lap", "version": "1.0.0"},
        "model": base["model"],
        "data_pack": base["data_pack"],
        "vehicle_id": base["request"]["vehicle_id"],
        "era": base["request"]["era"],
        "seeds": SEEDS,
        "circuits": [
            {"id": circuit_id, "archetype": archetype}
            for circuit_id, archetype, _ in CIRCUITS
        ],
        "configurations": configurations,
        "planned_run_count": len(configurations) * len(SEEDS),
        "audit_sample": {
            "retain_complete_run_bundles": 0,
            "reason": "The setup sweep retains compact governed metrics; multi-lap strategy evidence is a separate campaign.",
        },
    }
    manifest_path = CAMPAIGN_ROOT / f"{CAMPAIGN_NAME}.json"
    manifest_bytes = canonical_pretty(manifest)
    manifest_path.write_bytes(manifest_bytes)
    checksum = hashlib.sha256(manifest_bytes).hexdigest()
    (CAMPAIGN_ROOT / f"{CAMPAIGN_NAME}.sha256").write_text(
        f"{checksum}  {manifest_path.name}\n"
    )
    print(
        f"generated {len(configurations)} scenarios and {manifest['planned_run_count']} planned runs; "
        f"manifest sha256:{checksum}"
    )


if __name__ == "__main__":
    main()
