#!/usr/bin/env python3
"""Execute three representative Catalog 1.9 scenarios twice with native Rust."""

from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import subprocess
from typing import Any

from build_campaign import (
    AcceptanceBuildError,
    MANIFEST_PATH,
    SCENARIO_ROOT,
    canonical_pretty,
    sha256,
)


ROOT = pathlib.Path(__file__).resolve().parents[2]
DEFAULT_RUNNER = ROOT / "target" / "debug" / "pitgun"
DEFAULT_CATALOG = ROOT / "catalogs" / "racing" / "v1.9.0"
RESULT_ROOT = ROOT / "experiments" / "opponent_acceptance" / "results"
REPORT_PATH = RESULT_ROOT / "local-native-preflight-v1.json"
CHECKSUM_PATH = REPORT_PATH.with_suffix(".sha256")

SENTINELS = (
    "monza-early-42-naive",
    "budapest-mid-4242-balanced",
    "singapore-late-20260825-circuit-informed",
)


def load_manifest() -> tuple[dict[str, Any], str]:
    manifest_bytes = MANIFEST_PATH.read_bytes()
    checksum = MANIFEST_PATH.with_suffix(".sha256").read_text().split()
    if len(checksum) != 2 or checksum[1] != MANIFEST_PATH.name:
        raise AcceptanceBuildError("acceptance manifest checksum format is invalid")
    actual = hashlib.sha256(manifest_bytes).hexdigest()
    if checksum[0] != actual:
        raise AcceptanceBuildError("acceptance manifest checksum mismatch")
    manifest = json.loads(manifest_bytes)
    if manifest.get("planned_run_count") != 135:
        raise AcceptanceBuildError("acceptance manifest is incomplete")
    return manifest, "sha256:" + actual


def execute(
    runner: pathlib.Path,
    catalog: pathlib.Path,
    scenario: pathlib.Path,
    seed: int,
) -> bytes:
    completed = subprocess.run(
        [
            str(runner),
            "run",
            "racing",
            "--scenario",
            str(scenario),
            "--seed",
            str(seed),
            "--catalog-release",
            str(catalog),
        ],
        cwd=ROOT,
        check=True,
        capture_output=True,
        timeout=240,
    )
    if completed.stderr:
        raise AcceptanceBuildError(
            f"native runner unexpectedly wrote stderr: {completed.stderr.decode()[:500]}"
        )
    return completed.stdout


def compact_evidence(run: dict[str, Any], result: dict[str, Any], result_digest: str) -> dict[str, Any]:
    standings = result.get("summary", {}).get("standings", [])
    player_rows = [row for row in standings if row.get("competitor_id") == "player"]
    if len(player_rows) != 1 or len(standings) != 10:
        raise AcceptanceBuildError(f"invalid native standings for {run['run_key']}")
    player = player_rows[0]
    ordered = sorted(standings, key=lambda row: int(row["position"]))
    leader = ordered[0]
    last = ordered[-1]
    return {
        "run_key": run["run_key"],
        "circuit_id": run["circuit_id"],
        "circuit_class": run["circuit_class"],
        "progression": run["progression"],
        "era": run["era"],
        "seed": run["seed"],
        "player_reference": run["player_reference"],
        "player_budget": run["player_budget"],
        "opponent_budget": {
            "minimum": run["opponent_budget_min"],
            "median": run["opponent_budget_median"],
            "maximum": run["opponent_budget_max"],
        },
        "scenario_digest": result["scenario_digest"],
        "configuration_id": result["configuration_id"],
        "run_id": result["run_id"],
        "contract_digest": result["contract_digest"],
        "output_digest": result["output_digest"],
        "telemetry_summary_digest": result["telemetry_summary_digest"],
        "result_digest": result_digest,
        "player_position": int(player["position"]),
        "player_gap_to_leader_ms": int(player["gap_to_leader_ms"]),
        "leader_competitor_id": leader["competitor_id"],
        "field_spread_ms": int(last["total_time_ms"]) - int(leader["total_time_ms"]),
        "player_best_lap_ms": int(player["best_lap_ms"]),
        "telemetry_frame_count": int(result["summary"]["telemetry_frame_count"]),
    }


