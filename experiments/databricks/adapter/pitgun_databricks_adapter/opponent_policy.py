"""Deterministically select a governed Racing opponent-policy proposal."""

from __future__ import annotations

import hashlib
import json
import math
from typing import Any


POLICY_SCHEMA_VERSION = "pitgun.racing-opponent-policy/v1"
SELECTION_METHOD = "pitgun.constrained-role-composition/v1"
REQUIRED_FAMILIES = 3
MIN_SUCCESSFUL_SEEDS = 3
MAX_LAP_STDDEV_MS = 50.0
SELECTION_WEIGHTS = {
    "pace": 0.50,
    "robustness": 0.25,
    "setup_diversity": 0.25,
}


class OpponentPolicySelectionError(ValueError):
    """Raised when governed evidence cannot produce a valid policy proposal."""


def canonical_json(value: Any) -> str:
    """Serialize one policy value using the repository's stable JSON profile."""

    return json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
        allow_nan=False,
    )


def digest_json(value: Any) -> str:
    """Return the SHA-256 identity of canonical JSON bytes."""

    return "sha256:" + hashlib.sha256(canonical_json(value).encode()).hexdigest()


def _require_digest(label: str, value: Any) -> str:
    if not isinstance(value, str) or not value.startswith("sha256:") or len(value) != 71:
        raise OpponentPolicySelectionError(f"{label} must be one sha256 digest")
    return value


def _finite_number(label: str, value: Any) -> float:
    number = float(value)
    if not math.isfinite(number):
        raise OpponentPolicySelectionError(f"{label} must be finite")
    return number


def _normalized(value: float, minimum: float, maximum: float, *, inverse: bool = False) -> float:
    if maximum == minimum:
        return 1.0
    score = (value - minimum) / (maximum - minimum)
    return 1.0 - score if inverse else score


def _setup_distance(left: dict[str, float], right: dict[str, float]) -> float:
    return math.sqrt(
        (left["downforce_slider"] - right["downforce_slider"]) ** 2
        + (left["gear_ratio_slider"] - right["gear_ratio_slider"]) ** 2
    )


