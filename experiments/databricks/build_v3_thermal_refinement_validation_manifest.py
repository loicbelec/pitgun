#!/usr/bin/env python3
"""Freeze the pre-registered thermal family validation for Databricks."""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import pathlib
from typing import Any


ROOT = pathlib.Path(__file__).resolve().parents[2]
CONTRACT = ROOT / "experiments" / "racing_v3_thermal_refinement" / "contract-v1.json"
LOCAL_RESULT = ROOT / "experiments" / "racing_v3_thermal_refinement" / "results" / "local-refinement-v1.json"
SOURCE_CAMPAIGN = ROOT / "experiments" / "databricks" / "campaigns" / "racing-v3-thermal-adequacy-v1.json"
BASE_SCENARIO = ROOT / "apps" / "pitgun-cli" / "scenarios" / "racing-batch-v1" / "balanced.json"
BASE_PROFILE = ROOT / "experiments" / "racing_v3_decision_surface" / "profile-v8.engine-thermal-resolution.json"
OUTPUT = ROOT / "experiments" / "databricks" / "campaigns" / "racing-v3-thermal-refinement-validation-v2.json"
SCHEMA_VERSION = "pitgun.racing-v3-thermal-adequacy-campaign/v1"
CAMPAIGN_ID = "racing-v3-thermal-refinement-validation-2026-v2"
CAMPAIGN_NAME = "racing-v3-thermal-refinement-validation-v2"


class ValidationManifestError(RuntimeError):
    """Raised when frozen source evidence cannot produce the validation plan."""


def canonical_pretty(value: object) -> bytes:
    return (json.dumps(value, indent=2, ensure_ascii=False) + "\n").encode()


