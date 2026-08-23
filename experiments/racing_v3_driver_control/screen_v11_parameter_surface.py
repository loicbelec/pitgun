#!/usr/bin/env python3
"""Replay the governed 33-profile surface against Model V3 candidate 0.13.0."""

from __future__ import annotations

import argparse
import concurrent.futures
import copy
import json
import math
import pathlib
import statistics
import subprocess
import tempfile
from collections import Counter, defaultdict
from typing import Any


ROOT = pathlib.Path(__file__).resolve().parents[2]
MANIFEST = (
    ROOT
    / "experiments/databricks/campaigns/racing-v3-driver-control-surface-v2.json"
)
DEFAULT_OUTPUT = (
    pathlib.Path(__file__).parent
    / "results/local-driver-friction-parameter-screen-v1.json"
)
SCHEMA_VERSION = "pitgun.racing-v3-driver-friction-parameter-screen/v1"
PROFILE_VERSION = "pitgun.racing-v3-experiment-profile/v11"
PROBE_SCHEMA_VERSION = "pitgun.racing-v3-driver-control-probe/v1"


class SurfaceError(RuntimeError):
    """Raised when the deterministic local surface cannot be reconciled."""


def canonical_pretty(value: object) -> bytes:
    return (json.dumps(value, indent=2, ensure_ascii=False) + "\n").encode()


def materialize(manifest: dict[str, Any]) -> list[dict[str, Any]]:
    result = []
    for row in manifest["configurations"]:
        profile = copy.deepcopy(manifest["profiles"][row["profile_ref"]])
        profile["schema_version"] = PROFILE_VERSION
        result.append(
            dict(row)
            | {
                "profile": profile,
                "scenario": manifest["scenarios"][row["scenario_ref"]],
                "driver_experiment": manifest["driver_experiments"][
                    row["driver_experiment_ref"]
                ],
            }
        )
    if len(result) != 1584:
        raise SurfaceError("governed source surface no longer contains 1,584 cases")
    return result


def execute(runner: pathlib.Path, item: dict[str, Any]) -> dict[str, Any]:
    with tempfile.TemporaryDirectory(prefix="pitgun-v11-surface-") as directory:
        temporary = pathlib.Path(directory)
        paths = {
            "scenario": temporary / "scenario.json",
            "profile": temporary / "profile.json",
            "driver_experiment": temporary / "driver-experiment.json",
        }
        for key, path in paths.items():
            path.write_bytes(canonical_pretty(item[key]))
        completed = subprocess.run(
            [
                str(runner),
                str(paths["scenario"]),
                str(paths["profile"]),
                str(paths["driver_experiment"]),
                str(item["seed"]),
            ],
            cwd=ROOT,
            check=False,
            capture_output=True,
            timeout=240,
        )
    if completed.returncode != 0:
        raise SurfaceError(
            f"probe failed for {item['execution_key']}: "
            + completed.stderr.decode(errors="replace").strip()
        )
    if completed.stderr:
        raise SurfaceError("successful probe wrote to stderr")
    try:
        probe = json.loads(completed.stdout)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise SurfaceError("probe returned invalid JSON") from error
    if probe.get("schema_version") != PROBE_SCHEMA_VERSION:
        raise SurfaceError("probe returned an unsupported result")

    laps = [float(value) for value in probe["player_lap_times_ms"]]
    resolution = probe["driver_control_resolutions"]["player"]
    diagnostics = probe["driver_control_diagnostics"]["player"]
    tire = probe["tire_diagnostics"]
    wear = probe["tire_degradation_diagnostics"]
    metadata = {
        key: item[key]
        for key in (
            "execution_key",
            "parameter_set_id",
            "circuit_id",
            "circuit_slug",
            "circuit_archetype",
            "horizon",
            "laps",
            "driver_id",
            "mode",
            "tire_id",
            "seed",
        )
    }
    return metadata | {
        "experimental_execution_id": probe["experimental_execution_id"],
        "model": probe["model"],
        "total_time_ms": probe["total_time_ms"],
        "mean_lap_ms": statistics.fmean(laps),
        "final_tire_temperature_c": probe["final_tire_temperature_c"],
        "final_tire_wear_pct": probe["final_tire_wear_pct"],
        "control_error_amplitude": resolution["control_error_amplitude"],
        "correction_workload_multiplier": resolution[
            "correction_workload_multiplier"
        ],
        "correction_force_capacity_fraction": diagnostics[
            "correction_force_capacity_fraction"
        ],
        "correction_contact_workload_mj": tire[
            "correction_contact_workload_mj"
        ],
        "requested_correction_wear_fraction": wear[
            "requested_correction_wear_fraction"
        ],
    }


def pathological(row: dict[str, Any]) -> bool:
    values = (
        row["total_time_ms"],
        row["mean_lap_ms"],
        row["final_tire_temperature_c"],
        row["final_tire_wear_pct"],
        row["correction_force_capacity_fraction"],
    )
    return (
        not all(math.isfinite(float(value)) for value in values)
        or not 0.0 <= row["final_tire_temperature_c"] <= 180.0
        or not 0.0 <= row["final_tire_wear_pct"] < 100.0
        or not 0.0 < row["correction_force_capacity_fraction"] <= 1.0
    )


