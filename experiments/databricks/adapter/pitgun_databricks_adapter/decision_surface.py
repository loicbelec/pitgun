"""Load the immutable Racing V3 multi-era decision-surface campaign."""

from __future__ import annotations

import functools
import hashlib
import importlib.resources
import json
import re
from typing import Any


CAMPAIGNS = frozenset({"racing-v3-decision-surface-v1"})
SCHEMA_VERSION = "pitgun.racing-v3-decision-surface-campaign/v1"
MODEL_ID = "pitgun.racing-v3-candidate"
MODEL_VERSION = "0.9.0"
SEEDS = frozenset({"7", "42"})
SHA256_PATTERN = re.compile(r"sha256:[0-9a-f]{64}")
EXECUTION_KEY_PATTERN = re.compile(r"v3ds-[0-9]{4}-[0-9]{6}")


class DecisionSurfaceCampaignError(ValueError):
    """Raised when the packaged decision-surface campaign is invalid."""


def canonical_pretty(value: object) -> bytes:
    """Use the exact JSON representation used by the accepted local audit."""

    return (json.dumps(value, indent=2, ensure_ascii=False) + "\n").encode()


def sha256(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


def _contains_remote_reference(value: object) -> bool:
    if isinstance(value, str):
        return value.startswith(("http://", "https://", "dbfs:/", "s3://"))
    if isinstance(value, dict):
        return any(_contains_remote_reference(item) for item in value.values())
    if isinstance(value, list):
        return any(_contains_remote_reference(item) for item in value)
    return False


@functools.lru_cache(maxsize=1)
def load_decision_surface_campaign(
    name: str = "racing-v3-decision-surface-v1",
) -> tuple[dict[str, Any], str]:
    """Return the checksummed explicit campaign embedded in the wheel."""

    if name not in CAMPAIGNS:
        raise DecisionSurfaceCampaignError("campaign is not packaged or allowlisted")
    package = importlib.resources.files("pitgun_databricks_adapter") / "campaigns"
    filename = name + ".json"
    checksum_parts = package.joinpath(name + ".sha256").read_text().split()
    payload = package.joinpath(filename).read_bytes()
    if len(checksum_parts) != 2 or checksum_parts[1] != filename:
        raise DecisionSurfaceCampaignError("campaign checksum has an invalid format")
    digest = hashlib.sha256(payload).hexdigest()
    if digest != checksum_parts[0]:
        raise DecisionSurfaceCampaignError("campaign does not match its checksum")

    manifest = json.loads(payload)
    if manifest.get("schema_version") != SCHEMA_VERSION:
        raise DecisionSurfaceCampaignError("unsupported decision-surface campaign")
    if manifest.get("execution_class") != "experimental-v3-physics":
        raise DecisionSurfaceCampaignError("campaign has an invalid execution class")
    if manifest.get("promotion_policy") != "human-review-required":
        raise DecisionSurfaceCampaignError("campaign cannot auto-promote")
    if manifest.get("automatic_catalog_promotion") is not False:
        raise DecisionSurfaceCampaignError("automatic catalog promotion is forbidden")

    model = manifest.get("model", {})
    if model.get("id") != MODEL_ID or model.get("version") != MODEL_VERSION:
        raise DecisionSurfaceCampaignError("campaign targets an unsupported model")
    for identity in (model, manifest.get("local_evidence", {}), manifest.get("runner", {})):
        for key, value in identity.items():
            if key.endswith("digest") and not SHA256_PATTERN.fullmatch(str(value)):
                raise DecisionSurfaceCampaignError(f"invalid identity digest: {key}")

    profiles = manifest.get("profiles", {})
    scenarios = manifest.get("scenarios", {})
    configurations = manifest.get("configurations", [])
    if not profiles or not scenarios or not configurations:
        raise DecisionSurfaceCampaignError("campaign inputs are incomplete")
    if manifest.get("planned_run_count") != len(configurations):
        raise DecisionSurfaceCampaignError("planned run count does not reconcile")
    if manifest.get("unique_configuration_count") != len(
        {row.get("configuration_id") for row in configurations}
    ):
        raise DecisionSurfaceCampaignError("configuration count does not reconcile")
    if manifest.get("unique_scenario_count") != len(scenarios):
        raise DecisionSurfaceCampaignError("scenario count does not reconcile")
    if _contains_remote_reference(manifest):
        raise DecisionSurfaceCampaignError("remote campaign inputs are forbidden")

    execution_keys = []
    natural_keys = []
    for row in configurations:
        execution_key = str(row.get("execution_key", ""))
        if not EXECUTION_KEY_PATTERN.fullmatch(execution_key):
            raise DecisionSurfaceCampaignError("invalid execution key")
        execution_keys.append(execution_key)
        seed = str(row.get("seed", ""))
        if seed not in SEEDS:
            raise DecisionSurfaceCampaignError("seed is outside the reviewed plan")
        scenario_ref = row.get("scenario_ref")
        profile_ref = row.get("profile_ref")
        if scenario_ref not in scenarios or profile_ref not in profiles:
            raise DecisionSurfaceCampaignError("configuration input reference is missing")
        for key in (
            "configuration_id",
            "expected_experimental_execution_id",
            "expected_probe_result_digest",
            "expected_compact_point_digest",
        ):
            if not SHA256_PATTERN.fullmatch(str(row.get(key, ""))):
                raise DecisionSurfaceCampaignError(f"configuration {key} is invalid")
        natural_keys.append((row["configuration_id"], seed))

    if len(set(execution_keys)) != len(execution_keys):
        raise DecisionSurfaceCampaignError("execution keys are not unique")
    if len(set(natural_keys)) != len(natural_keys):
        raise DecisionSurfaceCampaignError("configuration/seed keys are not unique")
    if {row["seed"] for row in configurations} != SEEDS:
        raise DecisionSurfaceCampaignError("campaign does not contain both reviewed seeds")
    return manifest, "sha256:" + digest


def materialize_decision_surface_plan(
    manifest: dict[str, Any],
) -> list[dict[str, Any]]:
    """Resolve deduplicated inputs into an exact stable execution plan."""

    if manifest.get("schema_version") != SCHEMA_VERSION:
        raise DecisionSurfaceCampaignError("unsupported decision-surface campaign")
    profiles = manifest["profiles"]
    scenarios = manifest["scenarios"]
    return [
        dict(row, scenario=scenarios[row["scenario_ref"]], profile=profiles[row["profile_ref"]])
        for row in manifest["configurations"]
    ]
