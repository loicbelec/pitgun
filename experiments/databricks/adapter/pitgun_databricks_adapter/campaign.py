"""Load and validate the immutable reference campaign packaged in the wheel."""

from __future__ import annotations

import hashlib
import importlib.resources
import json
from typing import Any


CAMPAIGNS = frozenset(
    {
        "racing-reference-v1",
        "racing-circuit-sweep-v1",
        "racing-aero-candidate-validation-v1",
    }
)


class CampaignManifestError(ValueError):
    """Raised when the packaged campaign is invalid or has changed in place."""


def load_calibration_campaign(name: str) -> tuple[dict[str, Any], str]:
    """Return one validated, allowlisted manifest and its exact digest."""

    if name not in CAMPAIGNS:
        raise CampaignManifestError("campaign is not packaged or allowlisted")

    package = importlib.resources.files("pitgun_databricks_adapter") / "campaigns"
    manifest_filename = name + ".json"
    checksum_filename = name + ".sha256"
    manifest_bytes = package.joinpath(manifest_filename).read_bytes()
    checksum_parts = package.joinpath(checksum_filename).read_text().split()
    if len(checksum_parts) != 2 or checksum_parts[1] != manifest_filename:
        raise CampaignManifestError("campaign checksum file has an invalid format")

    digest = hashlib.sha256(manifest_bytes).hexdigest()
    if digest != checksum_parts[0]:
        raise CampaignManifestError("packaged campaign does not match its checksum")

    manifest = json.loads(manifest_bytes)
    schema_version = manifest.get("schema_version")
    if schema_version not in {
        "pitgun.calibration-campaign/v1",
        "pitgun.calibration-campaign/v2",
        "pitgun.calibration-campaign/v3",
    }:
        raise CampaignManifestError("unsupported campaign manifest version")

    seeds = manifest.get("seeds", [])
    configurations = (
        manifest.get("configuration_families", [])
        if schema_version == "pitgun.calibration-campaign/v1"
        else manifest.get("configurations", [])
    )
    planned = len(configurations) * len(seeds)
    if planned == 0 or manifest.get("planned_run_count") != planned:
        raise CampaignManifestError("planned run count does not reconcile")
    if len({configuration.get("id") for configuration in configurations}) != len(
        configurations
    ):
        raise CampaignManifestError("configuration identifiers are not unique")
    identity_key = (
        "expected_experimental_configuration_id"
        if schema_version == "pitgun.calibration-campaign/v3"
        else "expected_configuration_id"
    )
    if len(
        {configuration.get(identity_key) for configuration in configurations}
    ) != len(configurations):
        raise CampaignManifestError("expected configuration identities are not unique")
    if len(set(seeds)) != len(seeds):
        raise CampaignManifestError("campaign seeds are not unique")

    if schema_version in {
        "pitgun.calibration-campaign/v2",
        "pitgun.calibration-campaign/v3",
    }:
        circuits = manifest.get("circuits", [])
        circuit_ids = {circuit.get("id") for circuit in circuits}
        if not circuit_ids or len(circuit_ids) != len(circuits):
            raise CampaignManifestError("campaign circuit identifiers are invalid")
        if any(
            configuration.get("circuit_id") not in circuit_ids
            for configuration in configurations
        ):
            raise CampaignManifestError(
                "configuration references an undeclared circuit"
            )

    if schema_version == "pitgun.calibration-campaign/v3":
        responses = manifest.get("responses", [])
        response_ids = {response.get("id") for response in responses}
        if not response_ids or len(response_ids) != len(responses):
            raise CampaignManifestError("campaign response identifiers are invalid")
        if any(
            configuration.get("response_id") not in response_ids
            for configuration in configurations
        ):
            raise CampaignManifestError(
                "configuration references an undeclared response"
            )
        if manifest.get("promotion_policy") != "human-review-required":
            raise CampaignManifestError("experimental campaign cannot auto-promote")

    return manifest, "sha256:" + digest


def load_reference_campaign() -> tuple[dict[str, Any], str]:
    """Compatibility wrapper for the original immutable campaign."""

    return load_calibration_campaign("racing-reference-v1")


def materialize_plan(manifest: dict[str, Any]) -> list[dict[str, Any]]:
    """Materialize the stable Cartesian product in deterministic order."""

    if manifest["schema_version"] in {
        "pitgun.calibration-campaign/v2",
        "pitgun.calibration-campaign/v3",
    }:
        configurations = manifest["configurations"]
    else:
        configurations = [
            {
                **family,
                "configuration_family": family["id"],
                "circuit_id": manifest["circuit_id"],
                "circuit_archetype": "reference",
                "scenario_resource": family["id"],
            }
            for family in manifest["configuration_families"]
        ]

    plan = [
        {
            "configuration_id": configuration["id"],
            "configuration_family": configuration["configuration_family"],
            "circuit_id": configuration["circuit_id"],
            "circuit_archetype": configuration["circuit_archetype"],
            "scenario_resource": configuration["scenario_resource"],
            "expected_configuration_id": configuration.get("expected_configuration_id"),
            "expected_experimental_configuration_id": configuration.get(
                "expected_experimental_configuration_id"
            ),
            "expected_scenario_digest": configuration["expected_scenario_digest"],
            "response_id": configuration.get("response_id"),
            "response_resource": configuration.get("response_resource"),
            "expected_tuning_response_digest": configuration.get(
                "expected_tuning_response_digest"
            ),
            "setup": configuration["setup"],
            "strategy": configuration["strategy"],
            "seed": seed,
        }
        for configuration in configurations
        for seed in manifest["seeds"]
    ]
    return plan