def build_report(runner: pathlib.Path, catalog: pathlib.Path) -> dict[str, Any]:
    manifest, manifest_digest = load_manifest()
    runs = {run["run_key"]: run for run in manifest["runs"]}
    runner_digest = sha256(runner.read_bytes())
    evidence = []
    for run_key in SENTINELS:
        run = runs.get(run_key)
        if not run:
            raise AcceptanceBuildError(f"missing preflight sentinel: {run_key}")
        scenario = SCENARIO_ROOT / f"{run['scenario_resource']}.json"
        first = execute(runner, catalog, scenario, int(run["seed"]))
        second = execute(runner, catalog, scenario, int(run["seed"]))
        if first != second:
            raise AcceptanceBuildError(f"non-deterministic native retry: {run_key}")
        result = json.loads(first)
        if result.get("run_id") != result.get("contract_digest"):
            raise AcceptanceBuildError(f"native identity mismatch: {run_key}")
        if result.get("model") != {
            "id": manifest["catalog"]["model_id"],
            "version": manifest["catalog"]["model_version"],
            "digest": manifest["catalog"]["model_digest"],
        }:
            raise AcceptanceBuildError(f"native model identity mismatch: {run_key}")
        if result.get("data_pack") != {
            "id": "pitgun.racing.simulation",
            "version": manifest["catalog"]["version"],
            "digest": manifest["catalog"]["simulation_pack_digest"],
        }:
            raise AcceptanceBuildError(f"native data-pack identity mismatch: {run_key}")
        evidence.append(compact_evidence(run, result, sha256(first)))

    return {
        "schema_version": "pitgun.opponent-acceptance-local-preflight/v1",
        "purpose": "Bounded native retry gate before governed Databricks execution.",
        "campaign_id": manifest["campaign_id"],
        "manifest_digest": manifest_digest,
        "catalog": manifest["catalog"],
        "runner": {
            "name": "pitgun-cli",
            "artifact_digest": runner_digest,
        },
        "sentinel_selection": {
            "rule": "one authored scenario per progression and player reference across three circuit classes",
            "run_keys": list(SENTINELS),
        },
        "deterministic_retry_count": 2,
        "all_retries_byte_identical": True,
        "evidence": evidence,
        "decision": {
            "databricks_handoff": "READY",
            "game_or_catalog_promotion": "FORBIDDEN",
            "policy_mutation": "FORBIDDEN",
        },
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--runner", type=pathlib.Path, default=DEFAULT_RUNNER)
    parser.add_argument("--catalog-release", type=pathlib.Path, default=DEFAULT_CATALOG)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    if not args.runner.is_file():
        raise AcceptanceBuildError(f"native runner is missing: {args.runner}")
    if not args.catalog_release.is_dir():
        raise AcceptanceBuildError(f"Catalog 1.9 release is missing: {args.catalog_release}")

    report = build_report(args.runner.resolve(), args.catalog_release.resolve())
    report_bytes = canonical_pretty(report)
    checksum_bytes = (
        f"{hashlib.sha256(report_bytes).hexdigest()}  {REPORT_PATH.name}\n"
    ).encode()
    if args.check:
        if REPORT_PATH.read_bytes() != report_bytes or CHECKSUM_PATH.read_bytes() != checksum_bytes:
            raise AcceptanceBuildError("native preflight evidence is stale")
    else:
        RESULT_ROOT.mkdir(parents=True, exist_ok=True)
        REPORT_PATH.write_bytes(report_bytes)
        CHECKSUM_PATH.write_bytes(checksum_bytes)
    action = "validated" if args.check else "recorded"
    print(f"{action} {len(SENTINELS)} native sentinels with byte-identical retries")
    print(f"preflight report {sha256(report_bytes)}")


if __name__ == "__main__":
    main()
