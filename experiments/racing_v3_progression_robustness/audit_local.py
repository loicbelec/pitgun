#!/usr/bin/env python3
"""Audit Model V3 development and setup choices across progression and held-out tracks."""

from __future__ import annotations

import argparse
import concurrent.futures
import copy
import hashlib
import json
import pathlib
import statistics
import subprocess
import sys
import tempfile
from collections import defaultdict
from typing import Any


ROOT = pathlib.Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))

from experiments.racing_v3_validation import validate_local as shared  # noqa: E402


PROFILE = (
    ROOT
    / "experiments"
    / "racing_v3_decision_surface"
    / "profile-v7.compound-degradation.json"
)
STRATEGY_REPORT = (
    ROOT
    / "experiments"
    / "racing_v3_tire_degradation"
    / "results"
    / "local-tire-degradation-v1.json"
)
OUTPUT = pathlib.Path(__file__).parent / "results" / "local-progression-robustness-v1.json"
SCHEMA_VERSION = "pitgun.racing-v3-progression-robustness/v1"
PROBE_SCHEMA_VERSION = "pitgun.racing-v3-decision-surface-probe/v1"
MAX_JOBS = 16
EFFECT_THRESHOLD_MS = 5.0

AXES = ("aero", "chassis", "cooling", "engine")
POINT_KEYS = {axis: f"{axis}_points" for axis in AXES}
CALIBRATION_CIRCUITS = (
    ("it-1922", "monza", "power"),
    ("mc-1929", "monaco", "high-downforce"),
    ("jp-1962", "suzuka", "mixed"),
)
HELD_OUT_CIRCUITS = shared.CIRCUITS
CIRCUITS = tuple(
    (identifier, slug, archetype, split)
    for split, rows in (
        ("calibration", CALIBRATION_CIRCUITS),
        ("held_out", HELD_OUT_CIRCUITS),
    )
    for identifier, slug, archetype in rows
)
VEHICLES = (
    ("classic_v8_1960", 1, "era1-classic60"),
    ("classic_v8_1970", 3, "eras2-4-classic70"),
    ("modern_v6t", 5, "era5-modern"),
    ("f1_2026", 5, "era5-2026"),
)
PROGRESSION = (
    ("early", 4, 1),
    ("mid", 27, 3),
    ("late", 37, 4),
)
SEEDS = (7, 42)
SETUP_LEVELS = (0.0, 0.25, 0.5, 0.75, 1.0)
LAPS = 3


class AuditError(RuntimeError):
    """Raised when the governed audit cannot produce reproducible evidence."""


def canonical_pretty(value: object) -> bytes:
    return (json.dumps(value, indent=2, ensure_ascii=False) + "\n").encode()


