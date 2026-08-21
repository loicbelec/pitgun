#!/usr/bin/env python3
"""Run the adaptive local engine-thermal screen for Racing Model V3."""

from __future__ import annotations

import argparse
import concurrent.futures
import copy
import hashlib
import json
import math
import pathlib
import statistics
import subprocess
import tempfile
from typing import Any, Iterable


ROOT = pathlib.Path(__file__).resolve().parents[2]
BASE_SCENARIO = (
    ROOT / "apps" / "pitgun-cli" / "scenarios" / "racing-batch-v1" / "balanced.json"
)
BASE_PROFILE = (
    ROOT
    / "experiments"
    / "racing_v3_decision_surface"
    / "profile-v8.engine-thermal-resolution.json"
)
DEFAULT_OUTPUT = pathlib.Path(__file__).parent / "results" / "local-thermal-screen-v1.json"
SCHEMA_VERSION = "pitgun.racing-v3-thermal-local-screen/v1"
PROBE_SCHEMA_VERSION = "pitgun.racing-v3-decision-surface-probe/v1"
EXPECTED_MODEL_VERSION = "0.10.0"
MAX_JOBS = 16
MAX_ADAPTIVE_SAMPLES = 256
MAX_REFINEMENT_SAMPLES = 128

# Calibration and held-out groups are kept explicit so the later Databricks
# campaign cannot accidentally optimize and validate on the same circuits.
CIRCUITS = (
    ("it-1922", "monza", "power", "calibration"),
    ("mc-1929", "monaco", "high-downforce", "calibration"),
    ("jp-1962", "suzuka", "mixed", "calibration"),
    ("be-1925", "spa", "high-speed-elevation", "held-out"),
    ("sg-2008", "singapore", "street-high-downforce", "held-out"),
    ("mx-1962", "mexico", "altitude", "held-out"),
)

# Eras 2, 3 and 4 deliberately share the authored classic V8 1970 vehicle but
# remain separate progression contexts. All enabled physical vehicles appear.
VEHICLE_ANCHORS = (
    ("era1-classic60", 1, "classic_v8_1960"),
    ("era2-classic70", 2, "classic_v8_1970"),
    ("era3-classic70", 3, "classic_v8_1970"),
    ("era4-classic70", 4, "classic_v8_1970"),
    ("era5-v6t", 5, "modern_v6t"),
    ("era5-hybrid", 5, "f1_2026"),
)

WORKLOADS = (("short", 3), ("long", 18))
COOLING_LEVELS = (0, 10, 20)
ADAPTIVE_SEEDS = (42, 99)

# Ranges are intentionally broad enough to expose shape and interactions, yet
# remain inside the governed Rust validation boundary introduced by V8.
NUMERIC_AXES: tuple[tuple[str, float, float], ...] = (
    ("thermal_capacity_multiplier", 0.50, 1.75),
    ("heat_generation_multiplier", 0.50, 1.75),
    ("static_cooling_multiplier", 0.25, 2.00),
    ("speed_cooling_multiplier", 0.25, 2.00),
    ("soft_limit_offset_c", -15.0, 15.0),
    ("derate_slope_multiplier", 0.50, 2.00),
    ("minimum_power_fraction", 0.10, 0.50),
    ("cooling_drag_area_m2_at_cap", 0.00, 0.15),
)

HALTON_PRIMES = (2, 3, 5, 7, 11, 13, 17, 19, 23, 29)


class ThermalScreenError(RuntimeError):
    """Raised when the campaign cannot produce governed evidence."""


def canonical_pretty(value: object) -> bytes:
    return (json.dumps(value, indent=2, ensure_ascii=False) + "\n").encode()


