#!/usr/bin/env python3
"""Freeze the accepted local V3 progression audit as a Databricks campaign."""

from __future__ import annotations

import argparse
import concurrent.futures
import hashlib
import json
import pathlib
import subprocess
import sys
import tempfile
from typing import Any


ROOT = pathlib.Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
OUTPUT = (
    ROOT
    / "experiments"
    / "databricks"
    / "campaigns"
    / "racing-v3-decision-surface-v1.json"
)
LOCAL_REPORT = (
    ROOT
    / "experiments"
    / "racing_v3_progression_robustness"
    / "results"
    / "local-progression-robustness-v1.json"
)
PROFILE = (
    ROOT
    / "experiments"
    / "racing_v3_decision_surface"
    / "profile-v7.compound-degradation.json"
)
RUNNER = ROOT / "target" / "release" / "examples" / "v3_decision_surface_probe"
SCHEMA_VERSION = "pitgun.racing-v3-decision-surface-campaign/v1"


def canonical_pretty(value: object) -> bytes:
    return (json.dumps(value, indent=2, ensure_ascii=False) + "\n").encode()


def sha256(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


def point_key(point: dict[str, Any]) -> tuple[Any, ...]:
    return (
        point["family"],
        point["split"],
        point["circuit_id"],
        point["vehicle_id"],
        point["progression"],
        point["case_id"],
        int(point["seed"]),
    )


def run_probe(
    runner: pathlib.Path,
    profile: pathlib.Path,
    point: dict[str, Any],
) -> dict[str, Any]:
    with tempfile.TemporaryDirectory(prefix="pitgun-v3-databricks-manifest-") as tmp:
        scenario = pathlib.Path(tmp) / "scenario.json"
        scenario.write_bytes(canonical_pretty(point["scenario"]))
        completed = subprocess.run(
            [str(runner), str(scenario), str(profile), str(point["seed"])],
            cwd=ROOT,
            capture_output=True,
            check=False,
            timeout=120,
        )
    if completed.returncode != 0:
        raise RuntimeError(
            f"probe failed for {point_key(point)}: "
            + completed.stderr.decode(errors="replace")[:1000]
        )
    if completed.stderr:
        raise RuntimeError(f"successful probe wrote to stderr for {point_key(point)}")
    result = json.loads(completed.stdout)
    if result.get("schema_version") != "pitgun.racing-v3-decision-surface-probe/v1":
        raise RuntimeError("probe returned an unsupported contract")
    return result


def build_manifest(
    runner: pathlib.Path = RUNNER,
    report_path: pathlib.Path = LOCAL_REPORT,
    profile_path: pathlib.Path = PROFILE,
    jobs: int = 16,
) -> dict[str, Any]:
    if not runner.is_file():
        raise RuntimeError(f"missing local decision-surface probe: {runner}")
    report_bytes = report_path.read_bytes()
    sidecar = report_path.with_suffix(".sha256").read_text().strip()
    if sidecar != sha256(report_bytes):
        raise RuntimeError("local audit report does not match its checksum")
    report = json.loads(report_bytes)

    from experiments.racing_v3_progression_robustness import audit_local
    from experiments.racing_v3_validation import validate_local

    plan = audit_local.build_plan(json.loads(validate_local.BASE_SCENARIO.read_bytes()))
    expected_points = {point_key(point): point for point in report["points"]}
    if not (len(plan) == len(expected_points) == 4928):
        raise RuntimeError("local plan and stored evidence do not reconcile")
    with concurrent.futures.ThreadPoolExecutor(max_workers=jobs) as executor:
        results = list(
            executor.map(lambda point: run_probe(runner, profile_path, point), plan)
        )

    profile_value = json.loads(profile_path.read_bytes())
    profile_refs: dict[str, dict[str, Any]] = {}
    scenario_refs: dict[str, dict[str, Any]] = {}
    configurations = []
    model = None
    if len(results) != len(plan):
        raise RuntimeError("probe execution count does not reconcile")
    for index, (planned, result) in enumerate(zip(plan, results)):
        key = point_key(planned)
        expected_point = expected_points.get(key)
        if expected_point is None:
            raise RuntimeError(f"unplanned local evidence point: {key}")
        probe_result_digest = sha256(canonical_pretty(result))
        if probe_result_digest != expected_point["result_digest"]:
            raise RuntimeError(f"local replay changed for {key}")
        if model is None:
            model = result["model"]
        elif model != result["model"]:
            raise RuntimeError("campaign mixes multiple model identities")

        scenario_ref = result["scenario_digest"]
        profile_ref = result["profile_digest"]
        scenario_refs.setdefault(scenario_ref, planned["scenario"])
        profile_refs.setdefault(profile_ref, profile_value)
        metadata = {k: v for k, v in planned.items() if k not in {"scenario", "seed"}}
        configuration_id = sha256(
            canonical_pretty(
                {
                    "schema_version": "pitgun.racing-v3-decision-surface-configuration/v1",
                    "metadata": metadata,
                    "profile_digest": profile_ref,
                    "scenario_digest": scenario_ref,
                }
            )
        )
        compact = dict(expected_point)
        configurations.append(
            {
                "execution_key": f"v3ds-{int(planned['seed']):04d}-{index:06d}",
                "configuration_id": configuration_id,
                "family": planned["family"],
                "case_id": planned["case_id"],
                "circuit_id": planned["circuit_id"],
                "circuit_slug": planned["circuit_slug"],
                "circuit_archetype": planned["circuit_archetype"],
                "split": planned["split"],
                "vehicle_id": planned["vehicle_id"],
                "vehicle_anchor": planned["vehicle_anchor"],
                "era": planned["era"],
                "progression": planned["progression"],
                "budget": planned["budget"],
                "seed": str(planned["seed"]),
                "metadata": metadata,
                "scenario_ref": scenario_ref,
                "profile_ref": profile_ref,
                "expected_experimental_execution_id": result[
                    "experimental_execution_id"
                ],
                "expected_probe_result_digest": probe_result_digest,
                "expected_compact_point_digest": sha256(canonical_pretty(compact)),
            }
        )

    configurations.sort(key=lambda row: row["execution_key"])
    local_summary = {
        key: sha256(canonical_pretty(report[key]))
        for key in (
            "development_summary",
            "marginal_summary",
            "setup_summary",
            "strategy_evidence",
            "vehicle_progression_verdicts",
            "verdicts",
        )
    }
    local_point_set_digest = sha256(canonical_pretty(report["points"]))
    return {
        "schema_version": SCHEMA_VERSION,
        "campaign_id": "racing-v3-decision-surface-2026-v1",
        "execution_class": "experimental-v3-physics",
        "question": (
            "Do Model V3 development, setup, thermal, and strategy responses remain "
            "active, non-dominant, and transferable across progression and held-out tracks?"
        ),
        "parameter_space_version": "racing-v3-progression-robustness-v1",
        "promotion_policy": "human-review-required",
        "automatic_catalog_promotion": False,
        "acceptance_criteria": {
            "exact_local_point_parity": True,
            "calibration_and_held_out_reported_separately": True,
            "automatic_release": False,
        },
        "model": model,
        "runner": {
            "kind": "v3_decision_surface_probe",
            "contract": "pitgun.racing-v3-decision-surface-probe/v1",
            "local_artifact_digest": report["campaign"]["runner"]["digest"],
        },
        "data_pack": {
            "id": "pitgun.racing-v3-experiment-profile",
            "version": "v7.compound-degradation",
            "digest": sha256(profile_path.read_bytes()),
            "canonical_digest": next(iter(profile_refs)),
        },
        "local_evidence": {
            "schema_version": report["schema_version"],
            "artifact_digest": sha256(report_bytes),
            "point_set_digest": local_point_set_digest,
            "summary_digests": local_summary,
        },
        "dimensions": {
            "families": sorted({row["family"] for row in configurations}),
            "splits": sorted({row["split"] for row in configurations}),
            "circuits": sorted({row["circuit_id"] for row in configurations}),
            "vehicles": sorted({row["vehicle_id"] for row in configurations}),
            "progression": report["campaign"]["progression_budgets"],
            "seeds": [str(seed) for seed in report["campaign"]["seeds"]],
        },
        "planned_run_count": len(configurations),
        "unique_configuration_count": len(
            {row["configuration_id"] for row in configurations}
        ),
        "unique_scenario_count": len(scenario_refs),
        "profiles": dict(sorted(profile_refs.items())),
        "scenarios": dict(sorted(scenario_refs.items())),
        "configurations": configurations,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--runner", type=pathlib.Path, default=RUNNER)
    parser.add_argument("--report", type=pathlib.Path, default=LOCAL_REPORT)
    parser.add_argument("--profile", type=pathlib.Path, default=PROFILE)
    parser.add_argument("--output", type=pathlib.Path, default=OUTPUT)
    parser.add_argument("--jobs", type=int, default=16)
    arguments = parser.parse_args()
    manifest = build_manifest(
        arguments.runner.resolve(),
        arguments.report.resolve(),
        arguments.profile.resolve(),
        arguments.jobs,
    )
    payload = canonical_pretty(manifest)
    arguments.output.parent.mkdir(parents=True, exist_ok=True)
    arguments.output.write_bytes(payload)
    arguments.output.with_suffix(".sha256").write_text(
        hashlib.sha256(payload).hexdigest() + "  " + arguments.output.name + "\n"
    )
    print(
        json.dumps(
            {
                "path": str(arguments.output),
                "digest": sha256(payload),
                "planned_run_count": manifest["planned_run_count"],
                "unique_configuration_count": manifest[
                    "unique_configuration_count"
                ],
                "unique_scenario_count": manifest["unique_scenario_count"],
            },
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
