#!/usr/bin/env python3
"""Freeze the Model 0.13 equal-budget driving-mode response campaign."""

from __future__ import annotations

import copy
import hashlib
import json
import pathlib
from typing import Any


ROOT = pathlib.Path(__file__).resolve().parents[2]
EXPERIMENT = ROOT / "experiments/racing_v3_driver_control"
LOCAL_REPORT = (
    EXPERIMENT / "results/equal-budget-driver-control-halton-19-v1.json"
)
LOCAL_REVIEW = EXPERIMENT / "results/equal-budget-driver-control-review-v1.json"
BASE_SCENARIO = ROOT / "apps/pitgun-cli/scenarios/racing-batch-v1/balanced.json"
BASE_PROFILE = EXPERIMENT / "shortlist/profile-v11.halton-19.json"
DRIVERS = EXPERIMENT / "driver-archetypes-equal-budget-v1.json"
OUTPUT = (
    ROOT
    / "experiments/databricks/campaigns/racing-v3-driver-mode-surface-v3.json"
)
SCHEMA_VERSION = "pitgun.racing-v3-driver-control-surface-campaign/v2"
CAMPAIGN_ID = "racing-v3-driver-mode-surface-2026-v3"
MODEL = {
    "id": "pitgun.racing-v3-candidate",
    "version": "0.13.0",
    "digest": "sha256:34c95f6c345d30db12e2d036d1ded1a93fb689db37df579ac61d77b553537a4f",
}

CIRCUITS = (
    ("mc-1929", "monaco", "low-speed-high-downforce", 78),
    ("it-1922", "monza", "high-speed-low-downforce", 53),
)
DRIVER_IDS = (
    "balanced_reference",
    "limit_specialist",
    "smooth_operator",
    "tire_manager",
)
MODES = ("manage", "balanced", "attack")
HORIZONS = (("short", 5), ("race-length", None))
SEEDS = (7, 42)
INITIAL_FUEL_MASS_KG = 150.0


class DriverModeManifestError(RuntimeError):
    """Raised when governed mode-response inputs cannot be frozen."""


def canonical_pretty(value: object) -> bytes:
    return (json.dumps(value, indent=2, ensure_ascii=False) + "\n").encode()


def digest_bytes(value: bytes) -> str:
    return "sha256:" + hashlib.sha256(value).hexdigest()


def digest_json(value: object) -> str:
    return digest_bytes(canonical_pretty(value))


def checked_json(path: pathlib.Path, schema_version: str) -> tuple[dict[str, Any], bytes]:
    payload = path.read_bytes()
    value = json.loads(payload)
    if value.get("schema_version") != schema_version:
        raise DriverModeManifestError(f"unsupported governed source: {path}")
    checksum = path.with_suffix(".sha256")
    if checksum.is_file():
        expected = checksum.read_text().split()[0]
        if expected.removeprefix("sha256:") != hashlib.sha256(payload).hexdigest():
            raise DriverModeManifestError(f"governed source checksum mismatch: {path}")
    return value, payload


def halton(index: int, base: int) -> float:
    value = 0.0
    fraction = 1.0 / base
    while index:
        value += fraction * (index % base)
        index //= base
        fraction /= base
    return value


def validate_driver_control_profile(profile: dict[str, Any]) -> None:
    """Mirror public bounds; the packaged Rust preflight remains authoritative."""

    modes = profile["mode_commitments"]
    if not 0.0 <= modes["manage"] < modes["balanced"] < modes["attack"] <= 1.0:
        raise DriverModeManifestError("mode commitments violate the contract")
    if profile["base_control_error"] + profile["commitment_error_gain"] > 0.25:
        raise DriverModeManifestError("combined control error violates the contract")
    if not 1.0 <= profile["commitment_error_exponent"] <= 4.0:
        raise DriverModeManifestError("commitment error exponent violates the contract")
    if not 0.0 <= profile["correction_workload_gain"] <= 10.0:
        raise DriverModeManifestError("correction workload gain violates the contract")


