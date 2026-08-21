"""Load the immutable Racing V3 thermal-adequacy campaign."""

from __future__ import annotations

import functools
import hashlib
import importlib.resources
import json
import re
from typing import Any


CAMPAIGN_NAME = "racing-v3-thermal-adequacy-v1"
REFINEMENT_VALIDATION_CAMPAIGN_NAME = "racing-v3-thermal-refinement-validation-v1"
CAMPAIGN_NAMES = frozenset({CAMPAIGN_NAME, REFINEMENT_VALIDATION_CAMPAIGN_NAME})
SCHEMA_VERSION = "pitgun.racing-v3-thermal-adequacy-campaign/v1"
REVIEW_NAME = "racing-v3-thermal-adequacy-review-v1"
REVIEW_SCHEMA_VERSION = "pitgun.racing-v3-thermal-adequacy-review/v1"
MODEL_ID = "pitgun.racing-v3-candidate"
MODEL_VERSION = "0.10.0"
SHA256_PATTERN = re.compile(r"sha256:[0-9a-f]{64}")
EXECUTION_KEY_PATTERN = re.compile(r"v3th-[0-9a-f]{16}")


class ThermalSurfaceCampaignError(ValueError):
    """Raised when the packaged thermal campaign is incomplete or changed."""


def load_thermal_surface_review(
    name: str = REVIEW_NAME,
) -> tuple[dict[str, Any], str]:
    """Load the immutable human review without granting a promotion path."""

    if name != REVIEW_NAME:
        raise ThermalSurfaceCampaignError(
            "thermal review is not packaged or allowlisted"
        )
    resource = (
        importlib.resources.files("pitgun_databricks_adapter")
        / "reviews"
        / f"{name}.json"
    )
    payload = resource.read_bytes()
    review = json.loads(payload)
    if review.get("schema_version") != REVIEW_SCHEMA_VERSION:
        raise ThermalSurfaceCampaignError("unsupported thermal review")
    if review.get("automatic_catalog_promotion") is not False:
        raise ThermalSurfaceCampaignError(
            "thermal review cannot enable automatic promotion"
        )
    if (
        review.get("next_gate", {}).get("automatic_catalog_promotion")
        is not False
    ):
        raise ThermalSurfaceCampaignError(
            "thermal refinement gate cannot enable automatic promotion"
        )
    return review, "sha256:" + hashlib.sha256(payload).hexdigest()


def _contains_remote_reference(value: object) -> bool:
    if isinstance(value, str):
        return value.startswith(("http://", "https://", "dbfs:/", "s3://"))
    if isinstance(value, dict):
        return any(_contains_remote_reference(item) for item in value.values())
    if isinstance(value, list):
        return any(_contains_remote_reference(item) for item in value)
    return False