def sha256(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


def halton(index: int, base: int) -> float:
    """Return a deterministic radical-inverse sample in [0, 1)."""
    if index < 1 or base < 2:
        raise ValueError("Halton index must be positive and base at least two")
    value = 0.0
    fraction = 1.0 / base
    current = index
    while current:
        value += fraction * (current % base)
        current //= base
        fraction /= base
    return value


def thermal_parameters(profile: dict[str, Any]) -> dict[str, Any]:
    return profile["engine_thermal_resolution"]


def activation_profiles(base: dict[str, Any]) -> list[dict[str, Any]]:
    profiles = [
        {
            "parameter_set_id": "activation-baseline",
            "axis": "baseline",
            "level": "baseline",
            "profile": copy.deepcopy(base),
        }
    ]
    for axis, low, high in NUMERIC_AXES:
        for level, value in (("low", low), ("high", high)):
            profile = copy.deepcopy(base)
            thermal_parameters(profile)[axis] = value
            profiles.append(
                {
                    "parameter_set_id": f"activation-{axis}-{level}",
                    "axis": axis,
                    "level": level,
                    "profile": profile,
                }
            )
    for width in (5.0, 15.0):
        profile = copy.deepcopy(base)
        thermal = thermal_parameters(profile)
        thermal["derating_shape"] = "smooth-knee"
        thermal["smooth_knee_width_c"] = width
        profiles.append(
            {
                "parameter_set_id": f"activation-smooth-knee-{int(width)}c",
                "axis": "derating_shape",
                "level": f"smooth-{int(width)}c",
                "profile": profile,
            }
        )
    return profiles


def adaptive_profiles(
    base: dict[str, Any], active_axes: Iterable[str], sample_count: int
) -> list[dict[str, Any]]:
    active = set(active_axes)
    profiles = []
    for sample_index in range(1, sample_count + 1):
        profile = copy.deepcopy(base)
        thermal = thermal_parameters(profile)
        sampled: dict[str, Any] = {}
        dimension = 0
        for axis, low, high in NUMERIC_AXES:
            if axis not in active:
                continue
            value = low + (high - low) * halton(sample_index, HALTON_PRIMES[dimension])
            thermal[axis] = value
            sampled[axis] = value
            dimension += 1
        if "derating_shape" in active:
            shape_sample = halton(sample_index, HALTON_PRIMES[dimension])
            dimension += 1
            if shape_sample >= 0.5:
                width = 2.0 + 18.0 * halton(sample_index, HALTON_PRIMES[dimension])
                thermal["derating_shape"] = "smooth-knee"
                thermal["smooth_knee_width_c"] = width
                sampled["derating_shape"] = "smooth-knee"
                sampled["smooth_knee_width_c"] = width
            else:
                thermal["derating_shape"] = "linear-threshold"
                thermal["smooth_knee_width_c"] = 0.0
                sampled["derating_shape"] = "linear-threshold"
                sampled["smooth_knee_width_c"] = 0.0
        profiles.append(
            {
                "parameter_set_id": f"adaptive-{sample_index:03d}",
                "parameters": sampled,
                "profile": profile,
            }
        )
    return profiles


def refinement_profiles(
    base: dict[str, Any],
    active_axes: Iterable[str],
    broad_profiles: list[dict[str, Any]],
    broad_aggregates: list[dict[str, Any]],
    sample_count: int,
) -> tuple[list[dict[str, Any]], list[str]]:
    """Concentrate deterministic points around the healthy/hot transition."""
    active = set(active_axes)
    profiles_by_id = {
        item["parameter_set_id"]: item for item in broad_profiles
    }
    healthy = sorted(
        (
            item
            for item in broad_aggregates
            if item["pathological_execution_count"] == 0
        ),
        key=lambda item: (
            abs(item["maximum_engine_temperature_c"] - 120.0),
            item["parameter_set_id"],
        ),
    )[:4]
    hot = sorted(
        (
            item
            for item in broad_aggregates
            if item["pathological_execution_count"] > 0
        ),
        key=lambda item: (
            item["pathological_execution_count"],
            abs(item["maximum_engine_temperature_c"] - 150.0),
            item["parameter_set_id"],
        ),
    )[:4]
    anchor_ids = [item["parameter_set_id"] for item in healthy + hot]
    if not anchor_ids:
        raise ThermalScreenError("adaptive stage produced no refinement anchors")

    refined = []
    for sample_index in range(1, sample_count + 1):
        anchor_id = anchor_ids[(sample_index - 1) % len(anchor_ids)]
        anchor = profiles_by_id[anchor_id]
        profile = copy.deepcopy(anchor["profile"])
        thermal = thermal_parameters(profile)
        sampled: dict[str, Any] = {}
        dimension = 0
        for axis, low, high in NUMERIC_AXES:
            if axis not in active:
                continue
            center = thermal[axis]
            local_half_width = (high - low) * 0.12
            offset = 2.0 * halton(sample_index, HALTON_PRIMES[dimension]) - 1.0
            value = min(high, max(low, center + offset * local_half_width))
            thermal[axis] = value
            sampled[axis] = value
            dimension += 1
        if "derating_shape" in active:
            current_shape = thermal["derating_shape"]
            toggle = halton(sample_index, HALTON_PRIMES[dimension]) >= 0.75
            dimension += 1
            shape = (
                "smooth-knee"
                if current_shape == "linear-threshold" and toggle
                else "linear-threshold"
                if current_shape == "smooth-knee" and toggle
                else current_shape
            )
            if shape == "smooth-knee":
                current_width = (
                    thermal["smooth_knee_width_c"]
                    if current_shape == "smooth-knee"
                    else 10.0
                )
                width_offset = (
                    2.0 * halton(sample_index, HALTON_PRIMES[dimension]) - 1.0
                ) * 3.0
                width = min(20.0, max(2.0, current_width + width_offset))
                thermal["derating_shape"] = shape
                thermal["smooth_knee_width_c"] = width
                sampled["derating_shape"] = shape
                sampled["smooth_knee_width_c"] = width
            else:
                thermal["derating_shape"] = shape
                thermal["smooth_knee_width_c"] = 0.0
                sampled["derating_shape"] = shape
                sampled["smooth_knee_width_c"] = 0.0
        refined.append(
            {
                "parameter_set_id": f"refinement-{sample_index:03d}",
                "anchor_parameter_set_id": anchor_id,
                "parameters": sampled,
                "profile": profile,
            }
        )
    return refined, anchor_ids


def configure_scenario(
    base: dict[str, Any],
    *,
    circuit_id: str,
    vehicle_id: str,
    era: int,
    laps: int,
    cooling_points: int,
) -> dict[str, Any]:
    scenario = copy.deepcopy(base)
    request = scenario["request"]
    request.update(
        {
            "track_id": circuit_id,
            "vehicle_id": vehicle_id,
            "era": era,
            "laps": laps,
            "hz": 5.0,
            "initial_fuel_mass_kg": 80.0,
        }
    )
    competitor = request["competitors"][0]
    # The high cap isolates cooling as a physical input during this screen; the
    # later gameplay campaign will restore the fixed development budget.
    competitor["budget_cap"] = 100.0
    competitor["tuning"] = {
        "engine_points": 10.0,
        "cooling_points": float(cooling_points),
        "aero_points": 10.0,
        "chassis_points": 10.0,
        "downforce_slider": 0.5,
        "gear_ratio_slider": 0.5,
    }
    competitor.pop("stint_strategy", None)
    request.pop("pit_strategy", None)
    return scenario


def context_metadata() -> list[dict[str, Any]]:
    contexts = []
    for anchor_id, era, vehicle_id in VEHICLE_ANCHORS:
        for circuit_id, slug, archetype, partition in CIRCUITS:
            contexts.append(
                {
                    "context_id": f"{anchor_id}-{slug}",
                    "anchor_id": anchor_id,
                    "era": era,
                    "vehicle_id": vehicle_id,
                    "circuit_id": circuit_id,
                    "circuit_slug": slug,
                    "circuit_archetype": archetype,
                    "partition": partition,
                }
            )
    return contexts


def activation_plan(
    base_scenario: dict[str, Any], profiles: list[dict[str, Any]]
) -> list[dict[str, Any]]:
    plan = []
    for context in context_metadata():
        for workload, laps in WORKLOADS:
            for parameter_set in profiles:
                plan.append(
                    {
                        **context,
                        "stage": "activation",
                        "workload": workload,
                        "laps": laps,
                        "cooling_points": 10,
                        "seed": 42,
                        "parameter_set_id": parameter_set["parameter_set_id"],
                        "axis": parameter_set["axis"],
                        "level": parameter_set["level"],
                        "scenario": configure_scenario(
                            base_scenario,
                            circuit_id=context["circuit_id"],
                            vehicle_id=context["vehicle_id"],
                            era=context["era"],
                            laps=laps,
                            cooling_points=10,
                        ),
                        "profile": parameter_set["profile"],
                    }
                )
    return plan


def joint_plan(
    base_scenario: dict[str, Any],
    profiles: list[dict[str, Any]],
    *,
    stage: str,
) -> list[dict[str, Any]]:
    plan = []
    for context in context_metadata():
        for parameter_set in profiles:
            # Cooling extremes use seed 42; the neutral point is replayed with
            # a second seed to expose seed-sensitive rankings.
            for cooling_points, seed in (
                (0, 42),
                (10, 42),
                (10, 99),
                (20, 42),
            ):
                plan.append(
                    {
                        **context,
                        "stage": stage,
                        "workload": "long",
                        "laps": 18,
                        "cooling_points": cooling_points,
                        "seed": seed,
                        "parameter_set_id": parameter_set["parameter_set_id"],
                        **(
                            {
                                "anchor_parameter_set_id": parameter_set[
                                    "anchor_parameter_set_id"
                                ]
                            }
                            if "anchor_parameter_set_id" in parameter_set
                            else {}
                        ),
                        "scenario": configure_scenario(
                            base_scenario,
                            circuit_id=context["circuit_id"],
                            vehicle_id=context["vehicle_id"],
                            era=context["era"],
                            laps=18,
                            cooling_points=cooling_points,
                        ),
                        "profile": parameter_set["profile"],
                    }
                )
    return plan


def execute_point(runner: pathlib.Path, point: dict[str, Any]) -> dict[str, Any]:
    with tempfile.TemporaryDirectory(prefix="pitgun-v3-thermal-") as directory:
        temporary = pathlib.Path(directory)
        scenario_path = temporary / "scenario.json"
        profile_path = temporary / "profile.json"
        scenario_path.write_bytes(canonical_pretty(point["scenario"]))
        profile_path.write_bytes(canonical_pretty(point["profile"]))
        completed = subprocess.run(
            [str(runner), str(scenario_path), str(profile_path), str(point["seed"])],
            cwd=ROOT,
            check=False,
            capture_output=True,
            timeout=180,
        )
    if completed.returncode != 0:
        raise ThermalScreenError(
            f"probe failed for {point['stage']} {point['parameter_set_id']} "
            f"{point['context_id']}: "
            + completed.stderr.decode(errors="replace").strip()
        )
    if completed.stderr:
        raise ThermalScreenError(
            f"successful probe wrote to stderr for {point['parameter_set_id']}"
        )
    try:
        result = json.loads(completed.stdout)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ThermalScreenError("probe returned invalid JSON") from error
    if result.get("schema_version") != PROBE_SCHEMA_VERSION:
        raise ThermalScreenError("probe returned an unsupported result")
    metadata = {
        key: value
        for key, value in point.items()
        if key not in {"scenario", "profile"}
    }
    diagnostics = result["mechanical_diagnostics"]
    return metadata | {
        "schema_version": result["schema_version"],
        "experimental_execution_id": result["experimental_execution_id"],
        "model": result["model"],
        "scenario_digest": result["scenario_digest"],
        "profile_digest": result["profile_digest"],
        "total_time_ms": result["total_time_ms"],
        "observed_maximum_speed_kph": result["observed_maximum_speed_kph"],
        "mechanical_diagnostics": {
            "maximum_engine_temperature_c": diagnostics[
                "maximum_engine_temperature_c"
            ],
            "engine_derated_time_s": diagnostics["engine_derated_time_s"],
            "generated_engine_heat_kj": diagnostics["generated_engine_heat_kj"],
            "removed_engine_heat_kj": diagnostics["removed_engine_heat_kj"],
            "fixed_drag_area_m2": diagnostics["fixed_drag_area_m2"],
        },
    }


def execute_plan(
    runner: pathlib.Path, plan: list[dict[str, Any]], jobs: int
) -> list[dict[str, Any]]:
    with concurrent.futures.ThreadPoolExecutor(max_workers=jobs) as executor:
        points = list(executor.map(lambda point: execute_point(runner, point), plan))
    points.sort(
        key=lambda point: (
            point["stage"],
            point["parameter_set_id"],
            point["context_id"],
            point["workload"],
            point["cooling_points"],
            point["seed"],
        )
    )
    return points


def thermal_metrics(point: dict[str, Any]) -> dict[str, float]:
    diagnostics = point["mechanical_diagnostics"]
    total_time_s = point["total_time_ms"] / 1000.0
    return {
        "total_time_ms": float(point["total_time_ms"]),
        "maximum_engine_temperature_c": diagnostics["maximum_engine_temperature_c"],
        "engine_derated_time_s": diagnostics["engine_derated_time_s"],
        "engine_derated_fraction": (
            diagnostics["engine_derated_time_s"] / total_time_s
            if total_time_s > 0.0
            else 0.0
        ),
        "generated_engine_heat_kj": diagnostics["generated_engine_heat_kj"],
        "removed_engine_heat_kj": diagnostics["removed_engine_heat_kj"],
        "fixed_drag_area_m2": diagnostics["fixed_drag_area_m2"],
    }


def activation_summary(points: list[dict[str, Any]]) -> dict[str, Any]:
    baseline = {
        (point["context_id"], point["workload"]): point
        for point in points
        if point["axis"] == "baseline"
    }
    axes = []
    for axis in [item[0] for item in NUMERIC_AXES] + ["derating_shape"]:
        selected = [point for point in points if point["axis"] == axis]
        effects = []
        for point in selected:
            reference = baseline[(point["context_id"], point["workload"])]
            current = thermal_metrics(point)
            base = thermal_metrics(reference)
            effects.append(
                {
                    "context_id": point["context_id"],
                    "partition": point["partition"],
                    "workload": point["workload"],
                    "level": point["level"],
                    "relative_time_effect_ppm": (
                        (current["total_time_ms"] - base["total_time_ms"])
                        / base["total_time_ms"]
                        * 1_000_000.0
                    ),
                    "temperature_effect_c": (
                        current["maximum_engine_temperature_c"]
                        - base["maximum_engine_temperature_c"]
                    ),
                    "derated_time_effect_s": (
                        current["engine_derated_time_s"]
                        - base["engine_derated_time_s"]
                    ),
                    "generated_heat_effect_kj": (
                        current["generated_engine_heat_kj"]
                        - base["generated_engine_heat_kj"]
                    ),
                    "removed_heat_effect_kj": (
                        current["removed_engine_heat_kj"]
                        - base["removed_engine_heat_kj"]
                    ),
                    "drag_effect_m2": (
                        current["fixed_drag_area_m2"] - base["fixed_drag_area_m2"]
                    ),
                }
            )
        maxima = {
            "absolute_relative_time_effect_ppm": max(
                (abs(effect["relative_time_effect_ppm"]) for effect in effects),
                default=0.0,
            ),
            "absolute_temperature_effect_c": max(
                (abs(effect["temperature_effect_c"]) for effect in effects),
                default=0.0,
            ),
            "absolute_derated_time_effect_s": max(
                (abs(effect["derated_time_effect_s"]) for effect in effects),
                default=0.0,
            ),
            "absolute_heat_effect_kj": max(
                (
                    max(
                        abs(effect["generated_heat_effect_kj"]),
                        abs(effect["removed_heat_effect_kj"]),
                    )
                    for effect in effects
                ),
                default=0.0,
            ),
            "absolute_drag_effect_m2": max(
                (abs(effect["drag_effect_m2"]) for effect in effects),
                default=0.0,
            ),
        }
        active = (
            maxima["absolute_relative_time_effect_ppm"] >= 10.0
            or maxima["absolute_temperature_effect_c"] >= 0.05
            or maxima["absolute_derated_time_effect_s"] >= 0.01
            or maxima["absolute_heat_effect_kj"] >= 1.0
            or maxima["absolute_drag_effect_m2"] >= 0.0001
        )
        axes.append(
            {
                "axis": axis,
                "active": active,
                "maxima": maxima,
                "effects": effects,
            }
        )
    measured_active_axes = [item["axis"] for item in axes if item["active"]]
    conditional_axes = []
    if {
        "heat_generation_multiplier",
        "thermal_capacity_multiplier",
        "soft_limit_offset_c",
    } & set(measured_active_axes):
        # These controls only become observable after an upstream combination
        # crosses the thermal limit. Keeping them is the adaptive dependency
        # closure, not an assertion that their isolated effect was measurable.
        conditional_axes = [
            axis
            for axis in (
                "derate_slope_multiplier",
                "minimum_power_fraction",
                "derating_shape",
            )
            if axis not in measured_active_axes
        ]
    selected_axes = measured_active_axes + conditional_axes
    return {
        "axis_count": len(axes),
        "active_axis_count": sum(item["active"] for item in axes),
        "measured_active_axes": measured_active_axes,
        "conditionally_selected_axes": conditional_axes,
        "selected_axis_count": len(selected_axes),
        "selected_axes": selected_axes,
        "structurally_inactive_axes": [
            item["axis"]
            for item in axes
            if not item["active"] and item["axis"] not in conditional_axes
        ],
        "axes": axes,
    }


def aggregate_adaptive(
    points: list[dict[str, Any]], profiles: list[dict[str, Any]]
) -> list[dict[str, Any]]:
    parameters = {
        item["parameter_set_id"]: item["parameters"] for item in profiles
    }
    aggregates = []
    for parameter_set_id in sorted(parameters):
        selected = [
            point for point in points if point["parameter_set_id"] == parameter_set_id
        ]
        neutral = [point for point in selected if point["cooling_points"] == 10]
        low = {
            (point["context_id"], point["seed"]): point
            for point in selected
            if point["cooling_points"] == 0
        }
        high = {
            (point["context_id"], point["seed"]): point
            for point in selected
            if point["cooling_points"] == 20
        }
        cooling_pairs = []
        for key in sorted(set(low) & set(high)):
            low_metrics = thermal_metrics(low[key])
            high_metrics = thermal_metrics(high[key])
            cooling_pairs.append(
                {
                    "time_effect_ms": (
                        high_metrics["total_time_ms"] - low_metrics["total_time_ms"]
                    ),
                    "temperature_effect_c": (
                        high_metrics["maximum_engine_temperature_c"]
                        - low_metrics["maximum_engine_temperature_c"]
                    ),
                    "derated_time_effect_s": (
                        high_metrics["engine_derated_time_s"]
                        - low_metrics["engine_derated_time_s"]
                    ),
                    "drag_effect_m2": (
                        high_metrics["fixed_drag_area_m2"]
                        - low_metrics["fixed_drag_area_m2"]
                    ),
                }
            )
        metrics = [thermal_metrics(point) for point in neutral]
        pathological = [
            point
            for point in neutral
            if thermal_metrics(point)["maximum_engine_temperature_c"] > 180.0
            or thermal_metrics(point)["engine_derated_fraction"] > 0.50
            or not all(math.isfinite(value) for value in thermal_metrics(point).values())
        ]
        aggregates.append(
            {
                "parameter_set_id": parameter_set_id,
                "parameters": parameters[parameter_set_id],
                "execution_count": len(selected),
                "median_total_time_ms": statistics.median(
                    metric["total_time_ms"] for metric in metrics
                ),
                "median_maximum_engine_temperature_c": statistics.median(
                    metric["maximum_engine_temperature_c"] for metric in metrics
                ),
                "maximum_engine_temperature_c": max(
                    metric["maximum_engine_temperature_c"] for metric in metrics
                ),
                "median_engine_derated_fraction": statistics.median(
                    metric["engine_derated_fraction"] for metric in metrics
                ),
                "maximum_engine_derated_fraction": max(
                    metric["engine_derated_fraction"] for metric in metrics
                ),
                "median_cooling_time_effect_ms": statistics.median(
                    pair["time_effect_ms"] for pair in cooling_pairs
                ),
                "median_cooling_temperature_effect_c": statistics.median(
                    pair["temperature_effect_c"] for pair in cooling_pairs
                ),
                "median_cooling_derated_time_effect_s": statistics.median(
                    pair["derated_time_effect_s"] for pair in cooling_pairs
                ),
                "median_cooling_drag_effect_m2": statistics.median(
                    pair["drag_effect_m2"] for pair in cooling_pairs
                ),
                "pathological_execution_count": len(pathological),
            }
        )
    return aggregates


def dominates(left: dict[str, Any], right: dict[str, Any]) -> bool:
    keys = (
        "median_total_time_ms",
        "maximum_engine_derated_fraction",
        "median_cooling_time_effect_ms",
    )
    return all(left[key] <= right[key] for key in keys) and any(
        left[key] < right[key] for key in keys
    )


def pareto_summary(aggregates: list[dict[str, Any]]) -> dict[str, Any]:
    pathological = [
        item for item in aggregates if item["pathological_execution_count"] > 0
    ]
    disengaged = [
        item
        for item in aggregates
        if item["pathological_execution_count"] == 0
        and item["maximum_engine_temperature_c"] < 100.0
    ]
    eligible = [
        item
        for item in aggregates
        if item["pathological_execution_count"] == 0
        and item["maximum_engine_temperature_c"] >= 100.0
    ]
    frontier = [
        candidate
        for candidate in eligible
        if not any(
            dominates(other, candidate)
            for other in eligible
            if other["parameter_set_id"] != candidate["parameter_set_id"]
        )
    ]
    frontier.sort(key=lambda item: item["parameter_set_id"])
    return {
        "objective_direction": {
            "median_total_time_ms": "minimize",
            "maximum_engine_derated_fraction": "minimize",
            "median_cooling_time_effect_ms": "minimize",
        },
        "eligibility": {
            "maximum_engine_temperature_c": "[100, 180] degC",
            "maximum_engine_derated_fraction": "<= 0.50",
            "meaning": (
                "The engine must exhibit a measurable thermal excursion without "
                "entering the pathological guard region."
            ),
        },
        "eligible_parameter_set_count": len(eligible),
        "thermally_disengaged_parameter_set_count": len(disengaged),
        "pathological_parameter_set_count": len(pathological),
        "frontier_parameter_set_count": len(frontier),
        "frontier_parameter_set_ids": [
            item["parameter_set_id"] for item in frontier
        ],
        "frontier": frontier,
        "interpretation": (
            "The frontier is diagnostic, not a production recommendation. "
            "Physical targets and held-out Databricks replay remain mandatory."
        ),
    }


def verify_model(points: list[dict[str, Any]]) -> dict[str, Any]:
    identities = {json.dumps(point["model"], sort_keys=True) for point in points}
    if len(identities) != 1:
        raise ThermalScreenError("campaign mixed multiple model identities")
    identity = json.loads(next(iter(identities)))
    if identity.get("version") != EXPECTED_MODEL_VERSION:
        raise ThermalScreenError(
            f"campaign selected model {identity.get('version')}, expected {EXPECTED_MODEL_VERSION}"
        )
    return identity


def build_report(
    runner: pathlib.Path,
    jobs: int,
    adaptive_sample_count: int,
    refinement_sample_count: int,
    profile_path: pathlib.Path = BASE_PROFILE,
) -> dict[str, Any]:
    if not runner.is_file():
        raise ThermalScreenError(f"missing V3 probe runner: {runner}")
    if jobs < 1 or jobs > MAX_JOBS:
        raise ValueError(f"jobs must be between 1 and {MAX_JOBS}")
    if adaptive_sample_count < 1 or adaptive_sample_count > MAX_ADAPTIVE_SAMPLES:
        raise ValueError(
            f"adaptive samples must be between 1 and {MAX_ADAPTIVE_SAMPLES}"
        )
    if refinement_sample_count < 1 or refinement_sample_count > MAX_REFINEMENT_SAMPLES:
        raise ValueError(
            f"refinement samples must be between 1 and {MAX_REFINEMENT_SAMPLES}"
        )

    scenario_bytes = BASE_SCENARIO.read_bytes()
    profile_bytes = profile_path.read_bytes()
    base_scenario = json.loads(scenario_bytes)
    base_profile = json.loads(profile_bytes)

    activation_parameter_sets = activation_profiles(base_profile)
    first_plan = activation_plan(base_scenario, activation_parameter_sets)
    first_points = execute_plan(runner, first_plan, jobs)
    activation = activation_summary(first_points)

    adaptive_parameter_sets = adaptive_profiles(
        base_profile, activation["selected_axes"], adaptive_sample_count
    )
    second_plan = joint_plan(
        base_scenario, adaptive_parameter_sets, stage="adaptive"
    )
    second_points = execute_plan(runner, second_plan, jobs)
    broad_aggregates = aggregate_adaptive(second_points, adaptive_parameter_sets)
    refined_parameter_sets, refinement_anchor_ids = refinement_profiles(
        base_profile,
        activation["selected_axes"],
        adaptive_parameter_sets,
        broad_aggregates,
        refinement_sample_count,
    )
    third_plan = joint_plan(
        base_scenario, refined_parameter_sets, stage="refinement"
    )
    third_points = execute_plan(runner, third_plan, jobs)
    all_points = first_points + second_points + third_points
    model = verify_model(all_points)
    refined_aggregates = aggregate_adaptive(third_points, refined_parameter_sets)
    combined_aggregates = broad_aggregates + refined_aggregates

    return {
        "schema_version": SCHEMA_VERSION,
        "campaign": {
            "purpose": "adaptive local thermal-surface screen before governed Databricks replay",
            "model": model,
            "runner": {
                "path": str(runner.relative_to(ROOT)),
                "digest": sha256(runner.read_bytes()),
            },
            "base_scenario_path": str(BASE_SCENARIO.relative_to(ROOT)),
            "base_scenario_digest": sha256(scenario_bytes),
            "base_profile_path": str(profile_path.relative_to(ROOT)),
            "base_profile_digest": sha256(profile_bytes),
            "circuits": [
                {
                    "id": identifier,
                    "slug": slug,
                    "archetype": archetype,
                    "partition": partition,
                }
                for identifier, slug, archetype, partition in CIRCUITS
            ],
            "vehicle_anchors": [
                {"id": identifier, "era": era, "vehicle_id": vehicle_id}
                for identifier, era, vehicle_id in VEHICLE_ANCHORS
            ],
            "activation_execution_count": len(first_points),
            "adaptive_sample_count": adaptive_sample_count,
            "adaptive_execution_count": len(second_points),
            "refinement_sample_count": refinement_sample_count,
            "refinement_execution_count": len(third_points),
            "execution_count": len(all_points),
            "simulated_lap_count": sum(point["laps"] for point in all_points),
        },
        "governance": {
            "production_change_authorized": False,
            "game_change_authorized": False,
            "catalog_change_authorized": False,
            "databricks_handoff_authorized": True,
            "reason": (
                "This report selects observable regions for independent replay; "
                "it does not calibrate or publish thermal coefficients."
            ),
        },
        "activation_summary": activation,
        "adaptive_parameter_sets": [
            {
                "parameter_set_id": item["parameter_set_id"],
                "parameters": item["parameters"],
            }
            for item in adaptive_parameter_sets
        ],
        "adaptive_aggregates": broad_aggregates,
        "refinement": {
            "anchor_parameter_set_ids": refinement_anchor_ids,
            "parameter_sets": [
                {
                    "parameter_set_id": item["parameter_set_id"],
                    "anchor_parameter_set_id": item["anchor_parameter_set_id"],
                    "parameters": item["parameters"],
                }
                for item in refined_parameter_sets
            ],
            "aggregates": refined_aggregates,
        },
        "pareto_summary": pareto_summary(combined_aggregates),
        "points": [
            {
                key: value
                for key, value in point.items()
                if key not in {"schema_version", "model"}
            }
            for point in all_points
        ],
    }


def write_or_check(report: dict[str, Any], output: pathlib.Path, check: bool) -> None:
    report_bytes = canonical_pretty(report)
    digest_bytes = (sha256(report_bytes) + "\n").encode()
    digest_path = output.with_suffix(".sha256")
    if check:
        if output.read_bytes() != report_bytes or digest_path.read_bytes() != digest_bytes:
            raise ThermalScreenError("stored thermal artifacts do not match replay")
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
    parser.add_argument("--jobs", type=int, default=8)
    parser.add_argument("--adaptive-samples", type=int, default=64)
    parser.add_argument("--refinement-samples", type=int, default=32)
    parser.add_argument("--profile", type=pathlib.Path, default=BASE_PROFILE)
    parser.add_argument("--output", type=pathlib.Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--check", action="store_true")
    arguments = parser.parse_args()
    try:
        report = build_report(
            arguments.runner.resolve(),
            arguments.jobs,
            arguments.adaptive_samples,
            arguments.refinement_samples,
            arguments.profile.resolve(),
        )
        write_or_check(report, arguments.output.resolve(), arguments.check)
    except (OSError, ValueError, ThermalScreenError) as error:
        print(f"error: {error}", file=__import__("sys").stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
