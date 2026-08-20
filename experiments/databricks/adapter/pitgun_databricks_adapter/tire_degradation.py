"""Load the immutable Racing V3 tire-degradation campaign."""

from __future__ import annotations

import functools
import hashlib
import importlib.resources
import json
import re
from typing import Any


CAMPAIGNS = frozenset({"racing-v3-tire-degradation-v1"})
SCHEMA_VERSION = "pitgun.racing-v3-physics-campaign/v1"
MODEL_ID = "pitgun.racing-v3-candidate"
MODEL_VERSION = "0.9.0"
SHA256_PATTERN = re.compile(r"sha256:[0-9a-f]{64}")


class TireDegradationCampaignError(ValueError):
    """Raised when the packaged V3 campaign is invalid or changed in place."""


def _canonical_digest(value: object) -> str:
    payload = json.dumps(value, sort_keys=True, separators=(",", ":")).encode()
    return "sha256:" + hashlib.sha256(payload).hexdigest()


def _contains_remote_reference(value: object) -> bool:
    if isinstance(value, str):
        return value.startswith(("http://", "https://", "dbfs:/", "s3://"))
    if isinstance(value, dict):
        return any(_contains_remote_reference(item) for item in value.values())
    if isinstance(value, list):
        return any(_contains_remote_reference(item) for item in value)
    return False


@functools.lru_cache(maxsize=1)
def load_tire_degradation_campaign(
    name: str = "racing-v3-tire-degradation-v1",
) -> tuple[dict[str, Any], str]:
    """Return one checksummed, bounded V3 campaign embedded in the wheel."""

    if name not in CAMPAIGNS:
        raise TireDegradationCampaignError("campaign is not packaged or allowlisted")

    package = importlib.resources.files("pitgun_databricks_adapter") / "campaigns"
    manifest_filename = name + ".json"
    checksum_parts = package.joinpath(name + ".sha256").read_text().split()
    manifest_bytes = package.joinpath(manifest_filename).read_bytes()
    if len(checksum_parts) != 2 or checksum_parts[1] != manifest_filename:
        raise TireDegradationCampaignError("campaign checksum has an invalid format")
    digest = hashlib.sha256(manifest_bytes).hexdigest()
    if digest != checksum_parts[0]:
        raise TireDegradationCampaignError("campaign does not match its checksum")

    manifest = json.loads(manifest_bytes)
    if manifest.get("schema_version") != SCHEMA_VERSION:
        raise TireDegradationCampaignError("unsupported tire campaign version")
    if manifest.get("execution_class") != "experimental-v3-physics":
        raise TireDegradationCampaignError("campaign has an invalid execution class")
    if manifest.get("promotion_policy") != "human-review-required":
        raise TireDegradationCampaignError("physics campaign cannot auto-promote")
    if manifest.get("automatic_catalog_promotion") is not False:
        raise TireDegradationCampaignError("automatic catalog promotion is forbidden")

    model = manifest.get("model", {})
    if model.get("id") != MODEL_ID or model.get("version") != MODEL_VERSION:
        raise TireDegradationCampaignError("campaign targets an unsupported model")
    if not SHA256_PATTERN.fullmatch(str(model.get("digest", ""))):
        raise TireDegradationCampaignError("campaign model digest is invalid")

    configurations = manifest.get("configurations", [])
    if not configurations or manifest.get("planned_run_count") != len(configurations):
        raise TireDegradationCampaignError("planned run count does not reconcile")
    for key in (
        "id",
        "expected_experimental_configuration_id",
    ):
        values = [configuration.get(key) for configuration in configurations]
        if len(set(values)) != len(values):
            raise TireDegradationCampaignError(f"configuration {key} values are not unique")
    physical_execution_count = len(
        {
            configuration.get("expected_experimental_execution_id")
            for configuration in configurations
        }
    )
    if manifest.get("unique_physical_execution_count") != physical_execution_count:
        raise TireDegradationCampaignError(
            "unique physical execution count does not reconcile"
        )

    declared_circuits = {row.get("id") for row in manifest.get("circuits", [])}
    declared_vehicles = {row.get("id") for row in manifest.get("vehicles", [])}
    if None in declared_circuits or None in declared_vehicles:
        raise TireDegradationCampaignError("campaign dimensions are invalid")

    for configuration in configurations:
        if configuration.get("circuit_id") not in declared_circuits:
            raise TireDegradationCampaignError("configuration circuit is undeclared")
        if configuration.get("vehicle_id") not in declared_vehicles:
            raise TireDegradationCampaignError("configuration vehicle is undeclared")
        if configuration.get("seed") != "42":
            raise TireDegradationCampaignError("campaign seed is outside the reviewed plan")
        for key in (
            "expected_scenario_digest",
            "expected_profile_digest",
            "expected_experimental_configuration_id",
            "expected_experimental_execution_id",
        ):
            if not SHA256_PATTERN.fullmatch(str(configuration.get(key, ""))):
                raise TireDegradationCampaignError(f"configuration {key} is invalid")
        expected_configuration_id = _canonical_digest(
            {
                "analysis_role": configuration["id"],
                "profile_digest": configuration["expected_profile_digest"],
                "scenario_digest": configuration["expected_scenario_digest"],
            }
        )
        if (
            configuration["expected_experimental_configuration_id"]
            != expected_configuration_id
        ):
            raise TireDegradationCampaignError("configuration identity does not reconcile")
        if not isinstance(configuration.get("scenario"), dict) or not isinstance(
            configuration.get("profile"), dict
        ):
            raise TireDegradationCampaignError("configuration inputs are not embedded JSON")
        if _contains_remote_reference(configuration):
            raise TireDegradationCampaignError("remote campaign inputs are forbidden")

    return manifest, "sha256:" + digest


def materialize_tire_degradation_plan(
    manifest: dict[str, Any],
) -> list[dict[str, Any]]:
    """Return the exact explicit plan in stable manifest order."""

    if manifest.get("schema_version") != SCHEMA_VERSION:
        raise TireDegradationCampaignError("unsupported tire campaign version")
    return [dict(configuration) for configuration in manifest["configurations"]]
