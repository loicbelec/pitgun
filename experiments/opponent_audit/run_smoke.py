#!/usr/bin/env python3
"""Materialize and execute the controlled Racing V2 opponent smoke matrix."""

from __future__ import annotations

import argparse
import concurrent.futures
import hashlib
import json
import pathlib
import statistics
import subprocess
from typing import Any


ROOT = pathlib.Path(__file__).resolve().parents[2]
DEFAULT_SOURCE = (
    ROOT.parent / "game" / "docs" / "gameplay" / "opponent-contract-audit-v1.json"
)
SCENARIO_ROOT = ROOT / "experiments" / "opponent_audit" / "scenarios"
RESULT_PATH = (
    ROOT / "experiments" / "opponent_audit" / "results" / "racing-opponent-smoke-v1.json"
)
REPORT_PATH = ROOT / "docs" / "RACING_OPPONENT_AUDIT_SMOKE_V1.md"
SCHEMA_VERSION = "pitgun.racing-opponent-smoke/v1"
MODEL_DIGEST = "sha256:a372f990c320d10207220f98ca4bf677607fc5c13918c73b47dfbb8949b106d2"
SOURCE_SCHEMA = "pitgun.opponent-contract-audit/v1"
SOURCE_ARTIFACT_DIGEST = "sha256:3a17c8353cba45966b7cf807bd7ef16b125bd98761667b1c76c6fac0938c4e3c"
SELECTED_SEED = 42
SELECTED_STRATEGY = "balanced-one-stop"


class OpponentSmokeError(RuntimeError):
    """Raised when the bounded smoke input or execution is invalid."""


def canonical_pretty(value: object) -> bytes:
    return (json.dumps(value, indent=2, ensure_ascii=False) + "\n").encode()


