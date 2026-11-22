#!/usr/bin/env python3
"""Frozen native screen/regressions, then serial paired cost after other tests end.

Usage: qualify_local.py screen|cost EXAMPLE_DIRECTORY PRIVATE_DESTINATION
"""
import hashlib
import json
from pathlib import Path
import statistics
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[2]
EXP = Path(__file__).resolve().parent


def read(path):
    return json.loads(path.read_text())


def digest(path):
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    action = sys.argv[1]
    binaries, destination = map(lambda p: Path(p).resolve(), sys.argv[2:])
    if action not in ["screen", "cost"]:
        raise ValueError("expected screen|cost")
    plan = read(EXP / "plan-v1.json")
    if action == "screen":
        destination.mkdir(parents=True, exist_ok=False)
    source = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    if subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT, text=True).strip():
        raise RuntimeError("commit code and plan before measurement")
    identity = {"source_commit": source, "plan_digest": digest(EXP / "plan-v1.json"), "runner_digest": digest(Path(__file__)), "binary_digest": digest(binaries / "lap_boundary_probe")}
    with tempfile.TemporaryDirectory(prefix="pitgun-boundary-") as temp:
        scratch = Path(temp) / "case.json"
        def invoke(name, case, args):
            scratch.write_text(json.dumps(case))
            p = subprocess.run([str(binaries / name), *map(str, args)], text=True, capture_output=True, timeout=150)
            if p.returncode:
                raise RuntimeError(p.stderr)
            return json.loads(p.stdout)
        def boundary(scenario, mode):
            case = {"id": f"{scenario}:{mode['id']}", "frozen_case": scenario, "forecast": mode["forecast"], "recompute_invariant": mode["recompute_invariant"]}
            return invoke("lap_boundary_probe", case, [ROOT, scratch])
        if action == "screen":
            passes = []
            for repeat in range(plan["repeats"]):
                rows = {}
                for scenario in plan["scenarios"]:
                    for mode in plan["modes"]:
                        print(f"screen{repeat+1}: {scenario} {mode['id']}", flush=True)
                        sample = boundary(scenario, mode)
                        rows[sample["data"]["id"]] = sample
                        (destination / f"screen-{repeat}-{scenario}-{mode['id']}.json").write_text(json.dumps(sample, indent=2) + "\n")
                passes.append(rows)
            cached_pairs_equal = True
            all_floor_complete = True
            for scenario in plan["scenarios"]:
                a, b = [passes[0][f"{scenario}:{mode}"]["data"] for mode in ["cached", "recompute"]]
                cached_pairs_equal &= a["physical_input_digest"] == b["physical_input_digest"] and a["outcome"] == b["outcome"] and a["laps"] == b["laps"]
                all_floor_complete &= a["outcome"]["ok"] and b["outcome"]["ok"]
            repeat_exact = {k:v["data"] for k,v in passes[0].items()} == {k:v["data"] for k,v in passes[1].items()}
            driver = ROOT / "experiments/thermal_driver_v1"
            golden = {r["id"]:r for r in read(driver / "results/qualification-v1.json")["results"]}
            regression = []
            for case in read(driver / "plan-v1.json")["cases"]:
                print(f"driver regression: {case['id']}", flush=True)
                actual = invoke("thermal_driver_probe", case, [ROOT, scratch, driver / "profile-v1.json"])
                old = golden[case["id"]]
                regression.append({"id":case["id"],"equal":actual["input_digest"] == old["input_digest"] and actual["outcome"] == old["outcome"],"input_digest":actual["input_digest"],"outcome":actual["outcome"]})
            requests = {c["id"]:c["request"] for c in read(ROOT / "experiments/physics_foundation/plan-v1.json")["cases"]}
            requests.update({c["id"]:c["request"] for c in read(ROOT / "experiments/longitudinal_energy/plan-v1.json")["extra_cases"]})
            historical = []
            def signature(result):
                return {id:{"input_digest":s["input_digest"],"ok":s["output"]["ok"],"digest":s["output"].get("result_digest"),"error":s["output"].get("error")} for id,s in result["competitors"].items()}
            for case in read(ROOT / "experiments/longitudinal_energy/results/local-energy-v1.json")["cases"]:
                request = requests[case["id"]]
                if len(request["input"]["competitors"]) != 1:
                    continue
                for mode in ["legacy", "energy"]:
                    print(f"historical: {case['id']} {mode}", flush=True)
                    actual = invoke("longitudinal_energy_probe", request, [scratch, ROOT / "catalogs/racing/v1.8.1", mode])
                    observed = signature(actual)
                    historical.append({"id":case["id"],"mode":mode,"equal":observed == signature(case["outcomes"][mode]),"observed":observed})
            report = {"schema":"pitgun.lap-boundary-screen/v1","identity":identity,"repeat_exact":repeat_exact,"cached_pairs_equal":cached_pairs_equal,"all_floor_complete":all_floor_complete,"driver_regression_equal":all(r["equal"] for r in regression),"historical_equal":all(r["equal"] for r in historical),"results":{k:v["data"] for k,v in passes[0].items()},"driver_regression":regression,"historical":historical}
            (destination / "screen-v1.json").write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
            print(json.dumps({k:v for k,v in report.items() if isinstance(v,bool)}))
        else:
            screen = read(destination / "screen-v1.json")
            if screen["identity"] != identity or not all(screen[k] for k in ["repeat_exact","cached_pairs_equal","all_floor_complete","driver_regression_equal","historical_equal"]):
                raise RuntimeError("first qualify identical outputs under the same source/binary")
            samples, summary = [], []
            for scenario in plan["cost_scenarios"]:
                timings = {"cached":[], "recompute":[]}
                for repetition in range(plan["cost_repetitions"]):
                    modes = plan["modes"][:2] if repetition % 2 == 0 else list(reversed(plan["modes"][:2]))
                    for mode in modes:
                        print(f"cost{repetition+1}: {scenario} {mode['id']}", flush=True)
                        sample = boundary(scenario, mode)
                        if sample["data"] != screen["results"][f"{scenario}:{mode['id']}"]:
                            raise RuntimeError("cost run did not reproduce the qualified data")
                        timings[mode["id"]].append(sample["performance"]["solver_ms"])
                        samples.append({"scenario":scenario,"mode":mode["id"],"repetition":repetition,"performance":sample["performance"]})
                cached, repeated = [statistics.median(timings[k]) for k in ["cached","recompute"]]
                summary.append({"scenario":scenario,"cached_median_ms":cached,"recompute_median_ms":repeated,"speedup_ratio":repeated/cached,"time_reduction_fraction":1-cached/repeated,"samples":timings})
            report = {"schema":"pitgun.lap-boundary-cost/v1","identity":identity,"scope":"serial alternating independent native processes; sum of advance calls only, excludes catalog/metrics/digest/JSON. Other machine activity not controlled; not browser/WASM/VPS or statistical throughput qualification.","summary":summary,"samples":samples}
            (destination / "cost-v1.json").write_text(json.dumps(report, indent=2) + "\n")
            print(json.dumps(summary, indent=2))


if __name__ == "__main__":
    main()