@functools.lru_cache(maxsize=len(CAMPAIGN_NAMES))
def load_thermal_surface_campaign(
    name: str = CAMPAIGN_NAME,
) -> tuple[dict[str, Any], str]:
    """Return the exact checksummed campaign embedded in the adapter wheel."""

    if name not in CAMPAIGN_NAMES:
        raise ThermalSurfaceCampaignError("campaign is not packaged or allowlisted")
    root = importlib.resources.files("pitgun_databricks_adapter") / "campaigns"
    filename = name + ".json"
    payload = root.joinpath(filename).read_bytes()
    checksum = root.joinpath(name + ".sha256").read_text().split()
    digest = hashlib.sha256(payload).hexdigest()
    if len(checksum) != 2 or checksum[1] != filename or checksum[0] != digest:
        raise ThermalSurfaceCampaignError("campaign does not match its checksum")

    manifest = json.loads(payload)
    if manifest.get("schema_version") != SCHEMA_VERSION:
        raise ThermalSurfaceCampaignError("unsupported thermal campaign")
    if manifest.get("execution_class") != "experimental-v3-thermal-physics":
        raise ThermalSurfaceCampaignError("invalid thermal execution class")
    if manifest.get("promotion_policy") != "human-review-required":
        raise ThermalSurfaceCampaignError("thermal campaign cannot auto-promote")
    if manifest.get("automatic_catalog_promotion") is not False:
        raise ThermalSurfaceCampaignError("automatic catalog promotion is forbidden")
    if manifest.get("model", {}).get("id") != MODEL_ID or manifest.get(
        "model", {}
    ).get("version") != MODEL_VERSION:
        raise ThermalSurfaceCampaignError("campaign targets an unsupported model")
    if _contains_remote_reference(manifest):
        raise ThermalSurfaceCampaignError("remote thermal inputs are forbidden")

    profiles = manifest.get("profiles", {})
    scenarios = manifest.get("scenarios", {})
    configurations = manifest.get("configurations", [])
    if not profiles or not scenarios or not configurations:
        raise ThermalSurfaceCampaignError("thermal campaign inputs are incomplete")
    if manifest.get("planned_run_count") != len(configurations):
        raise ThermalSurfaceCampaignError("planned run count does not reconcile")
    if manifest.get("unique_profile_count") != len(profiles):
        raise ThermalSurfaceCampaignError("profile count does not reconcile")
    if manifest.get("unique_scenario_count") != len(scenarios):
        raise ThermalSurfaceCampaignError("scenario count does not reconcile")

    execution_keys = set()
    configuration_ids = set()
    local_replay_count = 0
    for row in configurations:
        if not EXECUTION_KEY_PATTERN.fullmatch(str(row.get("execution_key", ""))):
            raise ThermalSurfaceCampaignError("invalid thermal execution key")
        if not SHA256_PATTERN.fullmatch(str(row.get("configuration_id", ""))):
            raise ThermalSurfaceCampaignError("invalid thermal configuration identity")
        if row.get("profile_ref") not in profiles or row.get("scenario_ref") not in scenarios:
            raise ThermalSurfaceCampaignError("thermal input reference is missing")
        if row["execution_key"] in execution_keys:
            raise ThermalSurfaceCampaignError("thermal execution keys are not unique")
        if row["configuration_id"] in configuration_ids:
            raise ThermalSurfaceCampaignError("thermal configurations are not unique")
        execution_keys.add(row["execution_key"])
        configuration_ids.add(row["configuration_id"])
        expected = row.get("expected_local_evidence")
        if expected is not None:
            local_replay_count += 1
            for key in ("experimental_execution_id", "scenario_digest", "profile_digest"):
                if not SHA256_PATTERN.fullmatch(str(expected.get(key, ""))):
                    raise ThermalSurfaceCampaignError(
                        f"local replay {key} is not content-addressed"
                    )
            if len(expected.get("metrics", {})) != 7:
                raise ThermalSurfaceCampaignError("local replay metrics are incomplete")

    if manifest.get("local_replay_run_count") != local_replay_count:
        raise ThermalSurfaceCampaignError("local replay count does not reconcile")
    if manifest.get("new_evidence_run_count") != len(configurations) - local_replay_count:
        raise ThermalSurfaceCampaignError("new evidence count does not reconcile")
    if manifest.get("governance", {}).get("local_replay_is_independent_validation"):
        raise ThermalSurfaceCampaignError("local selection evidence was mislabeled")
    if not manifest.get("governance", {}).get(
        "final_validation_inputs_reserved_during_local_selection"
    ):
        raise ThermalSurfaceCampaignError("independent validation is not reserved")
    return manifest, "sha256:" + digest


def materialize_thermal_surface_plan(
    manifest: dict[str, Any],
) -> list[dict[str, Any]]:
    """Resolve deduplicated profiles and scenarios into an immutable plan."""

    if manifest.get("schema_version") != SCHEMA_VERSION:
        raise ThermalSurfaceCampaignError("unsupported thermal campaign")
    return [
        dict(
            row,
            profile=manifest["profiles"][row["profile_ref"]],
            scenario=manifest["scenarios"][row["scenario_ref"]],
        )
        for row in manifest["configurations"]
    ]


@functools.lru_cache(maxsize=len(CAMPAIGN_NAMES))
def _execution_index(name: str) -> dict[str, dict[str, Any]]:
    manifest, _ = load_thermal_surface_campaign(name)
    return {
        row["execution_key"]: row for row in materialize_thermal_surface_plan(manifest)
    }


def load_thermal_surface_execution(
    execution_key: str, name: str = CAMPAIGN_NAME
) -> dict[str, Any]:
    try:
        return _execution_index(name)[execution_key]
    except KeyError as error:
        raise ThermalSurfaceCampaignError(
            "thermal execution is not packaged or allowlisted"
        ) from error