def sha256(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


def validate_source(source: dict[str, Any]) -> None:
    if source.get("schemaVersion") != SOURCE_SCHEMA:
        raise OpponentSmokeError("unsupported game opponent audit schema")
    claimed = source.get("artifactDigest")
    payload = {key: value for key, value in source.items() if key != "artifactDigest"}
    calculated = sha256(canonical_pretty(payload))
    if claimed != calculated or claimed != SOURCE_ARTIFACT_DIGEST:
        raise OpponentSmokeError(
            f"game opponent artifact changed: expected {SOURCE_ARTIFACT_DIGEST}, got {claimed}"
        )
    catalog = source.get("catalog", {})
    if (catalog.get("version"), catalog.get("modelVersion")) != ("1.2.0", "2.0.0"):
        raise OpponentSmokeError("smoke requires Racing Catalog 1.2.0 / model 2.0.0")


def balanced_points(budget: int) -> dict[str, int]:
    quotient, remainder = divmod(budget, 4)
    axes = ("aero", "chassis", "cooling", "engine")
    return {
        axis: quotient + (1 if index < remainder else 0)
        for index, axis in enumerate(axes)
    }


def build_resolved_scenario(source: dict[str, Any], selected: dict[str, Any]) -> dict[str, Any]:
    if sha256(canonical_pretty(selected["contracts"])) != selected["contractDigest"]:
        raise OpponentSmokeError(f"contract digest mismatch for {selected['id']}")
    budget = int(selected["input"]["playerBudget"])
    points = balanced_points(budget)
    player = {
        "id": "player",
        "driver_id": "default",
        "name": "Controlled Neutral Reference",
        "team_id": "pitgun",
        "is_player": True,
        "tuning": {
            "engine_points": points["engine"],
            "cooling_points": points["cooling"],
            "aero_points": points["aero"],
            "chassis_points": points["chassis"],
            "downforce_slider": 0.5,
            "gear_ratio_slider": 0.5,
        },
        "budget_cap": budget,
        "stint_strategy": selected["input"]["playerStrategy"],
    }
    opponents = [
        {key: value for key, value in opponent.items() if key not in {"style", "points"}}
        for opponent in selected["contracts"]
    ]
    catalog = source["catalog"]
    return {
        "schema_version": "pitgun.racing-resolved-scenario/v1",
        "scenario": {"id": "racing.opponent-audit-smoke", "version": "1.0.0"},
        "model": {
            "id": catalog["modelId"],
            "version": catalog["modelVersion"],
            "digest": MODEL_DIGEST,
        },
        "data_pack": {
            "id": "pitgun.racing.simulation",
            "version": catalog["version"],
            "digest": catalog["simulationPackDigest"],
        },
        "clock": {"tick_numerator_us": 200000, "tick_denominator": 1},
        "request": {
            "track_id": selected["identity"]["circuitId"],
            "laps": selected["input"]["laps"],
            "competitors": [player, *opponents],
            "vehicle_id": selected["input"]["vehicleId"],
            "era": selected["input"]["era"],
            "hz": 5.0,
        },
    }


def execute_once(
    runner: pathlib.Path,
    catalog_release: pathlib.Path,
    scenario_path: pathlib.Path,
) -> bytes:
    completed = subprocess.run(
        [
            str(runner),
            "run",
            "racing",
            "--scenario",
            str(scenario_path),
            "--catalog-release",
            str(catalog_release),
            "--seed",
            str(SELECTED_SEED),
        ],
        cwd=ROOT,
        check=False,
        capture_output=True,
        timeout=180,
    )
    if completed.returncode != 0:
        raise OpponentSmokeError(
            f"runner failed for {scenario_path.name}: "
            + completed.stderr.decode(errors="replace").strip()
        )
    if completed.stderr:
        raise OpponentSmokeError(f"successful runner wrote stderr for {scenario_path.name}")
    return completed.stdout


def execute_scenario(
    runner: pathlib.Path,
    catalog_release: pathlib.Path,
    scenario_path: pathlib.Path,
    source_scenario: dict[str, Any],
) -> dict[str, Any]:
    first = execute_once(runner, catalog_release, scenario_path)
    repeated = execute_once(runner, catalog_release, scenario_path)
    if first != repeated:
        raise OpponentSmokeError(f"retry changed canonical bytes for {scenario_path.name}")
    try:
        result = json.loads(first)
        standings = result["summary"]["standings"]
    except (UnicodeDecodeError, json.JSONDecodeError, KeyError, TypeError) as error:
        raise OpponentSmokeError(f"invalid runner result for {scenario_path.name}") from error
    player = next(row for row in standings if row["competitor_id"] == "player")
    opponent_budgets = [contract["budget_cap"] for contract in source_scenario["contracts"]]
    tuning_keys = {
        json.dumps(contract["tuning"], sort_keys=True)
        for contract in source_scenario["contracts"]
    }
    strategy_keys = {
        json.dumps(contract["stint_strategy"], sort_keys=True)
        for contract in source_scenario["contracts"]
    }
    total_times = [row["total_time_ms"] for row in standings]
    return {
        "source_scenario_id": source_scenario["id"],
        "circuit_id": source_scenario["identity"]["circuitId"],
        "progression": source_scenario["identity"]["progression"],
        "era": source_scenario["input"]["era"],
        "player_budget": source_scenario["input"]["playerBudget"],
        "source_contract_digest": source_scenario["contractDigest"],
        "scenario_digest": result["scenario_digest"],
        "configuration_id": result["configuration_id"],
        "run_id": result["run_id"],
        "output_digest": result["output_digest"],
        "retry_byte_identical": True,
        "player": {
            "position": player["position"],
            "gap_to_leader_ms": player["gap_to_leader_ms"],
            "best_lap_ms": player["best_lap_ms"],
        },
        "field": {
            "spread_ms": max(total_times) - min(total_times),
            "opponent_budget_min": min(opponent_budgets),
            "opponent_budget_max": max(opponent_budgets),
            "distinct_opponent_tunings": len(tuning_keys),
            "distinct_opponent_strategies": len(strategy_keys),
        },
    }


def summarize(runs: list[dict[str, Any]]) -> dict[str, Any]:
    gaps = sorted(run["player"]["gap_to_leader_ms"] for run in runs)
    positions = [run["player"]["position"] for run in runs]
    return {
        "completed_runs": len(runs),
        "player_wins": sum(position == 1 for position in positions),
        "player_podiums": sum(position <= 3 for position in positions),
        "player_win_rate": sum(position == 1 for position in positions) / len(runs),
        "player_podium_rate": sum(position <= 3 for position in positions) / len(runs),
        "median_player_gap_to_leader_ms": statistics.median(gaps),
        "maximum_player_gap_to_leader_ms": max(gaps),
        "minimum_player_position": min(positions),
        "maximum_player_position": max(positions),
        "all_retries_byte_identical": all(run["retry_byte_identical"] for run in runs),
    }


def markdown_report(result: dict[str, Any]) -> str:
    summary = result["summary"]
    rows = "\n".join(
        f"| {run['circuit_id']} | {run['progression']} | {run['player']['position']} | "
        f"{run['player']['gap_to_leader_ms']:,} | {run['field']['spread_ms']:,} | "
        f"{run['field']['distinct_opponent_tunings']}/9 | "
        f"{run['field']['distinct_opponent_strategies']}/9 |"
        for run in result["runs"]
    )
    return f"""# Racing Opponent Audit — Local Smoke V1

Status: completed locally as the bounded preflight for game issue
`loicbelec/pitgun-game#156` and framework issue #209.

## Outcome

The exact current game opponents were executed through `pitgun run racing`
with Racing Catalog 1.2.0 and physical model 2.0.0. The controlled player uses
the same base budget, equal development allocation, neutral sliders and the
authored balanced one-stop strategy. It is not a copied player setup.

- Completed runs: {summary['completed_runs']}/15
- Player wins: {summary['player_wins']} ({summary['player_win_rate']:.0%})
- Player podiums: {summary['player_podiums']} ({summary['player_podium_rate']:.0%})
- Player position range: P{summary['minimum_player_position']}–P{summary['maximum_player_position']}
- Median gap to leader: {summary['median_player_gap_to_leader_ms']:,.0f} ms
- Largest gap to leader: {summary['maximum_player_gap_to_leader_ms']:,} ms
- Byte-identical retries: {str(summary['all_retries_byte_identical']).lower()}
- Result artifact: `{result['artifact_digest']}`

## Matrix

| Circuit | Progression | Player | Gap to leader (ms) | Field spread (ms) | Distinct setups | Distinct strategies |
|---|---:|---:|---:|---:|---:|---:|
{rows}

## Interpretation boundary

This smoke proves the cross-repository contract and execution path and provides
an initial diagnosis. It does not decide game balance: one neutral reference,
one seed and one strategy cannot represent player skill. The governed
Databricks campaign will add reviewed player references and seeds, persist Delta
lineage and make the policy decision auditable.

No career, leaderboard, private setup or observed telemetry data was consumed.
"""


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", type=pathlib.Path, default=DEFAULT_SOURCE)
    parser.add_argument("--runner", type=pathlib.Path, default=ROOT / "target" / "debug" / "pitgun")
    parser.add_argument(
        "--catalog-release",
        type=pathlib.Path,
        default=ROOT / "catalogs" / "racing" / "v1.2.0",
    )
    parser.add_argument("--jobs", type=int, default=4)
    args = parser.parse_args()
    if not args.runner.is_file():
        raise SystemExit(f"missing runner: {args.runner}; run cargo build -p pitgun-cli")

    source = json.loads(args.source.read_text())
    validate_source(source)
    selected = [
        scenario
        for scenario in source["scenarios"]
        if scenario["identity"]["seed"] == SELECTED_SEED
        and scenario["identity"]["strategyProfile"] == SELECTED_STRATEGY
    ]
    selected.sort(key=lambda scenario: scenario["id"])
    if len(selected) != 15:
        raise OpponentSmokeError(f"expected 15 selected scenarios, got {len(selected)}")

    SCENARIO_ROOT.mkdir(parents=True, exist_ok=True)
    expected_paths = []
    for source_scenario in selected:
        path = SCENARIO_ROOT / f"{source_scenario['id']}.json"
        path.write_bytes(canonical_pretty(build_resolved_scenario(source, source_scenario)))
        expected_paths.append(path)
    unexpected = set(SCENARIO_ROOT.glob("*.json")) - set(expected_paths)
    if unexpected:
        raise OpponentSmokeError("unplanned scenario files: " + ", ".join(map(str, sorted(unexpected))))

    by_id = {scenario["id"]: scenario for scenario in selected}
    with concurrent.futures.ThreadPoolExecutor(max_workers=args.jobs) as executor:
        futures = [
            executor.submit(
                execute_scenario,
                args.runner.resolve(),
                args.catalog_release.resolve(),
                path,
                by_id[path.stem],
            )
            for path in expected_paths
        ]
        runs = [future.result() for future in futures]
    runs.sort(key=lambda run: (run["circuit_id"], run["era"]))

    payload = {
        "schema_version": SCHEMA_VERSION,
        "source": {
            "repository": "loicbelec/pitgun-game",
            "schema_version": SOURCE_SCHEMA,
            "artifact_digest": SOURCE_ARTIFACT_DIGEST,
        },
        "execution": {
            "catalog_id": source["catalog"]["id"],
            "catalog_version": source["catalog"]["version"],
            "model_id": source["catalog"]["modelId"],
            "model_version": source["catalog"]["modelVersion"],
            "seed": SELECTED_SEED,
            "strategy_profile": SELECTED_STRATEGY,
            "controlled_player_reference": "equal-budget-balanced-neutral",
            "retry_count_per_scenario": 2,
        },
        "summary": summarize(runs),
        "runs": runs,
    }
    result = {**payload, "artifact_digest": sha256(canonical_pretty(payload))}
    RESULT_PATH.parent.mkdir(parents=True, exist_ok=True)
    RESULT_PATH.write_bytes(canonical_pretty(result))
    REPORT_PATH.write_text(markdown_report(result))
    print(
        f"completed {len(runs)} local smoke runs with byte-identical retries; "
        f"artifact {result['artifact_digest']}"
    )


if __name__ == "__main__":
    main()
