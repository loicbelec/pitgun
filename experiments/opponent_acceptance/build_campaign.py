#!/usr/bin/env python3
"""Freeze the Catalog 1.9 opponent acceptance campaign from pitgun-game."""

from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import statistics
from typing import Any


ROOT = pathlib.Path(__file__).resolve().parents[2]
DEFAULT_SOURCE = (
    ROOT.parent
    / "game"
    / "docs"
    / "gameplay"
    / "opponent-acceptance-corpus-v2.json"
)
SCENARIO_ROOT = ROOT / "experiments" / "opponent_acceptance" / "scenarios"
MANIFEST_PATH = (
    ROOT
    / "experiments"
    / "databricks"
    / "campaigns"
    / "racing-opponent-acceptance-v1.json"
)
CHECKSUM_PATH = MANIFEST_PATH.with_suffix(".sha256")

SOURCE_REPOSITORY = "loicbelec/pitgun-game"
SOURCE_REVISION = "fccffba05b5047afbfba797adaf105d6a2de5d11"
SOURCE_SCHEMA = "pitgun.opponent-acceptance-corpus/v2"
SOURCE_FILE_DIGEST = (
    "sha256:30120317b9d9177de4941ca23091ff5a9c342ac19c3ee8256eb0811a2acc7249"
)
SOURCE_ARTIFACT_DIGEST = (
    "sha256:5abd976b1353e5e178a6cd66144c7bc3d0deae3e8a6912318354a21d8b67c363"
)
CAMPAIGN_SCHEMA = "pitgun.opponent-acceptance-campaign/v1"
CAMPAIGN_ID = "racing-opponent-acceptance-2026-v1"
SCENARIO_ID = "racing.opponent-acceptance-matrix"
SCENARIO_VERSION = "1.0.0"

EXPECTED_CIRCUITS = {"BUDAPEST", "MONACO", "MONZA", "SINGAPORE", "SUZUKA"}
EXPECTED_PROGRESSION = {"early": (1, 4), "mid": (3, 27), "late": (5, 37)}
EXPECTED_SEEDS = {42, 4242, 20260825}
EXPECTED_REFERENCES = {"naive", "balanced", "circuit-informed"}
EXPECTED_CATALOG = {
    "id": "pitgun.racing",
    "version": "1.9.0",
    "manifestDigest": (
        "sha256:30621c0a2c6ff232eacecfb92737372e0f8766e121b45c8b2fb48cc762ca49ac"
    ),
    "simulationPackDigest": (
        "sha256:b0341f1e867fc0217daaf83d1ffb34e807826c45cd3cf4b8a380b504f2e27d00"
    ),
    "modelId": "pitgun.racing-v3-candidate",
    "modelVersion": "0.15.0",
    "modelDigest": (
        "sha256:3038739c9059c12cf47feb4de6a3fd791d9f94290d9e83405a61bd966eea540f"
    ),
    "opponentPolicyResource": "simulation/policies/competitive.json",
    "opponentPolicyResourceDigest": (
        "sha256:3ae8204156754fcf6758bcb452e0b648582e0e8afa196365ea9167e2203a3370"
    ),
}
PHYSICAL_DRIVERS = {
    "balanced_reference",
    "limit_specialist",
    "smooth_operator",
    "tire_manager",
}
FORBIDDEN_KEYS = {
    "careerId",
    "career_id",
    "leaderboard",
    "playerName",
    "telemetry",
}


class AcceptanceBuildError(ValueError):
    """Raised when the cross-repository acceptance contract changed."""


def canonical_pretty(value: Any) -> bytes:
    return (json.dumps(value, indent=2, ensure_ascii=False) + "\n").encode()


