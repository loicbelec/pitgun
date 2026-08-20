#!/usr/bin/env python3
"""Run the bounded local screen for the Racing Model V3 decision surface."""

from __future__ import annotations

import argparse
import concurrent.futures
import copy
import hashlib
import json
import pathlib
import statistics
import subprocess
import tempfile
from typing import Any


ROOT = pathlib.Path(__file__).resolve().parents[2]
BASE_SCENARIO = (
    ROOT / "apps" / "pitgun-cli" / "scenarios" / "racing-batch-v1" / "balanced.json"
)
BASE_PROFILE = pathlib.Path(__file__).with_name("profile-v1.default.json")
DEFAULT_OUTPUT = pathlib.Path(__file__).parent / "results" / "local-screen-v1.json"
SCHEMA_VERSION = "pitgun.racing-v3-local-screen/v1"
SEEDS = (7, 42, 99)
CIRCUITS = (
    ("it-1922", "power", "monza"),
    ("mc-1929", "high-downforce", "monaco"),
    ("jp-1962", "mixed", "suzuka"),
)
MAX_JOBS = 16


class ScreenError(RuntimeError):
    """Raised when the bounded screen cannot produce governed evidence."""


def canonical_pretty(value: object) -> bytes:
    return (json.dumps(value, indent=2, ensure_ascii=False) + "\n").encode()


