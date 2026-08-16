#!/usr/bin/env python3
"""Freeze the economy-backed Racing V2 development-budget campaign."""

from __future__ import annotations

import copy
import hashlib
import json
import pathlib
from typing import Any


ROOT = pathlib.Path(__file__).resolve().parents[2]
SOURCE_MANIFEST = (
    ROOT / "experiments" / "databricks" / "campaigns" / "racing-strategy-effect-v1.json"
)
SOURCE_CHECKSUM = SOURCE_MANIFEST.with_suffix(".sha256")
SOURCE_SCENARIOS = ROOT / "experiments" / "strategy_effect" / "scenarios"
SCENARIO_ROOT = ROOT / "experiments" / "budget_effect_v2" / "scenarios"
MANIFEST_PATH = (
    ROOT / "experiments" / "databricks" / "campaigns" / "racing-budget-effect-v2.json"
)
CHECKSUM_PATH = MANIFEST_PATH.with_suffix(".sha256")

SCHEMA_VERSION = "pitgun.budget-effect-campaign/v2"
CAMPAIGN_ID = "racing-budget-effect-2026-v2"
SCENARIO_IDENTITY = {"id": "racing.budget-effect-campaign", "version": "2.0.0"}
POINT_KEYS = ("aero_points", "chassis_points", "cooling_points", "engine_points")
PROGRESSION_ARTIFACT = {
    "repository": "loicbelec/pitgun-game",
    "git_revision": "1eebd08e4a375a5570a73c7a6adcd16cc8736e8a",
    "path": "docs/gameplay/ai-calibration-progression-v1.json",
    "schema_version": "pitgun.ai-calibration-progression/v1",
    "artifact_digest": (
        "sha256:1e5a082ff05b8c66d43ad0d69306af608c2b9fd4253a4d479d3c0f9c03daf23c"
    ),
}
TREATMENTS = {
    "early": {"below": 3, "reference": 4, "above": 5},
    "mid": {"below": 24, "reference": 27, "above": 30},
    "late": {"below": 33, "reference": 37, "above": 41},
}


class BudgetEffectV2BuildError(ValueError):
    """Raised when the source cannot produce an exact V2 triplet."""


def canonical_bytes(value: Any) -> bytes:
    return (json.dumps(value, indent=2, ensure_ascii=False) + "\n").encode()


def compact_bytes(value: Any) -> bytes:
    return json.dumps(
        value, sort_keys=True, separators=(",", ":"), ensure_ascii=False
    ).encode()