def sha256(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


def load_checked(path: pathlib.Path, expected_schema: str) -> tuple[dict[str, Any], bytes]:
    payload = path.read_bytes()
    checksum = path.with_suffix(".sha256").read_text().split()
    if len(checksum) == 1:
        matches = checksum[0] == sha256(payload)
    else:
        matches = len(checksum) == 2 and checksum[1] == path.name and checksum[0] in {
            sha256(payload),
            hashlib.sha256(payload).hexdigest(),
        }
    if not matches:
        raise ValidationManifestError(f"{path.name} does not match its checksum")
    value = json.loads(payload)
    if value.get("schema_version") != expected_schema:
        raise ValidationManifestError(f"{path.name} has an unsupported schema")
    return value, payload


def configure_scenario(base: dict[str, Any], *, vehicle_id: str, era: int, cooling_points: int) -> dict[str, Any]:
    scenario = copy.deepcopy(base)
    request = scenario["request"]
    request.update(
        {
            "track_id": "gb-1948",
            "vehicle_id": vehicle_id,
            "era": era,
            "laps": 52,
            "hz": 5.0,
            # Full-race validation isolates thermal behavior. The original
            # 80 kg screen reservoir depleted before 52 laps on V6T engines.
            "initial_fuel_mass_kg": 130.0,
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


def resolved_profiles(base_profile: dict[str, Any], contract: dict[str, Any]) -> dict[str, dict[str, Any]]:
    historical = copy.deepcopy(base_profile)
    f1_2026 = copy.deepcopy(base_profile)
    f1_2026["engine_thermal_resolution"].update(contract["anchor_parameters"])
    modern = copy.deepcopy(f1_2026)
    modern["engine_thermal_resolution"]["soft_limit_offset_c"] = -3.0
    return {
        "historical-default": historical,
        "modern-v6t-soft-limit--3.0c": modern,
        "f1-2026-adaptive-038": f1_2026,
    }


def build_manifest(
    contract_path: pathlib.Path = CONTRACT,
    result_path: pathlib.Path = LOCAL_RESULT,
    source_campaign_path: pathlib.Path = SOURCE_CAMPAIGN,
    base_scenario_path: pathlib.Path = BASE_SCENARIO,
    base_profile_path: pathlib.Path = BASE_PROFILE,
) -> dict[str, Any]:
    contract, contract_bytes = load_checked(contract_path, "pitgun.racing-v3-thermal-refinement-contract/v1")
    result, result_bytes = load_checked(result_path, "pitgun.racing-v3-thermal-refinement-local/v1")
    if result["contract_digest"] != sha256(contract_bytes):
        raise ValidationManifestError("local result does not reference the contract")
    if result["selection_verdict"] != "PASS" or result["selected_parameter_set_id"] != "soft-limit--3.0c":
        raise ValidationManifestError("local selection did not authorize the candidate")
    reserved = contract["reserved_final_validation"]
    if reserved["circuit"]["id"] != "gb-1948" or reserved["seed"] != 20260901 or reserved["workload"] != "full-race":
        raise ValidationManifestError("reserved validation boundary changed")

    source_campaign_bytes = source_campaign_path.read_bytes()
    if sha256(source_campaign_bytes) != contract["source_evidence"]["campaign_digest"]:
        raise ValidationManifestError("reviewed source campaign changed")
    source_campaign = json.loads(source_campaign_bytes)
    scenario_bytes = base_scenario_path.read_bytes()
    base_profile_bytes = base_profile_path.read_bytes()
    base_scenario = json.loads(scenario_bytes)
    base_profile = json.loads(base_profile_bytes)
    family_profiles = resolved_profiles(base_profile, contract)
    anchors = (
        ("era1-classic60", 1, "classic_v8_1960", "historical_v8", "historical-default"),
        ("era4-classic70", 4, "classic_v8_1970", "historical_v8", "historical-default"),
        ("era5-v6t", 5, "modern_v6t", "modern_v6t", "modern-v6t-soft-limit--3.0c"),
        ("era5-hybrid", 5, "f1_2026", "f1_2026", "f1-2026-adaptive-038"),
    )

    profiles: dict[str, dict[str, Any]] = {}
    profile_refs = {}
    for parameter_set_id, profile in family_profiles.items():
        digest = sha256(canonical_pretty(profile))
        profiles[digest] = profile
        profile_refs[parameter_set_id] = digest

    scenarios: dict[str, dict[str, Any]] = {}
    configurations = []
    for anchor_id, era, vehicle_id, family, parameter_set_id in anchors:
        for cooling_points in (0, 10, 20):
            scenario = configure_scenario(base_scenario, vehicle_id=vehicle_id, era=era, cooling_points=cooling_points)
            scenario_digest = sha256(canonical_pretty(scenario))
            scenarios[scenario_digest] = scenario
            natural_key = {
                "schema_version": "pitgun.racing-v3-thermal-execution-key/v1",
                "campaign_id": CAMPAIGN_ID,
                "anchor_id": anchor_id,
                "profile_digest": profile_refs[parameter_set_id],
                "scenario_digest": scenario_digest,
                "seed": "20260901",
            }
            identity = sha256(canonical_pretty(natural_key))
            configurations.append(
                {
                    "execution_key": "v3th-" + identity[7:23],
                    "configuration_id": identity,
                    "parameter_set_id": parameter_set_id,
                    "parameter_origin": "pre_registered_family_validation",
                    "profile_ref": profile_refs[parameter_set_id],
                    "scenario_ref": scenario_digest,
                    "anchor_id": anchor_id,
                    "era": era,
                    "vehicle_id": vehicle_id,
                    "vehicle_family": family,
                    "circuit_id": "gb-1948",
                    "circuit_slug": "silverstone",
                    "circuit_archetype": "high-speed-mixed",
                    "split": "independent_final_validation",
                    "workload": "full-race",
                    "laps": 52,
                    "cooling_points": cooling_points,
                    "seed": "20260901",
                    "expected_local_evidence": None,
                }
            )
    configurations.sort(key=lambda item: item["execution_key"])
    if len(configurations) != 12:
        raise ValidationManifestError("validation must contain exactly 12 runs")
    if len({item["configuration_id"] for item in configurations}) != 12:
        raise ValidationManifestError("validation identities are not unique")

    return {
        "schema_version": SCHEMA_VERSION,
        "campaign_id": CAMPAIGN_ID,
        "execution_class": "experimental-v3-thermal-physics",
        "parameter_space_version": "racing-v3-thermal-refinement-validation-v2",
        "question": "Do the frozen era-specific thermal profiles transfer to a full Silverstone race without reusing local selection evidence?",
        "promotion_policy": "human-review-required",
        "automatic_catalog_promotion": False,
        "model": source_campaign["model"],
        "runner": source_campaign["runner"],
        "source_evidence": {
            "schema_version": result["schema_version"],
            "artifact_digest": sha256(result_bytes),
            "contract_digest": sha256(contract_bytes),
            "reviewed_campaign_digest": sha256(source_campaign_bytes),
            "base_scenario_digest": sha256(scenario_bytes),
            "base_profile_digest": sha256(base_profile_bytes),
            "selected_parameter_set_id": result["selected_parameter_set_id"],
            "supersedes_campaign_id": "racing-v3-thermal-refinement-validation-2026-v1",
            "experimental_fuel_reservoir_kg": 130.0,
            "correction_reason": "The inherited 80 kg local-screen reservoir depleted before the reserved 52-lap workload for modern V6T and F1 2026.",
        },
        "adequacy_contract": {
            "classification_only_not_real_f1_calibration": True,
            "global_pathology_guards": {
                "maximum_engine_temperature_c": 180.0,
                "maximum_engine_derated_fraction": 1.0,
                "all_metrics_finite": True,
                "zero_cooling_severity_is_not_a_pathology": True,
            },
            "historical_v8": {"required": ["safe", "cooling_not_beneficial"], "thermal_engagement_required": False},
            "modern_v6t": {"required": ["safe", "monotonic_thermal_recovery", "interior_cooling_optimum"]},
            "f1_2026": {
                "required": ["safe", "monotonic_thermal_recovery", "interior_cooling_optimum"],
                "energy_controller_pace_feedback_in_scope": False,
            },
            "verdicts": ["PASS", "REFINE", "STRUCTURAL_CHANGE_REQUIRED"],
        },
        "dimensions": {
            "profiles_by_family": {
                "historical_v8": "historical-default",
                "modern_v6t": "modern-v6t-soft-limit--3.0c",
                "f1_2026": "f1-2026-adaptive-038",
            },
            "vehicles": [item[2] for item in anchors],
            "eras": sorted({item[1] for item in anchors}),
            "circuits": ["gb-1948"],
            "splits": ["independent_final_validation"],
            "cooling_levels": [0, 10, 20],
            "reserved_validation_seed": 20260901,
            "workloads": {"full-race": 52},
        },
        "governance": {
            "rust_is_sole_physics_evaluator": True,
            "private_player_data_allowed": False,
            "automatic_game_or_catalog_promotion": False,
            "automatic_authority_or_verifier_promotion": False,
            "local_replay_is_independent_validation": False,
            "final_validation_inputs_reserved_during_local_selection": True,
            "all_configurations_are_new_validation_evidence": True,
        },
        "planned_run_count": len(configurations),
        "local_replay_run_count": 0,
        "new_evidence_run_count": len(configurations),
        "unique_scenario_count": len(scenarios),
        "unique_profile_count": len(profiles),
        "parameter_sets": [
            {"parameter_set_id": identifier, "origin": "pre_registered_family_validation", "profile_ref": profile_refs[identifier]}
            for identifier in sorted(family_profiles)
        ],
        "profiles": dict(sorted(profiles.items())),
        "scenarios": dict(sorted(scenarios.items())),
        "configurations": configurations,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=pathlib.Path, default=OUTPUT)
    arguments = parser.parse_args()
    manifest = build_manifest()
    payload = canonical_pretty(manifest)
    arguments.output.parent.mkdir(parents=True, exist_ok=True)
    arguments.output.write_bytes(payload)
    arguments.output.with_suffix(".sha256").write_text(
        hashlib.sha256(payload).hexdigest() + "  " + arguments.output.name + "\n"
    )
    print(json.dumps({"campaign_name": CAMPAIGN_NAME, "campaign_id": CAMPAIGN_ID, "digest": sha256(payload), "planned_run_count": manifest["planned_run_count"]}, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
