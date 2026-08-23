#!/usr/bin/env python3
"""Review equal-budget driver archetypes against the frozen V11 shortlist."""

from __future__ import annotations

import argparse
import json
import pathlib
from typing import Any

from review_v11_shortlist import (
    ReviewError,
    canonical_pretty,
    content_digest,
    file_digest,
    pathological,
    winner_counts,
)


EXPERIMENT = pathlib.Path(__file__).parent
ROSTER = EXPERIMENT / "driver-archetypes-equal-budget-v1.json"
SHORTLIST = EXPERIMENT / "shortlist" / "shortlist-v1.json"
DEFAULT_OUTPUT = (
    EXPERIMENT / "results" / "equal-budget-driver-control-review-v1.json"
)
SCHEMA_VERSION = "pitgun.racing-v3-equal-budget-driver-review/v1"


def validate_roster(roster: dict[str, Any]) -> dict[str, float]:
    if roster.get("schema_version") != "pitgun.racing-driver-archetype-screen/v2":
        raise ReviewError("unsupported equal-budget roster schema")
    design = roster["design"]
    expected_budget = float(design["trait_budget"])
    tolerance = float(design["budget_tolerance"])
    budgets = {}
    ids = set()
    for driver in roster["drivers"]:
        driver_id = driver["id"]
        if driver_id in ids:
            raise ReviewError(f"duplicate driver id: {driver_id}")
        ids.add(driver_id)
        traits = driver["traits"]
        if set(traits) != {"limit_exploitation", "consistency", "tire_management"}:
            raise ReviewError(f"unexpected traits for {driver_id}")
        if not all(0.0 <= float(value) <= 1.0 for value in traits.values()):
            raise ReviewError(f"out-of-range trait for {driver_id}")
        budget = sum(float(value) for value in traits.values())
        if abs(budget - expected_budget) > tolerance:
            raise ReviewError(
                f"driver {driver_id} uses budget {budget}, expected {expected_budget}"
            )
        budgets[driver_id] = budget
    if ids != {
        "balanced_reference",
        "limit_specialist",
        "smooth_operator",
        "tire_manager",
    }:
        raise ReviewError("equal-budget roster does not contain the governed archetypes")
    return dict(sorted(budgets.items()))


def summarize(
    entry: dict[str, Any], roster_digest: str
) -> dict[str, Any]:
    parameter_set_id = entry["parameter_set_id"]
    report_path = (
        EXPERIMENT
        / "results"
        / f"equal-budget-driver-control-{parameter_set_id}-v1.json"
    )
    report = json.loads(report_path.read_bytes())
    campaign = report["campaign"]
    if campaign["profile_digest"] != entry["profile_digest"]:
        raise ReviewError(f"profile mismatch for {parameter_set_id}")
    if campaign["driver_archetype_digest"] != roster_digest:
        raise ReviewError(f"roster mismatch for {parameter_set_id}")
    if (
        campaign["configuration_count"] != 702
        or campaign["simulated_lap_count"] != 23790
        or not campaign["complete"]
    ):
        raise ReviewError(f"incomplete equal-budget replay for {parameter_set_id}")

    pathology_count = sum(pathological(row) for row in report["runs"])
    checks = report["analysis"]["checks"]
    driver_context_is_active = checks["no_universal_driver_mode_winner"]["passed"]
    mode_context_is_active = checks["no_universal_mode_winner"]["passed"]
    return {
        "parameter_set_id": parameter_set_id,
        "profile_digest": entry["profile_digest"],
        "report_path": f"results/{report_path.name}",
        "report_sha256": file_digest(report_path),
        "configuration_count": campaign["configuration_count"],
        "simulated_lap_count": campaign["simulated_lap_count"],
        "pathological_execution_count": pathology_count,
        "physical_check_results": {
            key: value["passed"] for key, value in checks.items()
        },
        "contextual_driver_winner": driver_context_is_active,
        "contextual_mode_winner": mode_context_is_active,
        "winner_analysis": winner_counts(report["runs"]),
        "effect_summary": report["analysis"]["effect_summary"],
        "full_gate_passed": all(value["passed"] for value in checks.values())
        and pathology_count == 0,
    }


def review() -> dict[str, Any]:
    roster = json.loads(ROSTER.read_bytes())
    shortlist = json.loads(SHORTLIST.read_bytes())
    budgets = validate_roster(roster)
    roster_digest = content_digest(roster)
    profiles = [summarize(entry, roster_digest) for entry in shortlist["profiles"]]
    contextual_driver_profiles = [
        profile["parameter_set_id"]
        for profile in profiles
        if profile["contextual_driver_winner"]
    ]
    selected = [
        profile["parameter_set_id"]
        for profile in profiles
        if profile["full_gate_passed"]
    ]
    if selected:
        verdict = "HUMAN_REVIEW_REQUIRED"
        recommendation = (
            "At least one frozen profile passed with the equal-budget roster. "
            "Confirm the exact evidence on Databricks before human selection."
        )
    elif contextual_driver_profiles:
        verdict = "MODE_RESPONSE_REFINEMENT_REQUIRED"
        recommendation = (
            "Retain the equal-budget roster as the next experimental baseline. "
            "It removes the universal driver for halton-19 and halton-27, but "
            "attack still wins every global context. Explore only mode commitment "
            "and correction-cost response next; do not retune driver traits."
        )
    else:
        verdict = "DRIVER_RESPONSE_REFINEMENT_REQUIRED"
        recommendation = (
            "Equal trait budgets did not remove the universal driver. Change the "
            "trait-to-physics response before any further coefficient campaign."
        )
    return {
        "schema_version": SCHEMA_VERSION,
        "model": shortlist["model"],
        "roster_digest": roster_digest,
        "trait_budget": roster["design"]["trait_budget"],
        "driver_trait_budgets": budgets,
        "shortlist_manifest_digest": content_digest(shortlist),
        "profiles": profiles,
        "contextual_driver_profile_ids": contextual_driver_profiles,
        "selected_profile_ids": selected,
        "verdict": verdict,
        "recommendation": recommendation,
        "automatic_publication_performed": False,
    }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=pathlib.Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--check", action="store_true")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    encoded = canonical_pretty(review())
    if args.check:
        if not args.output.is_file() or args.output.read_bytes() != encoded:
            raise ReviewError(f"{args.output} is missing or not reproducible")
        print(f"reproduced {args.output}")
        return 0
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_bytes(encoded)
    print(f"wrote {args.output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
