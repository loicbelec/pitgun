#!/usr/bin/env python3
"""Serial era-II qualification; actual fixture components and bonus floors."""
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
        def add(stage, setup, recipe_id, recipe, *, gear=.5, downforce=.5, mode="balanced", seed=None, suffix=""):
            profile=plan["stages"][stage]
            result.append({"id":f"{circuit}/{stage}/{setup}/{recipe_id}{suffix}","circuit":circuit,
                           "seed":plan["seed"] if seed is None else seed,"fixture_id":profile["fixture_id"],
                           "discretionary_allocation":profile["setups"][setup],"gear":gear,"downforce":downforce,
                           "mode":mode,"strategy":strategy(laps,recipe["tires"],recipe["fractions"])})
        for stage, profile in plan["stages"].items():
            for setup in profile["setups"]:
                for recipe_id, recipe in plan["normal_plans"].items():add(stage,setup,recipe_id,recipe)
        for recipe_id, recipe in plan["diagnostic_plans"].items():add("exit","balanced",recipe_id,recipe)
        sm=plan["normal_plans"]["SM-50"]
        for gear in plan["gear_values"]:add("exit","balanced","SM-50",sm,gear=gear,suffix=f"/gear-{gear}")
        for downforce in plan["downforce_values"]:add("exit","balanced","SM-50",sm,downforce=downforce,suffix=f"/downforce-{downforce}")
        add("exit","balanced","SM-50",sm,seed=plan["reserved_seed"],suffix="/reserved-seed")
        if circuit in plan["driver_mode_contexts"]:
            for mode in plan["modes"]:add("exit","balanced","SM-50",sm,mode=mode,suffix="/"+mode)
    assert len(result)==plan["expected_cases"]
    assert len({r["id"] for r in result})==len(result)
    for r in result:
        stage=next(p for p in plan["stages"].values() if p["fixture_id"]==r["fixture_id"])
        assert sum(r["discretionary_allocation"])==stage["discretionary_points"]
        assert not stage["downforce_locked"] or r["downforce"]==.5
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
    fixture_id = data["case"]["fixture_id"]
    fixtures = json.loads((ROOT / "experiments/opponent_physics_v2/results/career-fixtures-v1.json").read_text())
    fixture = next(f for f in fixtures["fixtures"] if f["id"] == fixture_id)
    offsets = [fixture["offsets"][axis] for axis in ["aero", "chassis", "engine", "cooling"]]
    points = [p + b for p, b in zip(data["case"]["discretionary_allocation"], offsets)]
    if (data["receipt"]["bonus_offsets"] != offsets or data["receipt"]["points"] != points
        or data["receipt"]["budget"] != fixture["discretionaryPoints"] + sum(offsets)):
        errors.append("budget_or_bonus_binding")
    if data["receipt"]["components"] != fixture["technology"].get("components"):
        errors.append("installed_components_binding")
    return errors


class Campaign:
    def __init__(self, plan, binary, cache):
        self.plan, self.binary, self.cache = plan, binary.resolve(), cache.resolve()
        self.policy = ROOT / plan["policy"]
        assert sha(ROOT / plan["fixtures"]) == plan["fixtures_sha256"], "career fixtures changed"
        assert sha(self.policy) == plan["policy_sha256"], "physical policy changed"
        paths = list((ROOT / "crates").rglob("*.rs")) + list((ROOT / "crates").rglob("Cargo.toml"))
        paths += [ROOT / "Cargo.toml", ROOT / "Cargo.lock", HERE / "plan-v1.json", Path(__file__).resolve()]
        paths += [p for p in (ROOT / "catalogs/racing/v1.10.1").rglob("*") if p.is_file()]
        paths += [ROOT / "experiments/qualified_career_v1/native-reference-v1.json", ROOT / plan["fixtures"], self.policy]
        self.identity = {"binary_sha256": sha(self.binary),
                         "sources": {str(p.relative_to(ROOT)): sha(p) for p in sorted(set(paths))}}
        self.cache.mkdir(parents=True, exist_ok=True)
        pin = self.cache / "pin-v1.json"
        if pin.exists():
            self.pin = json.loads(pin.read_text())
            if self.pin["identity"] != self.identity:
                raise RuntimeError("changed executable/source/policy: use a fresh namespace")
        else:
            self.pin = {"schema": "pitgun.era2-strategy-kernel-pin/v1", "identity": self.identity,
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
    # One worker across pilot/resume invocations sharing this campaign root.
    import fcntl
    args.cache.parent.mkdir(parents=True,exist_ok=True)
    worker_lock=(args.cache.parent/"era2-native-worker.lock").open("a")
    fcntl.flock(worker_lock.fileno(),fcntl.LOCK_EX|fcntl.LOCK_NB)
    plan = json.loads((HERE / "plan-v1.json").read_text())
    matrix = cases(plan)
    env = dict(os.environ, CARGO_BUILD_JOBS="2", CARGO_TARGET_DIR=str(args.target_dir.resolve()))
    subprocess.run(["cargo", "build", "--offline", "--release", "-p", "pitgun-racing-simulator",
                    "--example", "era2_strategy_probe"], cwd=ROOT, env=env, check=True)
    campaign = Campaign(plan, args.target_dir / "release/examples/era2_strategy_probe", args.cache)
    rows, validations = [], []
    started = time.monotonic()

    def checkpoint():
        write(args.output, {"schema": "pitgun.era2-strategy-qualification-results/v1", "plan": plan,
                            "provenance": campaign.pin, "rows": rows, "validations": validations,
                            "complete": len(rows) == len(matrix) and len(validations) == plan["expected_repeats"]})

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
    for circuit in plan["repeat_contexts"]:
        for stage,setup,recipe in plan["repeat_selection"]:
            baseline=lookup[f"{circuit}/{stage}/{setup}/{recipe}"]
            fresh=campaign.execute(copy.deepcopy(baseline["case"]),validation="repeat")
            exact=(not baseline["checks"] and not fresh["checks"] and
                   baseline["data"]["outcome"].get("result_digest")==fresh["data"]["outcome"].get("result_digest") and
                   baseline["data"]["receipt"]["physical_input_digest"]==fresh["data"].get("receipt",{}).get("physical_input_digest"))
            validations.append({"kind":"repeat","baseline_id":baseline["case"]["id"],"exact":bool(exact),"fresh":fresh})
            checkpoint()
            print(f"repeat {len(validations)}/{plan['expected_repeats']} {baseline['case']['id']} exact={exact}",flush=True)
    campaign.verify()
    print(f"Finished: {len(rows)} cases, {len(validations)} validation runs; "
          f"{sum(bool(r['checks']) for r in rows)} cases to review.", flush=True)


if __name__ == "__main__":
    main()
