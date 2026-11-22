#!/usr/bin/env python3
"""Pinned exposed native tire-policy screen, exact repeat and old-output regressions."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]

def read(path):
    return json.loads(path.read_bytes())

def digest(path):
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()

def main():
    binaries, destination = [Path(p).resolve() for p in sys.argv[1:]]
    if subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT, text=True).strip():
        raise RuntimeError("commit code/plan/profile before qualification")
    destination.mkdir(parents=True, exist_ok=False)
    plan = read(HERE / "plan-v1.json")
    files = {
        "probe": "crates/pitgun-racing-simulator/examples/tire_operating_probe.rs",
        "preparation": "crates/pitgun-racing-simulator/src/tire_preparation.rs",
        "detector": "crates/pitgun-racing-simulator/src/tire_events.rs",
        "helper": "crates/pitgun-racing-simulator/examples/support/frozen_thermal_input.rs",
        "solver": "crates/pitgun-racing-solver/src/lib.rs",
        "thermal": "crates/pitgun-racing-solver/src/thermal_physics.rs",
    }
    identity = {
        "source_commit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
        "plan_digest": digest(HERE / "plan-v1.json"),
        "profile_digest": digest(HERE / "profile-v1.json"),
        "runner_digest": digest(Path(__file__)),
        "source_digests": {k:digest(ROOT/v) for k,v in files.items()},
        "binaries": {n:digest(binaries/n) for n in ["tire_operating_probe","thermal_driver_probe","longitudinal_energy_probe","native_derating_events_probe"]},
        "lock_digest": digest(ROOT / "Cargo.lock"),
    }
    def invoke(name, args):
        p = subprocess.run([str(binaries/name), *map(str,args)], capture_output=True, text=True, timeout=plan["timeout_s"])
        if p.returncode:
            raise RuntimeError(p.stderr[:1500])
        value = json.loads(p.stdout)
        if value["binary_digest"] != identity["binaries"][name]:
            raise RuntimeError("unexpected executable identity")
        return value
    with tempfile.TemporaryDirectory(prefix="pitgun-tire-policy-") as tmp:
        scratch = Path(tmp)/"case.json"
        passes = []
        for repetition in range(plan["repeats"]):
            rows = []
            for case in plan["cases"]:
                print(f"pass{repetition+1}: {case['id']}", flush=True)
                scratch.write_text(json.dumps(case))
                row = invoke("tire_operating_probe",[ROOT,scratch,HERE/"profile-v1.json"])
                if row["sources"] != identity["source_digests"]:
                    raise RuntimeError("stale compiled source")
                rows.append(row)
            passes.append(rows)
        if passes[0] != passes[1]:
            raise RuntimeError("repeat differs, including errors/accepted prefixes/events")
        drivers = []
        old = read(ROOT/"experiments/thermal_driver_v1/results/qualification-v1.json")
        golden = {r["id"]:r for r in old["results"]}
        for case in read(ROOT/"experiments/thermal_driver_v1/plan-v1.json")["cases"]:
            print(f"driver regression: {case['id']}",flush=True)
            scratch.write_text(json.dumps(case))
            row = invoke("thermal_driver_probe",[ROOT,scratch,ROOT/"experiments/thermal_driver_v1/profile-v1.json"])
            ref = golden[case["id"]]
            equal = row["input_digest"] == ref["input_digest"] and row["outcome"] == ref["outcome"]
            if not equal: raise RuntimeError("driver regression differs: "+case["id"])
            drivers.append({"id":case["id"],"exact":equal,"input_digest":row["input_digest"]})
        requests = {c["id"]:c["request"] for c in read(ROOT/"experiments/physics_foundation/plan-v1.json")["cases"]}
        requests.update({c["id"]:c["request"] for c in read(ROOT/"experiments/longitudinal_energy/plan-v1.json")["extra_cases"]})
        historical = []
        def signature(result):
            return {k:{"input_digest":v["input_digest"],"ok":v["output"]["ok"],"digest":v["output"].get("result_digest"),"error":v["output"].get("error")} for k,v in result["competitors"].items()}
        for c in read(ROOT/"experiments/longitudinal_energy/results/local-energy-v1.json")["cases"]:
            if len(requests[c["id"]]["input"]["competitors"]) != 1: continue
            for mode in ["legacy","energy"]:
                print(f"historical: {c['id']} {mode}",flush=True)
                scratch.write_text(json.dumps(requests[c["id"]]))
                row = invoke("longitudinal_energy_probe",[scratch,ROOT/"catalogs/racing/v1.8.1",mode])
                if signature(row) != signature(c["outcomes"][mode]): raise RuntimeError("historical outcome differs")
                historical.append({"id":c["id"],"mode":mode,"exact":True})
        print("historical V1 physical-event journals",flush=True)
        journal = invoke("native_derating_events_probe",[ROOT,"--tires"])
        old_journal = read(ROOT/"experiments/racing_physical_events/results/tire-v1.json")["bank"]
        if journal["cases"] != old_journal["cases"] or journal["policy"] != old_journal["policy"] or journal["tire_policy"] != old_journal["tire_policy"]:
            raise RuntimeError("old physical outcomes/events/checkpoint evidence differs")
        baseline_equal = all(r["outcome"].get("frozen_outcome_equal") is True for r in passes[0] if r["case"]["baseline"])
        if not baseline_equal: raise RuntimeError("unchanged thermal baseline differs")
        if not all(r["maximum_surface_jump_c"] == 0 and r["maximum_carcass_jump_c"] == 0 for r in passes[0]): raise RuntimeError("thermal continuity differs")
        report = {"schema":"pitgun.tire-operating-screen/v1","identity":identity,"repeat_exact":True,"baseline_equal":baseline_equal,"continuity_exact":True,"driver_regressions":drivers,"historical_regressions":historical,"old_event_cases_equal":len(journal["cases"]),"results":passes[0]}
        path = destination/"screen-v1.json"
        path.write_text(json.dumps(report,sort_keys=True,indent=2)+"\n")
        path.with_suffix(".sha256").write_text(digest(path)+"\n")
        print(json.dumps({"repeat_exact":True,"baseline_equal":baseline_equal,"continuity_exact":True,"driver_regressions":len(drivers),"historical_regressions":len(historical),"old_event_cases_equal":len(journal["cases"]),"outcomes":[{"id":r["case"]["id"],"ok":r["outcome"]["ok"],"error":r["outcome"].get("error")} for r in passes[0]]},indent=2))

if __name__ == "__main__":
    main()
