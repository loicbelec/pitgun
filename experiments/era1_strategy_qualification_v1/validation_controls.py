#!/usr/bin/env python3
"""Compatible full-result repeats; retain legacy-switch rejections separately."""
import argparse
import copy
import hashlib
from pathlib import Path

from report import load
from run import Campaign, encoded, sha, write


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("primary", type=Path)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--cache", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    primary = load(args.primary)
    assert primary["complete"]
    plan = primary["plan"]
    selection = [v for v in primary["validations"] if v["kind"] == "recompute"]
    assert len(selection) == 6
    assert all(v["fresh"]["data"]["outcome"].get("error") ==
               "reachable wear forecast does not use the legacy invariant-work switch" for v in selection)
    lookup = {r["case"]["id"]: r for r in primary["rows"]}
    cases = [copy.deepcopy(lookup[v["baseline_id"]]["case"]) for v in selection]
    assert all(not case["recompute_invariant"] for case in cases)
    manifest = {"schema": "pitgun.era1-compatible-validation/v1", "cases": cases,
                "cases_sha256": hashlib.sha256(encoded(cases)).hexdigest(),
                "runner_sha256": sha(Path(__file__).resolve()),
                "primary_file_sha256": sha(args.primary),
                "reason": "The frozen harness selected a legacy-only work switch rejected by ReachableWearV1. Keep all6 rejections. Repeat the same6 preselected physical cases without the inapplicable switch; this is full-result repeatability, not a second boundary algorithm."}
    args.cache.mkdir(parents=True, exist_ok=True)
    write(args.cache / "compatible-validation-manifest.json", manifest)
    campaign = Campaign(plan, args.binary, args.cache)
    validations = []
    for case in cases:
        baseline = lookup[case["id"]]
        fresh = campaign.execute(case, validation="reachable-repeat")
        exact = (not baseline["checks"] and not fresh["checks"] and
                 baseline["data"]["outcome"]["result_digest"] == fresh["data"]["outcome"].get("result_digest") and
                 baseline["data"]["receipt"]["physical_input_digest"] == fresh["data"].get("receipt", {}).get("physical_input_digest"))
        validations.append({"kind": "compatible-repeat", "baseline_id": case["id"], "exact": bool(exact), "fresh": fresh})
        write(args.output, {"manifest": manifest, "provenance": campaign.pin, "validations": validations,
                            "complete": len(validations) == 6})
        print(f"compatible validation {len(validations)}/6 {case['id']} exact={exact}", flush=True)
    campaign.verify()


if __name__ == "__main__":
    main()
