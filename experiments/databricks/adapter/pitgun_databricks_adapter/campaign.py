"""Load and validate the immutable reference campaign packaged in the wheel."""

from __future__ import annotations

import hashlib
import importlib.resources
import json
from typing import Any


CAMPAIGN_FILENAME = "racing-reference-v1.json"
CHECKSUM_FILENAME = "racing-reference-v1.sha256"


class CampaignManifestError(ValueError):
    """Raised when the packaged campaign is invalid or has changed in place."""


def load_reference_campaign() -> tuple[dict[str, Any], str]:
    """Return the validated manifest and the digest of its exact packaged bytes."""

    package = importlib.resources.files("pitgun_databricks_adapter") / "campaigns"
    manifest_bytes = package.joinpath(CAMPAIGN_FILENAME).read_bytes()
    checksum_parts = package.joinpath(CHECKSUM_FILENAME).read_text().split()
    if len(checksum_parts) != 2 or checksum_parts[1] != CAMPAIGN_FILENAME:
        raise CampaignManifestError("campaign checksum file has an invalid format")

    digest = hashlib.sha256(manifest_bytes).hexdigest()
    if digest != checksum_parts[0]:
        raise CampaignManifestError("packaged campaign does not match its checksum")

    manifest = json.loads(manifest_bytes)
    if manifest.get("schema_version") != "pitgun.calibration-campaign/v1":
        raise CampaignManifestError("unsupported campaign manifest version")

    families = manifest.get("configuration_families", [])
    seeds = manifest.get("seeds", [])
    planned = len(families) * len(seeds)
    if planned == 0 or manifest.get("planned_run_count") != planned:
        raise CampaignManifestError("planned run count does not reconcile")
    if len({family.get("id") for family in families}) != len(families):
        raise CampaignManifestError("configuration family identifiers are not unique")
    if len(set(seeds)) != len(seeds):
        raise CampaignManifestError("campaign seeds are not unique")

    return manifest, "sha256:" + digest


def materialize_plan(manifest: dict[str, Any]) -> list[dict[str, Any]]:
    """Materialize the stable Cartesian product in deterministic order."""

    return [
        {
            "configuration_family": family["id"],
            "expected_configuration_id": family["expected_configuration_id"],
            "expected_scenario_digest": family["expected_scenario_digest"],
            "setup": family["setup"],
            "strategy": family["strategy"],
            "seed": seed,
        }
        for family in manifest["configuration_families"]
        for seed in manifest["seeds"]
    ]
