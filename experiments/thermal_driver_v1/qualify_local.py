#!/usr/bin/env python3
"""Bounded native qualification. No builds, network, browser or deployments.

Usage: qualify_local.py PROBE ENERGY_PROBE PRIVATE_OUTPUT
Requires a committed source tree. Stores raw receipts privately as it progresses.
"""
import hashlib
import json
import pathlib
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[2]
EXP = pathlib.Path(__file__).resolve().parent


def read(path):
    return json.loads(path.read_text())


def digest(path):
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def invoke(binary, args, payload, scratch):
    scratch.write_text(json.dumps(payload))
    p = subprocess.run([str(binary), *map(str, args)], capture_output=True, text=True, timeout=120)
    if p.returncode:
        raise RuntimeError(f"probe failed: {p.returncode}: {p.stderr}")
    return json.loads(p.stdout)


def main():
    probe, energy_probe, output = map(lambda p: pathlib.Path(p).resolve(), sys.argv[1:])
    output.mkdir(parents=True, exist_ok=False)
    plan = read(EXP / "plan-v1.json")
    cases = plan["cases"]
    source = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    dirty = subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT, text=True).strip()
    if dirty:
        raise RuntimeError("commit sources and plan before measuring")
    results = []
    with tempfile.TemporaryDirectory(prefix="pitgun-driver-") as temp:
        scratch = pathlib.Path(temp) / "case.json"
        for repeat in range(plan["repeats"]):
            samples = []
            for case in cases:
                print(f"pass {repeat + 1}: {case['id']}", flush=True)
                receipt = invoke(probe, [ROOT, scratch, EXP / "profile-v1.json"], case, scratch)
                samples.append(receipt)
                (output / f"{repeat}-{case['id']}.json").write_text(json.dumps(receipt, indent=2) + "\n")
            results.append(samples)
        # Eleven historical one-car scenarios, two equations each. The grid10
        # scenario is outside this focused driver qualification, explicitly omitted.
        energy_plan = read(ROOT / "experiments/longitudinal_energy/plan-v1.json")
        requests = {c["id"]: c["request"] for c in read(ROOT / "experiments/physics_foundation/plan-v1.json")["cases"]}
        requests.update({c["id"]: c["request"] for c in energy_plan["extra_cases"]})
        golden = read(ROOT / "experiments/longitudinal_energy/results/local-energy-v1.json")
        historical = []
        for old_case in golden["cases"]:
            request = requests[old_case["id"]]
            if len(request["input"]["competitors"]) != 1:
                continue
            for mode in ["legacy", "energy"]:
                print(f"historical: {old_case['id']} {mode}", flush=True)
                receipt = invoke(energy_probe, [scratch, ROOT / "catalogs/racing/v1.8.1", mode], request, scratch)
                def signature(data):
                    return {k: {"input_digest": v["input_digest"], "ok": v["output"]["ok"], "digest": v["output"].get("result_digest"), "error": v["output"].get("error")} for k, v in data["competitors"].items()}
                new = signature(receipt)
                old = signature(old_case["outcomes"][mode])
                historical.append({"case": old_case["id"], "mode": mode, "equal": new == old, "observed": new, "frozen": old})
                (output / f"historical-{old_case['id']}-{mode}.json").write_text(json.dumps(receipt, indent=2) + "\n")
    report = {
        "schema": "pitgun.thermal-driver-qualification-receipt/v1",
        "identity": {"source_commit": source, "plan_digest": digest(EXP / "plan-v1.json"), "profile_digest": digest(EXP / "profile-v1.json"), "runner_digest": digest(pathlib.Path(__file__)), "binary_digest": digest(probe), "energy_binary_digest": digest(energy_probe)},
        "repeat_exact": results[0] == results[1],
        "all_completed": all(r["outcome"]["ok"] for r in results[0]),
        "physical_baseline_equal": all(r["outcome"].get("frozen_digest_equal") is True for r in results[0] if r["resolution"] is None),
        "service_equal": all(r["outcome"].get("service_equal") is True for r, c in zip(results[0], cases) if c.get("compare_service")),
        "historical_equal": all(r["equal"] for r in historical),
        "historical": historical,
        "results": results[0],
        "scope": plan["scope"],
    }
    (output / "qualification-v1.json").write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    print(json.dumps({k: v for k, v in report.items() if isinstance(v, bool)}, indent=2))


if __name__ == "__main__":
    main()
