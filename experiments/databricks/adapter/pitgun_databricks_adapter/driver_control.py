"""Load the immutable Racing V3 driver-control response surface."""

from __future__ import annotations

import functools
import hashlib
import importlib.resources
import json
import re
from typing import Any


CAMPAIGN_NAME = "racing-v3-driver-control-surface-v2"
SCHEMA_VERSION = "pitgun.racing-v3-driver-control-surface-campaign/v1"
MODEL_ID = "pitgun.racing-v3-candidate"
MODEL_VERSION = "0.12.0"
MODE_CAMPAIGN_NAME = "racing-v3-driver-mode-surface-v3"
MODE_SCHEMA_VERSION = "pitgun.racing-v3-driver-control-surface-campaign/v2"
MODE_MODEL_VERSION = "0.13.0"
CAMPAIGN_SPECS = {
    CAMPAIGN_NAME: {
        "schema_version": SCHEMA_VERSION,
        "model": {
            "id": MODEL_ID,
            "version": MODEL_VERSION,
            "digest": "sha256:5e505840e341181bb87af53d5915fd351d085a6b9a940c56c9683718df31741b",
        },
        "predecessor_campaign_id": "racing-v3-driver-control-surface-2026-v1",
    },
    MODE_CAMPAIGN_NAME: {
        "schema_version": MODE_SCHEMA_VERSION,
        "model": {
            "id": MODEL_ID,
            "version": MODE_MODEL_VERSION,
            "digest": "sha256:34c95f6c345d30db12e2d036d1ded1a93fb689db37df579ac61d77b553537a4f",
        },
        "predecessor_campaign_id": "racing-v3-driver-control-surface-2026-v2",
    },
}
EXECUTION_KEY_PATTERN = re.compile(r"v3dc-[0-9a-f]{16}")
SHA256_PATTERN = re.compile(r"sha256:[0-9a-f]{64}")


class DriverControlCampaignError(ValueError):
    """Raised when the packaged campaign is incomplete or changed."""


def _contains_remote_reference(value: object) -> bool:
    if isinstance(value, str):
        return value.startswith(("http://", "https://", "dbfs:/", "s3://"))
    if isinstance(value, dict):
        return any(_contains_remote_reference(item) for item in value.values())
    if isinstance(value, list):
        return any(_contains_remote_reference(item) for item in value)
    return False


