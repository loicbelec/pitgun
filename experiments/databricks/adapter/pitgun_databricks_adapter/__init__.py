"""Bounded adapter for executing the packaged Pitgun Racing runner."""

from .campaign import (
    load_calibration_campaign,
    load_reference_campaign,
    materialize_plan,
)
from .opponent_policy import canonical_json, digest_json, select_reference_policy
from .runner import (
    execute_packaged_racing,
    execute_packaged_racing_scenario,
    execute_packaged_tuning_response,
    inspect_packaged_runner,
    inspect_packaged_tuning_response_probe,
)

__all__ = [
    "execute_packaged_racing",
    "execute_packaged_racing_scenario",
    "inspect_packaged_runner",
    "execute_packaged_tuning_response",
    "inspect_packaged_tuning_response_probe",
    "load_reference_campaign",
    "load_calibration_campaign",
    "materialize_plan",
    "canonical_json",
    "digest_json",
    "select_reference_policy",
]
