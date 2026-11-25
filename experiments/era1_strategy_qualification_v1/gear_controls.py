#!/usr/bin/env python3
"""Post-plan confound check: legal gear-only contrasts with downforce fixed."""
import argparse
import copy
import hashlib
import json
from pathlib import Path

from run import Campaign, ROOT, cases, encoded, sha, write


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--cache", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    plan = json.loads((Path(__file__).parent / "plan-v1.json").read_text())
    matrix = []
    for base in cases(plan):
        if (base["id"].split("/")[1] in plan["setups"] and base["id"].endswith("/SM-50")):
            for label, gear in [("long", .85), ("short", .35)]:
                case = copy.deepcopy(base)
                case["id"] += "/gear-" + label
                case["gear"] = gear
                assert case["downforce"] == .5
                matrix.append(case)
    assert len(matrix) == 18
    # Manifest retained before execution. Cases depend only on the original
    # matrix definitions, never on their finishing times or selected winners.
    manifest = {"schema": "pitgun.era1-gear-confound-control/v1", "cases": matrix,
                "cases_sha256": hashlib.sha256(encoded(matrix)).hexdigest(),
                "runner_sha256": sha(Path(__file__).resolve()),
                "reason": "Era-I baseline aero has no adjustable_downforce. Original paired sliders remain physical diagnostics; these isolate legal gearing on all three point profiles and circuits, with the same SM plan."}
    args.cache.mkdir(parents=True, exist_ok=True)
    lock = args.cache / "gear-controls-manifest.json"
    if lock.exists() and json.loads(lock.read_text()) != manifest:
        raise RuntimeError("gear-control manifest mismatch")
    write(lock, manifest)
    campaign = Campaign(plan, args.binary, args.cache)
    rows = []
    for case in matrix:
        rows.append(campaign.execute(case))
        write(args.output, {"manifest": manifest, "provenance": campaign.pin, "rows": rows,
                            "complete": len(rows) == len(matrix)})
        print(f"gear-only {len(rows)}/18 {case['id']} {rows[-1]['checks'] or 'OK'}", flush=True)
    campaign.verify()


if __name__ == "__main__":
    main()
