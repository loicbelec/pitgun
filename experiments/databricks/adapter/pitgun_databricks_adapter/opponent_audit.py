"""Load the immutable explicit Racing V2 opponent-audit campaign."""

from __future__ import annotations

import hashlib
import importlib.resources
import json
import re
from typing import Any


CAMPAIGN_NAME = "racing-opponent-audit-v1"
SCHEMA_VERSION = "pitgun.opponent-audit-campaign/v1"
RESOURCE_PATTERN = re.compile(
    r"[a-z0-9]+(?:-[a-z0-9]+)*(?:--[a-z0-9]+(?:-[a-z0-9]+)*)?"
)


class OpponentAuditManifestError(ValueError):
    """Raised when the opponent-audit plan or one packaged resource changed."""


def _sha256(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


def _validate_manifest(
    manifest: dict[str, Any], resource_digests: dict[str, str]
) -> None:
    if manifest.get("schema_version") != SCHEMA_VERSION:
        raise OpponentAuditManifestError("unsupported opponent audit campaign")
    runs = manifest.get("runs", [])
    if manifest.get("planned_run_count") != 180 or len(runs) != 180:
        raise OpponentAuditManifestError("opponent audit plan must contain 180 runs")

    run_keys = [run.get("run_key") for run in runs]
    resources = [run.get("scenario_resource") for run in runs]
    if len(set(run_keys)) != len(runs) or len(set(resources)) != len(runs):
        raise OpponentAuditManifestError("opponent audit run identities are not unique")
    if any(
        not isinstance(resource, str) or not RESOURCE_PATTERN.fullmatch(resource)
        for resource in resources
    ):
        raise OpponentAuditManifestError("opponent audit resource identifier is unsafe")

    for run in runs:
        resource = run["scenario_resource"]
        if resource_digests.get(resource) != run.get("scenario_resource_digest"):
            raise OpponentAuditManifestError(
                f"packaged scenario changed or is missing: {resource}"
            )

    expected_axes = {
        (
            circuit,
            progression,
            seed,
            strategy,
            player_reference,
        )
        for circuit in {"BUDAPEST", "MONACO", "MONZA", "SINGAPORE", "SUZUKA"}
        for progression in {"early", "mid", "late"}
        for seed in {42, 4242, 20260813}
        for strategy in {"balanced-one-stop", "late-one-stop"}
        for player_reference in {"neutral", "circuit-informed"}
    }
    actual_axes = {
        (
            run.get("circuit_id"),
            run.get("progression"),
            run.get("seed"),
            run.get("strategy_profile"),
            run.get("player_reference"),
        )
        for run in runs
    }
    if actual_axes != expected_axes:
        raise OpponentAuditManifestError("opponent audit matrix is incomplete")

    governance = manifest.get("governance", {})
    if any(
        governance.get(key) is not False
        for key in (
            "private_player_data_allowed",
            "automatic_game_or_catalog_promotion",
            "circuit_informed_reference_is_validated_optimum",
        )
    ):
        raise OpponentAuditManifestError("opponent audit governance is unsafe")


def load_opponent_audit_campaign() -> tuple[dict[str, Any], str]:
    """Return the validated explicit plan and its exact manifest digest."""

    package = importlib.resources.files("pitgun_databricks_adapter")
    campaign_root = package.joinpath("campaigns")
    manifest_name = CAMPAIGN_NAME + ".json"
    manifest_bytes = campaign_root.joinpath(manifest_name).read_bytes()
    checksum_parts = (
        campaign_root.joinpath(CAMPAIGN_NAME + ".sha256").read_text().split()
    )
    if len(checksum_parts) != 2 or checksum_parts[1] != manifest_name:
        raise OpponentAuditManifestError("campaign checksum file has an invalid format")
    digest = hashlib.sha256(manifest_bytes).hexdigest()
    if checksum_parts[0] != digest:
        raise OpponentAuditManifestError(
            "campaign manifest does not match its checksum"
        )

    manifest = json.loads(manifest_bytes)
    resource_digests = {
        run["scenario_resource"]: _sha256(
            package.joinpath(
                "scenarios", f"{run['scenario_resource']}.json"
            ).read_bytes()
        )
        for run in manifest.get("runs", [])
        if isinstance(run.get("scenario_resource"), str)
        and RESOURCE_PATTERN.fullmatch(run["scenario_resource"])
    }
    _validate_manifest(manifest, resource_digests)
    return manifest, "sha256:" + digest


def materialize_opponent_audit_plan(manifest: dict[str, Any]) -> list[dict[str, Any]]:
    """Return the already-explicit immutable run list in its reviewed order."""

    return [dict(run) for run in manifest["runs"]]
