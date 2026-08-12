"""Bounded adapter for executing the packaged Pitgun Racing runner."""

from .campaign import load_calibration_campaign, load_reference_campaign, materialize_plan
from .opponent_policy import canonical_json, digest_json, select_reference_policy
from .runner import (
    execute_packaged_racing,
    execute_packaged_racing_scenario,
    inspect_packaged_runner,
)

__all__ = [
    "execute_packaged_racing",
    "execute_packaged_racing_scenario",
    "inspect_packaged_runner",
    "load_reference_campaign",
    "load_calibration_campaign",
    "materialize_plan",
    "canonical_json",
    "digest_json",
    "select_reference_policy",
]
