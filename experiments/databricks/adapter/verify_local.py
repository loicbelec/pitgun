#!/usr/bin/env python3
"""Verify the packaged Linux runner against the published Racing fixture."""

from pitgun_databricks_adapter import (
    execute_packaged_racing,
    execute_packaged_racing_catalog_scenario,
    execute_packaged_racing_scenario,
    execute_packaged_tuning_response,
    execute_packaged_v3_tire_degradation,
    execute_packaged_v3_driver_control,
    inspect_packaged_runner,
    load_calibration_campaign,
    load_candidate_review_policy,
    load_budget_effect_v2_campaign,
    load_early_allocation_effect_campaign,
    load_opponent_audit_campaign,
    load_opponent_acceptance_campaign,
    load_reference_campaign,
    load_tire_degradation_campaign,
    load_driver_control_campaign,
    materialize_opponent_audit_plan,
    materialize_opponent_acceptance_plan,
    materialize_budget_effect_v2_plan,
    materialize_early_allocation_effect_plan,
    materialize_plan,
    materialize_tire_degradation_plan,
    materialize_driver_control_plan,
    validate_packaged_v3_driver_control_profiles,
)


EXPECTED_CONFIGURATION_ID = (
    "sha256:12a4207b2c26c814763a2a488054f7421e7cc3836a35e26fc16d96477c8744d7"
)
EXPECTED_RUN_ID = (
    "sha256:89dc458a7460056dd519f5cda74c55c2b2b47f7091f1309ae10d11a2eb46a64a"
)
EXPECTED_RESULT_DIGEST = (
    "sha256:685d3870898f0ccfaa88a1d46e3f0dc3a24131a341c61e08ac98f3560d853923"
)


result = execute_packaged_racing(42)
runner_identity = inspect_packaged_runner()
manifest, manifest_digest = load_reference_campaign()
plan = materialize_plan(manifest)
assert result["result"]["configuration_id"] == EXPECTED_CONFIGURATION_ID
assert result["result"]["run_id"] == EXPECTED_RUN_ID
assert result["canonical_result_digest"] == EXPECTED_RESULT_DIGEST
assert result["host"] == {"machine": "aarch64", "system": "Linux"}
assert result["adapter"]["version"].startswith("0.3.0a1+g")
assert result["runner_artifact"]["digest"] == runner_identity["digest"]
assert manifest_digest.startswith("sha256:")
assert len(plan) == manifest["planned_run_count"] == 9
print(
    "local packaged runner verified: "
    f"adapter={result['adapter']['version']} "
    f"{result['canonical_result_digest']} "
    f"startup={result['measurements']['startup_probe_duration_ms']}ms "
    f"execution={result['measurements']['execution_duration_ms']}ms"
)

opponent_smoke = execute_packaged_racing_catalog_scenario(
    42, "monza-early-42-balanced-one-stop"
)["result"]
assert opponent_smoke["model"] == {
    "id": "pitgun.racing",
    "version": "2.0.0",
    "digest": "sha256:a372f990c320d10207220f98ca4bf677607fc5c13918c73b47dfbb8949b106d2",
}
assert opponent_smoke["data_pack"] == {
    "id": "pitgun.racing.simulation",
    "version": "1.2.0",
    "digest": "sha256:8961087a8a04a3cafb157a63b7bd2c8daa1e29500e180c5d723324d79374549f",
}
assert opponent_smoke["seed"] == "42"
assert len(opponent_smoke["summary"]["standings"]) == 10

opponent_manifest, opponent_manifest_digest = load_opponent_audit_campaign()
opponent_plan = materialize_opponent_audit_plan(opponent_manifest)
assert opponent_manifest_digest.startswith("sha256:")
assert len(opponent_plan) == opponent_manifest["planned_run_count"] == 180
first_opponent_run = opponent_plan[0]
campaign_smoke = execute_packaged_racing_catalog_scenario(
    int(first_opponent_run["seed"]), first_opponent_run["scenario_resource"]
)["result"]
assert campaign_smoke["model"]["version"] == "2.0.0"
assert campaign_smoke["data_pack"]["version"] == "1.2.0"
assert len(campaign_smoke["summary"]["standings"]) == 10

