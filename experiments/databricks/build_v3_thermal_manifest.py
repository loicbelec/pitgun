#!/usr/bin/env python3
"""Freeze the reviewed Model V3 thermal surface as a Databricks campaign."""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import pathlib
from typing import Any


ROOT = pathlib.Path(__file__).resolve().parents[2]
LOCAL_REPORT = (
    ROOT
    / "experiments"
    / "racing_v3_thermal"
    / "results"
    / "local-thermal-screen-v1.json"
)
BASE_SCENARIO = (
    ROOT / "apps" / "pitgun-cli" / "scenarios" / "racing-batch-v1" / "balanced.json"
)
BASE_PROFILE = (
    ROOT
    / "experiments"
    / "racing_v3_decision_surface"
    / "profile-v8.engine-thermal-resolution.json"
)
OUTPUT = (
    ROOT
    / "experiments"
    / "databricks"
    / "campaigns"
    / "racing-v3-thermal-adequacy-v1.json"
)
SCHEMA_VERSION = "pitgun.racing-v3-thermal-adequacy-campaign/v1"
CAMPAIGN_ID = "racing-v3-thermal-adequacy-2026-v1"

REPLAY_CIRCUITS = (
    ("it-1922", "monza", "power", "local_replay_calibration"),
    ("sg-2008", "singapore", "street-high-downforce", "local_replay_held_out"),
)
VALIDATION_CIRCUIT = (
    "es-1991",
    "barcelona",
    "mixed",
    "final_validation",
)
VEHICLE_ANCHORS = (
    ("era1-classic60", 1, "classic_v8_1960", "historical_v8"),
    ("era2-classic70", 2, "classic_v8_1970", "historical_v8"),
    ("era3-classic70", 3, "classic_v8_1970", "historical_v8"),
    ("era4-classic70", 4, "classic_v8_1970", "historical_v8"),
    ("era5-v6t", 5, "modern_v6t", "modern_v6t"),
    ("era5-hybrid", 5, "f1_2026", "f1_2026"),
)
COOLING_LEVELS = (0, 10, 20)
LOCAL_SEEDS = (42, 99)
VALIDATION_SEED = 20260821
THERMAL_PARAMETER_KEYS = (
    "thermal_capacity_multiplier",
    "heat_generation_multiplier",
    "static_cooling_multiplier",
    "speed_cooling_multiplier",
    "soft_limit_offset_c",
    "derate_slope_multiplier",
    "minimum_power_fraction",
    "derating_shape",
    "smooth_knee_width_c",
    "cooling_drag_area_m2_at_cap",
)


class ThermalManifestError(RuntimeError):
    """Raised when the governed source evidence cannot produce the plan."""


def canonical_pretty(value: object) -> bytes:
    return (json.dumps(value, indent=2, ensure_ascii=False) + "\n").encode()


