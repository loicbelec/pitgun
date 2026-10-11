#!/usr/bin/env python3
"""Generate and replay a bounded candidate challenge using the Rust workload."""

import argparse
import copy
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent
SOURCE = ROOT / "experiments/opponent_acceptance/scenarios/suzuka-mid-42-balanced.json"
BINARY = ROOT / "target/release/examples/engineer_baseline_probe"


def digest(data):
    return "sha256:" + hashlib.sha256(data).hexdigest()


def encoded(value):
    # Stored-file encoding, not a substitute for the Rust JCS contract.
    return (json.dumps(value, indent=2, ensure_ascii=False, allow_nan=False) + "\n").encode()


def plan(stints):
    stops, total = [], 0
    for _, laps in stints[:-1]:
        total += laps
        stops.append(total)
    return {"stints": [{"tire_id": tire, "laps": laps} for tire, laps in stints], "pit_laps": stops}


def build_manifest():
    source_bytes = SOURCE.read_bytes()
    source = json.loads(source_bytes)
    request = copy.deepcopy(source["request"])
    request.pop("seed", None)
    request["laps"] = 12
    request["pit_strategy"] = {"player_pit_laps": []}
    for competitor in request["competitors"]:
        competitor["stint_strategy"] = plan([("medium", 6), ("hard", 6)])
    plans = {
        "soft-no-stop": [("soft", 12)],
        "medium-no-stop": [("medium", 12)],
        "hard-no-stop": [("hard", 12)],
        "early-mh": [("medium", 4), ("hard", 8)],
        "balanced-mh": [("medium", 6), ("hard", 6)],
        "late-mh": [("medium", 8), ("hard", 4)],
        "two-stop-smh": [("soft", 4), ("medium", 4), ("hard", 4)],
    }
    modes = {
        "balanced": [],
        "attack": [(1, "attack")],
        "manage": [(1, "manage")],
        "manage-then-attack": [(1, "manage"), (6, "attack")],
    }
    cases = []
    for name, stints in plans.items():
        for mode, transitions in modes.items():
            current = copy.deepcopy(request)
            player = next(c for c in current["competitors"] if c["id"] == "player")
            player["stint_strategy"] = plan(stints)
            current["pit_strategy"]["player_pit_laps"] = player["stint_strategy"]["pit_laps"]
            events = [{"sequence": index, "competitor_id": "player",
                       "effective_at": {"lap_index": lap, "segment_index": 0}, "mode": value}
                      for index, (lap, value) in enumerate(transitions)]
            cases.append({"id": f"{name}--{mode}", "case": {
                "request": current, "seed": 42,
                "timeline": {"schema_version": "pitgun.racing-driver-instruction-timeline/v1", "events": events},
            }})
    return {
        "schema_version": "pitgun.engineer-baseline-screen/v1",
        "status": "candidate-screen-not-official-challenge",
        "source": {"path": str(SOURCE.relative_to(ROOT)), "file_digest": digest(source_bytes)},
        "model": source["model"], "data_pack": source["data_pack"],
        "baseline": "balanced-mh--balanced",
        "fixed": {"track": "SUZUKA", "laps": 12, "seed": "42", "fuel": "catalog-owned",
                  "opponent_plan": "medium-6-hard-6", "opponent_modes": "balanced"},
        "cases": cases,
    }


def screen(manifest):
    rows = []
    with tempfile.TemporaryDirectory(prefix="pitgun-engineer-") as temporary:
        path = Path(temporary) / "case.json"
        for entry in manifest["cases"]:
            path.write_bytes(encoded(entry["case"]))
            command = [str(BINARY), str(path)]
            first = subprocess.check_output(command, cwd=ROOT)
            second = subprocess.check_output(command, cwd=ROOT)
            if first != second:
                raise RuntimeError(f"fresh-process repetition differs: {entry['id']}")
            result = json.loads(first)
            if result["model"] != manifest["model"]:
                raise RuntimeError("probe model differs from frozen source model")
            player = next(s for s in result["output"]["standings"] if s["competitor_id"] == "player")
            rows.append({"id": entry["id"], "player": player, "result": result,
                         "fresh_process_repeat_equal": True})
            print(entry["id"], player["status"], player["total_time_ms"], flush=True)
    baseline = next(row["player"]["total_time_ms"] for row in rows if row["id"] == manifest["baseline"])
    for row in rows:
        row["delta_to_baseline_ms"] = row["player"]["total_time_ms"] - baseline
    ranked = sorted((row for row in rows if row["player"]["status"]["type"] == "finished"),
                    key=lambda row: (row["player"]["total_time_ms"], row["id"]))
    return {
        "schema_version": "pitgun.engineer-baseline-results/v1",
        "manifest_file_digest": digest(encoded(manifest)),
        "probe_source_digest": digest((ROOT / "crates/pitgun-racing-simulator/examples/engineer_baseline_probe.rs").read_bytes()),
        "baseline": manifest["baseline"], "baseline_time_ms": baseline,
        "ranking_finished": [row["id"] for row in ranked],
        "rows": rows,
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true", help="Reexecute frozen cases and compare stored bytes")
    args = parser.parse_args()
    manifest_path = HERE / "manifest.json"
    manifest = build_manifest()
    if args.check and manifest_path.read_bytes() != encoded(manifest):
        raise RuntimeError("manifest changed")
    report = screen(manifest)
    for path, data in [(manifest_path, encoded(manifest)), (HERE / "results.json", encoded(report))]:
        if args.check:
            if path.read_bytes() != data:
                raise RuntimeError(f"stored artifact differs: {path}")
        elif path.exists():
            if path.read_bytes() != data:
                raise RuntimeError(f"refusing to overwrite changed evidence: {path}")
        else:
            path.write_bytes(data)
    # Runtime identity is operational evidence; it is excluded from deterministic report equality.
    runtime = {"binary_digest": digest(BINARY.read_bytes()),
               "framework_commit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
               "rustc": subprocess.check_output(["rustc", "-Vv"], text=True).strip(),
               "report_file_digest": digest(encoded(report))}
    runtime_path = HERE / "runtime.json"
    if not args.check and not runtime_path.exists():
        runtime_path.write_bytes(encoded(runtime))
    print("CHECKED" if args.check else "RECORDED", len(report["rows"]), "cases; each executed twice")


if __name__ == "__main__":
    main()
