#!/usr/bin/env python3
"""Freeze the bounded Model V3 driver-control coefficient campaign."""

from __future__ import annotations

import copy
import hashlib
import json
import pathlib
from typing import Any


ROOT = pathlib.Path(__file__).resolve().parents[2]
LOCAL_REPORT = ROOT / "experiments/racing_v3_driver_control/results/local-driver-control-screen-v1.json"
BASE_SCENARIO = ROOT / "apps/pitgun-cli/scenarios/racing-batch-v1/balanced.json"
BASE_PROFILE = ROOT / "experiments/racing_v3_driver_control/profile-v10.driver-control.json"
DRIVERS = ROOT / "experiments/racing_v3_driver_control/driver-archetypes-v1.json"
OUTPUT = ROOT / "experiments/databricks/campaigns/racing-v3-driver-control-surface-v2.json"
SCHEMA_VERSION = "pitgun.racing-v3-driver-control-surface-campaign/v1"
CAMPAIGN_ID = "racing-v3-driver-control-surface-2026-v2"
MODEL = {
    "id": "pitgun.racing-v3-candidate",
    "version": "0.12.0",
    "digest": "sha256:5e505840e341181bb87af53d5915fd351d085a6b9a940c56c9683718df31741b",
}

CIRCUITS = (
    ("mc-1929", "monaco", "low-speed-high-downforce", 78),
    ("it-1922", "monza", "high-speed-low-downforce", 53),
)
DRIVER_IDS = ("limit_specialist", "tire_manager")
MODES = ("manage", "balanced", "attack")
HORIZONS = (("short", 5), ("race-length", None))
SEEDS = (7, 42)
INITIAL_FUEL_MASS_KG = 150.0


class DriverControlManifestError(RuntimeError):
    """Raised when governed driver-control inputs cannot be frozen."""


def canonical_pretty(value: object) -> bytes:
    return (json.dumps(value, indent=2, ensure_ascii=False) + "\n").encode()


def digest_bytes(value: bytes) -> str:
    return "sha256:" + hashlib.sha256(value).hexdigest()


def digest_json(value: object) -> str:
    return digest_bytes(canonical_pretty(value))


def halton(index: int, base: int) -> float:
    value = 0.0
    fraction = 1.0 / base
    while index:
        value += fraction * (index % base)
        index //= base
        fraction /= base
    return value


def checked_json(path: pathlib.Path, schema_version: str) -> tuple[dict[str, Any], bytes]:
    payload = path.read_bytes()
    value = json.loads(payload)
    if value.get("schema_version") != schema_version:
        raise DriverControlManifestError(f"unsupported governed source: {path}")
    return value, payload


def load_checked_report() -> tuple[dict[str, Any], bytes]:
    report, payload = checked_json(
        LOCAL_REPORT, "pitgun.racing-v3-driver-control-local-screen/v1"
    )
    expected = LOCAL_REPORT.with_suffix(".sha256").read_text().strip()
    if expected != digest_bytes(payload):
        raise DriverControlManifestError("local driver-control report checksum mismatch")
    if report.get("campaign", {}).get("configuration_count") != 702:
        raise DriverControlManifestError("local driver-control evidence is incomplete")
    return report, payload


def parameter_sets(base_profile: dict[str, Any]) -> list[dict[str, Any]]:
    baseline = copy.deepcopy(base_profile["driver_control_profile"])
    values = [
        {
            "parameter_set_id": "baseline-v10",
            "origin": "local-screen-baseline",
            "parameters": baseline,
        }
    ]
    for index in range(1, 33):
        manage = 0.58 + 0.16 * halton(index, 2)
        balanced = manage + 0.04 + 0.10 * halton(index, 3)
        attack = balanced + 0.01 + 0.07 * halton(index, 5)
        profile = copy.deepcopy(baseline)
        profile["mode_commitments"] = {
            "manage": round(manage, 8),
            "balanced": round(balanced, 8),
            "attack": round(attack, 8),
        }
        profile["commitment_error_gain"] = round(0.12 + 0.125 * halton(index, 7), 8)
        profile["commitment_error_exponent"] = round(1.0 + 3.0 * halton(index, 11), 8)
        profile["correction_workload_gain"] = round(4.0 + 6.0 * halton(index, 13), 8)
        values.append(
            {
                "parameter_set_id": f"halton-{index:02d}",
                "origin": "deterministic-bounded-halton",
                "parameters": profile,
            }
        )
    for value in values:
        validate_driver_control_profile(value["parameters"])
    return values


