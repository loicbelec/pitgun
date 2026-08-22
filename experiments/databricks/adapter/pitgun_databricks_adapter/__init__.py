"""Bounded adapter for executing the packaged Pitgun Racing runner."""

from .budget_effect import (
    extract_budget_effect_evidence,
    load_budget_effect_campaign,
    materialize_budget_effect_plan,
    summarize_budget_effect,
)
from .budget_effect_v2 import (
    extract_budget_effect_v2_evidence,
    load_budget_effect_v2_campaign,
    materialize_budget_effect_v2_plan,
    summarize_budget_effect_v2,
)
from .early_allocation_effect import (
    extract_early_allocation_effect_evidence,
    load_early_allocation_effect_campaign,
    materialize_early_allocation_effect_plan,
    summarize_early_allocation_effect,
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
    execute_packaged_v3_tire_degradation,
    execute_packaged_v3_decision_surface,
    execute_packaged_v3_thermal_surface,
    execute_packaged_v3_driver_control,
    inspect_packaged_runner,
    inspect_packaged_tuning_response_probe,
    inspect_packaged_v3_validation_probe,
    inspect_packaged_v3_decision_surface_probe,
    inspect_packaged_v3_driver_control_probe,
)
from .driver_control import (
    load_driver_control_campaign,
    load_driver_control_execution,
    materialize_driver_control_plan,
)
from .decision_surface import (
    load_decision_surface_campaign,
    load_decision_surface_execution,
    materialize_decision_surface_plan,
)
from .tire_degradation import (
    load_tire_degradation_campaign,
    materialize_tire_degradation_plan,
)
from .thermal_surface import (
    load_thermal_surface_campaign,
    load_thermal_surface_execution,
    load_thermal_surface_review,
    materialize_thermal_surface_plan,
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
    "execute_packaged_v3_tire_degradation",
    "execute_packaged_v3_decision_surface",
    "execute_packaged_v3_thermal_surface",
    "execute_packaged_v3_driver_control",
    "inspect_packaged_tuning_response_probe",
    "inspect_packaged_v3_validation_probe",
    "inspect_packaged_v3_decision_surface_probe",
    "inspect_packaged_v3_driver_control_probe",
    "load_driver_control_campaign",
    "load_driver_control_execution",
    "materialize_driver_control_plan",
    "load_decision_surface_campaign",
    "load_decision_surface_execution",
    "materialize_decision_surface_plan",
    "load_tire_degradation_campaign",
    "materialize_tire_degradation_plan",
    "load_thermal_surface_campaign",
    "load_thermal_surface_execution",
    "load_thermal_surface_review",
    "materialize_thermal_surface_plan",
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
    "extract_budget_effect_evidence",
    "summarize_budget_effect",
    "load_budget_effect_v2_campaign",
    "materialize_budget_effect_v2_plan",
    "extract_budget_effect_v2_evidence",
    "summarize_budget_effect_v2",
    "load_early_allocation_effect_campaign",
    "materialize_early_allocation_effect_plan",
    "extract_early_allocation_effect_evidence",
    "summarize_early_allocation_effect",
]
