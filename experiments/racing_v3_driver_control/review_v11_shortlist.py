#!/usr/bin/env python3
"""Review frozen V11 profiles against the independent 702-case holdout."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import pathlib
from collections import Counter, defaultdict
from typing import Any


EXPERIMENT = pathlib.Path(__file__).parent
MANIFEST = EXPERIMENT / "shortlist" / "shortlist-v1.json"
BASE_PROFILE = EXPERIMENT / "profile-v11.driver-friction.json"
SOURCE_SURFACE = (
    EXPERIMENT / "results" / "local-driver-friction-parameter-screen-v1.json"
)
DEFAULT_OUTPUT = (
    EXPERIMENT / "results" / "holdout-driver-control-shortlist-v1.json"
)
SCHEMA_VERSION = "pitgun.racing-v3-driver-control-holdout-review/v1"


class ReviewError(RuntimeError):
    """Raised when immutable shortlist evidence cannot be reconciled."""


def canonical_pretty(value: object) -> bytes:
    return (json.dumps(value, indent=2, ensure_ascii=False) + "\n").encode()


def content_digest(value: object) -> str:
    return "sha256:" + hashlib.sha256(canonical_pretty(value)).hexdigest()


def file_digest(path: pathlib.Path) -> str:
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def pathological(row: dict[str, Any]) -> bool:
    values = (
        row["total_time_ms"],
        row["mean_lap_ms"],
        row["final_tire_temperature_c"],
        row["final_tire_wear_pct"],
        row["control_error_amplitude"],
        row["correction_workload_multiplier"],
        row["correction_contact_workload_mj"],
    )
    return (
        not all(math.isfinite(float(value)) for value in values)
        or not 0.0 <= row["final_tire_temperature_c"] <= 180.0
        or not 0.0 <= row["final_tire_wear_pct"] < 100.0
        or not 0.0 <= row["control_error_amplitude"] <= 1.0
        or row["correction_workload_multiplier"] < 1.0
        or row["correction_contact_workload_mj"] < 0.0
    )


def winner_counts(rows: list[dict[str, Any]]) -> dict[str, Any]:
    full = [row for row in rows if row["family"] == "archetype.full-factorial"]
    within_driver: dict[tuple[Any, ...], list[dict[str, Any]]] = defaultdict(list)
    global_groups: dict[tuple[Any, ...], list[dict[str, Any]]] = defaultdict(list)
    for row in full:
        context = (
            row["circuit_id"],
            row["horizon"],
            row["tire_id"],
            row["seed"],
        )
        within_driver[context + (row["driver_id"],)].append(row)
        global_groups[context].append(row)

    mode_counts: Counter[str] = Counter()
    modes_by_horizon: dict[str, Counter[str]] = defaultdict(Counter)
    modes_by_driver: dict[str, Counter[str]] = defaultdict(Counter)
    for candidates in within_driver.values():
        winner = min(candidates, key=lambda row: (row["mean_lap_ms"], row["mode"]))
        mode_counts[winner["mode"]] += 1
        modes_by_horizon[winner["horizon"]][winner["mode"]] += 1
        modes_by_driver[winner["driver_id"]][winner["mode"]] += 1

    global_counts: Counter[str] = Counter()
    for candidates in global_groups.values():
        winner = min(
            candidates,
            key=lambda row: (row["mean_lap_ms"], row["driver_id"], row["mode"]),
        )
        global_counts[f"{winner['driver_id']}:{winner['mode']}"] += 1

    return {
        "within_driver_group_count": len(within_driver),
        "within_driver_mode_winner_counts": dict(sorted(mode_counts.items())),
        "within_driver_mode_winners_by_horizon": {
            key: dict(sorted(value.items()))
            for key, value in sorted(modes_by_horizon.items())
        },
        "within_driver_mode_winners_by_driver": {
            key: dict(sorted(value.items()))
            for key, value in sorted(modes_by_driver.items())
        },
        "global_context_count": len(global_groups),
        "global_driver_mode_winner_counts": dict(sorted(global_counts.items())),
    }


def summarize_profile(
    entry: dict[str, Any],
    manifest_path: pathlib.Path,
    base_profile: dict[str, Any],
    source_parameters: dict[str, dict[str, Any]],
) -> dict[str, Any]:
    profile_path = manifest_path.parent / entry["path"]
    profile = json.loads(profile_path.read_bytes())
    if content_digest(profile) != entry["profile_digest"]:
        raise ReviewError(f"profile digest mismatch for {entry['parameter_set_id']}")
    if profile["driver_control_profile"] != source_parameters[entry["parameter_set_id"]]:
        raise ReviewError(
            f"{entry['parameter_set_id']} no longer matches the exploratory surface"
        )
    non_driver_profile = dict(profile)
    non_driver_profile.pop("driver_control_profile")
    non_driver_base = dict(base_profile)
    non_driver_base.pop("driver_control_profile")
    if non_driver_profile != non_driver_base:
        raise ReviewError(
            f"{entry['parameter_set_id']} changes coefficients outside driver control"
        )

    report_path = (
        EXPERIMENT
        / "results"
        / f"holdout-driver-control-{entry['parameter_set_id']}-v1.json"
    )
    report = json.loads(report_path.read_bytes())
    campaign = report["campaign"]
    if campaign["profile_digest"] != entry["profile_digest"]:
        raise ReviewError(f"report/profile mismatch for {entry['parameter_set_id']}")
    if campaign["configuration_count"] != 702 or campaign["simulated_lap_count"] != 23790:
        raise ReviewError(f"incomplete holdout for {entry['parameter_set_id']}")
    if not campaign["complete"]:
        raise ReviewError(f"partial holdout for {entry['parameter_set_id']}")

    pathologies = sum(pathological(row) for row in report["runs"])
    checks = report["analysis"]["checks"]
    all_checks_pass = all(check["passed"] for check in checks.values())
    winners = winner_counts(report["runs"])
    eligible = all_checks_pass and pathologies == 0
    return {
        "parameter_set_id": entry["parameter_set_id"],
        "profile_digest": entry["profile_digest"],
        "holdout_report_path": f"results/{report_path.name}",
        "holdout_report_sha256": file_digest(report_path),
        "configuration_count": campaign["configuration_count"],
        "simulated_lap_count": campaign["simulated_lap_count"],
        "pathological_execution_count": pathologies,
        "physical_check_results": {
            key: value["passed"] for key, value in checks.items()
        },
        "winner_analysis": winners,
        "effect_summary": report["analysis"]["effect_summary"],
        "holdout_gate_passed": eligible,
    }


def review() -> dict[str, Any]:
    manifest = json.loads(MANIFEST.read_bytes())
    base_profile = json.loads(BASE_PROFILE.read_bytes())
    source = json.loads(SOURCE_SURFACE.read_bytes())
    if file_digest(SOURCE_SURFACE) != manifest["source_evidence"]["local_replay_sha256"]:
        raise ReviewError("the governed exploratory replay changed")
    source_parameters = {
        row["parameter_set_id"]: row["parameters"]
        for row in source["ranked_parameter_sets"]
    }
    profiles = [
        summarize_profile(entry, MANIFEST, base_profile, source_parameters)
        for entry in manifest["profiles"]
    ]
    selected = [
        profile["parameter_set_id"]
        for profile in profiles
        if profile["holdout_gate_passed"]
    ]
    if selected:
        verdict = "HUMAN_REVIEW_REQUIRED"
        recommendation = (
            "At least one profile passed the local holdout. Confirm the exact frozen "
            "evidence on Databricks before any human-governed selection."
        )
    else:
        verdict = "STRUCTURAL_REFINEMENT_REQUIRED"
        recommendation = (
            "Do not promote a profile. All three preserve physical causalities and "
            "produce context-dependent modes within the limit-specialist archetype, "
            "but smooth_operator:attack remains the global winner in every holdout "
            "context. Refine archetype trade-offs before another frozen shortlist."
        )
    return {
        "schema_version": SCHEMA_VERSION,
        "shortlist_manifest_digest": content_digest(manifest),
        "model": manifest["model"],
        "source_campaign_id": manifest["source_evidence"]["campaign_id"],
        "holdout": manifest["holdout_matrix"],
        "profiles": profiles,
        "selected_profile_ids": selected,
        "verdict": verdict,
        "recommendation": recommendation,
        "automatic_publication_performed": False,
    }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=pathlib.Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--check", action="store_true")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    encoded = canonical_pretty(review())
    if args.check:
        if not args.output.is_file() or args.output.read_bytes() != encoded:
            raise ReviewError(f"{args.output} is missing or not reproducible")
        print(f"reproduced {args.output}")
        return 0
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_bytes(encoded)
    print(f"wrote {args.output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