acceptance_manifest, acceptance_manifest_digest = (
    load_opponent_acceptance_campaign()
)
acceptance_plan = materialize_opponent_acceptance_plan(acceptance_manifest)
assert acceptance_manifest_digest.startswith("sha256:")
assert len(acceptance_plan) == acceptance_manifest["planned_run_count"] == 135
first_acceptance = acceptance_plan[0]
acceptance_smoke = execute_packaged_racing_catalog_scenario(
    int(first_acceptance["seed"]),
    first_acceptance["scenario_resource"],
    "racing-v1-9-0",
)["result"]
assert acceptance_smoke["model"] == {
    "id": acceptance_manifest["catalog"]["model_id"],
    "version": acceptance_manifest["catalog"]["model_version"],
    "digest": acceptance_manifest["catalog"]["model_digest"],
}
assert acceptance_smoke["data_pack"]["version"] == "1.9.0"
assert len(acceptance_smoke["summary"]["standings"]) == 10

budget_v2_manifest, budget_v2_digest = load_budget_effect_v2_campaign()
budget_v2_plan = materialize_budget_effect_v2_plan(budget_v2_manifest)
assert budget_v2_digest.startswith("sha256:")
assert len(budget_v2_plan) == budget_v2_manifest["planned_run_count"] == 135
first_budget_v2 = budget_v2_plan[0]
budget_v2_smoke = execute_packaged_racing_catalog_scenario(
    int(first_budget_v2["seed"]), first_budget_v2["scenario_resource"]
)["result"]
assert budget_v2_smoke["scenario"] == {
    "id": "racing.budget-effect-campaign",
    "version": "2.0.0",
}
assert len(budget_v2_smoke["summary"]["standings"]) == 10

early_manifest, early_digest = load_early_allocation_effect_campaign()
early_plan = materialize_early_allocation_effect_plan(early_manifest)
assert early_digest.startswith("sha256:")
assert len(early_plan) == early_manifest["planned_run_count"] == 135
first_early = next(row for row in early_plan if row["treatment"] == "add_aero")
early_smoke = execute_packaged_racing_catalog_scenario(
    int(first_early["seed"]), first_early["scenario_resource"]
)["result"]
assert early_smoke["scenario"] == {
    "id": "racing.early-allocation-effect-campaign",
    "version": "1.0.0",
}
assert len(early_smoke["summary"]["standings"]) == 10

families = {
    family: execute_packaged_racing(42, family)["result"]["configuration_id"]
    for family in ("balanced", "high-downforce", "low-downforce")
}
assert len(set(families.values())) == len(families)

sweep_manifest, sweep_manifest_digest = load_calibration_campaign(
    "racing-circuit-sweep-v1"
)
sweep_plan = materialize_plan(sweep_manifest)
assert sweep_manifest_digest.startswith("sha256:")
assert len(sweep_plan) == sweep_manifest["planned_run_count"] == 105
first_sweep_entry = sweep_plan[0]
first_sweep_result = execute_packaged_racing_scenario(
    int(first_sweep_entry["seed"]), first_sweep_entry["scenario_resource"]
)["result"]
assert (
    first_sweep_result["configuration_id"]
    == first_sweep_entry["expected_configuration_id"]
)
assert (
    first_sweep_result["scenario_digest"]
    == first_sweep_entry["expected_scenario_digest"]
)