def sha256(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


def reject_forbidden_keys(value: Any, path: str = "$") -> None:
    if isinstance(value, dict):
        forbidden = FORBIDDEN_KEYS.intersection(value)
        if forbidden:
            raise AcceptanceBuildError(
                f"private game state at {path}: {', '.join(sorted(forbidden))}"
            )
        for key, nested in value.items():
            reject_forbidden_keys(nested, f"{path}.{key}")
    elif isinstance(value, list):
        for index, nested in enumerate(value):
            reject_forbidden_keys(nested, f"{path}[{index}]")


def source_artifact_digest(source: dict[str, Any]) -> str:
    payload = {key: value for key, value in source.items() if key != "artifactDigest"}
    return sha256(canonical_pretty(payload))


def load_source(path: pathlib.Path) -> tuple[dict[str, Any], str]:
    raw = path.read_bytes()
    raw_digest = sha256(raw)
    if raw_digest != SOURCE_FILE_DIGEST:
        raise AcceptanceBuildError(
            f"game corpus file changed: {raw_digest}; expected {SOURCE_FILE_DIGEST}"
        )
    source = json.loads(raw)
    if source.get("schemaVersion") != SOURCE_SCHEMA:
        raise AcceptanceBuildError("unsupported game acceptance corpus schema")
    if source.get("artifactDigest") != SOURCE_ARTIFACT_DIGEST:
        raise AcceptanceBuildError("game corpus declares an unexpected artifact digest")
    if source_artifact_digest(source) != SOURCE_ARTIFACT_DIGEST:
        raise AcceptanceBuildError("game corpus internal artifact digest is invalid")
    reject_forbidden_keys(source)
    validate_source(source)
    return source, raw_digest


def validate_source(source: dict[str, Any]) -> None:
    if source.get("catalog") != EXPECTED_CATALOG:
        raise AcceptanceBuildError(
            "game corpus is not bound to exact Catalog 1.9 / Model 0.15 identities"
        )
    if source.get("opponentPolicy") != {
        "id": "pitgun.racing.opponents.competitive",
        "version": "2.0.0",
    }:
        raise AcceptanceBuildError("game corpus opponent policy changed")

    matrix = source.get("matrix", {})
    if matrix.get("fields") != 45 or matrix.get("scenarios") != 135:
        raise AcceptanceBuildError("game acceptance corpus matrix count changed")
    if {item["id"] for item in matrix.get("circuits", [])} != EXPECTED_CIRCUITS:
        raise AcceptanceBuildError("game acceptance circuit axis changed")
    actual_progression = {
        item["id"]: (item["era"], item["playerBudget"])
        for item in matrix.get("progression", [])
    }
    if actual_progression != EXPECTED_PROGRESSION:
        raise AcceptanceBuildError("game acceptance progression axis changed")
    if set(matrix.get("seeds", [])) != EXPECTED_SEEDS:
        raise AcceptanceBuildError("game acceptance seed axis changed")
    if set(matrix.get("playerReferences", [])) != EXPECTED_REFERENCES:
        raise AcceptanceBuildError("game acceptance player references changed")

    fields = source.get("fields", [])
    expected_fields = {
        (circuit, progression, seed)
        for circuit in EXPECTED_CIRCUITS
        for progression in EXPECTED_PROGRESSION
        for seed in EXPECTED_SEEDS
    }
    actual_fields = {
        (field.get("circuitId"), field.get("progression"), field.get("seed"))
        for field in fields
    }
    if actual_fields != expected_fields or len(fields) != len(expected_fields):
        raise AcceptanceBuildError("game acceptance field matrix is incomplete")

    for field in fields:
        progression = field["progression"]
        expected_era, expected_budget = EXPECTED_PROGRESSION[progression]
        if field.get("era") != expected_era or field.get("playerBudget") != expected_budget:
            raise AcceptanceBuildError(f"invalid progression context in {field['id']}")
        scenarios = field.get("scenarios", [])
        if {item.get("playerReference") for item in scenarios} != EXPECTED_REFERENCES:
            raise AcceptanceBuildError(f"incomplete paired references in {field['id']}")
        if len({item.get("opponentContractDigest") for item in scenarios}) != 1:
            raise AcceptanceBuildError(f"opponents changed inside paired field {field['id']}")
        if len({item.get("opponentComponentDigest") for item in scenarios}) != 1:
            raise AcceptanceBuildError(f"components changed inside paired field {field['id']}")
        for scenario in scenarios:
            validate_source_scenario(field, scenario)


def validate_source_scenario(field: dict[str, Any], scenario: dict[str, Any]) -> None:
    request = scenario.get("request", {})
    if "initial_fuel_mass_kg" in request:
        raise AcceptanceBuildError(
            f"client fuel override bypasses Catalog 1.9 in {scenario.get('id')}"
        )
    if sha256(canonical_pretty(request)) != scenario.get("requestDigest"):
        raise AcceptanceBuildError(f"request digest mismatch in {scenario.get('id')}")
    if (
        request.get("track_id") != field["circuitId"]
        or request.get("era") != field["era"]
        or request.get("seed") != field["seed"]
        or request.get("laps") != field["laps"]
        or request.get("hz") != 10
    ):
        raise AcceptanceBuildError(f"request identity mismatch in {scenario['id']}")
    competitors = request.get("competitors", [])
    players = [item for item in competitors if item.get("is_player") is True]
    opponents = [item for item in competitors if item.get("is_player") is False]
    if len(players) != 1 or len(opponents) != 9:
        raise AcceptanceBuildError(f"invalid field composition in {scenario['id']}")
    if players[0].get("driver_id") != "balanced_reference":
        raise AcceptanceBuildError(f"player driver changed in {scenario['id']}")
    if any(item.get("driver_id") not in PHYSICAL_DRIVERS for item in opponents):
        raise AcceptanceBuildError(f"unreviewed physical driver in {scenario['id']}")
    if sha256(canonical_pretty(opponents)) != scenario.get("opponentContractDigest"):
        raise AcceptanceBuildError(f"opponent digest mismatch in {scenario['id']}")
    components = {
        key: value
        for key, value in request.get("competitor_vehicle_components", {}).items()
        if key != "player"
    }
    if sha256(canonical_pretty(components)) != scenario.get("opponentComponentDigest"):
        raise AcceptanceBuildError(f"opponent component digest mismatch in {scenario['id']}")


def rust_wire_request(request: dict[str, Any]) -> dict[str, Any]:
    """Remove the two documented browser-only competitor metadata fields."""

    resolved = json.loads(json.dumps(request))
    for competitor in resolved["competitors"]:
        competitor.pop("style", None)
        competitor.pop("points", None)
    return resolved


def build_resolved_scenario(source: dict[str, Any], selected: dict[str, Any]) -> dict[str, Any]:
    catalog = source["catalog"]
    return {
        "schema_version": "pitgun.racing-resolved-scenario/v1",
        "scenario": {"id": SCENARIO_ID, "version": SCENARIO_VERSION},
        "model": {
            "id": catalog["modelId"],
            "version": catalog["modelVersion"],
            "digest": catalog["modelDigest"],
        },
        "data_pack": {
            "id": "pitgun.racing.simulation",
            "version": catalog["version"],
            "digest": catalog["simulationPackDigest"],
        },
        "clock": {"tick_numerator_us": 100000, "tick_denominator": 1},
        "request": rust_wire_request(selected["request"]),
    }


def build_manifest(
    source: dict[str, Any], source_file_digest: str
) -> tuple[dict[str, Any], dict[pathlib.Path, bytes]]:
    runs: list[dict[str, Any]] = []
    resources: dict[pathlib.Path, bytes] = {}
    for field in sorted(source["fields"], key=lambda item: item["id"]):
        for selected in sorted(field["scenarios"], key=lambda item: item["id"]):
            resource = selected["id"]
            resource_path = SCENARIO_ROOT / f"{resource}.json"
            scenario_bytes = canonical_pretty(build_resolved_scenario(source, selected))
            resources[resource_path] = scenario_bytes
            opponents = [
                item
                for item in selected["request"]["competitors"]
                if not item["is_player"]
            ]
            opponent_budgets = [int(item["budget_cap"]) for item in opponents]
            player = next(
                item
                for item in selected["request"]["competitors"]
                if item["is_player"]
            )
            runs.append(
                {
                    "run_key": resource,
                    "scenario_resource": resource,
                    "scenario_resource_digest": sha256(scenario_bytes),
                    "source_request_digest": selected["requestDigest"],
                    "source_field_id": field["id"],
                    "source_opponent_contract_digest": selected[
                        "opponentContractDigest"
                    ],
                    "source_opponent_component_digest": selected[
                        "opponentComponentDigest"
                    ],
                    "circuit_id": field["circuitId"],
                    "circuit_class": field["circuitClass"],
                    "progression": field["progression"],
                    "era": field["era"],
                    "seed": field["seed"],
                    "laps": field["laps"],
                    "player_reference": selected["playerReference"],
                    "player_budget": int(player["budget_cap"]),
                    "opponent_budget_min": min(opponent_budgets),
                    "opponent_budget_median": statistics.median(opponent_budgets),
                    "opponent_budget_max": max(opponent_budgets),
                    "player_setup": selected["playerConfiguration"]["setup"],
                    "player_strategy_profile": selected["playerConfiguration"][
                        "strategyProfile"
                    ],
                }
            )

    return (
        {
            "schema_version": CAMPAIGN_SCHEMA,
            "campaign_id": CAMPAIGN_ID,
            "question": (
                "Does Catalog 1.9 opponent policy V2 challenge controlled player "
                "journeys fairly across representative circuits and progression?"
            ),
            "source": {
                "repository": SOURCE_REPOSITORY,
                "revision": SOURCE_REVISION,
                "schema_version": SOURCE_SCHEMA,
                "file_digest": source_file_digest,
                "artifact_digest": SOURCE_ARTIFACT_DIGEST,
            },
            "catalog": {
                "id": source["catalog"]["id"],
                "version": source["catalog"]["version"],
                "manifest_digest": source["catalog"]["manifestDigest"],
                "simulation_pack_digest": source["catalog"][
                    "simulationPackDigest"
                ],
                "model_id": source["catalog"]["modelId"],
                "model_version": source["catalog"]["modelVersion"],
                "model_digest": source["catalog"]["modelDigest"],
                "opponent_policy_id": source["opponentPolicy"]["id"],
                "opponent_policy_version": source["opponentPolicy"]["version"],
                "opponent_policy_resource_digest": source["catalog"][
                    "opponentPolicyResourceDigest"
                ],
            },
            "matrix": {
                "circuits": source["matrix"]["circuits"],
                "progression": source["matrix"]["progression"],
                "seeds": source["matrix"]["seeds"],
                "player_references": source["matrix"]["playerReferences"],
                "paired_field_count": 45,
            },
            "planned_run_count": len(runs),
            "runs": runs,
            "acceptance_gates": {
                "deterministic_retry_identity": True,
                "no_universal_naive_victory_pattern": True,
                "informed_value_required_across_multiple_circuit_classes": True,
                "budget_parity_must_be_reported": True,
                "human_verdict_required": True,
            },
            "governance": {
                "private_player_data_allowed": False,
                "automatic_game_or_catalog_promotion": False,
                "automatic_policy_mutation": False,
                "circuit_informed_reference_is_validated_optimum": False,
            },
        },
        resources,
    )


def validate_manifest(manifest: dict[str, Any], resources: dict[pathlib.Path, bytes]) -> None:
    runs = manifest.get("runs", [])
    if manifest.get("schema_version") != CAMPAIGN_SCHEMA:
        raise AcceptanceBuildError("invalid acceptance campaign schema")
    if manifest.get("planned_run_count") != 135 or len(runs) != 135:
        raise AcceptanceBuildError("acceptance campaign must contain 135 runs")
    if len({run["run_key"] for run in runs}) != 135:
        raise AcceptanceBuildError("acceptance run keys are not unique")
    axes = {
        (
            run["circuit_id"],
            run["progression"],
            run["seed"],
            run["player_reference"],
        )
        for run in runs
    }
    expected_axes = {
        (circuit, progression, seed, reference)
        for circuit in EXPECTED_CIRCUITS
        for progression in EXPECTED_PROGRESSION
        for seed in EXPECTED_SEEDS
        for reference in EXPECTED_REFERENCES
    }
    if axes != expected_axes:
        raise AcceptanceBuildError("acceptance manifest matrix is incomplete")
    for run in runs:
        resource_path = SCENARIO_ROOT / f"{run['scenario_resource']}.json"
        if sha256(resources.get(resource_path, b"")) != run["scenario_resource_digest"]:
            raise AcceptanceBuildError(f"scenario changed or is missing: {resource_path}")
    governance = manifest.get("governance", {})
    if any(value is not False for value in governance.values()):
        raise AcceptanceBuildError("acceptance campaign governance is unsafe")


def write_or_check(path: pathlib.Path, expected: bytes, check: bool) -> None:
    if check:
        if not path.exists() or path.read_bytes() != expected:
            raise AcceptanceBuildError(f"generated artifact is stale: {path}")
    else:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(expected)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", type=pathlib.Path, default=DEFAULT_SOURCE)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    source, source_file_digest = load_source(args.source)
    manifest, resources = build_manifest(source, source_file_digest)
    validate_manifest(manifest, resources)
    manifest_bytes = canonical_pretty(manifest)
    checksum_bytes = (
        f"{hashlib.sha256(manifest_bytes).hexdigest()}  {MANIFEST_PATH.name}\n"
    ).encode()

    expected_paths = set(resources)
    unexpected = set(SCENARIO_ROOT.glob("*.json")) - expected_paths
    if unexpected:
        raise AcceptanceBuildError(
            "unplanned acceptance scenarios: "
            + ", ".join(path.name for path in sorted(unexpected))
        )
    for path, data in resources.items():
        write_or_check(path, data, args.check)
    write_or_check(MANIFEST_PATH, manifest_bytes, args.check)
    write_or_check(CHECKSUM_PATH, checksum_bytes, args.check)
    action = "validated" if args.check else "froze"
    print(
        f"{action} {len(resources)} Catalog 1.9 acceptance scenarios; "
        f"manifest {sha256(manifest_bytes)}"
    )


if __name__ == "__main__":
    main()
