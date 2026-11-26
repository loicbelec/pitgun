#!/usr/bin/env python3
"""Serial, resumable era-I qualification under an immutable physical policy."""
import argparse
import copy
from datetime import datetime, timezone
import gzip
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]


def encoded(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":"), allow_nan=False).encode()


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write(path, value):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    raw = json.dumps(value, indent=2, allow_nan=False).encode() + b"\n"
    if path.suffix == ".gz":
        raw = gzip.compress(raw, mtime=0)
    temporary = path.with_name(path.name + ".tmp")
    temporary.write_bytes(raw)
    temporary.replace(path)


def strategy(laps, tires, fractions):
    pits = [laps * numerator // denominator for numerator, denominator in fractions]
    bounds = [0] + pits + [laps]
    if len(tires) != len(pits) + 1 or sorted(set(bounds)) != bounds:
        raise ValueError("invalid integer pit boundaries")
    return {"stints": [{"tire_id": tire, "laps": bounds[i + 1] - bounds[i]}
                       for i, tire in enumerate(tires)], "pit_laps": pits}


def cases(plan):
    result = []
    for context in plan["contexts"]:
        circuit, laps = context["circuit"], context["laps"]

        def add(setup_id, plan_id, setup, recipe, mode="balanced", seed=None, suffix=""):
            result.append({"id": f"{circuit}/{setup_id}/{plan_id}{suffix}", "circuit": circuit,
                           "seed": plan["seed"] if seed is None else seed,
                           "allocation": setup["allocation"], "downforce": setup["downforce"],
                           "gear": setup["gear"], "mode": mode, "recompute_invariant": False,
                           "strategy": strategy(laps, recipe["tires"], recipe["fractions"])})

        for setup_id, setup in plan["setups"].items():
            for plan_id, recipe in plan["plans"].items():
                add(setup_id, plan_id, setup, recipe)
        for setup_id, setup in plan["slider_variants"].items():
            for plan_id in plan["slider_plans"]:
                add(setup_id, plan_id, setup, plan["plans"][plan_id])
        balanced = plan["setups"]["balanced"]
        for label, fraction in plan["timing_variants"].items():
            add("balanced", label, balanced, {"tires": ["soft", "medium"], "fractions": [fraction]})
        for mode in plan["modes"]:
            add("balanced", "SM-50", balanced, plan["plans"]["SM-50"], mode=mode, suffix="/" + mode)
        add("balanced", "SM-50", balanced, plan["plans"]["SM-50"],
            seed=plan["reserved_seed"], suffix="/reserved-seed")
    assert len(result) == plan["expected_cases"]
    assert len({row["id"] for row in result}) == len(result)
    for row in result:
        assert sum(row["allocation"]) == 8
    return result


def numerical_checks(data):
    """A physical failure is retained; a false successful certificate is rejected."""
    if not data["outcome"]["ok"]:
        return ["physical_or_resolution_failure"]
    errors = []
    if len(data["laps"]) != data["receipt"]["laps"]:
        errors.append("incomplete_finish")
    if max(data["maximum_carry_jumps_speed_engine_surface_carcass_wear"]) > 1e-8:
        errors.append("boundary_jump")
    if not data["pit_resets_exact"]:
        errors.append("pit_preparation")
    expected = len(data["receipt"]["stops"]) * data["receipt"]["pit_loss_s"]
    if abs(data["outcome"]["service_time_s"] - expected) > 1e-8:
        errors.append("service_cost")
    if data["outcome"]["elapsed_accounting_residual_s"] > 1e-7:
        errors.append("elapsed_accounting")
    drive = sum(lap["time_s"] for lap in data["laps"])
    if abs(data["outcome"]["time_s"] - drive - data["outcome"]["service_time_s"]) > 1e-7:
        errors.append("independent_elapsed_accounting")
    ledger = data["thermal_ledger"]
    if ledger["maximum_tire_balance_residual_j"] > 1e-5:
        errors.append("tire_balance")
    residual = (ledger["tire_generated_heat_j"] + ledger["tire_direct_environment_heat_j"]
                - ledger["tire_rolling_dissipation_j"] - ledger["tire_deformation_dissipation_j"])
    if abs(residual) > 1e-5:
        errors.append("dissipation_accounting")
    return errors


class Campaign:
    def __init__(self, plan, binary, cache):
        self.plan, self.binary, self.cache = plan, binary.resolve(), cache.resolve()
        self.policy = ROOT / plan["policy"]
        assert sha(self.policy) == plan["policy_sha256"], "physical policy changed"
        paths = list((ROOT / "crates").rglob("*.rs")) + list((ROOT / "crates").rglob("Cargo.toml"))
        paths += [ROOT / "Cargo.toml", ROOT / "Cargo.lock", HERE / "plan-v1.json", Path(__file__).resolve()]
        paths += [p for p in (ROOT / "catalogs/racing/v1.10.1").rglob("*") if p.is_file()]
        paths += [ROOT / "experiments/qualified_career_v1/native-reference-v1.json", self.policy]
        self.identity = {"binary_sha256": sha(self.binary),
                         "sources": {str(p.relative_to(ROOT)): sha(p) for p in sorted(set(paths))}}
        self.cache.mkdir(parents=True, exist_ok=True)
        pin = self.cache / "pin-v1.json"
        if pin.exists():
            self.pin = json.loads(pin.read_text())
            if self.pin["identity"] != self.identity:
                raise RuntimeError("changed executable/source/policy: use a fresh namespace")
        else:
            self.pin = {"schema": "pitgun.era1-strategy-kernel-pin/v1", "identity": self.identity,
                        "utc": datetime.now(timezone.utc).isoformat(),
                        "source_commit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
                        "dirty_at_start": bool(subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT, text=True))}
            write(pin, self.pin)

    def verify(self):
        assert sha(self.binary) == self.identity["binary_sha256"], "executable changed"
        assert all(sha(ROOT / path) == expected for path, expected in self.identity["sources"].items()), "source changed"

    def execute(self, case, validation=None):
        self.verify()
        key = hashlib.sha256(encoded(case)).hexdigest()
        target = self.cache / (key + ("." + validation if validation else "") + ".json")
        if target.exists():
            saved = json.loads(target.read_text())
            if saved["case"] != case or saved["binary_sha256"] != self.identity["binary_sha256"]:
                raise RuntimeError("cache identity mismatch")
            if saved["data"]["case"] != case or numerical_checks(saved["data"]) != saved["checks"]:
                raise RuntimeError("cached native echo/check mismatch")
            return saved
        request = self.cache / "current-case.json"
        write(request, case)
        started = time.monotonic()
        run = subprocess.run([str(self.binary), str(self.policy), str(request)], cwd=ROOT, text=True, capture_output=True)
        self.verify()
        data = json.loads(run.stdout) if run.returncode == 0 else {
            "case": case, "outcome": {"ok": False, "phase": "process", "error": run.stderr}}
        # The Racing contract omits empty optional pit_laps on serialization.
        # Normalize that documented default; compare every effective field.
        data["case"]["strategy"].setdefault("pit_laps", [])
        if data["case"] != case:
            write(self.cache / (key + ".unexpected-response.json"), {"request": case, "native": data})
            raise RuntimeError("native request echo mismatch")
        saved = {"case": case, "data": data, "wall_s": time.monotonic() - started,
                 "binary_sha256": self.identity["binary_sha256"], "checks": numerical_checks(data),
                 "returncode": run.returncode, "stderr": run.stderr}
        write(target, saved)
        return saved


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--target-dir", type=Path, required=True)
    parser.add_argument("--cache", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--limit", type=int)
    args = parser.parse_args()
    plan = json.loads((HERE / "plan-v1.json").read_text())
    matrix = cases(plan)
    env = dict(os.environ, CARGO_BUILD_JOBS="2", CARGO_TARGET_DIR=str(args.target_dir.resolve()))
    subprocess.run(["cargo", "build", "--offline", "--release", "-p", "pitgun-racing-simulator",
                    "--example", "era1_strategy_probe"], cwd=ROOT, env=env, check=True)
    campaign = Campaign(plan, args.target_dir / "release/examples/era1_strategy_probe", args.cache)
    rows, validations = [], []
    started = time.monotonic()

    def checkpoint():
        write(args.output, {"schema": "pitgun.era1-strategy-qualification-results/v1", "plan": plan,
                            "provenance": campaign.pin, "rows": rows, "validations": validations,
                            "complete": len(rows) == len(matrix) and len(validations) == plan["expected_validation_runs"]})

    for case in matrix:
        if args.limit is not None and len(rows) >= args.limit:
            checkpoint()
            print("Partial checkpoint retained; not a completed qualification.", flush=True)
            return
        row = campaign.execute(case)
        rows.append(row)
        checkpoint()
        elapsed = time.monotonic() - started
        print(f"{len(rows)}/{len(matrix)} {case['id']} "
              f"{'OK' if not row['checks'] else row['checks']} {elapsed:.1f}s elapsed", flush=True)
    lookup = {row["case"]["id"]: row for row in rows}
    for context in plan["contexts"]:
        for kind, selection in [("repeat", plan["repeat_selection"]), ("recompute", plan["recompute_selection"])]:
            for setup, recipe in selection:
                baseline = lookup[f"{context['circuit']}/{setup}/{recipe}"]
                case = copy.deepcopy(baseline["case"])
                if kind == "recompute":
                    case["recompute_invariant"] = True
                fresh = campaign.execute(case, validation=kind)
                exact = (not baseline["checks"] and not fresh["checks"] and
                         baseline["data"]["outcome"].get("result_digest") == fresh["data"]["outcome"].get("result_digest") and
                         baseline["data"]["receipt"]["physical_input_digest"] == fresh["data"]["receipt"]["physical_input_digest"])
                validations.append({"kind": kind, "baseline_id": baseline["case"]["id"], "exact": bool(exact), "fresh": fresh})
                checkpoint()
                print(f"validation {len(validations)}/12 {kind} {baseline['case']['id']} exact={exact}", flush=True)
    campaign.verify()
    print(f"Finished: {len(rows)} cases, {len(validations)} validation runs; "
          f"{sum(bool(r['checks']) for r in rows)} cases to review.", flush=True)


if __name__ == "__main__":
    main()