def analyze(
    rows: list[dict[str, Any]], parameter_sets: list[dict[str, Any]]
) -> list[dict[str, Any]]:
    grouped: dict[tuple[Any, ...], dict[str, dict[str, Any]]] = defaultdict(dict)
    pathologies: Counter[str] = Counter()
    for row in rows:
        pathologies[row["parameter_set_id"]] += pathological(row)
        group = (
            row["parameter_set_id"],
            row["circuit_id"],
            row["horizon"],
            row["driver_id"],
            row["seed"],
        )
        grouped[group][row["mode"]] = row

    evaluations = []
    for parameter_set in parameter_sets:
        parameter_set_id = parameter_set["parameter_set_id"]
        short_groups = long_groups = short_attack_wins = long_attack_wins = 0
        ordering_failures = 0
        short_gains, long_wear_costs = [], []
        for group, modes in grouped.items():
            if group[0] != parameter_set_id:
                continue
            if set(modes) != {"manage", "balanced", "attack"}:
                raise SurfaceError("local surface contains an incomplete paired group")
            winner = min(modes, key=lambda mode: (modes[mode]["mean_lap_ms"], mode))
            if group[2] == "short":
                short_groups += 1
                short_attack_wins += winner == "attack"
                short_gains.append(
                    modes["manage"]["mean_lap_ms"]
                    - modes["attack"]["mean_lap_ms"]
                )
            else:
                long_groups += 1
                long_attack_wins += winner == "attack"
                long_wear_costs.append(
                    modes["attack"]["final_tire_wear_pct"]
                    - modes["manage"]["final_tire_wear_pct"]
                )
            if not (
                modes["attack"]["control_error_amplitude"]
                > modes["manage"]["control_error_amplitude"]
                and modes["attack"]["correction_contact_workload_mj"]
                > modes["manage"]["correction_contact_workload_mj"]
                and modes["attack"]["correction_force_capacity_fraction"]
                < modes["manage"]["correction_force_capacity_fraction"]
            ):
                ordering_failures += 1
        eligible = (
            short_attack_wins > 0
            and long_attack_wins < long_groups
            and ordering_failures == 0
            and pathologies[parameter_set_id] == 0
        )
        evaluations.append(
            {
                "parameter_set_id": parameter_set_id,
                "origin": parameter_set["origin"],
                "short_group_count": short_groups,
                "short_attack_win_count": short_attack_wins,
                "race_length_group_count": long_groups,
                "race_length_attack_win_count": long_attack_wins,
                "physical_ordering_failure_count": ordering_failures,
                "pathological_execution_count": pathologies[parameter_set_id],
                "median_short_attack_gain_ms": statistics.median(short_gains),
                "median_long_attack_wear_cost_percentage_points": (
                    statistics.median(long_wear_costs)
                ),
                "selection_gate_passed": eligible,
                "parameters": parameter_set["parameters"],
            }
        )
    return sorted(
        evaluations,
        key=lambda row: (
            not row["selection_gate_passed"],
            row["race_length_attack_win_count"],
            abs(row["short_attack_win_count"] - row["short_group_count"] / 2),
            row["parameter_set_id"],
        ),
    )


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--jobs", type=int, default=8)
    parser.add_argument("--output", type=pathlib.Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--check", action="store_true")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if not 1 <= args.jobs <= 16:
        raise SurfaceError("--jobs must be in [1, 16]")
    manifest = json.loads(MANIFEST.read_bytes())
    plan = materialize(manifest)
    runner = ROOT / "target/release/examples/v3_driver_control_probe"
    if not runner.is_file():
        raise SurfaceError("build the release v3_driver_control_probe first")

    rows = []
    with concurrent.futures.ThreadPoolExecutor(max_workers=args.jobs) as executor:
        futures = [executor.submit(execute, runner, item) for item in plan]
        for index, future in enumerate(concurrent.futures.as_completed(futures), start=1):
            rows.append(future.result())
            if index % 100 == 0 or index == len(futures):
                print(f"completed {index}/{len(futures)}", flush=True)
    rows.sort(key=lambda row: row["execution_key"])
    ranking = analyze(rows, manifest["parameter_sets"])
    report = {
        "schema_version": SCHEMA_VERSION,
        "source_campaign_id": manifest["campaign_id"],
        "source_manifest_digest": (
            "sha256:e751b3d827ffe8b431089b38669486e0b225e8db60763d0b46db2c08337d5a33"
        ),
        "model": rows[0]["model"],
        "execution_count": len(rows),
        "pathological_execution_count": sum(pathological(row) for row in rows),
        "selection_gate_pass_count": sum(
            row["selection_gate_passed"] for row in ranking
        ),
        "ranked_parameter_sets": ranking,
        "runs": rows,
    }
    encoded = canonical_pretty(report)
    if args.check:
        if not args.output.is_file() or args.output.read_bytes() != encoded:
            raise SurfaceError(f"{args.output} is missing or not reproducible")
        print(f"reproduced {args.output}")
        return 0
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_bytes(encoded)
    print(f"wrote {args.output}")
    print(f"selection gate passes: {report['selection_gate_pass_count']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
