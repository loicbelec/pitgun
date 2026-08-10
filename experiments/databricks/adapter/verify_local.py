#!/usr/bin/env python3
"""Verify the packaged Linux runner against the published Racing fixture."""

from pitgun_databricks_adapter import execute_packaged_racing


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
assert result["result"]["configuration_id"] == EXPECTED_CONFIGURATION_ID
assert result["result"]["run_id"] == EXPECTED_RUN_ID
assert result["canonical_result_digest"] == EXPECTED_RESULT_DIGEST
assert result["host"] == {"machine": "aarch64", "system": "Linux"}
assert result["adapter"]["version"].startswith("0.1.0a1+g")
print(
    "local packaged runner verified: "
    f"adapter={result['adapter']['version']} "
    f"{result['canonical_result_digest']} "
    f"startup={result['measurements']['startup_probe_duration_ms']}ms "
    f"execution={result['measurements']['execution_duration_ms']}ms"
)