def sha256(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


def balanced_points(budget: int) -> dict[str, float]:
    quotient, remainder = divmod(budget, len(AXES))
    return {
        POINT_KEYS[axis]: float(quotient + (index < remainder))
        for index, axis in enumerate(AXES)
    }


def transfer_points(budget: int, delta: int, donor: str, target: str) -> dict[str, float]:
    points = balanced_points(budget)
    donor_key = POINT_KEYS[donor]
    target_key = POINT_KEYS[target]
    if donor == target or points[donor_key] < delta:
        raise ValueError("development transfer is impossible")
    points[donor_key] -= delta
    points[target_key] += delta
    assert sum(points.values()) == budget
    return points


def configured_scenario(
    base: dict[str, Any],
    *,
    circuit_id: str,
    vehicle_id: str,
    era: int,
    points: dict[str, float],
    downforce: float = 0.5,
    gearing: float = 0.5,
) -> dict[str, Any]:
    scenario = shared.configure_scenario(
        base,
        circuit_id=circuit_id,
        vehicle_id=vehicle_id,
        era=era,
        laps=LAPS,
    )
    player = scenario["request"]["competitors"][0]
    player["budget_cap"] = float(sum(points.values()))
    player["tuning"].update(points)
    player["tuning"].update(
        {"downforce_slider": downforce, "gear_ratio_slider": gearing}
    )
    return scenario


def build_plan(base: dict[str, Any]) -> list[dict[str, Any]]:
    plan: list[dict[str, Any]] = []
    for circuit_id, circuit_slug, archetype, split in CIRCUITS:
        for vehicle_id, era, vehicle_anchor in VEHICLES:
            for progression, budget, delta in PROGRESSION:
                allocations = [("balanced", None, None, balanced_points(budget))]
                allocations.extend(
                    (
                        f"transfer-{donor}-to-{target}",
                        donor,
                        target,
                        transfer_points(budget, delta, donor, target),
                    )
                    for donor in AXES
                    for target in AXES
                    if donor != target
                )
                for allocation_id, donor, target, points in allocations:
                    for seed in SEEDS:
                        plan.append(
                            {
                                "family": "development.transfer",
                                "case_id": allocation_id,
                                "circuit_id": circuit_id,
                                "circuit_slug": circuit_slug,
                                "circuit_archetype": archetype,
                                "split": split,
                                "vehicle_id": vehicle_id,
                                "vehicle_anchor": vehicle_anchor,
                                "era": era,
                                "progression": progression,
                                "budget": budget,
                                "transfer_delta": delta,
                                "donor_axis": donor,
                                "target_axis": target,
                                "allocation": points,
                                "seed": seed,
                                "scenario": configured_scenario(
                                    base,
                                    circuit_id=circuit_id,
                                    vehicle_id=vehicle_id,
                                    era=era,
                                    points=points,
                                ),
                            }
                        )

                baseline = balanced_points(budget)
                for axis in AXES:
                    for direction, change in (("minus", -1.0), ("plus", 1.0)):
                        points = dict(baseline)
                        points[POINT_KEYS[axis]] += change
                        for seed in SEEDS:
                            plan.append(
                                {
                                    "family": "development.marginal",
                                    "case_id": f"marginal-{axis}-{direction}",
                                    "circuit_id": circuit_id,
                                    "circuit_slug": circuit_slug,
                                    "circuit_archetype": archetype,
                                    "split": split,
                                    "vehicle_id": vehicle_id,
                                    "vehicle_anchor": vehicle_anchor,
                                    "era": era,
                                    "progression": progression,
                                    "budget": budget,
                                    "actual_budget": int(sum(points.values())),
                                    "axis": axis,
                                    "direction": direction,
                                    "allocation": points,
                                    "seed": seed,
                                    "scenario": configured_scenario(
                                        base,
                                        circuit_id=circuit_id,
                                        vehicle_id=vehicle_id,
                                        era=era,
                                        points=points,
                                    ),
                                }
                            )

            setup_points = balanced_points(27)
            for downforce in SETUP_LEVELS:
                for gearing in SETUP_LEVELS:
                    for seed in SEEDS:
                        plan.append(
                            {
                                "family": "setup.grid",
                                "case_id": f"setup-df-{downforce:g}-gear-{gearing:g}",
                                "circuit_id": circuit_id,
                                "circuit_slug": circuit_slug,
                                "circuit_archetype": archetype,
                                "split": split,
                                "vehicle_id": vehicle_id,
                                "vehicle_anchor": vehicle_anchor,
                                "era": era,
                                "progression": "mid",
                                "budget": 27,
                                "downforce_slider": downforce,
                                "gear_ratio_slider": gearing,
                                "seed": seed,
                                "scenario": configured_scenario(
                                    base,
                                    circuit_id=circuit_id,
                                    vehicle_id=vehicle_id,
                                    era=era,
                                    points=setup_points,
                                    downforce=downforce,
                                    gearing=gearing,
                                ),
                            }
                        )
    assert len(plan) == 4928
    return plan


def compact_result(point: dict[str, Any], result: dict[str, Any]) -> dict[str, Any]:
    mechanical = result["mechanical_diagnostics"]
    tire = result["tire_diagnostics"]
    fuel = result["fuel_mass_diagnostics"]
    degradation = result["tire_degradation_diagnostics"]
    metadata = {key: value for key, value in point.items() if key != "scenario"}
    return metadata | {
        "total_time_ms": result["total_time_ms"],
        "maximum_speed_kph": result["observed_maximum_speed_kph"],
        "maximum_engine_temperature_c": mechanical["maximum_engine_temperature_c"],
        "engine_derated_time_s": mechanical["engine_derated_time_s"],
        "maximum_tire_utilization": tire["maximum_combined_utilization"],
        "fuel_consumed_kg": fuel["fuel_consumed_kg"],
        "final_tire_wear_pct": result["final_tire_wear_pct"],
        "thermal_wear_multiplier": degradation["maximum_thermal_wear_multiplier"],
        "result_digest": sha256(canonical_pretty(result)),
    }


def execute_point(
    runner: pathlib.Path, profile_path: pathlib.Path, point: dict[str, Any]
) -> tuple[dict[str, Any], dict[str, Any]]:
    with tempfile.TemporaryDirectory(prefix="pitgun-v3-progression-") as directory:
        scenario_path = pathlib.Path(directory) / "scenario.json"
        scenario_path.write_bytes(canonical_pretty(point["scenario"]))
        completed = subprocess.run(
            [str(runner), str(scenario_path), str(profile_path), str(point["seed"])],
            cwd=ROOT,
            check=False,
            capture_output=True,
            timeout=120,
        )
    if completed.returncode != 0:
        raise AuditError(
            f"probe failed for {point['case_id']} {point['circuit_slug']} "
            f"{point['vehicle_id']}: "
            + completed.stderr.decode(errors="replace").strip()
        )
    if completed.stderr:
        raise AuditError(f"successful probe wrote to stderr for {point['case_id']}")
    try:
        result = json.loads(completed.stdout)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise AuditError("probe returned invalid JSON") from error
    if result.get("schema_version") != PROBE_SCHEMA_VERSION:
        raise AuditError("probe returned an unsupported result")
    return compact_result(point, result), result["model"]


def grouped(points: list[dict[str, Any]], family: str) -> dict[tuple[Any, ...], list[dict[str, Any]]]:
    rows: dict[tuple[Any, ...], list[dict[str, Any]]] = defaultdict(list)
    for point in points:
        if point["family"] != family:
            continue
        key = (
            point["split"],
            point["circuit_id"],
            point["circuit_slug"],
            point["vehicle_id"],
            point["vehicle_anchor"],
            point["progression"],
            point["budget"],
        )
        rows[key].append(point)
    return rows


def development_summary(points: list[dict[str, Any]]) -> dict[str, Any]:
    groups = []
    for key, rows in sorted(grouped(points, "development.transfer").items()):
        split, circuit_id, circuit_slug, vehicle_id, vehicle_anchor, progression, budget = key
        baseline_rows = [row for row in rows if row["case_id"] == "balanced"]
        baseline_ms = statistics.median(row["total_time_ms"] for row in baseline_rows)
        variants = []
        for case_id in sorted({row["case_id"] for row in rows if row["case_id"] != "balanced"}):
            selected = [row for row in rows if row["case_id"] == case_id]
            median_ms = statistics.median(row["total_time_ms"] for row in selected)
            sample = selected[0]
            variants.append(
                {
                    "case_id": case_id,
                    "donor_axis": sample["donor_axis"],
                    "target_axis": sample["target_axis"],
                    "allocation": sample["allocation"],
                    "median_total_time_ms": median_ms,
                    "benefit_vs_balanced_ms": baseline_ms - median_ms,
                    "benefit_per_point_ms": (
                        baseline_ms - median_ms
                    ) / sample["transfer_delta"],
                }
            )
        fastest = min(
            [{"case_id": "balanced", "median_total_time_ms": baseline_ms}, *variants],
            key=lambda row: (row["median_total_time_ms"], row["case_id"]),
        )
        axis_effects = {}
        for axis in AXES:
            inbound = [row["benefit_per_point_ms"] for row in variants if row["target_axis"] == axis]
            outbound = [row["benefit_per_point_ms"] for row in variants if row["donor_axis"] == axis]
            axis_effects[axis] = {
                "median_inbound_benefit_per_point_ms": statistics.median(inbound),
                "median_outbound_transfer_benefit_per_point_ms": statistics.median(outbound),
                "maximum_absolute_effect_per_point_ms": max(
                    abs(value) for value in [*inbound, *outbound]
                ),
                "dominant_in_group": (
                    all(value > EFFECT_THRESHOLD_MS for value in inbound)
                    and all(value < -EFFECT_THRESHOLD_MS for value in outbound)
                ),
                "inactive_in_group": all(
                    abs(value) <= EFFECT_THRESHOLD_MS for value in [*inbound, *outbound]
                ),
            }
        groups.append(
            {
                "split": split,
                "circuit_id": circuit_id,
                "circuit_slug": circuit_slug,
                "vehicle_id": vehicle_id,
                "vehicle_anchor": vehicle_anchor,
                "progression": progression,
                "budget": budget,
                "balanced_total_time_ms": baseline_ms,
                "balanced_regret_ms": baseline_ms - fastest["median_total_time_ms"],
                "fastest_case_id": fastest["case_id"],
                "axis_effects": axis_effects,
                "variants": variants,
            }
        )
    progression_effects = []
    for vehicle_id, _, vehicle_anchor in VEHICLES:
        for split in ("calibration", "held_out"):
            for axis in AXES:
                anchors = []
                for progression, budget, _ in PROGRESSION:
                    selected = [
                        row
                        for row in groups
                        if row["vehicle_id"] == vehicle_id
                        and row["split"] == split
                        and row["progression"] == progression
                    ]
                    all_effects = [
                        value
                        for row in selected
                        for variant in row["variants"]
                        if axis in (variant["donor_axis"], variant["target_axis"])
                        for value in [variant["benefit_per_point_ms"]]
                    ]
                    anchors.append(
                        {
                            "progression": progression,
                            "budget": budget,
                            "median_absolute_transfer_effect_per_point_ms": statistics.median(
                                abs(value) for value in all_effects
                            ),
                        }
                    )
                early = anchors[0]["median_absolute_transfer_effect_per_point_ms"]
                late = anchors[-1]["median_absolute_transfer_effect_per_point_ms"]
                ratio = late / early if early else None
                progression_effects.append(
                    {
                        "vehicle_id": vehicle_id,
                        "vehicle_anchor": vehicle_anchor,
                        "split": split,
                        "axis": axis,
                        "late_to_early_absolute_effect_ratio": ratio,
                        "shape": (
                            "inactive"
                            if ratio is None
                            else "diminishing"
                            if ratio < 0.5
                            else "increasing"
                            if ratio > 1.5
                            else "stable"
                        ),
                        "anchors": anchors,
                    }
                )
    return {
        "group_count": len(groups),
        "progression_effects": progression_effects,
        "groups": groups,
    }


def marginal_summary(
    points: list[dict[str, Any]], development: dict[str, Any]
) -> dict[str, Any]:
    baseline = {
        (
            row["split"],
            row["circuit_id"],
            row["vehicle_id"],
            row["progression"],
        ): row["balanced_total_time_ms"]
        for row in development["groups"]
    }
    groups = []
    for key, rows in sorted(grouped(points, "development.marginal").items()):
        split, circuit_id, circuit_slug, vehicle_id, vehicle_anchor, progression, budget = key
        baseline_ms = baseline[(split, circuit_id, vehicle_id, progression)]
        axes = {}
        for axis in AXES:
            plus = statistics.median(
                row["total_time_ms"]
                for row in rows
                if row["axis"] == axis and row["direction"] == "plus"
            )
            minus = statistics.median(
                row["total_time_ms"]
                for row in rows
                if row["axis"] == axis and row["direction"] == "minus"
            )
            add_benefit = baseline_ms - plus
            remove_penalty = minus - baseline_ms
            axes[axis] = {
                "plus_total_time_ms": plus,
                "minus_total_time_ms": minus,
                "add_one_benefit_ms": add_benefit,
                "remove_one_penalty_ms": remove_penalty,
                "symmetric_marginal_benefit_ms": statistics.median(
                    (add_benefit, remove_penalty)
                ),
                "inactive_in_group": (
                    abs(add_benefit) <= EFFECT_THRESHOLD_MS
                    and abs(remove_penalty) <= EFFECT_THRESHOLD_MS
                ),
                "harmful_in_group": (
                    add_benefit < -EFFECT_THRESHOLD_MS
                    and remove_penalty < -EFFECT_THRESHOLD_MS
                ),
            }
        groups.append(
            {
                "split": split,
                "circuit_id": circuit_id,
                "circuit_slug": circuit_slug,
                "vehicle_id": vehicle_id,
                "vehicle_anchor": vehicle_anchor,
                "progression": progression,
                "budget": budget,
                "balanced_total_time_ms": baseline_ms,
                "axis_effects": axes,
            }
        )

    progression_effects = []
    for vehicle_id, _, vehicle_anchor in VEHICLES:
        for split in ("calibration", "held_out"):
            for axis in AXES:
                anchors = []
                for progression, budget, _ in PROGRESSION:
                    selected = [
                        row
                        for row in groups
                        if row["vehicle_id"] == vehicle_id
                        and row["split"] == split
                        and row["progression"] == progression
                    ]
                    effects = [
                        row["axis_effects"][axis]["symmetric_marginal_benefit_ms"]
                        for row in selected
                    ]
                    anchors.append(
                        {
                            "progression": progression,
                            "budget": budget,
                            "median_marginal_benefit_ms": statistics.median(effects),
                            "median_absolute_marginal_effect_ms": statistics.median(
                                abs(value) for value in effects
                            ),
                        }
                    )
                early = anchors[0]["median_absolute_marginal_effect_ms"]
                late = anchors[-1]["median_absolute_marginal_effect_ms"]
                ratio = late / early if early else None
                progression_effects.append(
                    {
                        "vehicle_id": vehicle_id,
                        "vehicle_anchor": vehicle_anchor,
                        "split": split,
                        "axis": axis,
                        "late_to_early_absolute_effect_ratio": ratio,
                        "shape": (
                            "inactive"
                            if ratio is None
                            else "diminishing"
                            if ratio < 0.5
                            else "increasing"
                            if ratio > 1.5
                            else "stable"
                        ),
                        "anchors": anchors,
                    }
                )
    return {
        "group_count": len(groups),
        "progression_effects": progression_effects,
        "groups": groups,
    }


def setup_summary(points: list[dict[str, Any]]) -> dict[str, Any]:
    groups = []
    for key, rows in sorted(grouped(points, "setup.grid").items()):
        split, circuit_id, circuit_slug, vehicle_id, vehicle_anchor, progression, budget = key
        levels = []
        for case_id in sorted({row["case_id"] for row in rows}):
            selected = [row for row in rows if row["case_id"] == case_id]
            sample = selected[0]
            levels.append(
                {
                    "case_id": case_id,
                    "downforce_slider": sample["downforce_slider"],
                    "gear_ratio_slider": sample["gear_ratio_slider"],
                    "median_total_time_ms": statistics.median(
                        row["total_time_ms"] for row in selected
                    ),
                }
            )
        fastest = min(levels, key=lambda row: (row["median_total_time_ms"], row["case_id"]))
        neutral = next(
            row
            for row in levels
            if row["downforce_slider"] == row["gear_ratio_slider"] == 0.5
        )
        groups.append(
            {
                "split": split,
                "circuit_id": circuit_id,
                "circuit_slug": circuit_slug,
                "vehicle_id": vehicle_id,
                "vehicle_anchor": vehicle_anchor,
                "progression": progression,
                "budget": budget,
                "fastest_case_id": fastest["case_id"],
                "neutral_regret_ms": neutral["median_total_time_ms"]
                - fastest["median_total_time_ms"],
                "levels": levels,
            }
        )
    return {"group_count": len(groups), "groups": groups}


def load_strategy_evidence(path: pathlib.Path) -> dict[str, Any]:
    report_bytes = path.read_bytes()
    if path.with_suffix(".sha256").read_text().strip() != sha256(report_bytes):
        raise AuditError("strategy evidence does not match its SHA-256 sidecar")
    report = json.loads(report_bytes)
    groups = report["strategy_summary"]["groups"]
    generic_stop_lap = 16
    regrets = []
    for group in groups:
        fastest = min(group["windows"], key=lambda row: row["total_time_ms"])
        generic = next(row for row in group["windows"] if row["stop_lap"] == generic_stop_lap)
        regrets.append(generic["total_time_ms"] - fastest["total_time_ms"])
    return {
        "path": str(path.relative_to(ROOT)),
        "digest": sha256(report_bytes),
        "model": report["campaign"]["model"],
        "group_count": len(groups),
        "generic_stop_lap": generic_stop_lap,
        "fastest_stop_laps": report["strategy_summary"]["fastest_stop_laps"],
        "median_generic_strategy_regret_ms": statistics.median(regrets),
        "maximum_generic_strategy_regret_ms": max(regrets),
    }


def vehicle_progression_verdicts(
    development: dict[str, Any], marginals: dict[str, Any], setup: dict[str, Any]
) -> list[dict[str, Any]]:
    verdicts = []
    for vehicle_id, _, vehicle_anchor in VEHICLES:
        vehicle_setups = [row for row in setup["groups"] if row["vehicle_id"] == vehicle_id]
        for progression, budget, _ in PROGRESSION:
            rows = [
                row
                for row in development["groups"]
                if row["vehicle_id"] == vehicle_id and row["progression"] == progression
            ]
            marginal_rows = [
                row
                for row in marginals["groups"]
                if row["vehicle_id"] == vehicle_id and row["progression"] == progression
            ]
            inactive = [
                axis
                for axis in AXES
                if all(
                    row["axis_effects"][axis]["inactive_in_group"]
                    for row in marginal_rows
                )
            ]
            harmful = [
                axis
                for axis in AXES
                if all(
                    row["axis_effects"][axis]["harmful_in_group"]
                    for row in marginal_rows
                )
            ]
            dominant = [
                axis
                for axis in AXES
                if all(row["axis_effects"][axis]["dominant_in_group"] for row in rows)
            ]
            calibration_fastest = {
                row["fastest_case_id"] for row in rows if row["split"] == "calibration"
            }
            held_out_fastest = {
                row["fastest_case_id"] for row in rows if row["split"] == "held_out"
            }
            if inactive or harmful or dominant:
                verdict = "STRUCTURAL_CHANGE_REQUIRED"
            elif len(held_out_fastest) == 1 or calibration_fastest.isdisjoint(held_out_fastest):
                verdict = "REFINE"
            else:
                verdict = "PASS"
            setup_optima = {
                row["fastest_case_id"] for row in vehicle_setups
            }
            verdicts.append(
                {
                    "vehicle_id": vehicle_id,
                    "vehicle_anchor": vehicle_anchor,
                    "progression": progression,
                    "budget": budget,
                    "verdict": verdict,
                    "inactive_axes": inactive,
                    "universally_harmful_axes": harmful,
                    "universally_dominant_axes": dominant,
                    "calibration_fastest_allocations": sorted(calibration_fastest),
                    "held_out_fastest_allocations": sorted(held_out_fastest),
                    "distinct_setup_optimum_count": len(setup_optima),
                    "reason": (
                        "At least one development control is globally inactive, harmful or dominant at this vehicle/progression anchor."
                        if inactive or harmful or dominant
                        else "Held-out allocation behavior requires refinement before calibration."
                        if verdict == "REFINE"
                        else "All development controls are active without a universal winner, and allocation behavior transfers to held-out tracks."
                    ),
                }
            )
    return verdicts


def campaign_verdicts(
    setup: dict[str, Any],
    strategy: dict[str, Any],
    anchors: list[dict[str, Any]],
) -> list[dict[str, str]]:
    inactive = sorted({axis for row in anchors for axis in row["inactive_axes"]})
    harmful = sorted(
        {axis for row in anchors for axis in row["universally_harmful_axes"]}
    )
    dominant = sorted(
        {axis for row in anchors for axis in row["universally_dominant_axes"]}
    )
    calibration_optima = {
        row["fastest_case_id"] for row in setup["groups"] if row["split"] == "calibration"
    }
    heldout_optima = {
        row["fastest_case_id"] for row in setup["groups"] if row["split"] == "held_out"
    }
    return [
        {
            "capability": "multi_era_development_surface",
            "verdict": (
                "STRUCTURAL_CHANGE_REQUIRED"
                if inactive or harmful or dominant
                else "PASS"
            ),
            "reason": (
                f"Axes inactive at one or more full vehicle/progression anchors: {', '.join(inactive) or 'none'}; "
                f"axes harmful at one or more full anchors: {', '.join(harmful) or 'none'}; "
                f"axes dominant at one or more full anchors: {', '.join(dominant) or 'none'}."
            ),
        },
        {
            "capability": "held_out_setup_specialization",
            "verdict": "PASS" if len(heldout_optima) > 1 else "REFINE",
            "reason": (
                f"Calibration exposes {len(calibration_optima)} setup optima and held-out tracks expose {len(heldout_optima)}."
            ),
        },
        {
            "capability": "generic_strategy_robustness",
            "verdict": "REFINE",
            "reason": (
                f"The linked generic L{strategy['generic_stop_lap']} stop loses a median "
                f"{strategy['median_generic_strategy_regret_ms']:.1f} ms, while L{strategy['fastest_stop_laps'][0]} remains universally fastest."
            ),
        },
        {
            "capability": "production_or_opponent_policy_change",
            "verdict": "REFINE",
            "reason": "This deterministic local campaign diagnoses the 0.9 model; it authorizes neither parameter tuning nor a catalog/opponent-policy change.",
        },
    ]


def build_report(
    runner: pathlib.Path,
    jobs: int,
    profile_path: pathlib.Path = PROFILE,
    strategy_path: pathlib.Path = STRATEGY_REPORT,
) -> dict[str, Any]:
    if not runner.is_file():
        raise AuditError(f"missing V3 decision-surface probe: {runner}")
    if jobs < 1 or jobs > MAX_JOBS:
        raise ValueError(f"jobs must be between 1 and {MAX_JOBS}")
    scenario_bytes = shared.BASE_SCENARIO.read_bytes()
    profile_bytes = profile_path.read_bytes()
    plan = build_plan(json.loads(scenario_bytes))
    identities: list[dict[str, Any]] = []
    with concurrent.futures.ThreadPoolExecutor(max_workers=jobs) as executor:
        executed = list(
            executor.map(
                lambda point: execute_point(runner, profile_path, point), plan
            )
        )
    points = []
    for point, identity in executed:
        points.append(point)
        identities.append(identity)
    points.sort(
        key=lambda row: (
            row["family"],
            row["split"],
            row["circuit_id"],
            row["vehicle_id"],
            row["progression"],
            row["case_id"],
            row["seed"],
        )
    )
    identity_set = {json.dumps(identity, sort_keys=True) for identity in identities}
    if len(identity_set) != 1:
        raise AuditError("campaign mixed multiple model identities")
    model = json.loads(next(iter(identity_set)))
    strategy = load_strategy_evidence(strategy_path)
    if strategy["model"] != model:
        raise AuditError("development and strategy evidence bind different models")
    development = development_summary(points)
    marginals = marginal_summary(points, development)
    setup = setup_summary(points)
    anchors = vehicle_progression_verdicts(development, marginals, setup)
    return {
        "schema_version": SCHEMA_VERSION,
        "campaign": {
            "purpose": "bounded multi-era progression and held-out robustness audit",
            "model": model,
            "runner": {
                "path": str(runner.relative_to(ROOT)),
                "digest": sha256(runner.read_bytes()),
            },
            "base_scenario_path": str(shared.BASE_SCENARIO.relative_to(ROOT)),
            "base_scenario_digest": sha256(scenario_bytes),
            "profile_path": str(profile_path.relative_to(ROOT)),
            "profile_digest": sha256(profile_bytes),
            "execution_count": len(points),
            "simulated_lap_count": len(points) * LAPS,
            "seeds": list(SEEDS),
            "progression_budgets": {
                progression: budget for progression, budget, _ in PROGRESSION
            },
            "calibration_circuits": [row[0] for row in CALIBRATION_CIRCUITS],
            "held_out_circuits": [row[0] for row in HELD_OUT_CIRCUITS],
            "vehicles": [row[0] for row in VEHICLES],
        },
        "verdicts": campaign_verdicts(setup, strategy, anchors),
        "vehicle_progression_verdicts": anchors,
        "development_summary": development,
        "marginal_summary": marginals,
        "setup_summary": setup,
        "strategy_evidence": strategy,
        "points": points,
    }


def write_or_check(report: dict[str, Any], output: pathlib.Path, check: bool) -> None:
    report_bytes = canonical_pretty(report)
    digest_bytes = (sha256(report_bytes) + "\n").encode()
    digest_path = output.with_suffix(".sha256")
    if check:
        if output.read_bytes() != report_bytes or digest_path.read_bytes() != digest_bytes:
            raise AuditError("stored progression-robustness artifacts do not match replay")
        return
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_bytes(report_bytes)
    digest_path.write_bytes(digest_bytes)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--runner",
        type=pathlib.Path,
        default=ROOT / "target" / "release" / "examples" / "v3_decision_surface_probe",
    )
    parser.add_argument("--jobs", type=int, default=4)
    parser.add_argument("--profile", type=pathlib.Path, default=PROFILE)
    parser.add_argument("--strategy-report", type=pathlib.Path, default=STRATEGY_REPORT)
    parser.add_argument("--output", type=pathlib.Path, default=OUTPUT)
    parser.add_argument("--check", action="store_true")
    arguments = parser.parse_args()
    try:
        report = build_report(
            arguments.runner.resolve(),
            arguments.jobs,
            arguments.profile.resolve(),
            arguments.strategy_report.resolve(),
        )
        write_or_check(report, arguments.output.resolve(), arguments.check)
    except (OSError, ValueError, AuditError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