def sha256(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


def set_path(document: dict[str, Any], path: str, value: Any) -> None:
    cursor: dict[str, Any] = document
    parts = path.split(".")
    for part in parts[:-1]:
        cursor = cursor.setdefault(part, {})
    cursor[parts[-1]] = value


def development_pair(axis: str) -> tuple[dict[str, float], dict[str, float]]:
    """Return a low/high pair with the same forty-point total budget."""
    names = ("engine_points", "cooling_points", "aero_points", "chassis_points")
    low = {name: 12.0 for name in names}
    high = {name: 8.0 for name in names}
    low[axis] = 4.0
    high[axis] = 16.0
    assert sum(low.values()) == sum(high.values()) == 40.0
    return low, high


def scenario_variants(base: dict[str, Any]) -> list[dict[str, Any]]:
    variants: list[dict[str, Any]] = []

    def add(identifier: str, family: str, level: str, changes: dict[str, float]) -> None:
        scenario = copy.deepcopy(base)
        tuning = scenario["request"]["competitors"][0]["tuning"]
        tuning.update(
            {
                "engine_points": 10.0,
                "cooling_points": 10.0,
                "aero_points": 10.0,
                "chassis_points": 10.0,
                "downforce_slider": 0.5,
                "gear_ratio_slider": 0.5,
            }
        )
        tuning.update(changes)
        scenario["request"]["laps"] = 3
        variants.append(
            {"id": identifier, "family": family, "level": level, "scenario": scenario}
        )

    add(
        "gameplay-baseline",
        "baseline",
        "baseline",
        {
            "engine_points": 10.0,
            "cooling_points": 10.0,
            "aero_points": 10.0,
            "chassis_points": 10.0,
            "downforce_slider": 0.5,
            "gear_ratio_slider": 0.5,
        },
    )
    for axis in ("engine_points", "cooling_points", "aero_points", "chassis_points"):
        low, high = development_pair(axis)
        add(f"development-{axis}-low", f"development.{axis}", "low", low)
        add(f"development-{axis}-high", f"development.{axis}", "high", high)
    for axis in ("downforce_slider", "gear_ratio_slider"):
        add(f"setup-{axis}-low", f"setup.{axis}", "low", {axis: 0.2})
        add(f"setup-{axis}-high", f"setup.{axis}", "high", {axis: 0.8})
    for downforce in (0.2, 0.8):
        for gearing in (0.2, 0.8):
            add(
                f"setup-interaction-df-{downforce}-gear-{gearing}",
                "setup.interaction",
                f"df-{downforce}-gear-{gearing}",
                {"downforce_slider": downforce, "gear_ratio_slider": gearing},
            )
    return variants


PROFILE_AXES: tuple[tuple[str, str, float, float], ...] = (
    ("brake_force", "mechanical_overrides.maximum_brake_force_n", 14_000.0, 22_000.0),
    ("shift_duration", "mechanical_overrides.shift_duration_s", 0.03, 0.09),
    ("driveline_efficiency", "mechanical_overrides.driveline_efficiency", 0.92, 0.98),
    ("tire_load_sensitivity", "tire_contact.load_sensitivity_exponent", 0.04, 0.12),
    ("tire_heat_generation", "tire_contact.heat_generation_fraction", 0.003, 0.012),
    ("tire_static_cooling", "tire_contact.cooling_w_per_c", 40.0, 160.0),
    (
        "tire_wear_energy",
        "tire_contact.workload_energy_to_full_wear_j",
        4_000_000_000.0,
        12_000_000_000.0,
    ),
    ("driver_cornering", "driver_control_override.cornering_utilization", 0.85, 0.99),
    ("driver_braking", "driver_control_override.braking_utilization", 0.85, 0.99),
    ("driver_traction", "driver_control_override.traction_utilization", 0.85, 0.99),
    ("driver_control_error", "driver_control_override.control_error", 0.08, 0.005),
)


def profile_variants(base: dict[str, Any]) -> list[dict[str, Any]]:
    baseline = copy.deepcopy(base)
    baseline["driver_control_override"] = {
        "cornering_utilization": 0.98,
        "braking_utilization": 0.97,
        "traction_utilization": 0.98,
        "control_error": 0.01,
    }
    variants = [
        {"id": "physical-baseline", "family": "baseline", "level": "baseline", "profile": baseline}
    ]
    for identifier, path, low, high in PROFILE_AXES:
        for level, value in (("low", low), ("high", high)):
            profile = copy.deepcopy(baseline)
            set_path(profile, path, value)
            variants.append(
                {
                    "id": f"physical-{identifier}-{level}",
                    "family": f"physical.{identifier}",
                    "level": level,
                    "profile": profile,
                }
            )
    return variants


def build_plan(base_scenario: dict[str, Any], base_profile: dict[str, Any]) -> list[dict[str, Any]]:
    gameplay = scenario_variants(base_scenario)
    physical = profile_variants(base_profile)
    default_profile = next(item for item in physical if item["level"] == "baseline")
    default_scenario = next(item for item in gameplay if item["level"] == "baseline")
    cases = [(item, default_profile) for item in gameplay]
    cases.extend((default_scenario, item) for item in physical if item["level"] != "baseline")
    plan = []
    for scenario_variant, profile_variant in cases:
        for circuit_id, archetype, slug in CIRCUITS:
            for seed in SEEDS:
                scenario = copy.deepcopy(scenario_variant["scenario"])
                scenario["request"]["track_id"] = circuit_id
                plan.append(
                    {
                        "case_id": f"{scenario_variant['id']}__{profile_variant['id']}",
                        "family": (
                            scenario_variant["family"]
                            if scenario_variant["family"] != "baseline"
                            else profile_variant["family"]
                        ),
                        "level": (
                            scenario_variant["level"]
                            if scenario_variant["family"] != "baseline"
                            else profile_variant["level"]
                        ),
                        "circuit_id": circuit_id,
                        "circuit_archetype": archetype,
                        "circuit_slug": slug,
                        "seed": seed,
                        "scenario": scenario,
                        "profile": profile_variant["profile"],
                    }
                )
    return plan


def execute_point(runner: pathlib.Path, point: dict[str, Any]) -> dict[str, Any]:
    with tempfile.TemporaryDirectory(prefix="pitgun-v3-screen-") as directory:
        directory_path = pathlib.Path(directory)
        scenario_path = directory_path / "scenario.json"
        profile_path = directory_path / "profile.json"
        scenario_path.write_bytes(canonical_pretty(point["scenario"]))
        profile_path.write_bytes(canonical_pretty(point["profile"]))
        completed = subprocess.run(
            [str(runner), str(scenario_path), str(profile_path), str(point["seed"])],
            cwd=ROOT,
            check=False,
            capture_output=True,
            timeout=120,
        )
    if completed.returncode != 0:
        raise ScreenError(
            f"probe failed for {point['case_id']} {point['circuit_slug']} seed {point['seed']}: "
            f"{completed.stderr.decode(errors='replace').strip()}"
        )
    if completed.stderr:
        raise ScreenError(f"successful probe wrote to stderr for {point['case_id']}")
    try:
        result = json.loads(completed.stdout)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ScreenError("probe returned invalid JSON") from error
    if result.get("schema_version") != "pitgun.racing-v3-decision-surface-probe/v1":
        raise ScreenError("probe returned an unsupported result")
    return {
        key: point[key]
        for key in (
            "case_id",
            "family",
            "level",
            "circuit_id",
            "circuit_archetype",
            "circuit_slug",
            "seed",
        )
    } | result


def median_metrics(points: list[dict[str, Any]]) -> dict[str, float]:
    return {
        "total_time_ms": statistics.median(point["total_time_ms"] for point in points),
        "maximum_speed_kph": statistics.median(
            point["observed_maximum_speed_kph"] for point in points
        ),
        "maximum_engine_temperature_c": statistics.median(
            point["mechanical_diagnostics"]["maximum_engine_temperature_c"] for point in points
        ),
        "engine_derated_time_s": statistics.median(
            point["mechanical_diagnostics"]["engine_derated_time_s"] for point in points
        ),
        "maximum_tire_utilization": statistics.median(
            point["tire_diagnostics"]["maximum_combined_utilization"] for point in points
        ),
        "tire_generated_heat_kj": statistics.median(
            point["tire_diagnostics"]["generated_heat_kj"] for point in points
        ),
    }


def summarize_pairs(points: list[dict[str, Any]]) -> list[dict[str, Any]]:
    summaries = []
    families = sorted({point["family"] for point in points if point["family"] != "baseline"})
    for family in families:
        circuits = []
        for circuit_id, archetype, slug in CIRCUITS:
            selected = [
                point
                for point in points
                if point["family"] == family and point["circuit_id"] == circuit_id
            ]
            low = [point for point in selected if point["level"] == "low"]
            high = [point for point in selected if point["level"] == "high"]
            if not low or not high:
                continue
            low_metrics = median_metrics(low)
            high_metrics = median_metrics(high)
            circuits.append(
                {
                    "circuit_id": circuit_id,
                    "circuit_archetype": archetype,
                    "circuit_slug": slug,
                    "low": low_metrics,
                    "high": high_metrics,
                    "high_minus_low": {
                        key: high_metrics[key] - low_metrics[key] for key in low_metrics
                    },
                }
            )
        time_effects = [abs(item["high_minus_low"]["total_time_ms"]) for item in circuits]
        if not circuits:
            continue
        signs = {
            (effect > 0) - (effect < 0)
            for effect in (item["high_minus_low"]["total_time_ms"] for item in circuits)
            if effect != 0
        }
        summaries.append(
            {
                "family": family,
                "assessment": {
                    "active_above_5ms": bool(time_effects) and max(time_effects) > 5.0,
                    "maximum_absolute_time_effect_ms": max(time_effects, default=0.0),
                    "circuit_dependent_direction": len(signs) > 1,
                    "universally_faster_level": (
                        "high"
                        if all(
                            item["high_minus_low"]["total_time_ms"] < -5.0
                            for item in circuits
                        )
                        else "low"
                        if all(
                            item["high_minus_low"]["total_time_ms"] > 5.0
                            for item in circuits
                        )
                        else None
                    ),
                },
                "circuits": circuits,
            }
        )
    return summaries


def summarize_setup_interaction(points: list[dict[str, Any]]) -> dict[str, Any]:
    circuits = []
    for circuit_id, archetype, slug in CIRCUITS:
        selected = [
            point
            for point in points
            if point["family"] == "setup.interaction" and point["circuit_id"] == circuit_id
        ]
        levels = []
        for level in sorted({point["level"] for point in selected}):
            metrics = median_metrics([point for point in selected if point["level"] == level])
            levels.append({"level": level, **metrics})
        fastest = min(levels, key=lambda item: (item["total_time_ms"], item["level"]))
        circuits.append(
            {
                "circuit_id": circuit_id,
                "circuit_archetype": archetype,
                "circuit_slug": slug,
                "fastest_level": fastest["level"],
                "levels": levels,
            }
        )
    optima = {item["fastest_level"] for item in circuits}
    return {
        "distinct_optimum_count": len(optima),
        "circuit_specific_optima": len(optima) > 1,
        "circuits": circuits,
    }


def campaign_verdicts(
    pair_summaries: list[dict[str, Any]], setup_interaction: dict[str, Any]
) -> list[dict[str, str]]:
    by_family = {item["family"]: item for item in pair_summaries}
    physical = [
        item for item in pair_summaries if item["family"].startswith("physical.")
    ]
    development = [
        item for item in pair_summaries if item["family"].startswith("development.")
    ]
    setup = [by_family["setup.downforce_slider"], by_family["setup.gear_ratio_slider"]]
    physical_active = all(item["assessment"]["active_above_5ms"] for item in physical)
    setup_universal = all(
        item["assessment"]["universally_faster_level"] is not None for item in setup
    )
    chassis_dominant = (
        by_family["development.chassis_points"]["assessment"]["universally_faster_level"]
        == "high"
        and all(
            item["assessment"]["universally_faster_level"] == "low"
            for item in development
            if item["family"] != "development.chassis_points"
        )
    )
    return [
        {
            "capability": "physical_parameter_activation",
            "verdict": "PASS" if physical_active else "REFINE",
            "reason": (
                "Every screened V3 physical control changes at least one representative circuit by more than 5 ms."
                if physical_active
                else "At least one screened V3 physical control remains inactive in the reviewed range."
            ),
        },
        {
            "capability": "circuit_dependent_setup",
            "verdict": (
                "STRUCTURAL_CHANGE_REQUIRED"
                if setup_universal and not setup_interaction["circuit_specific_optima"]
                else "REFINE"
            ),
            "reason": (
                "The reviewed downforce and gearing directions are universal and the coarse interaction optimum is identical on every circuit."
                if setup_universal and not setup_interaction["circuit_specific_optima"]
                else "The screen found circuit-dependent setup behavior that requires a denser review."
            ),
        },
        {
            "capability": "development_specialization",
            "verdict": "STRUCTURAL_CHANGE_REQUIRED" if chassis_dominant else "REFINE",
            "reason": (
                "At constant budget, moving points toward chassis is faster on every reviewed circuit while moving them toward any other axis is slower."
                if chassis_dominant
                else "No single development axis universally dominates the reviewed fixed-budget simplex."
            ),
        },
        {
            "capability": "production_or_opponent_policy_change",
            "verdict": "REFINE",
            "reason": "This local screen is diagnostic evidence only; held-out circuits, active eras and Databricks replay remain required.",
        },
    ]


def build_report(runner: pathlib.Path, jobs: int) -> dict[str, Any]:
    if not runner.is_file():
        raise ScreenError(f"missing V3 probe runner: {runner}")
    if jobs < 1 or jobs > MAX_JOBS:
        raise ValueError(f"jobs must be between 1 and {MAX_JOBS}")
    scenario_bytes = BASE_SCENARIO.read_bytes()
    profile_bytes = BASE_PROFILE.read_bytes()
    plan = build_plan(json.loads(scenario_bytes), json.loads(profile_bytes))
    with concurrent.futures.ThreadPoolExecutor(max_workers=jobs) as executor:
        points = list(executor.map(lambda point: execute_point(runner, point), plan))
    points.sort(key=lambda point: (point["family"], point["level"], point["circuit_id"], point["seed"]))
    model_identities = {json.dumps(point["model"], sort_keys=True) for point in points}
    if len(model_identities) != 1:
        raise ScreenError("screen mixed multiple model identities")
    pair_summaries = summarize_pairs(points)
    setup_interaction = summarize_setup_interaction(points)
    return {
        "schema_version": SCHEMA_VERSION,
        "campaign": {
            "purpose": "bounded local activation screen before governed Databricks replay",
            "model": json.loads(next(iter(model_identities))),
            "runner": {
                "path": str(runner.relative_to(ROOT)),
                "digest": sha256(runner.read_bytes()),
            },
            "base_scenario_digest": sha256(scenario_bytes),
            "base_profile_digest": sha256(profile_bytes),
            "seeds": list(SEEDS),
            "circuits": [
                {"id": circuit_id, "archetype": archetype, "slug": slug}
                for circuit_id, archetype, slug in CIRCUITS
            ],
            "execution_count": len(points),
        },
        "verdicts": campaign_verdicts(pair_summaries, setup_interaction),
        "pair_summaries": pair_summaries,
        "setup_interaction": setup_interaction,
        "points": points,
    }


def write_or_check(report: dict[str, Any], output: pathlib.Path, check: bool) -> None:
    report_bytes = canonical_pretty(report)
    digest_bytes = (sha256(report_bytes) + "\n").encode()
    digest_path = output.with_suffix(".sha256")
    if check:
        if output.read_bytes() != report_bytes or digest_path.read_bytes() != digest_bytes:
            raise ScreenError("stored V3 local-screen artifacts do not match the replay")
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
    parser.add_argument("--output", type=pathlib.Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--check", action="store_true")
    arguments = parser.parse_args()
    try:
        report = build_report(arguments.runner.resolve(), arguments.jobs)
        write_or_check(report, arguments.output.resolve(), arguments.check)
    except (OSError, ValueError, ScreenError) as error:
        print(f"error: {error}", file=__import__("sys").stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
