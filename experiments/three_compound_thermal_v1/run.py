#!/usr/bin/env python3
"""Build the pinned local probe, run serially, retain success or failure evidence."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
from datetime import datetime, timezone

ROOT = Path(__file__).resolve().parents[2]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def sources():
    paths = list((ROOT / "crates").rglob("*.rs"))
    paths += list((ROOT / "crates").rglob("Cargo.toml"))
    paths += [ROOT / "Cargo.toml", ROOT / "Cargo.lock"]
    paths += [p for p in (ROOT / "catalogs/racing/v1.10.1").rglob("*") if p.is_file()]
    paths += [ROOT / "experiments/qualified_career_v1/native-reference-v1.json"]
    return {str(p.relative_to(ROOT)): digest(p) for p in sorted(paths)}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--target-dir", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--policy", type=Path, default=ROOT / "experiments/three_compound_thermal_v1/candidate-v1.json")
    args = parser.parse_args()
    policy = args.policy.resolve()
    source_hashes = sources()
    env = dict(os.environ, CARGO_BUILD_JOBS="2", CARGO_TARGET_DIR=str(args.target_dir.resolve()))
    subprocess.run(["cargo", "build", "--offline", "--release", "-p", "pitgun-racing-simulator",
                    "--example", "three_compound_thermal_probe"], cwd=ROOT, env=env, check=True)
    binary = args.target_dir.resolve() / "release/examples/three_compound_thermal_probe"
    command = [str(binary), str(policy), "--extended"]
    provenance = {
        "utc": datetime.now(timezone.utc).isoformat(),
        "source_commit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
        "source_dirty": bool(subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT, text=True)),
        "source_files_sha256": source_hashes,
        "policy_file_sha256": digest(policy),
        "binary_sha256": digest(binary),
        "command": command,
    }
    run = subprocess.run(command, cwd=ROOT, text=True, capture_output=True)
    stable = source_hashes == sources() and digest(policy) == provenance["policy_file_sha256"] and digest(binary) == provenance["binary_sha256"]
    report = {"provenance": provenance, "source_stable": stable, "returncode": run.returncode, "stderr": run.stderr}
    if run.returncode == 0:
        report["result"] = json.loads(run.stdout)
    else:
        report["failed_stdout"] = run.stdout
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + "\n")
    if run.returncode or not stable:
        raise SystemExit("Probe failed or sources changed; failed attempt retained in output.")
    cases = report["result"]["cases"]
    repeats = [c["repeat_exact"] for c in cases if c.get("repeat_exact") is not None]
    assert len(cases) == 41 and len(repeats) == 6 and all(repeats)
    for case in cases:
        ledger = case["summary"]["thermal_ledger"]
        assert ledger["maximum_tire_balance_residual_j"] < 1e-5
        assert abs(ledger["tire_generated_heat_j"] + ledger["tire_direct_environment_heat_j"]
                   - ledger["tire_rolling_dissipation_j"] - ledger["tire_deformation_dissipation_j"]) < 1e-5
    print(f"{len(cases)} cases + {len(repeats)} exact repeats; balances and source stability pass. {args.output}")


if __name__ == "__main__":
    main()
