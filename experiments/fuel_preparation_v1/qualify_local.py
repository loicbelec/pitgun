#!/usr/bin/env python3
"""Frozen local screen + previous driver-result regressions; no build/network/UI."""
import hashlib
import json
from pathlib import Path
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
    binaries, destination = map(lambda p: Path(p).resolve(), sys.argv[1:])
    destination.mkdir(parents=True, exist_ok=False)
    if subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT, text=True).strip():
        raise RuntimeError("commit source and plan before measuring")
    source = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    plan = read(EXP / "plan-v1.json")
    passes, regression = [], []
    with tempfile.TemporaryDirectory(prefix="pitgun-fuel-") as temp:
        scratch = Path(temp) / "case.json"
        def run(example, case, policy):
            scratch.write_text(json.dumps(case))
            result = subprocess.run([str(binaries / example), str(ROOT), str(scratch), str(policy)], text=True, capture_output=True, timeout=150)
            if result.returncode:
                raise RuntimeError(result.stderr)
            return json.loads(result.stdout)
        for repeat in range(plan["repeats"]):
            samples = []
            for case in plan["cases"]:
                print(f"fuel pass{repeat+1}: {case['id']}", flush=True)
                sample = run("fuel_preparation_probe", case, EXP / "profile-v1.json")
                (destination / f"{repeat}-{case['id']}.json").write_text(json.dumps(sample, indent=2) + "\n")
                samples.append(sample)
            passes.append(samples)
        driver = ROOT / "experiments/thermal_driver_v1"
        golden = {s["id"]: s for s in read(driver / "results/qualification-v1.json")["results"]}
        for case in read(driver / "plan-v1.json")["cases"]:
            print(f"driver regression: {case['id']}", flush=True)
            sample = run("thermal_driver_probe", case, driver / "profile-v1.json")
            expected = golden[case["id"]]
            equal = sample["input_digest"] == expected["input_digest"] and sample["outcome"] == expected["outcome"]
            regression.append({"id":case["id"], "equal":equal, "input_digest":sample["input_digest"], "outcome_digest":sample["outcome"].get("digest")})
            (destination / f"regression-{case['id']}.json").write_text(json.dumps(sample, indent=2) + "\n")
    report = {"schema":"pitgun.fuel-preparation-screen-receipt/v1", "identity":{"source_commit":source,"plan_digest":digest(EXP / "plan-v1.json"),"policy_digest":digest(EXP / "profile-v1.json"),"runner_digest":digest(Path(__file__)),"fuel_binary_digest":digest(binaries / "fuel_preparation_probe"),"driver_binary_digest":digest(binaries / "thermal_driver_probe")},"repeat_exact":passes[0] == passes[1],"driver_outputs_equal":all(r["equal"] for r in regression),"results":passes[0],"driver_regression":regression}
    (destination / "qualification-v1.json").write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    print(json.dumps({k:v for k,v in report.items() if isinstance(v,bool)}))


if __name__ == "__main__":
    main()