candidate_manifest, _ = load_calibration_campaign("racing-aero-candidate-validation-v1")
candidate_plan = materialize_plan(candidate_manifest)
assert len(candidate_plan) == candidate_manifest["planned_run_count"] == 210
review_policy, review_policy_digest = load_candidate_review_policy(
    "racing-aero-candidate-review-v1"
)
assert review_policy["automatic_promotion"] is False
assert review_policy_digest.startswith("sha256:")
for response_id in {entry["response_id"] for entry in candidate_plan}:
    candidate_entry = next(
        entry for entry in candidate_plan if entry["response_id"] == response_id
    )
    candidate_result = execute_packaged_tuning_response(
        int(candidate_entry["seed"]),
        candidate_entry["scenario_resource"],
        candidate_entry["response_resource"],
    )["result"]
    assert (
        candidate_result["scenario_digest"]
        == candidate_entry["expected_scenario_digest"]
    )
    assert (
        candidate_result["tuning_response_digest"]
        == candidate_entry["expected_tuning_response_digest"]
    )
    assert candidate_result["experimental_execution_id"].startswith("sha256:")

v3_tire_manifest, v3_tire_manifest_digest = load_tire_degradation_campaign()
v3_tire_plan = materialize_tire_degradation_plan(v3_tire_manifest)
assert v3_tire_manifest_digest.startswith("sha256:")
assert len(v3_tire_plan) == v3_tire_manifest["planned_run_count"] == 236
first_v3_tire = v3_tire_plan[0]
v3_tire_result = execute_packaged_v3_tire_degradation(first_v3_tire["id"])["result"]
assert v3_tire_result["model"] == v3_tire_manifest["model"]
assert v3_tire_result["scenario_digest"] == first_v3_tire["expected_scenario_digest"]
assert v3_tire_result["profile_digest"] == first_v3_tire["expected_profile_digest"]
assert (
    v3_tire_result["experimental_execution_id"]
    == first_v3_tire["expected_experimental_execution_id"]
)
assert "tire_degradation_diagnostics" in v3_tire_result

driver_campaign_name = "racing-v3-driver-mode-surface-v3"
driver_manifest, driver_manifest_digest = load_driver_control_campaign(
    driver_campaign_name
)
driver_plan = materialize_driver_control_plan(driver_manifest)
driver_preflight = validate_packaged_v3_driver_control_profiles(driver_campaign_name)
assert driver_manifest_digest.startswith("sha256:")
assert len(driver_plan) == driver_manifest["planned_run_count"] == 3168
assert driver_preflight["validated_profile_count"] == 33
first_driver = next(
    row for row in driver_plan if row["expected_local_evidence"] is not None
)
driver_result = execute_packaged_v3_driver_control(
    first_driver["execution_key"], driver_campaign_name
)["result"]
expected_driver = first_driver["expected_local_evidence"]
assert driver_result["experimental_execution_id"] == expected_driver["experimental_execution_id"]
assert driver_result["scenario_digest"] == expected_driver["scenario_digest"]
assert driver_result["profile_digest"] == expected_driver["profile_digest"]
assert driver_result["driver_experiment_digest"] == expected_driver["driver_experiment_digest"]

try:
    execute_packaged_racing(42, "../../arbitrary")
except ValueError:
    pass
else:
    raise AssertionError("an arbitrary scenario path was accepted")

try:
    execute_packaged_racing_scenario(42, "../../arbitrary")
except ValueError:
    pass
else:
    raise AssertionError("an arbitrary packaged scenario path was accepted")

try:
    execute_packaged_racing_catalog_scenario(
        42, "monza-early-42-balanced-one-stop", "../../arbitrary"
    )
except ValueError:
    pass
else:
    raise AssertionError("an arbitrary packaged catalog path was accepted")

try:
    execute_packaged_racing_catalog_scenario(
        42, "monza-early-42-balanced-one-stop", "racing-v9-9-9"
    )
except ValueError:
    pass
else:
    raise AssertionError("a missing packaged catalog was accepted")

try:
    execute_packaged_tuning_response(42, "monza--balanced", "../../arbitrary")
except ValueError:
    pass
else:
    raise AssertionError("an arbitrary tuning response path was accepted")

try:
    execute_packaged_v3_tire_degradation("../../arbitrary")
except ValueError:
    pass
else:
    raise AssertionError("an arbitrary V3 configuration path was accepted")

try:
    execute_packaged_v3_driver_control("../../arbitrary")
except ValueError:
    pass
else:
    raise AssertionError("an arbitrary driver-control execution was accepted")