def select_reference_policy(
    *,
    summaries: list[dict[str, Any]],
    lineage: dict[str, Any],
    policy_version: str = "1.0.0",
) -> dict[str, Any]:
    """Select exactly three role-diverse candidates from pinned campaign evidence.

    The reference campaign intentionally contains three families. V1 selects all
    three into different field roles; score order alone can never fill the grid.
    """

    if len(summaries) != REQUIRED_FAMILIES:
        raise OpponentPolicySelectionError(
            f"reference policy requires exactly {REQUIRED_FAMILIES} families"
        )

    normalized: list[dict[str, Any]] = []
    for raw in summaries:
        family = str(raw.get("configuration_family", "")).strip()
        configuration_id = _require_digest(
            f"{family or 'candidate'} configuration_id", raw.get("configuration_id")
        )
        setup = raw.get("setup")
        strategy = raw.get("strategy")
        if not family or not isinstance(setup, dict) or not isinstance(strategy, dict):
            raise OpponentPolicySelectionError("candidate family, setup and strategy are required")
        if strategy.get("id") != "single-lap-no-stop":
            raise OpponentPolicySelectionError(f"unsupported reference strategy for {family}")

        downforce = _finite_number(f"{family} downforce", setup.get("downforce_slider"))
        gearing = _finite_number(f"{family} gear ratio", setup.get("gear_ratio_slider"))
        if not 0.0 <= downforce <= 1.0 or not 0.0 <= gearing <= 1.0:
            raise OpponentPolicySelectionError(f"setup sliders are outside [0, 1] for {family}")

        successful_seed_count = int(raw.get("successful_seed_count", 0))
        mean_lap_time_ms = _finite_number(
            f"{family} mean lap time", raw.get("mean_lap_time_ms")
        )
        lap_time_stddev_ms = _finite_number(
            f"{family} lap standard deviation", raw.get("lap_time_stddev_ms")
        )
        mean_maximum_speed_kmh = _finite_number(
            f"{family} mean maximum speed", raw.get("mean_maximum_speed_kmh")
        )
        if successful_seed_count < MIN_SUCCESSFUL_SEEDS:
            raise OpponentPolicySelectionError(f"insufficient successful seeds for {family}")
        if lap_time_stddev_ms > MAX_LAP_STDDEV_MS:
            raise OpponentPolicySelectionError(f"seed variance exceeds the V1 bound for {family}")

        normalized.append(
            {
                "configuration_family": family,
                "configuration_id": configuration_id,
                "setup": {
                    "downforce_slider": downforce,
                    "gear_ratio_slider": gearing,
                },
                "strategy": {"id": "single-lap-no-stop"},
                "successful_seed_count": successful_seed_count,
                "mean_lap_time_ms": mean_lap_time_ms,
                "lap_time_stddev_ms": lap_time_stddev_ms,
                "mean_maximum_speed_kmh": mean_maximum_speed_kmh,
            }
        )

    normalized.sort(key=lambda item: item["configuration_family"])
    if len({item["configuration_family"] for item in normalized}) != len(normalized):
        raise OpponentPolicySelectionError("candidate family identifiers are not unique")
    setup_keys = {
        canonical_json(item["setup"])
        for item in normalized
    }
    if len(setup_keys) != REQUIRED_FAMILIES:
        raise OpponentPolicySelectionError("candidate setups are not materially distinct")

    pace_values = [item["mean_lap_time_ms"] for item in normalized]
    robustness_values = [item["lap_time_stddev_ms"] for item in normalized]
    diversity_values = []
    for candidate in normalized:
        distances = [
            _setup_distance(candidate["setup"], other["setup"])
            for other in normalized
            if other is not candidate
        ]
        diversity_values.append(min(distances))

    for index, candidate in enumerate(normalized):
        candidate["selection_score"] = round(
            SELECTION_WEIGHTS["pace"]
            * _normalized(
                candidate["mean_lap_time_ms"], min(pace_values), max(pace_values), inverse=True
            )
            + SELECTION_WEIGHTS["robustness"]
            * _normalized(
                candidate["lap_time_stddev_ms"],
                min(robustness_values),
                max(robustness_values),
                inverse=True,
            )
            + SELECTION_WEIGHTS["setup_diversity"]
            * _normalized(diversity_values[index], min(diversity_values), max(diversity_values)),
            6,
        )

    pace_order = sorted(normalized, key=lambda item: (item["mean_lap_time_ms"], item["configuration_family"]))
    roles = ("front-runner", "midfield", "challenger")
    for role, candidate in zip(roles, pace_order):
        candidate["role"] = role

    source = {
        "campaign_id": str(lineage["campaign_id"]),
        "campaign_manifest_digest": _require_digest(
            "campaign manifest digest", lineage["campaign_manifest_digest"]
        ),
        "delta_tables": {
            "runs": {
                "name": str(lineage["runs_table_name"]),
                "version": int(lineage["runs_table_version"]),
            },
            "metrics": {
                "name": str(lineage["metrics_table_name"]),
                "version": int(lineage["metrics_table_version"]),
            },
        },
        "mlflow_run_id": str(lineage["mlflow_run_id"]),
        "source_git_revision": str(lineage["source_git_revision"]),
        "model_digest": _require_digest("model digest", lineage["model_digest"]),
        "data_pack_digest": _require_digest("data pack digest", lineage["data_pack_digest"]),
    }
    candidate_set_digest = digest_json(normalized)

    profiles = []
    for candidate in sorted(normalized, key=lambda item: roles.index(item["role"])):
        setup = candidate["setup"]
        profiles.append(
            {
                "id": candidate["configuration_family"],
                "role": candidate["role"],
                "source_configuration_id": candidate["configuration_id"],
                "selection_score": candidate["selection_score"],
                "setup": {
                    "downforce_slider": {
                        "center": setup["downforce_slider"],
                        "min": setup["downforce_slider"],
                        "max": setup["downforce_slider"],
                    },
                    "gear_ratio_slider": {
                        "center": setup["gear_ratio_slider"],
                        "min": setup["gear_ratio_slider"],
                        "max": setup["gear_ratio_slider"],
                    },
                },
                "strategy": candidate["strategy"],
                "evidence": {
                    "successful_seed_count": candidate["successful_seed_count"],
                    "mean_lap_time_ms": round(candidate["mean_lap_time_ms"], 6),
                    "lap_time_stddev_ms": round(candidate["lap_time_stddev_ms"], 6),
                    "mean_maximum_speed_kmh": round(
                        candidate["mean_maximum_speed_kmh"], 6
                    ),
                },
            }
        )

    return {
        "schema_version": POLICY_SCHEMA_VERSION,
        "policy": {"id": "pitgun.racing.opponents.reference", "version": policy_version},
        "scope": {
            "circuit_model_id": str(lineage["circuit_model_id"]),
            "era": int(lineage["era"]),
            "difficulty_band": "competitive",
        },
        "composition": {
            "algorithm": "pitgun.seeded-role-composition/v1",
            "roles": [
                {"id": role, "count": 1, "eligible_profile_ids": [profiles[index]["id"]]}
                for index, role in enumerate(roles)
            ],
        },
        "profiles": profiles,
        "calibration": {
            "source": source,
            "candidate_set_digest": candidate_set_digest,
            "selection": {
                "method": SELECTION_METHOD,
                "weights": SELECTION_WEIGHTS,
                "constraints": {
                    "minimum_successful_seeds": MIN_SUCCESSFUL_SEEDS,
                    "maximum_lap_time_stddev_ms": MAX_LAP_STDDEV_MS,
                    "required_distinct_setup_count": REQUIRED_FAMILIES,
                    "hidden_player_data_allowed": False,
                },
            },
            "limitations": [
                "single-circuit physical model",
                "single-lap strategies only",
                "exact setup centers; bounded mutation is disabled until measured",
            ],
        },
    }