def validate_driver_control_profile(profile: dict[str, Any]) -> None:
    """Mirror the public contract bounds; the packaged Rust preflight is authoritative."""

    modes = profile["mode_commitments"]
    if not 0.0 <= modes["manage"] < modes["balanced"] < modes["attack"] <= 1.0:
        raise DriverControlManifestError("mode commitments violate the contract")
    for channel in ("cornering", "braking", "traction"):
        response = profile[channel]
        if not 0.0 <= response["floor"] <= 1.0 or not 0.0 <= response["span"] <= 1.0:
            raise DriverControlManifestError(f"{channel} response violates the contract")
        if response["floor"] + response["span"] > 1.0:
            raise DriverControlManifestError(f"{channel} response exceeds unity")
    if not 0.0 <= profile["base_control_error"]:
        raise DriverControlManifestError("base control error violates the contract")
    if not 0.0 <= profile["commitment_error_gain"]:
        raise DriverControlManifestError("commitment error gain violates the contract")
    if profile["base_control_error"] + profile["commitment_error_gain"] > 0.25:
        raise DriverControlManifestError("combined control error violates the contract")
    if not 1.0 <= profile["commitment_error_exponent"] <= 4.0:
        raise DriverControlManifestError("commitment error exponent violates the contract")
    if not 0.0 <= profile["correction_workload_gain"] <= 10.0:
        raise DriverControlManifestError("correction workload gain violates the contract")


def resolved_profile(base_profile: dict[str, Any], parameter_set: dict[str, Any]) -> dict[str, Any]:
    profile = copy.deepcopy(base_profile)
    profile["driver_control_profile"] = parameter_set["parameters"]
    return profile


def configure_scenario(
    base: dict[str, Any], *, circuit_id: str, laps: int, driver_id: str
) -> dict[str, Any]:
    scenario = copy.deepcopy(base)
    request = scenario["request"]
    request.update(
        {
            "track_id": circuit_id,
            "vehicle_id": "f1_2026",
            "era": 5,
            "laps": laps,
            "hz": 5.0,
            "initial_fuel_mass_kg": INITIAL_FUEL_MASS_KG,
        }
    )
    competitor = request["competitors"][0]
    competitor.update(
        {
            "driver_id": driver_id,
            "budget_cap": 100.0,
            "tuning": {
                "engine_points": 25.0,
                "cooling_points": 25.0,
                "aero_points": 25.0,
                "chassis_points": 25.0,
                "downforce_slider": 0.5,
                "gear_ratio_slider": 0.5,
            },
            "stint_strategy": {
                "stints": [{"tire_id": "medium", "laps": laps}],
                "pit_laps": [],
            },
        }
    )
    request.pop("pit_strategy", None)
    request.pop("competitor_vehicle_components", None)
    return scenario


def driver_experiment(driver: dict[str, Any], mode: str) -> dict[str, Any]:
    return {"drivers": {driver["id"]: driver}, "competitor_modes": {"player": mode}}


def expected_local_evidence(row: dict[str, Any]) -> dict[str, Any]:
    return {
        "experimental_execution_id": row["experimental_execution_id"],
        "scenario_digest": row["scenario_digest"],
        "profile_digest": row["profile_digest"],
        "driver_experiment_digest": row["driver_experiment_digest"],
        "metrics": {
            key: row[key]
            for key in (
                "total_time_ms",
                "best_lap_ms",
                "mean_lap_ms",
                "lap_time_stddev_ms",
                "final_tire_temperature_c",
                "final_tire_wear_pct",
                "requested_commitment",
                "control_error_amplitude",
                "correction_workload_multiplier",
                "correction_contact_workload_mj",
                "requested_correction_wear_fraction",
            )
        },
    }