def parameter_sets(base_profile: dict[str, Any]) -> list[dict[str, Any]]:
    anchor = copy.deepcopy(base_profile["driver_control_profile"])
    values = [
        {
            "parameter_set_id": "anchor-halton-19",
            "origin": "equal-budget-local-replay-anchor",
            "parameters": anchor,
        }
    ]
    for index in range(1, 33):
        manage = 0.62 + 0.12 * halton(index, 2)
        balanced = manage + 0.03 + 0.07 * halton(index, 3)
        attack = balanced + 0.005 + 0.045 * halton(index, 5)
        profile = copy.deepcopy(anchor)
        profile["mode_commitments"] = {
            "manage": round(manage, 8),
            "balanced": round(balanced, 8),
            "attack": round(attack, 8),
        }
        profile["commitment_error_gain"] = round(
            0.16 + 0.085 * halton(index, 7), 8
        )
        profile["commitment_error_exponent"] = round(
            1.0 + 3.0 * halton(index, 11), 8
        )
        profile["correction_workload_gain"] = round(
            6.0 + 4.0 * halton(index, 13), 8
        )
        values.append(
            {
                "parameter_set_id": f"mode-halton-{index:02d}",
                "origin": "deterministic-bounded-halton",
                "parameters": profile,
            }
        )
    for value in values:
        validate_driver_control_profile(value["parameters"])
    return values


def validate_roster(document: dict[str, Any]) -> dict[str, dict[str, Any]]:
    if document.get("schema_version") != "pitgun.racing-driver-archetype-screen/v2":
        raise DriverModeManifestError("unsupported equal-budget roster")
    expected_budget = float(document["design"]["trait_budget"])
    tolerance = float(document["design"]["budget_tolerance"])
    drivers = {row["id"]: row for row in document["drivers"]}
    if set(drivers) != set(DRIVER_IDS):
        raise DriverModeManifestError("equal-budget roster is incomplete")
    for driver_id, driver in drivers.items():
        budget = sum(float(value) for value in driver["traits"].values())
        if abs(budget - expected_budget) > tolerance:
            raise DriverModeManifestError(
                f"driver {driver_id} does not preserve the equal trait budget"
            )
    return drivers


