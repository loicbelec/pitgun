"""Bounded adapter for executing the packaged Pitgun Racing runner."""

from .budget_effect import (
    load_budget_effect_campaign,
    materialize_budget_effect_plan,
)

from .campaign import (
    load_calibration_campaign,
    load_reference_campaign,
    materialize_plan,
)
from .candidate_review import load_candidate_review_policy, review_candidate_evidence
from .opponent_policy import canonical_json, digest_json, select_reference_policy
from .opponent_audit import (
    load_opponent_audit_campaign,
    materialize_opponent_audit_plan,
)
from .opponent_audit_analysis import (
    diagnose_opponent_audit,
    extract_opponent_audit_evidence,
    summarize_opponent_audit,
)
from .runner import (
    execute_packaged_racing,
    execute_packaged_racing_catalog_scenario,
    execute_packaged_racing_scenario,
    execute_packaged_tuning_response,
    inspect_packaged_runner,
    inspect_packaged_tuning_response_probe,
)
from .strategy_effect import (
    extract_strategy_effect_evidence,
    load_strategy_effect_campaign,
    materialize_strategy_effect_plan,
    summarize_strategy_effect,
)

__all__ = [
    "execute_packaged_racing",
    "execute_packaged_racing_catalog_scenario",
    "execute_packaged_racing_scenario",
    "inspect_packaged_runner",
    "execute_packaged_tuning_response",
    "inspect_packaged_tuning_response_probe",
    "load_reference_campaign",
    "load_calibration_campaign",
    "materialize_plan",
    "load_candidate_review_policy",
    "review_candidate_evidence",
    "canonical_json",
    "digest_json",
    "select_reference_policy",
    "load_opponent_audit_campaign",
    "materialize_opponent_audit_plan",
    "extract_opponent_audit_evidence",
    "summarize_opponent_audit",
    "diagnose_opponent_audit",
    "load_strategy_effect_campaign",
    "materialize_strategy_effect_plan",
    "extract_strategy_effect_evidence",
    "summarize_strategy_effect",
    "load_budget_effect_campaign",
    "materialize_budget_effect_plan",
]