def sha256(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


def load_checked_report(path: pathlib.Path) -> tuple[dict[str, Any], bytes]:
    payload = path.read_bytes()
    if path.with_suffix(".sha256").read_text().strip() != sha256(payload):
        raise ThermalManifestError("local thermal report does not match its checksum")
    report = json.loads(payload)
    if report.get("schema_version") != "pitgun.racing-v3-thermal-local-screen/v1":
        raise ThermalManifestError("local thermal report has an unsupported contract")
    return report, payload


def retained_parameter_sets(report: dict[str, Any]) -> list[dict[str, Any]]:
    aggregates = report["adaptive_aggregates"] + report["refinement"]["aggregates"]
    selected = [
        item
        for item in aggregates
        if item["pathological_execution_count"] == 0
        and item["maximum_engine_temperature_c"] >= 100.0
    ]
    selected.sort(key=lambda item: item["parameter_set_id"])
    if len(selected) != 15:
        raise ThermalManifestError("expected exactly 15 retained local parameter sets")
    return [
        {
            "parameter_set_id": item["parameter_set_id"],
            "origin": "retained_local_engaged_healthy",
            "parameters": item["parameters"],
        }
        for item in selected
    ]


def interpolate_parameters(
    healthy: dict[str, Any], hot: dict[str, Any], fraction: float
) -> dict[str, Any]:
    values: dict[str, Any] = {}
    for key in THERMAL_PARAMETER_KEYS:
        left = healthy.get(key, 1.0 if key == "static_cooling_multiplier" else None)
        right = hot.get(key, 1.0 if key == "static_cooling_multiplier" else None)
        if key == "derating_shape":
            values[key] = left if fraction < 0.5 else right
        elif key == "smooth_knee_width_c":
            values[key] = (
                (1.0 - fraction) * float(left) + fraction * float(right)
                if values["derating_shape"] == "smooth-knee"
                else 0.0
            )
        else:
            values[key] = (1.0 - fraction) * float(left) + fraction * float(right)
    return values


def transition_parameter_sets(report: dict[str, Any]) -> list[dict[str, Any]]:
    source = {
        item["parameter_set_id"]: item["parameters"]
        for item in report["adaptive_parameter_sets"]
    }
    anchors = report["refinement"]["anchor_parameter_set_ids"]
    if len(anchors) != 8:
        raise ThermalManifestError("expected four healthy and four hot anchors")
    parameter_sets = []
    for pair_index, (healthy_id, hot_id) in enumerate(zip(anchors[:4], anchors[4:]), 1):
        for step, fraction in enumerate((1.0 / 3.0, 2.0 / 3.0), 1):
            parameter_sets.append(
                {
                    "parameter_set_id": f"transition-{pair_index:02d}-{step}",
                    "origin": "deterministic_healthy_hot_interpolation",
                    "source_parameter_set_ids": [healthy_id, hot_id],
                    "interpolation_fraction_from_healthy": fraction,
                    "parameters": interpolate_parameters(
                        source[healthy_id], source[hot_id], fraction
                    ),
                }
            )
    return parameter_sets


def resolved_profile(
    base_profile: dict[str, Any], parameter_set: dict[str, Any]
) -> dict[str, Any]:
    profile = copy.deepcopy(base_profile)
    thermal = profile["engine_thermal_resolution"]
    thermal.update(parameter_set["parameters"])
    return profile


def configure_scenario(
    base: dict[str, Any], *, circuit_id: str, vehicle_id: str, era: int,
    laps: int, cooling_points: int
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


def local_point_index(report: dict[str, Any]) -> dict[tuple[Any, ...], dict[str, Any]]:
    return {
        (
            point["parameter_set_id"],
            point["anchor_id"],
            point["circuit_id"],
            point["cooling_points"],
            point["seed"],
            point["laps"],
        ): point
        for point in report["points"]
        if point["stage"] in {"adaptive", "refinement"}
    }


def expected_local_evidence(point: dict[str, Any]) -> dict[str, Any]:
    diagnostics = point["mechanical_diagnostics"]
    return {
        "experimental_execution_id": point["experimental_execution_id"],
        "scenario_digest": point["scenario_digest"],
        "profile_digest": point["profile_digest"],
        "metrics": {
            "total_time_ms": point["total_time_ms"],
            "maximum_speed_kph": point["observed_maximum_speed_kph"],
            "maximum_engine_temperature_c": diagnostics[
                "maximum_engine_temperature_c"
            ],
            "engine_derated_time_s": diagnostics["engine_derated_time_s"],
            "generated_engine_heat_kj": diagnostics["generated_engine_heat_kj"],
            "removed_engine_heat_kj": diagnostics["removed_engine_heat_kj"],
            "fixed_drag_area_m2": diagnostics["fixed_drag_area_m2"],
        },
    }


def build_manifest(
    report_path: pathlib.Path = LOCAL_REPORT,
    base_scenario_path: pathlib.Path = BASE_SCENARIO,
    base_profile_path: pathlib.Path = BASE_PROFILE,
) -> dict[str, Any]:
    report, report_bytes = load_checked_report(report_path)
    scenario_bytes = base_scenario_path.read_bytes()
    profile_bytes = base_profile_path.read_bytes()
    base_scenario = json.loads(scenario_bytes)
    base_profile = json.loads(profile_bytes)
    retained = retained_parameter_sets(report)
    transition = transition_parameter_sets(report)
    parameter_sets = retained + transition
    retained_ids = {item["parameter_set_id"] for item in retained}
    local_points = local_point_index(report)

    profiles: dict[str, dict[str, Any]] = {}
    profile_refs: dict[str, str] = {}
    for parameter_set in parameter_sets:
        profile = resolved_profile(base_profile, parameter_set)
        digest = sha256(canonical_pretty(profile))
        profiles[digest] = profile
        profile_refs[parameter_set["parameter_set_id"]] = digest

    scenarios: dict[str, dict[str, Any]] = {}
    configurations = []

    def add_configuration(
        *, parameter_set: dict[str, Any], anchor: tuple[str, int, str, str],
        circuit: tuple[str, str, str, str], workload: str, laps: int,
        cooling_points: int, seed: int, split: str,
        local_evidence: dict[str, Any] | None = None,
    ) -> None:
        anchor_id, era, vehicle_id, vehicle_family = anchor
        circuit_id, circuit_slug, circuit_archetype, _ = circuit
        scenario = configure_scenario(
            base_scenario,
            circuit_id=circuit_id,
            vehicle_id=vehicle_id,
            era=era,
            laps=laps,
            cooling_points=cooling_points,
        )
        scenario_digest = sha256(canonical_pretty(scenario))
        scenarios[scenario_digest] = scenario
        profile_digest = profile_refs[parameter_set["parameter_set_id"]]
        natural_key = {
            "schema_version": "pitgun.racing-v3-thermal-execution-key/v1",
            "parameter_set_id": parameter_set["parameter_set_id"],
            "profile_digest": profile_digest,
            "scenario_digest": scenario_digest,
            "seed": str(seed),
            "split": split,
        }
        configurations.append(
            {
                "execution_key": "v3th-" + sha256(canonical_pretty(natural_key))[7:23],
                "configuration_id": sha256(canonical_pretty(natural_key)),
                "parameter_set_id": parameter_set["parameter_set_id"],
                "parameter_origin": parameter_set["origin"],
                "profile_ref": profile_digest,
                "scenario_ref": scenario_digest,
                "anchor_id": anchor_id,
                "era": era,
                "vehicle_id": vehicle_id,
                "vehicle_family": vehicle_family,
                "circuit_id": circuit_id,
                "circuit_slug": circuit_slug,
                "circuit_archetype": circuit_archetype,
                "split": split,
                "workload": workload,
                "laps": laps,
                "cooling_points": cooling_points,
                "seed": str(seed),
                "expected_local_evidence": local_evidence,
            }
        )

    for parameter_set in retained:
        for anchor in VEHICLE_ANCHORS:
            for circuit in REPLAY_CIRCUITS:
                for cooling_points, seed in ((0, 42), (10, 42), (10, 99), (20, 42)):
                    key = (
                        parameter_set["parameter_set_id"], anchor[0], circuit[0],
                        cooling_points, seed, 18,
                    )
                    if key not in local_points:
                        raise ThermalManifestError(f"missing local replay evidence: {key}")
                    add_configuration(
                        parameter_set=parameter_set,
                        anchor=anchor,
                        circuit=circuit,
                        workload="long",
                        laps=18,
                        cooling_points=cooling_points,
                        seed=seed,
                        split=circuit[3],
                        local_evidence=expected_local_evidence(local_points[key]),
                    )

    for parameter_set in transition:
        for anchor in VEHICLE_ANCHORS:
            for circuit in REPLAY_CIRCUITS:
                for cooling_points in COOLING_LEVELS:
                    add_configuration(
                        parameter_set=parameter_set,
                        anchor=anchor,
                        circuit=circuit,
                        workload="long",
                        laps=18,
                        cooling_points=cooling_points,
                        seed=42,
                        split="transition_densification",
                    )

    for parameter_set in parameter_sets:
        for anchor in VEHICLE_ANCHORS:
            for cooling_points in COOLING_LEVELS:
                add_configuration(
                    parameter_set=parameter_set,
                    anchor=anchor,
                    circuit=VALIDATION_CIRCUIT,
                    workload="long",
                    laps=18,
                    cooling_points=cooling_points,
                    seed=VALIDATION_SEED,
                    split="final_validation",
                )
            add_configuration(
                parameter_set=parameter_set,
                anchor=anchor,
                circuit=VALIDATION_CIRCUIT,
                workload="short",
                laps=3,
                cooling_points=10,
                seed=VALIDATION_SEED,
                split="final_validation",
            )

    configurations.sort(key=lambda item: item["execution_key"])
    if len({item["execution_key"] for item in configurations}) != len(configurations):
        raise ThermalManifestError("execution keys are not unique")
    if len({item["configuration_id"] for item in configurations}) != len(configurations):
        raise ThermalManifestError("configuration identities are not unique")
    if {item["parameter_set_id"] for item in configurations} != {
        item["parameter_set_id"] for item in parameter_sets
    }:
        raise ThermalManifestError("not every parameter set is executable")
    if not retained_ids <= {
        item["parameter_set_id"]
        for item in configurations
        if item["expected_local_evidence"] is not None
    }:
        raise ThermalManifestError("not every retained set has exact replay evidence")

    return {
        "schema_version": SCHEMA_VERSION,
        "campaign_id": CAMPAIGN_ID,
        "execution_class": "experimental-v3-thermal-physics",
        "question": (
            "Can one-node era-aware thermal parameter families produce safe, "
            "progressive and strategically non-dominant cooling behavior?"
        ),
        "promotion_policy": "human-review-required",
        "automatic_catalog_promotion": False,
        "model": report["campaign"]["model"],
        "runner": {
            "kind": "v3_decision_surface_probe",
            "contract": "pitgun.racing-v3-decision-surface-probe/v1",
            "local_artifact_digest": report["campaign"]["runner"]["digest"],
        },
        "source_evidence": {
            "schema_version": report["schema_version"],
            "artifact_digest": sha256(report_bytes),
            "base_scenario_digest": sha256(scenario_bytes),
            "base_profile_digest": sha256(profile_bytes),
            "retained_parameter_set_count": len(retained),
            "selection_rule": (
                "pathological_execution_count == 0 and "
                "maximum_engine_temperature_c >= 100"
            ),
        },
        "adequacy_contract": {
            "classification_only_not_real_f1_calibration": True,
            "global_pathology_guards": {
                "maximum_engine_temperature_c": 180.0,
                "maximum_engine_derated_fraction": 0.50,
                "all_metrics_finite": True,
            },
            "historical_v8": {
                "required": ["safe", "cooling_not_universally_dominant"],
                "thermal_engagement_required": False,
                "reason": "historical vehicles must not inherit unexplained hybrid behavior",
            },
            "modern_v6t": {
                "required": [
                    "safe", "long_run_thermal_engagement", "progressive_cooling_value",
                    "cooling_not_universally_dominant",
                ]
            },
            "f1_2026": {
                "required": [
                    "safe", "long_run_thermal_engagement", "progressive_cooling_value",
                    "cooling_not_universally_dominant",
                ],
                "energy_controller_pace_feedback_in_scope": False,
            },
            "verdicts": ["PASS", "REFINE", "STRUCTURAL_CHANGE_REQUIRED"],
        },
        "dimensions": {
            "parameter_sets": {
                "retained": len(retained),
                "transition_densification": len(transition),
            },
            "vehicles": [item[2] for item in VEHICLE_ANCHORS],
            "eras": sorted({item[1] for item in VEHICLE_ANCHORS}),
            "circuits": [item[0] for item in REPLAY_CIRCUITS]
            + [VALIDATION_CIRCUIT[0]],
            "splits": sorted({item["split"] for item in configurations}),
            "cooling_levels": list(COOLING_LEVELS),
            "local_seeds": list(LOCAL_SEEDS),
            "reserved_validation_seed": VALIDATION_SEED,
            "workloads": {"short": 3, "long": 18},
        },
        "governance": {
            "rust_is_sole_physics_evaluator": True,
            "private_player_data_allowed": False,
            "automatic_game_or_catalog_promotion": False,
            "automatic_authority_or_verifier_promotion": False,
            "automatic_energy_or_opponent_policy_promotion": False,
            "local_replay_is_independent_validation": False,
            "final_validation_inputs_reserved_during_local_selection": True,
        },
        "planned_run_count": len(configurations),
        "local_replay_run_count": sum(
            item["expected_local_evidence"] is not None for item in configurations
        ),
        "new_evidence_run_count": sum(
            item["expected_local_evidence"] is None for item in configurations
        ),
        "unique_scenario_count": len(scenarios),
        "unique_profile_count": len(profiles),
        "parameter_sets": parameter_sets,
        "profiles": dict(sorted(profiles.items())),
        "scenarios": dict(sorted(scenarios.items())),
        "configurations": configurations,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--report", type=pathlib.Path, default=LOCAL_REPORT)
    parser.add_argument("--base-scenario", type=pathlib.Path, default=BASE_SCENARIO)
    parser.add_argument("--base-profile", type=pathlib.Path, default=BASE_PROFILE)
    parser.add_argument("--output", type=pathlib.Path, default=OUTPUT)
    arguments = parser.parse_args()
    manifest = build_manifest(
        arguments.report.resolve(),
        arguments.base_scenario.resolve(),
        arguments.base_profile.resolve(),
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
                "local_replay_run_count": manifest["local_replay_run_count"],
                "new_evidence_run_count": manifest["new_evidence_run_count"],
            },
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
