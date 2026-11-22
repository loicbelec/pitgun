#!/usr/bin/env python3
"""Frozen local design: resolution, native baselines, coarse screen, confirmation."""
import concurrent.futures as cf
import hashlib
import json
import pathlib
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[2]
EXP = pathlib.Path(__file__).resolve().parent
PLAN = json.loads((EXP / "plan-v1.json").read_text())


def sha(path):
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def write(path, data):
    path.write_text(json.dumps(data, indent=2, allow_nan=False) + "\n")


def main():
    if len(sys.argv) != 2:
        raise SystemExit("usage: run_local.py /absolute/path/to/career_physics_probe")
    binary = pathlib.Path(sys.argv[1]).resolve()
    out = EXP / "results"
    out.mkdir(exist_ok=True)
    if (out / "campaign-v1.json").exists():
        raise SystemExit("refusing to overwrite a frozen campaign")
    fixtures = json.loads((ROOT / PLAN["fixtures"]).read_text())["fixtures"]
    by_id = {f["id"]: f for f in fixtures}
    sources = ["crates/pitgun-racing-simulator/src/thermal_career.rs",
               "crates/pitgun-racing-simulator/examples/career_physics_probe.rs",
               "crates/pitgun-racing-solver/src/lib.rs",
               "crates/pitgun-racing-solver/src/lap_boundary.rs",
               "crates/pitgun-racing-solver/src/tire_reachability.rs",
               "crates/pitgun-racing-solver/src/thermal_physics.rs",
               "crates/pitgun-racing-solver/src/longitudinal_energy.rs",
               "crates/pitgun-racing-simulator/src/thermal_driver.rs",
               "experiments/career_physics_v1/plan-v1.json",
               "experiments/career_physics_v1/profile-v1.json",
               "experiments/career_physics_v1/run_local.py", PLAN["fixtures"]]
    provenance = {"source_commit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
                  "sources": {s: sha(ROOT / s) for s in sources}, "binary_digest": sha(binary),
                  "performance": "no clocks collected; up to four concurrent native processes"}
    write(out / "provenance-v1.json", provenance)
    audits = [json.loads(subprocess.check_output([str(binary), str(ROOT), "audit"])) for _ in range(2)]
    if audits[0] != audits[1]:
        raise RuntimeError("mapping is not exact across repeats")
    write(out / "mapping-v1.json", audits[0])
    print("mapping:54 requests, exact repeat", flush=True)
    results = []
    with tempfile.TemporaryDirectory(prefix="pitgun-career-screen-") as td:
        tmp = pathlib.Path(td)

        def run(case):
            p = tmp / (case["id"] + ".json")
            write(p, case)
            try:
                data = json.loads(subprocess.check_output([str(binary), str(ROOT), str(p)], stderr=subprocess.PIPE))
            except subprocess.CalledProcessError as error:
                # Resolution/execution infrastructure errors are not simulated DNFs.
                raise RuntimeError(f"probe failure:{case['id']}:{error.stderr.decode()}") from error
            return {"case": case, "data": data}

        def batch(cases, repeat):
            with cf.ThreadPoolExecutor(max_workers=4) as pool:
                rows = list(pool.map(run, cases))
            if repeat:
                with cf.ThreadPoolExecutor(max_workers=4) as pool:
                    again = list(pool.map(run, cases))
                if rows != again:
                    raise RuntimeError("native result changed across exact repeat")
            results.extend(rows)
            print(f"completed {len(cases)} cases; exact repeat={repeat}", flush=True)
            return rows

        # The three authored doctrine routes are physical aliases here, not trials.
        base_cases = []
        for f in fixtures:
            if f["doctrine"] != "craft_engineering":
                continue
            for p in f["preparedRequests"]:
                for forecast in ["minimum_grip", "reachable_wear_v1"]:
                    base_cases.append({"id": f"base-{f['stage']}-{p['circuitId']}-{forecast}", "fixture_id": f["id"], "circuit": p["circuitId"], "forecast": forecast})
        baseline = batch(base_cases, True)
        write(out / "baselines-v1.json", baseline)
        screen = []
        for fid in PLAN["screen"]["fixtures"]:
            f = by_id[fid]
            budget = f["discretionaryPoints"]
            allocations = {}
            for fraction in PLAN["screen"]["concentrated_engine_fractions"]:
                engine = int(budget * fraction + 0.5)
                allocations[(0, budget-engine, engine, 0)] = "concentrated"
            authored = next(c for c in f["preparedRequests"][0]["request"]["input"]["competitors"] if c["is_player"])["tuning"]
            authored = tuple(int(authored[k+"_points"]-f["offsets"][k]) for k in ["aero", "chassis", "engine", "cooling"])
            allocations[authored] = "diversified"
            engine = int(budget * 0.5 + 0.5)
            for axis in [0,3]:
                for transfer in [1,2]:
                    allocation = [0,budget-engine,engine,0]
                    for j in range(transfer):
                        allocation[2 if j % 2 == 0 else 1] -= 1
                    allocation[axis] += transfer
                    allocations[tuple(allocation)] = "diversified"
            dfs = PLAN["screen"]["downforce_values_when_unlocked"] if f["sliders"]["downforce"][0] != f["sliders"]["downforce"][1] else [f["sliders"]["downforce"][0]]
            for circuit in PLAN["screen"]["circuits"]:
                for alloc, category in sorted(allocations.items()):
                    for gear in PLAN["screen"]["gear_values"]:
                        for df in dfs:
                            case = {"id":f"screen-{f['stage']}-{circuit}-{'_'.join(map(str,alloc))}-g{gear}-d{df}","fixture_id":fid,"circuit":circuit,"allocation":alloc,"gear":gear,"downforce":df,"five_laps":True,"forecast":"reachable_wear_v1"}
                            screen.append((case, category))
        screen_rows = batch([c for c,_ in screen], False)
        categories = {c["id"]:category for c,category in screen}
        groups = {}
        for row in screen_rows:
            case = row["case"]
            key = (case["fixture_id"],case["circuit"],categories[case["id"]])
            if row["data"]["outcome"]["ok"]:
                groups.setdefault(key, []).append(row)
        shortlist=[]
        for key, rows in sorted(groups.items()):
            chosen=min(rows,key=lambda r:(r["data"]["outcome"]["time_s"],r["case"]["id"]))
            for forecast in ["minimum_grip","reachable_wear_v1"]:
                case=dict(chosen["case"],id=chosen["case"]["id"].replace("screen-","confirm-")+"-"+forecast,forecast=forecast,five_laps=False)
                shortlist.append(case)
        write(out / "screen-v1.json", screen_rows)
        write(out / "shortlist-v1.json", shortlist)
        confirmation=batch(shortlist,True)
        write(out / "confirmation-v1.json",confirmation)
        write(out / "campaign-v1.json", {"schema":"pitgun.career-physics-campaign/v1","provenance":provenance,"baseline_count":len(baseline),"screen_count":len(screen_rows),"confirmation_count":len(confirmation),"mapping_exact_repeat":True,"baseline_exact_repeat":True,"confirmation_exact_repeat":True,"screen_exact_repeat":"not requested; shortlisted setups confirmed at full GP only","files":{p.name:sha(p) for p in sorted(out.glob("*-v1.json")) if p.name!="campaign-v1.json"}})
        print("campaign complete",flush=True)


if __name__ == "__main__":
    main()