def build_manifest() -> dict[str, Any]:
    report, report_payload = load_checked_report()
    base_scenario, base_scenario_payload = checked_json(
        BASE_SCENARIO, "pitgun.racing-resolved-scenario/v1"
    )
    base_profile, base_profile_payload = checked_json(
        BASE_PROFILE, "pitgun.racing-v3-experiment-profile/v10"
    )
    driver_document, driver_payload = checked_json(
        DRIVERS, "pitgun.racing-driver-archetype-screen/v1"
    )
    drivers = {row["id"]: row for row in driver_document["drivers"]}
    if not set(DRIVER_IDS) <= set(drivers):
        raise DriverControlManifestError("calibration drivers are missing")

    local_index = {
        (
            row["circuit_id"], row["horizon"], row["driver_id"], row["mode"],
            row["seed"], row["tire_id"],
        ): row
        for row in report["runs"]
        if row["family"] == "archetype.full-factorial"
    }
    profiles: dict[str, Any] = {}
    scenarios: dict[str, Any] = {}
    driver_experiments: dict[str, Any] = {}
    configurations: list[dict[str, Any]] = []
    sets = parameter_sets(base_profile)

    for parameter_set in sets:
        profile = resolved_profile(base_profile, parameter_set)
        profile_ref = digest_json(profile)
        profiles[profile_ref] = profile
        for circuit_id, circuit_slug, circuit_archetype, race_laps in CIRCUITS:
            for horizon, configured_laps in HORIZONS:
                laps = race_laps if configured_laps is None else configured_laps
                for driver_id in DRIVER_IDS:
                    driver = drivers[driver_id]
                    scenario = configure_scenario(
                        base_scenario, circuit_id=circuit_id, laps=laps, driver_id=driver_id
                    )
                    scenario_ref = digest_json(scenario)
                    scenarios[scenario_ref] = scenario
                    for mode in MODES:
                        experiment = driver_experiment(driver, mode)
                        experiment_ref = digest_json(experiment)
                        driver_experiments[experiment_ref] = experiment
                        for seed in SEEDS:
                            identity = {
                                "schema_version": SCHEMA_VERSION,
                                "parameter_set_id": parameter_set["parameter_set_id"],
                                "scenario_ref": scenario_ref,
                                "profile_ref": profile_ref,
                                "driver_experiment_ref": experiment_ref,
                                "seed": str(seed),
                            }
                            configuration_id = digest_json(identity)
                            expected = None
                            if parameter_set["parameter_set_id"] == "baseline-v10":
                                source = local_index.get(
                                    (circuit_id, horizon, driver_id, mode, seed, "medium")
                                )
                                if source is None:
                                    raise DriverControlManifestError("baseline replay evidence is missing")
                                expected = expected_local_evidence(source)
                            configurations.append(
                                identity
                                | {
                                    "configuration_id": configuration_id,
                                    "execution_key": "v3dc-" + configuration_id[7:23],
                                    "split": "surface_calibration",
                                    "circuit_id": circuit_id,
                                    "circuit_slug": circuit_slug,
                                    "circuit_archetype": circuit_archetype,
                                    "horizon": horizon,
                                    "laps": laps,
                                    "driver_id": driver_id,
                                    "mode": mode,
                                    "tire_id": "medium",
                                    "expected_local_evidence": expected,
                                }
                            )

    configurations.sort(key=lambda row: row["execution_key"])
    if len(configurations) != 1584:
        raise DriverControlManifestError(f"unexpected plan size: {len(configurations)}")
    return {
        "schema_version": SCHEMA_VERSION,
        "campaign_id": CAMPAIGN_ID,
        "question": "Which deterministic driver-control coefficients preserve short-run ATTACK utility without making ATTACK universally dominant over race distance?",
        "execution_class": "experimental-v3-driver-control-physics",
        "parameter_space_version": "racing-v3-driver-control-coefficients-v1",
        "promotion_policy": "human-review-required",
        "automatic_catalog_promotion": False,
        "model": MODEL,
        "source_evidence": {
            "local_report_digest": digest_bytes(report_payload),
            "base_scenario_digest": digest_bytes(base_scenario_payload),
            "base_profile_digest": digest_bytes(base_profile_payload),
            "driver_archetype_digest": digest_bytes(driver_payload),
            "local_review_verdict": report["analysis"]["review_verdict"],
        },
        "parameter_space": {
            "method": "baseline-plus-32-point-deterministic-halton-v2",
            "axes": {
                "mode_commitments": "ordered commitments with a 0.01-0.08 balanced-to-attack gap",
                "commitment_error_gain": [0.12, 0.245],
                "commitment_error_exponent": [1.0, 4.0],
                "correction_workload_gain": [4.0, 10.0],
            },
        },
        "supersedes": {
            "campaign_id": "racing-v3-driver-control-surface-2026-v1",
            "reason": "four V1 profiles exceeded the Rust correction_workload_gain maximum",
            "failed_run_id": "904736248097501",
        },
        "selection_contract": {
            "attack_must_win_some_short_groups": True,
            "attack_must_not_win_every_race_length_group": True,
            "mode_error_and_workload_ordering_required": True,
            "pathological_output_count_must_equal": 0,
            "ranking_is_decision_support_only": True,
        },
        "governance": {
            "local_replay_is_independent_validation": False,
            "complete_702_case_matrix_reserved_for_final_validation": True,
            "held_out_circuit": "jp-1962",
            "held_out_driver_ids": ["balanced_reference", "smooth_operator"],
            "held_out_compounds": ["soft", "hard"],
            "held_out_seed": "99",
        },
        "parameter_sets": sets,
        "profiles": dict(sorted(profiles.items())),
        "scenarios": dict(sorted(scenarios.items())),
        "driver_experiments": dict(sorted(driver_experiments.items())),
        "configurations": configurations,
        "planned_run_count": len(configurations),
        "local_replay_run_count": sum(
            row["expected_local_evidence"] is not None for row in configurations
        ),
        "new_evidence_run_count": sum(
            row["expected_local_evidence"] is None for row in configurations
        ),
        "unique_profile_count": len(profiles),
        "unique_scenario_count": len(scenarios),
        "unique_driver_experiment_count": len(driver_experiments),
    }


def main() -> None:
    manifest = build_manifest()
    payload = canonical_pretty(manifest)
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_bytes(payload)
    OUTPUT.with_suffix(".sha256").write_text(
        f"{hashlib.sha256(payload).hexdigest()}  {OUTPUT.name}\n"
    )
    print(f"wrote {OUTPUT} ({manifest['planned_run_count']} executions)")


if __name__ == "__main__":
    main()
