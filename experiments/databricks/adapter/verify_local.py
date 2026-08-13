#!/usr/bin/env python3
"""Verify the packaged Linux runner against the published Racing fixture."""

from pitgun_databricks_adapter import (
    execute_packaged_racing,
    execute_packaged_racing_catalog_scenario,
    execute_packaged_racing_scenario,
    execute_packaged_tuning_response,
    inspect_packaged_runner,
    load_calibration_campaign,
    load_candidate_review_policy,
    load_opponent_audit_campaign,
    load_reference_campaign,
    materialize_opponent_audit_plan,
    materialize_plan,
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
