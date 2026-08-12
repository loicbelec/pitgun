#!/usr/bin/env python3
"""Verify the packaged Linux runner against the published Racing fixture."""

from pitgun_databricks_adapter import (
    execute_packaged_racing,
    execute_packaged_racing_scenario,
    execute_packaged_tuning_response,
    inspect_packaged_runner,
    load_calibration_campaign,
    load_candidate_review_policy,
    load_reference_campaign,
    materialize_plan,
)


EXPECTED_CONFIGURATION_ID = (
    "sha256:12a4207b2c26c814763a2a488054f7421e7cc3836a35e26fc16d96477c8744d7"
)
EXPECTED_RUN_ID = (
    "sha256:89dc458a7460056dd519f5cda74c55c2b2b47f7091f1309ae10d11a2eb46a64a"
)
EXPECTED_RESULT_DIGEST = (
    "sha256:19c045b5ccfb1ad789e8a3d74110efec919694883e05c5da996575e6986dfdef"
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
    execute_packaged_tuning_response(42, "monza--balanced", "../../arbitrary")
except ValueError:
    pass
else:
    raise AssertionError("an arbitrary tuning response path was accepted")