def sha256(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


def balanced_points(budget: int) -> dict[str, int]:
    quotient, remainder = divmod(budget, len(POINT_KEYS))
    return {
        key: quotient + (1 if index < remainder else 0)
        for index, key in enumerate(POINT_KEYS)
    }


def proportional_points(tuning: dict[str, Any], budget: int) -> dict[str, int]:
    """Scale one authored allocation with deterministic largest remainders."""

    weights = [int(tuning[key]) for key in POINT_KEYS]
    total = sum(weights)
    if total <= 0:
        raise BudgetEffectV2BuildError("source opponent allocation is empty")
    numerators = [budget * weight for weight in weights]
    points = [numerator // total for numerator in numerators]
    remainder = budget - sum(points)
    order = sorted(
        range(len(POINT_KEYS)),
        key=lambda index: (-(numerators[index] % total), index),
    )
    for index in order[:remainder]:
        points[index] += 1
    return dict(zip(POINT_KEYS, points))


def controlled_player(scenario: dict[str, Any]) -> dict[str, Any]:
    players = [row for row in scenario["request"]["competitors"] if row["is_player"]]
    if len(players) != 1 or players[0]["id"] != "player":
        raise BudgetEffectV2BuildError("source scenario must contain one player")
    return players[0]


def invariant_projection(scenario: dict[str, Any]) -> dict[str, Any]:
    projection = copy.deepcopy(scenario)
    player = controlled_player(projection)
    player.pop("budget_cap")
    for key in POINT_KEYS:
        player["tuning"].pop(key)
    return projection


def load_source() -> tuple[dict[str, Any], str]:
    manifest_bytes = SOURCE_MANIFEST.read_bytes()
    expected_digest, expected_name = SOURCE_CHECKSUM.read_text().split()
    actual_digest = hashlib.sha256(manifest_bytes).hexdigest()
    if expected_name != SOURCE_MANIFEST.name or expected_digest != actual_digest:
        raise BudgetEffectV2BuildError("strategy campaign checksum mismatch")
    manifest = json.loads(manifest_bytes)
    if manifest.get("planned_pair_count") != 45:
        raise BudgetEffectV2BuildError("strategy source campaign is incomplete")
    return manifest, "sha256:" + actual_digest


def balanced_source_index(manifest: dict[str, Any]) -> dict[str, dict[str, Any]]:
    runs = {
        run["pair_key"]: run
        for run in manifest["runs"]
        if run["strategy_profile"] == "balanced-one-stop"
    }
    if len(runs) != 45:
        raise BudgetEffectV2BuildError("expected 45 balanced source scenarios")
    return runs


def load_source_scenario(run: dict[str, Any]) -> dict[str, Any]:
    path = SOURCE_SCENARIOS / f"{run['scenario_resource']}.json"
    data = path.read_bytes()
    if sha256(data) != run["scenario_resource_digest"]:
        raise BudgetEffectV2BuildError(f"source resource changed: {path.name}")
    return json.loads(data)


def normalize_opponent_field(scenario: dict[str, Any], reference_budget: int) -> None:
    opponents = [
        row for row in scenario["request"]["competitors"] if not row["is_player"]
    ]
    if len(opponents) != 9:
        raise BudgetEffectV2BuildError("source scenario must contain nine opponents")
    for opponent in opponents:
        allocation = proportional_points(opponent["tuning"], reference_budget)
        opponent["budget_cap"] = reference_budget
        opponent["tuning"].update(allocation)


def economy_backed_matrix(source: dict[str, Any]) -> dict[str, Any]:
    matrix = copy.deepcopy(source["matrix"])
    for progression in matrix["progression"]:
        progression.pop("playerBudget")
        progression["referenceBudget"] = TREATMENTS[progression["id"]]["reference"]
    return matrix


def build_manifest(
    source: dict[str, Any],
    source_digest: str,
    scenario_root: pathlib.Path = SCENARIO_ROOT,
) -> dict[str, Any]:
    runs = []
    expected_paths = set()
    scenario_root.mkdir(parents=True, exist_ok=True)
    for triplet_key, source_run in sorted(balanced_source_index(source).items()):
        progression = source_run["progression"]
        treatments = TREATMENTS.get(progression)
        if treatments is None:
            raise BudgetEffectV2BuildError(f"unknown progression: {progression}")
        reference_budget = treatments["reference"]
        base = load_source_scenario(source_run)
        base["scenario"] = dict(SCENARIO_IDENTITY)
        player = controlled_player(base)
        player["name"] = "Economy-backed Development Reference"
        normalize_opponent_field(base, reference_budget)
        invariant_digest = sha256(compact_bytes(invariant_projection(base)))

        for treatment, budget in treatments.items():
            scenario = copy.deepcopy(base)
            treated_player = controlled_player(scenario)
            treated_player["budget_cap"] = budget
            treated_player["tuning"].update(balanced_points(budget))
            if sha256(compact_bytes(invariant_projection(scenario))) != invariant_digest:
                raise BudgetEffectV2BuildError(
                    f"non-budget input changed inside triplet: {triplet_key}"
                )
            resource = f"budget-effect-v2-{triplet_key}--{treatment}"
            path = scenario_root / f"{resource}.json"
            scenario_bytes = canonical_bytes(scenario)
            path.write_bytes(scenario_bytes)
            expected_paths.add(path)
            runs.append(
                {
                    "run_key": resource,
                    "triplet_key": triplet_key,
                    "scenario_resource": resource,
                    "scenario_resource_digest": sha256(scenario_bytes),
                    "triplet_invariant_digest": invariant_digest,
                    "treatment": treatment,
                    "reference_budget": reference_budget,
                    "player_budget": budget,
                    "player_allocation": balanced_points(budget),
                    "opponent_budget": reference_budget,
                    "source_opponent_contract_digest": source_run[
                        "source_opponent_contract_digest"
                    ],
                    "source_resource_digest": source_run[
                        "scenario_resource_digest"
                    ],
                    "circuit_id": source_run["circuit_id"],
                    "progression": progression,
                    "era": int(source_run["era"]),
                    "seed": int(source_run["seed"]),
                }
            )

    unexpected = set(scenario_root.glob("*.json")) - expected_paths
    if unexpected:
        raise BudgetEffectV2BuildError(
            "unplanned V2 budget scenarios: "
            + ", ".join(path.name for path in sorted(unexpected))
        )
    return {
        "schema_version": SCHEMA_VERSION,
        "campaign_id": CAMPAIGN_ID,
        "question": (
            "How does controlled player competitiveness respond to development "
            "budget around economy-backed early, mid, and late progression totals?"
        ),
        "source": {
            "campaign_id": source["campaign_id"],
            "manifest_digest": source_digest,
            "upstream_opponent_campaign": source["source"],
            "gameplay_progression": dict(PROGRESSION_ARTIFACT),
        },
        "catalog": source["catalog"],
        "controlled_input": {
            "player_reference": "neutral",
            "player_strategy": "balanced-one-stop",
            "treatments_by_progression": copy.deepcopy(TREATMENTS),
            "allocation": "deterministic-balanced-four-axis",
            "opponent_field_normalization": {
                "budget": "economy-backed-progression-reference",
                "allocation": "source-relative-largest-remainder-with-integer-quantization",
                "all_opponents_share_reference_total": True,
                "early_quantization_caveat": (
                    "four total points quantize the audited field to one point per axis"
                ),
            },
            "only_allowed_triplet_difference": (
                "player.development_budget_and_balanced_point_allocation"
            ),
        },
        "matrix": economy_backed_matrix(source),
        "planned_triplet_count": 45,
        "planned_run_count": len(runs),
        "runs": runs,
        "governance": {
            "private_player_data_allowed": False,
            "automatic_game_or_catalog_promotion": False,
            "automatic_budget_target_selection_allowed": False,
        },
    }


def validate_manifest(
    manifest: dict[str, Any],
    resources: dict[str, bytes],
    source_digest: str,
) -> None:
    if manifest.get("schema_version") != SCHEMA_VERSION:
        raise BudgetEffectV2BuildError("unsupported V2 campaign schema")
    if manifest.get("campaign_id") != CAMPAIGN_ID:
        raise BudgetEffectV2BuildError("V2 campaign identity changed")
    if manifest.get("planned_triplet_count") != 45:
        raise BudgetEffectV2BuildError("V2 campaign must contain 45 triplets")
    runs = manifest.get("runs", [])
    if manifest.get("planned_run_count") != 135 or len(runs) != 135:
        raise BudgetEffectV2BuildError("V2 campaign must contain 135 runs")
    source = manifest.get("source", {})
    if source.get("manifest_digest") != source_digest:
        raise BudgetEffectV2BuildError("source strategy campaign changed")
    if source.get("gameplay_progression") != PROGRESSION_ARTIFACT:
        raise BudgetEffectV2BuildError("gameplay progression provenance changed")
    controlled = manifest.get("controlled_input", {})
    if controlled.get("treatments_by_progression") != TREATMENTS:
        raise BudgetEffectV2BuildError("economy-backed treatments changed")
    if controlled.get("only_allowed_triplet_difference") != (
        "player.development_budget_and_balanced_point_allocation"
    ):
        raise BudgetEffectV2BuildError("controlled treatment boundary changed")
    if (
        controlled.get("player_reference") != "neutral"
        or controlled.get("player_strategy") != "balanced-one-stop"
        or controlled.get("allocation") != "deterministic-balanced-four-axis"
        or controlled.get("opponent_field_normalization")
        != {
            "budget": "economy-backed-progression-reference",
            "allocation": "source-relative-largest-remainder-with-integer-quantization",
            "all_opponents_share_reference_total": True,
            "early_quantization_caveat": (
                "four total points quantize the audited field to one point per axis"
            ),
        }
    ):
        raise BudgetEffectV2BuildError("controlled V2 input changed")
    progression_matrix = manifest.get("matrix", {}).get("progression", [])
    if (
        len(progression_matrix) != len(TREATMENTS)
        or {row.get("id") for row in progression_matrix} != set(TREATMENTS)
        or any("playerBudget" in row for row in progression_matrix)
        or any(
            row.get("referenceBudget") != TREATMENTS[row["id"]]["reference"]
            for row in progression_matrix
        )
    ):
        raise BudgetEffectV2BuildError("V2 progression matrix is not economy-backed")
    governance = manifest.get("governance", {})
    if any(
        governance.get(key) is not False
        for key in (
            "private_player_data_allowed",
            "automatic_game_or_catalog_promotion",
            "automatic_budget_target_selection_allowed",
        )
    ):
        raise BudgetEffectV2BuildError("V2 campaign governance is unsafe")

    run_keys = [run.get("run_key") for run in runs]
    if len(set(run_keys)) != 135 or set(resources) != set(run_keys):
        raise BudgetEffectV2BuildError("V2 resources and run keys differ")
    triplets: dict[str, list[dict[str, Any]]] = {}
    for run in runs:
        resource = run["scenario_resource"]
        data = resources.get(resource)
        if data is None or sha256(data) != run["scenario_resource_digest"]:
            raise BudgetEffectV2BuildError(f"V2 scenario changed: {resource}")
        scenario = json.loads(data)
        if scenario.get("scenario") != SCENARIO_IDENTITY:
            raise BudgetEffectV2BuildError(f"V2 scenario identity changed: {resource}")
        catalog = manifest["catalog"]
        if scenario.get("model") != {
            "id": catalog["model_id"],
            "version": catalog["model_version"],
            "digest": catalog["model_digest"],
        }:
            raise BudgetEffectV2BuildError(f"V2 model identity changed: {resource}")
        if scenario.get("data_pack") != {
            "id": "pitgun.racing.simulation",
            "version": catalog["version"],
            "digest": catalog["simulation_pack_digest"],
        }:
            raise BudgetEffectV2BuildError(f"V2 data pack changed: {resource}")

        expected_budget = TREATMENTS[run["progression"]][run["treatment"]]
        reference_budget = TREATMENTS[run["progression"]]["reference"]
        player = controlled_player(scenario)
        allocation = {key: int(player["tuning"][key]) for key in POINT_KEYS}
        if (
            run["reference_budget"] != reference_budget
            or run["opponent_budget"] != reference_budget
            or run["player_budget"] != expected_budget
            or player["budget_cap"] != expected_budget
            or allocation != balanced_points(expected_budget)
            or run["player_allocation"] != allocation
            or max(allocation.values()) > 20
        ):
            raise BudgetEffectV2BuildError(f"invalid V2 treatment: {resource}")
        opponents = [
            row for row in scenario["request"]["competitors"] if not row["is_player"]
        ]
        if len(opponents) != 9:
            raise BudgetEffectV2BuildError(f"invalid V2 field: {resource}")
        for opponent in opponents:
            points = [int(opponent["tuning"][key]) for key in POINT_KEYS]
            if (
                int(opponent["budget_cap"]) != reference_budget
                or sum(points) != reference_budget
                or max(points) > 20
            ):
                raise BudgetEffectV2BuildError(
                    f"invalid normalized opponent: {resource}/{opponent['id']}"
                )
        if sha256(compact_bytes(invariant_projection(scenario))) != run[
            "triplet_invariant_digest"
        ]:
            raise BudgetEffectV2BuildError(f"V2 triplet invariant changed: {resource}")
        triplets.setdefault(run["triplet_key"], []).append(run)

    if len(triplets) != 45:
        raise BudgetEffectV2BuildError("V2 triplet keys are incomplete")
    for triplet_key, triplet in triplets.items():
        if {run["treatment"] for run in triplet} != set(
            TREATMENTS[triplet[0]["progression"]]
        ):
            raise BudgetEffectV2BuildError(
                f"V2 triplet treatments are incomplete: {triplet_key}"
            )
        if len({run["triplet_invariant_digest"] for run in triplet}) != 1:
            raise BudgetEffectV2BuildError(
                f"V2 triplet input changed: {triplet_key}"
            )
        if len({run["player_budget"] for run in triplet}) != 3:
            raise BudgetEffectV2BuildError(
                f"V2 triplet budgets are not distinct: {triplet_key}"
            )


def main() -> None:
    source, source_digest = load_source()
    manifest = build_manifest(source, source_digest)
    resources = {
        path.stem: path.read_bytes() for path in SCENARIO_ROOT.glob("*.json")
    }
    validate_manifest(manifest, resources, source_digest)
    manifest_bytes = canonical_bytes(manifest)
    MANIFEST_PATH.write_bytes(manifest_bytes)
    CHECKSUM_PATH.write_text(
        f"{hashlib.sha256(manifest_bytes).hexdigest()}  {MANIFEST_PATH.name}\n"
    )
    print(
        f"froze {manifest['planned_triplet_count']} economy-backed triplets and "
        f"{manifest['planned_run_count']} runs; manifest {sha256(manifest_bytes)}"
    )


if __name__ == "__main__":
    main()
