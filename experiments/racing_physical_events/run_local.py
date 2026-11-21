#!/usr/bin/env python3
"""Qualify opt-in native derating facts against frozen physical outputs."""
import argparse
import hashlib
import json
import pathlib
import platform
import subprocess
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[2]
HERE = pathlib.Path(__file__).resolve().parent
SOURCES = {
    "simulator": "crates/pitgun-racing-simulator/src/lib.rs",
    "detector": "crates/pitgun-racing-simulator/src/physical_events.rs",
    "probe": "crates/pitgun-racing-simulator/examples/native_derating_events_probe.rs",
}


def digest(raw):
    return "sha256:" + hashlib.sha256(raw).hexdigest()


def load(path):
    return json.loads(path.read_bytes())


def invoke(args):
    result = subprocess.run(args, cwd=ROOT, capture_output=True, timeout=180)
    if result.returncode:
        raise RuntimeError(result.stderr.decode(errors="replace")[:2000])
    return json.loads(result.stdout)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--probe", type=pathlib.Path, required=True)
    parser.add_argument("--legacy-probe", type=pathlib.Path, required=True)
    parser.add_argument("--output", type=pathlib.Path, default=HERE / "results/native-v1.json")
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    probe = args.probe.resolve(strict=True)
    legacy = args.legacy_probe.resolve(strict=True)
    files = [
        *SOURCES.values(),
        "crates/pitgun-racing-simulator/examples/thermal_tire_gameplay_probe.rs",
        "crates/pitgun-racing-solver/src/lib.rs",
        "crates/pitgun-racing-solver/src/longitudinal_energy.rs",
        "crates/pitgun-racing-solver/src/thermal_physics.rs",
        "experiments/racing_physical_events/run_local.py",
        "experiments/thermal_physics_v2/plan-v1.json",
        "experiments/thermal_physics_v2/profile-v1.json",
        "experiments/thermal_physics_v2/results/screen-v1.json",
        "experiments/longitudinal_energy/results/local-energy-v1.json",
        "Cargo.lock",
    ]
    subprocess.run(["git", "diff", "--exit-code", "HEAD", "--", *files], cwd=ROOT, check=True)
    for file in files:
        subprocess.run(["git", "ls-files", "--error-unmatch", file], cwd=ROOT, check=True,
                       stdout=subprocess.DEVNULL)
    identity = {
        "source_commit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
        "source_files": {file: digest((ROOT / file).read_bytes()) for file in files},
        "binary_digest": digest(probe.read_bytes()),
        "legacy_binary_digest": digest(legacy.read_bytes()),
        "platform": platform.platform(),
    }
    preflight = []
    old = load(ROOT / "experiments/longitudinal_energy/results/local-energy-v1.json")
    for case in old["cases"]:
        if len(case["request"]["input"]["competitors"]) != 1:
            continue
        for model in ("historical", "energy"):
            with tempfile.TemporaryDirectory(prefix="pitgun-events-preflight-") as tmp:
                request = pathlib.Path(tmp) / "request.json"
                request.write_text(json.dumps({"request": case["request"], "model": model}))
                data = invoke([str(legacy), str(request), str(ROOT / "catalogs/racing/v1.8.1")])
            reference = case["outcomes"]["legacy" if model == "historical" else "energy"]["competitors"]["player"]["output"]
            outcome = data["outcome"]
            key = "result_digest" if reference["ok"] else "error"
            if outcome["ok"] != reference["ok"] or outcome.get(key) != reference[key]:
                raise RuntimeError("changed frozen outcome: " + case["id"])
            if data["binary_digest"] != identity["legacy_binary_digest"]:
                raise RuntimeError("legacy executable identity differs")
            if data["sources"]["simulator"] != identity["source_files"][SOURCES["simulator"]]:
                raise RuntimeError("legacy probe compiled from stale Simulator")
            preflight.append({"case": case["id"], "model": model, "exact_frozen_outcome": True})
    if len(preflight) != 22:
        raise RuntimeError("unexpected historical preflight coverage")
    bank = invoke([str(probe), str(ROOT)])
    if bank["binary_digest"] != identity["binary_digest"]:
        raise RuntimeError("native executable identity differs")
    if bank["sources"] != {key: identity["source_files"][file] for key, file in SOURCES.items()}:
        raise RuntimeError("native probe compiled from stale sources")
    report = {
        "schema": "pitgun.native-derating-events-receipt/v1",
        "identity": identity,
        "preflight": preflight,
        "bank": bank,
        "scope": "Native observations only; no new physics, complete process checkpoint, graphical export, provider visibility or Authority admission.",
    }
    if args.check:
        archived = load(args.output)
        # An evidence-only commit does not change the qualified source identities.
        report["identity"]["source_commit"] = archived["identity"]["source_commit"]
        if report != archived:
            raise RuntimeError("repeat differs; archive untouched")
        print("PASS exact repeat: 22 frozen outcomes + five normal/service event cases")
    else:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        raw = (json.dumps(report, sort_keys=True, indent=2, ensure_ascii=False) + "\n").encode()
        args.output.write_bytes(raw)
        args.output.with_suffix(".sha256").write_text(digest(raw) + "\n")
        print("PASS archived: 22 frozen outcomes + five normal/service event cases")


if __name__ == "__main__":
    main()