def resolved_profile(base: dict[str, Any], parameter_set: dict[str, Any]) -> dict[str, Any]:
    profile = copy.deepcopy(base)
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
    local_report, local_report_payload = checked_json(
        LOCAL_REPORT, "pitgun.racing-v3-driver-control-local-screen/v2"
    )
    local_review, local_review_payload = checked_json(
        LOCAL_REVIEW, "pitgun.racing-v3-equal-budget-driver-review/v1"
    )
    if local_review.get("verdict") != "MODE_RESPONSE_REFINEMENT_REQUIRED":
        raise DriverModeManifestError("equal-budget evidence does not authorize this question")
    if local_report.get("campaign", {}).get("configuration_count") != 702:
        raise DriverModeManifestError("equal-budget anchor evidence is incomplete")

    base_scenario, base_scenario_payload = checked_json(
        BASE_SCENARIO, "pitgun.racing-resolved-scenario/v1"
    )
    base_profile, base_profile_payload = checked_json(
        BASE_PROFILE, "pitgun.racing-v3-experiment-profile/v11"
    )
    driver_document, driver_payload = checked_json(
        DRIVERS, "pitgun.racing-driver-archetype-screen/v2"
    )
    drivers = validate_roster(driver_document)
    roster_digest = digest_json(driver_document)
    if local_review["roster_digest"] != roster_digest:
        raise DriverModeManifestError("review and roster identities differ")

    local_index = {
        (
            row["circuit_id"],
            row["horizon"],
            row["driver_id"],
            row["mode"],
            row["seed"],
            row["tire_id"],
        ): row
        for row in local_report["runs"]
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
                    scenario = configure_scenario(
                        base_scenario,
                        circuit_id=circuit_id,
                        laps=laps,
                        driver_id=driver_id,
                    )
                    scenario_ref = digest_json(scenario)
                    scenarios[scenario_ref] = scenario
                    for mode in MODES:
                        experiment = driver_experiment(drivers[driver_id], mode)
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
                            if parameter_set["parameter_set_id"] == "anchor-halton-19":
                                source = local_index.get(
                                    (
                                        circuit_id,
                                        horizon,
                                        driver_id,
                                        mode,
                                        seed,
                                        "medium",
                                    )
                                )
                                if source is None:
                                    raise DriverModeManifestError(
                                        "anchor replay evidence is missing"
                                    )
                                expected = expected_local_evidence(source)
                            configurations.append(
                                identity
                                | {
                                    "configuration_id": configuration_id,
                                    "execution_key": "v3dc-" + configuration_id[7:23],
                                    "split": "equal_budget_mode_calibration",
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
    if len(configurations) != 3168:
        raise DriverModeManifestError(
            f"unexpected mode-response plan size: {len(configurations)}"
        )
    return {
        "schema_version": SCHEMA_VERSION,
        "campaign_id": CAMPAIGN_ID,
        "question": (
            "Which deterministic mode-response coefficients keep ATTACK useful "
            "over short runs without making it universally optimal over race distance "
            "for the fixed equal-budget driver roster?"
        ),
        "execution_class": "experimental-v3-driver-control-physics",
        "parameter_space_version": "racing-v3-equal-budget-mode-response-v1",
        "promotion_policy": "human-review-required",
        "automatic_catalog_promotion": False,
        "model": MODEL,
        "source_evidence": {
            "local_report_digest": digest_bytes(local_report_payload),
            "local_review_digest": digest_bytes(local_review_payload),
            "base_scenario_digest": digest_bytes(base_scenario_payload),
            "base_profile_digest": digest_bytes(base_profile_payload),
            "driver_archetype_digest": digest_bytes(driver_payload),
            "equal_budget_roster_digest": roster_digest,
            "local_review_verdict": local_review["verdict"],
        },
        "parameter_space": {
            "method": "anchor-plus-32-point-deterministic-halton-v3",
            "fixed": [
                "model-0.13",
                "equal-budget-driver-roster-v1",
                "vehicle-and-non-driver-physics",
                "100-point-even-vehicle-tuning",
            ],
            "axes": {
                "manage_commitment": [0.62, 0.74],
                "balanced_gap_from_manage": [0.03, 0.10],
                "attack_gap_from_balanced": [0.005, 0.05],
                "commitment_error_gain": [0.16, 0.245],
                "commitment_error_exponent": [1.0, 4.0],
                "correction_workload_gain": [6.0, 10.0],
            },
        },
        "predecessor_evidence": {
            "campaign_id": "racing-v3-driver-control-surface-2026-v2",
            "equal_budget_review_verdict": local_review["verdict"],
            "reason": (
                "equal budgets removed the universal driver for two profiles, "
                "but ATTACK remained the universal winning mode"
            ),
        },
        "selection_contract": {
            "attack_must_win_some_short_global_contexts": True,
            "attack_must_not_win_every_race_length_global_context": True,
            "minimum_global_winning_driver_count": 2,
            "mode_error_and_workload_ordering_required": True,
            "pathological_output_count_must_equal": 0,
            "ranking_is_decision_support_only": True,
        },
        "governance": {
            "local_replay_is_independent_validation": False,
            "complete_702_case_matrix_reserved_for_final_validation": True,
            "held_out_circuit": "jp-1962",
            "held_out_compounds": ["soft", "hard"],
            "held_out_seed": "99",
            "driver_traits_are_fixed": True,
            "model_and_non_driver_physics_are_fixed": True,
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
