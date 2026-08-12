"""Pure, deterministic review of governed tuning-response evidence."""

from __future__ import annotations

from collections import defaultdict
import hashlib
import importlib.resources
import json
import statistics
from typing import Any


POLICIES = frozenset({"racing-aero-candidate-review-v1"})


class CandidateReviewError(ValueError):
    """Raised when evidence is incomplete, inconsistent, or unreviewable."""


def _digest(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


def load_candidate_review_policy(name: str) -> tuple[dict[str, Any], str]:
    if name not in POLICIES:
        raise CandidateReviewError(
            "candidate review policy is not packaged or allowlisted"
        )
    resource = importlib.resources.files("pitgun_databricks_adapter").joinpath(
        "reviews", f"{name}.json"
    )
    payload = resource.read_bytes()
    policy = json.loads(payload)
    if policy.get("schema_version") != "pitgun.racing-candidate-review-policy/v1":
        raise CandidateReviewError("unsupported candidate review policy")
    if policy.get("automatic_promotion") is not False:
        raise CandidateReviewError(
            "candidate review policy cannot enable automatic promotion"
        )
    return policy, _digest(payload)


def _mean(values: list[float]) -> float:
    return statistics.fmean(values)


def review_candidate_evidence(
    manifest: dict[str, Any],
    manifest_digest: str,
    policy: dict[str, Any],
    policy_digest: str,
    rows: list[dict[str, Any]],
    evidence_versions: dict[str, int],
) -> dict[str, Any]:
    """Review exact persisted rows and return a human-gated decision report."""

    if manifest.get("campaign_id") != policy.get("campaign_id"):
        raise CandidateReviewError("policy and campaign identities differ")
    plan = {
        (
            configuration["expected_experimental_configuration_id"],
            str(seed),
        ): configuration
        for configuration in manifest["configurations"]
        for seed in manifest["seeds"]
    }
    if len(plan) != manifest["planned_run_count"]:
        raise CandidateReviewError("manifest natural keys do not reconcile")
    observed_keys = {
        (row["experimental_configuration_id"], str(row["seed"])) for row in rows
    }
    if len(rows) != len(observed_keys) or observed_keys != set(plan):
        raise CandidateReviewError(
            "persisted evidence does not match the immutable plan"
        )

    terminal_counts: dict[str, int] = defaultdict(int)
    measurements: dict[tuple[str, str, str], list[dict[str, float]]] = defaultdict(list)
    observed_maximum_speed = 0.0
    for row in rows:
        terminal_counts[row["execution_status"]] += 1
        if row["execution_status"] != "SUCCESS":
            continue
        key = (row["experimental_configuration_id"], str(row["seed"]))
        configuration = plan[key]
        result = json.loads(row["result_json"])
        response = result["setup_response"]
        measurements[
            (
                configuration["response_id"],
                configuration["circuit_id"],
                configuration["configuration_family"],
            )
        ].append(
            {
                "lap_time_ms": float(result["total_time_ms"]),
                "maximum_speed_kph": float(result["observed_maximum_speed_kph"]),
                "mean_straight_speed_kph": float(response["mean_straight_speed_kph"]),
                "mean_corner_speed_kph": float(response["mean_corner_speed_kph"]),
                "aerodynamic_drag_work_kj": float(response["aerodynamic_drag_work_kj"]),
                "mean_downforce_n": float(response["mean_downforce_n"]),
                "maximum_rpm_utilization": float(response["maximum_rpm_utilization"]),
            }
        )
        observed_maximum_speed = max(
            observed_maximum_speed, float(result["observed_maximum_speed_kph"])
        )

    setup_summaries = []
    by_response_circuit: dict[tuple[str, str], list[dict[str, Any]]] = defaultdict(list)
    for (response_id, circuit_id, family), samples in sorted(measurements.items()):
        lap_times = [sample["lap_time_ms"] for sample in samples]
        summary = {
            "response_id": response_id,
            "circuit_id": circuit_id,
            "configuration_family": family,
            "seed_count": len(samples),
            "mean_lap_time_ms": _mean(lap_times),
            "seed_lap_time_stddev_ms": statistics.pstdev(lap_times),
            "seed_lap_time_range_ms": max(lap_times) - min(lap_times),
        }
        for metric in (
            "maximum_speed_kph",
            "mean_straight_speed_kph",
            "mean_corner_speed_kph",
            "aerodynamic_drag_work_kj",
            "mean_downforce_n",
            "maximum_rpm_utilization",
        ):
            summary[f"mean_{metric}"] = _mean([sample[metric] for sample in samples])
        setup_summaries.append(summary)
        by_response_circuit[(response_id, circuit_id)].append(summary)

    circuit_summaries = []
    for (response_id, circuit_id), setups in sorted(by_response_circuit.items()):
        ranking = sorted(setups, key=lambda row: row["mean_lap_time_ms"])
        fastest = ranking[0]["mean_lap_time_ms"]
        slowest = ranking[-1]["mean_lap_time_ms"]
        discrimination = slowest - fastest
        maximum_seed_range = max(row["seed_lap_time_range_ms"] for row in setups)
        circuit_summaries.append(
            {
                "response_id": response_id,
                "circuit_id": circuit_id,
                "setup_ranking": [row["configuration_family"] for row in ranking],
                "best_configuration_family": ranking[0]["configuration_family"],
                "best_to_second_gap_ms": ranking[1]["mean_lap_time_ms"] - fastest,
                "setup_discrimination_ms": discrimination,
                "setup_discrimination_pct": 100.0 * discrimination / fastest,
                "maximum_seed_lap_time_range_ms": maximum_seed_range,
                "seed_noise_to_setup_signal": maximum_seed_range / discrimination
                if discrimination
                else None,
                "mean_lap_time_ms": _mean([row["mean_lap_time_ms"] for row in setups]),
            }
        )

    circuit_index = {
        (row["response_id"], row["circuit_id"]): row for row in circuit_summaries
    }
    historical_id = policy["historical_response_id"]
    candidate_id = policy["candidate_response_id"]
    gates = policy["gates"]
    comparisons = []
    for circuit_id in sorted(policy["expected_top_families"]):
        historical = circuit_index[(historical_id, circuit_id)]
        candidate = circuit_index[(candidate_id, circuit_id)]
        comparisons.append(
            {
                "circuit_id": circuit_id,
                "historical_best_configuration_family": historical[
                    "best_configuration_family"
                ],
                "candidate_best_configuration_family": candidate[
                    "best_configuration_family"
                ],
                "candidate_best_is_physically_coherent": candidate[
                    "best_configuration_family"
                ]
                in policy["expected_top_families"][circuit_id],
                "mean_pace_shift_pct": 100.0
                * (
                    candidate["mean_lap_time_ms"] / historical["mean_lap_time_ms"] - 1.0
                ),
                "historical_setup_discrimination_pct": historical[
                    "setup_discrimination_pct"
                ],
                "candidate_setup_discrimination_pct": candidate[
                    "setup_discrimination_pct"
                ],
                "candidate_seed_noise_to_setup_signal": candidate[
                    "seed_noise_to_setup_signal"
                ],
            }
        )

    success_rate = terminal_counts["SUCCESS"] / manifest["planned_run_count"]
    coherent_count = sum(
        row["candidate_best_is_physically_coherent"] for row in comparisons
    )
    hard_failures = []
    refinements = []
    if success_rate < gates["required_success_rate"]:
        hard_failures.append("incomplete-or-invalid-experimental-evidence")
    if observed_maximum_speed > gates["maximum_speed_kph"]:
        hard_failures.append("maximum-speed-guardrail-exceeded")
    for comparison in comparisons:
        circuit_id = comparison["circuit_id"]
        discrimination = comparison["candidate_setup_discrimination_pct"]
        noise_ratio = comparison["candidate_seed_noise_to_setup_signal"]
        if discrimination < gates["minimum_setup_discrimination_pct"]:
            refinements.append(f"{circuit_id}:setup-discrimination-too-low")
        if discrimination > gates["maximum_setup_discrimination_pct"]:
            refinements.append(f"{circuit_id}:setup-discrimination-too-high")
        if (
            noise_ratio is None
            or noise_ratio > gates["maximum_seed_noise_to_setup_signal"]
        ):
            refinements.append(f"{circuit_id}:seed-noise-too-high")
        if (
            abs(comparison["mean_pace_shift_pct"])
            > gates["maximum_absolute_mean_pace_shift_pct"]
        ):
            refinements.append(f"{circuit_id}:mean-pace-shift-too-large")
        if not comparison["candidate_best_is_physically_coherent"]:
            refinements.append(f"{circuit_id}:implausible-best-setup")
    if hard_failures:
        decision = "REJECT"
    elif (
        not refinements
        and coherent_count >= gates["required_physically_coherent_circuit_count"]
    ):
        decision = "PROMOTE"
    else:
        decision = "REFINE"

    return {
        "schema_version": "pitgun.racing-candidate-evidence-review/v1",
        "campaign_id": manifest["campaign_id"],
        "manifest_digest": manifest_digest,
        "policy_id": policy["id"],
        "policy_digest": policy_digest,
        "evidence_versions": evidence_versions,
        "planned_execution_count": manifest["planned_run_count"],
        "terminal_counts": dict(sorted(terminal_counts.items())),
        "success_rate": success_rate,
        "observed_maximum_speed_kph": observed_maximum_speed,
        "physically_coherent_circuit_count": coherent_count,
        "circuit_count": len(comparisons),
        "decision": decision,
        "hard_failures": hard_failures,
        "refinement_reasons": refinements,
        "comparisons": comparisons,
        "circuit_summaries": circuit_summaries,
        "setup_summaries": setup_summaries,
        "automatic_promotion": False,
    }
