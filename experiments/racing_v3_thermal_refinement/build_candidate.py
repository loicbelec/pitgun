#!/usr/bin/env python3
"""Build the reviewed family-specific thermal profile candidate."""

from __future__ import annotations

import hashlib
import json
import pathlib
from typing import Any


ROOT = pathlib.Path(__file__).resolve().parents[2]
CAMPAIGN = (
    ROOT
    / "experiments"
    / "databricks"
    / "campaigns"
    / "racing-v3-thermal-refinement-validation-v2.json"
)
REVIEW = (
    ROOT
    / "experiments"
    / "databricks"
    / "reviews"
    / "racing-v3-thermal-refinement-validation-review-v1.json"
)
OUTPUT_ROOT = ROOT / "experiments" / "racing_v3_thermal_refinement" / "candidates"
OUTPUT = OUTPUT_ROOT / "thermal-family-profile-v1.json"
CHECKSUM = OUTPUT_ROOT / "thermal-family-profile-v1.sha256"
SCHEMA_VERSION = "pitgun.racing-v3-thermal-family-profile-candidate/v1"
CANDIDATE_ID = "pitgun.racing-v3-thermal-family-profile"
CANDIDATE_VERSION = "1.0.0-rc.1"
EXPECTED_FAMILIES = frozenset({"historical_v8", "modern_v6t", "f1_2026"})


class CandidateBuildError(ValueError):
    """Raised when reviewed evidence cannot authorize the candidate."""


def sha256(payload: bytes) -> str:
    return "sha256:" + hashlib.sha256(payload).hexdigest()


def read_json(path: pathlib.Path) -> tuple[dict[str, Any], bytes]:
    payload = path.read_bytes()
    return json.loads(payload), payload


def build_candidate(
    campaign_path: pathlib.Path = CAMPAIGN,
    review_path: pathlib.Path = REVIEW,
) -> dict[str, Any]:
    campaign, campaign_bytes = read_json(campaign_path)
    review, review_bytes = read_json(review_path)
    campaign_digest = sha256(campaign_bytes)
    review_digest = sha256(review_bytes)

    if review.get("campaign_id") != campaign.get("campaign_id"):
        raise CandidateBuildError("review and campaign identities differ")
    if review.get("manifest_digest") != campaign_digest:
        raise CandidateBuildError("review does not pin the campaign payload")
    if review.get("automatic_catalog_promotion") is not False:
        raise CandidateBuildError("review cannot authorize automatic promotion")

    verdicts = review.get("per_family_verdicts", {})
    if set(verdicts) != EXPECTED_FAMILIES:
        raise CandidateBuildError("reviewed thermal families are incomplete")
    if any(row.get("verdict") != "PASS" for row in verdicts.values()):
        raise CandidateBuildError("only all-PASS evidence may create a candidate")

    parameter_sets = {
        row["parameter_set_id"]: row for row in campaign["parameter_sets"]
    }
    profile_ids = campaign["dimensions"]["profiles_by_family"]
    if set(profile_ids) != EXPECTED_FAMILIES:
        raise CandidateBuildError("campaign family profiles are incomplete")

    bindings: dict[str, dict[str, list[Any]]] = {}
    for family in sorted(EXPECTED_FAMILIES):
        rows = [
            row
            for row in campaign["configurations"]
            if row["vehicle_family"] == family
        ]
        bindings[family] = {
            "vehicle_ids": sorted({row["vehicle_id"] for row in rows}),
            "eras": sorted({row["era"] for row in rows}),
        }

    profiles = {}
    for family in sorted(EXPECTED_FAMILIES):
        parameter_set_id = profile_ids[family]
        verdict_parameter_set_id = verdicts[family]["candidate_parameter_set_id"]
        if parameter_set_id != verdict_parameter_set_id:
            raise CandidateBuildError(
                f"reviewed parameter set differs for family {family!r}"
            )
        parameter_set = parameter_sets[parameter_set_id]
        profile_ref = parameter_set["profile_ref"]
        profile = campaign["profiles"][profile_ref]
        profiles[family] = {
            "parameter_set_id": parameter_set_id,
            "validated_profile_ref": profile_ref,
            "bindings": bindings[family],
            "engine_thermal_resolution": profile["engine_thermal_resolution"],
        }

    return {
        "schema_version": SCHEMA_VERSION,
        "id": CANDIDATE_ID,
        "version": CANDIDATE_VERSION,
        "status": "REVIEWED_CANDIDATE",
        "model": campaign["model"],
        "source_evidence": {
            "campaign_id": campaign["campaign_id"],
            "campaign_digest": campaign_digest,
            "review_id": review["id"],
            "review_digest": review_digest,
            "databricks_job_id": review["databricks"]["job_id"],
            "databricks_run_id": review["databricks"]["run_id"],
            "read_only_review_run_id": "206668022732405",
            "evidence_versions": review["evidence_versions"],
        },
        "resolution_contract": {
            "resolver_owner": "pitgun-racing-simulator",
            "solver_input": "one resolved engine_thermal_resolution per execution",
            "selection_key": "vehicle_id",
            "unknown_vehicle_behavior": "reject",
            "era_only_selection_forbidden": True,
        },
        "profiles": profiles,
        "excluded_from_candidate": {
            "experimental_fuel_reservoir_kg": review["fuel_control"][
                "experimental_reservoir_kg"
            ],
            "reason": "Fuel was a validation nuisance-control input and is owned by the staged energy contract.",
            "owner_issue": 246,
        },
        "promotion": {
            "candidate_creation_authorized": True,
            "rust_wasm_integration_authorized": True,
            "catalog_publication_authorized": False,
            "authority_verifier_promotion_authorized": False,
            "game_staging_promotion_authorized": False,
            "production_promotion_authorized": False,
            "automatic_promotion": False,
        },
    }


def main() -> None:
    candidate = build_candidate()
    payload = (json.dumps(candidate, indent=2) + "\n").encode()
    OUTPUT_ROOT.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_bytes(payload)
    CHECKSUM.write_text(f"{hashlib.sha256(payload).hexdigest()}  {OUTPUT.name}\n")
    print(
        json.dumps(
            {
                "id": candidate["id"],
                "version": candidate["version"],
                "digest": sha256(payload),
                "families": sorted(candidate["profiles"]),
            },
            sort_keys=True,
        )
    )


if __name__ == "__main__":
    main()