@functools.lru_cache(maxsize=len(CAMPAIGN_SPECS))
def load_driver_control_campaign(
    name: str = CAMPAIGN_NAME,
) -> tuple[dict[str, Any], str]:
    spec = CAMPAIGN_SPECS.get(name)
    if spec is None:
        raise DriverControlCampaignError("campaign is not packaged or allowlisted")
    root = importlib.resources.files("pitgun_databricks_adapter") / "campaigns"
    filename = name + ".json"
    payload = root.joinpath(filename).read_bytes()
    checksum = root.joinpath(name + ".sha256").read_text().split()
    digest = hashlib.sha256(payload).hexdigest()
    if len(checksum) != 2 or checksum != [digest, filename]:
        raise DriverControlCampaignError("campaign does not match its checksum")
    manifest = json.loads(payload)
    if manifest.get("schema_version") != spec["schema_version"]:
        raise DriverControlCampaignError("unsupported driver-control campaign")
    if manifest.get("execution_class") != "experimental-v3-driver-control-physics":
        raise DriverControlCampaignError("invalid driver-control execution class")
    if manifest.get("promotion_policy") != "human-review-required":
        raise DriverControlCampaignError("human review is required")
    if manifest.get("automatic_catalog_promotion") is not False:
        raise DriverControlCampaignError("automatic catalog promotion is forbidden")
    predecessor = (
        manifest.get("supersedes", {}).get("campaign_id")
        if name == CAMPAIGN_NAME
        else manifest.get("predecessor_evidence", {}).get("campaign_id")
    )
    if predecessor != spec["predecessor_campaign_id"]:
        raise DriverControlCampaignError("campaign predecessor evidence is incomplete")
    model = manifest.get("model", {})
    if model != spec["model"]:
        raise DriverControlCampaignError("campaign targets an unsupported model")
    if _contains_remote_reference(manifest):
        raise DriverControlCampaignError("remote campaign inputs are forbidden")
    plan = manifest.get("configurations", [])
    profiles = manifest.get("profiles", {})
    scenarios = manifest.get("scenarios", {})
    experiments = manifest.get("driver_experiments", {})
    if not plan or not profiles or not scenarios or not experiments:
        raise DriverControlCampaignError("campaign inputs are incomplete")
    if manifest.get("planned_run_count") != len(plan):
        raise DriverControlCampaignError("planned run count does not reconcile")
    if manifest.get("unique_profile_count") != len(profiles):
        raise DriverControlCampaignError("profile count does not reconcile")
    if manifest.get("unique_scenario_count") != len(scenarios):
        raise DriverControlCampaignError("scenario count does not reconcile")
    if manifest.get("unique_driver_experiment_count") != len(experiments):
        raise DriverControlCampaignError("driver experiment count does not reconcile")
    keys: set[str] = set()
    identities: set[str] = set()
    replay_count = 0
    for row in plan:
        key = str(row.get("execution_key", ""))
        identity = str(row.get("configuration_id", ""))
        if not EXECUTION_KEY_PATTERN.fullmatch(key):
            raise DriverControlCampaignError("invalid driver-control execution key")
        if not SHA256_PATTERN.fullmatch(identity):
            raise DriverControlCampaignError("invalid driver-control identity")
        if key in keys or identity in identities:
            raise DriverControlCampaignError("campaign identities are not unique")
        if row.get("profile_ref") not in profiles:
            raise DriverControlCampaignError("profile reference is missing")
        if row.get("scenario_ref") not in scenarios:
            raise DriverControlCampaignError("scenario reference is missing")
        if row.get("driver_experiment_ref") not in experiments:
            raise DriverControlCampaignError("driver experiment reference is missing")
        keys.add(key)
        identities.add(identity)
        replay_count += row.get("expected_local_evidence") is not None
    if manifest.get("local_replay_run_count") != replay_count:
        raise DriverControlCampaignError("local replay count does not reconcile")
    governance = manifest.get("governance", {})
    if governance.get("local_replay_is_independent_validation"):
        raise DriverControlCampaignError("local replay was mislabeled as validation")
    if not governance.get("complete_702_case_matrix_reserved_for_final_validation"):
        raise DriverControlCampaignError("independent validation is not reserved")
    return manifest, "sha256:" + digest


def materialize_driver_control_plan(manifest: dict[str, Any]) -> list[dict[str, Any]]:
    supported_schemas = {
        spec["schema_version"] for spec in CAMPAIGN_SPECS.values()
    }
    if manifest.get("schema_version") not in supported_schemas:
        raise DriverControlCampaignError("unsupported driver-control campaign")
    return [
        dict(
            row,
            profile=manifest["profiles"][row["profile_ref"]],
            scenario=manifest["scenarios"][row["scenario_ref"]],
            driver_experiment=manifest["driver_experiments"][
                row["driver_experiment_ref"]
            ],
        )
        for row in manifest["configurations"]
    ]


@functools.lru_cache(maxsize=len(CAMPAIGN_SPECS))
def _execution_index(name: str) -> dict[str, dict[str, Any]]:
    manifest, _ = load_driver_control_campaign(name)
    return {
        row["execution_key"]: row for row in materialize_driver_control_plan(manifest)
    }


def load_driver_control_execution(
    execution_key: str, name: str = CAMPAIGN_NAME
) -> dict[str, Any]:
    try:
        return _execution_index(name)[execution_key]
    except KeyError as error:
        raise DriverControlCampaignError(
            "driver-control execution is not packaged or allowlisted"
        ) from error
