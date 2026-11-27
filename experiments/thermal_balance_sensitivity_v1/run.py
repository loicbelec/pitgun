#!/usr/bin/env python3
"""Bounded one-factor thermal sensitivity, then frozen held-out validation."""
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


def cases(plan, phase="development", selected=None):
    result=[]
    names=list(plan["policies"]) if phase == "development" else ["reference", selected]
    if phase == "validation" and (selected not in plan["policies"] or selected == "reference"):
        raise ValueError("validation requires a distinct preregistered selected policy")
    for name in names:
        for c in plan[phase+"_contexts"]:
            n=c["laps"]; first=n//2; second=n-first; a,b=c["family"]
            schedules={1:[(a,first),(b,second)],
                       2:[(a,first//2),(b,second),(a,first-first//2)],
                       3:[(a,first//2),(b,second//2),(a,first-first//2),(b,second-second//2)]}
            for stops in c["stops"]:
                stints=schedules[stops];bounds=[];count=0
                for _,length in stints[:-1]:count+=length;bounds.append(count)
                result.append({"id":f'{name}/{c["fixture_id"]}/{c["circuit"]}/{stops}',
                    "policy_id":name,"circuit":c["circuit"],"fixture_id":c["fixture_id"],"seed":plan["seed"],
                    "discretionary_allocation":c["allocation"],"gear":.5,"downforce":.5,"mode":"balanced",
                    "strategy":{"stints":[{"tire_id":t,"laps":l} for t,l in stints],"pit_laps":bounds}})
    assert len(result) == plan["expected_"+phase+"_cases"]
    assert len({c["id"] for c in result})==len(result)
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
    if data["receipt"]["initial_state"]["fuel_mass"] != 100.:
        errors.append("fuel_scope")
    if data["receipt"]["policy_id"] != data["case"]["policy_id"]:
        errors.append("policy_binding")
    for lap in data["laps"]:
        if abs(lap["hot_fraction"] + lap["cold_fraction"] + lap["peak_window_fraction"] - 1.) > 1e-8:
            errors.append("window_accounting")
            break
    return errors


class Campaign:
    def __init__(self, plan, binary, cache):
        self.plan, self.binary, self.cache = plan, binary.resolve(), cache.resolve()
        self.policies = {name:ROOT / p["path"] for name,p in plan["policies"].items()}
        assert sha(ROOT / plan["fixtures"]) == plan["fixtures_sha256"], "career fixtures changed"
        assert all(sha(self.policies[n]) == p["sha256"] for n,p in plan["policies"].items()), "policy changed"
        assert sha(ROOT / plan["baseline_archive"]) == plan["baseline_archive_sha256"], "baseline archive changed"
        paths = list((ROOT / "crates").rglob("*.rs")) + list((ROOT / "crates").rglob("Cargo.toml"))
        paths += [ROOT / "Cargo.toml", ROOT / "Cargo.lock", HERE / "plan-v1.json", HERE / "ACCEPTANCE.md", HERE / "test_run.py", HERE / "report.py", Path(__file__).resolve(), ROOT / plan["baseline_archive"]]
        paths += [p for p in (ROOT / "catalogs/racing/v1.10.1").rglob("*") if p.is_file()]
        paths += [ROOT / "experiments/qualified_career_v1/native-reference-v1.json", ROOT / plan["fixtures"], *self.policies.values()]
        self.identity = {"binary_sha256": sha(self.binary),
                         "sources": {str(p.relative_to(ROOT)): sha(p) for p in sorted(set(paths))}}
        self.cache.mkdir(parents=True, exist_ok=True)
        pin = self.cache / "pin-v1.json"
        if pin.exists():
            self.pin = json.loads(pin.read_text())
            if self.pin["identity"] != self.identity:
                raise RuntimeError("changed executable/source/policy: use a fresh namespace")
        else:
            self.pin = {"schema": "pitgun.thermal-balance-sensitivity-pin/v1", "identity": self.identity,
                        "utc": datetime.now(timezone.utc).isoformat(),
                        "source_commit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
                        "dirty_at_start": bool(subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT, text=True))}
            if self.pin["dirty_at_start"]:
                raise RuntimeError("commit sources before starting the campaign")
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
        run = subprocess.run([str(self.binary), str(self.policies[case["policy_id"]]), str(request)], cwd=ROOT, text=True, capture_output=True)
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
    import fcntl
    parser=argparse.ArgumentParser()
    parser.add_argument("--target-dir",type=Path,required=True)
    parser.add_argument("--cache",type=Path,required=True)
    parser.add_argument("--output",type=Path,required=True)
    parser.add_argument("--phase",choices=["development","validation"],required=True)
    parser.add_argument("--selection",type=Path)
    parser.add_argument("--development",type=Path)
    args=parser.parse_args()
    plan=json.loads((HERE/"plan-v1.json").read_text())
    selection=None
    if args.phase=="validation":
        selection=json.loads(args.selection.read_text())
        assert sha(args.development)==selection["development_archive_sha256"], "development evidence changed"
        assert selection["selected_policy"] and selection["development_pass"], "no eligible candidate"
        assert plan["policies"][selection["selected_policy"]]["sha256"]==selection["policy_sha256"]
    matrix=cases(plan,args.phase,selection["selected_policy"] if selection else None)
    args.cache.parent.mkdir(parents=True,exist_ok=True)
    lock=(args.cache.parent/"thermal-native-worker.lock").open("a")
    fcntl.flock(lock.fileno(),fcntl.LOCK_EX|fcntl.LOCK_NB)
    env=dict(os.environ,CARGO_BUILD_JOBS="2",CARGO_TARGET_DIR=str(args.target_dir.resolve()))
    subprocess.run(["cargo","build","--offline","--release","-p","pitgun-racing-simulator","--example","thermal_balance_sensitivity_probe"],cwd=ROOT,env=env,check=True)
    campaign=Campaign(plan,args.target_dir/"release/examples/thermal_balance_sensitivity_probe",args.cache)
    selection_hash=sha(args.selection) if selection else None
    if selection:
        frozen=args.cache/"selection-pin.json"
        pin={"selection_sha256":selection_hash,"development_sha256":sha(args.development)}
        if frozen.exists():assert json.loads(frozen.read_text())==pin,"selection changed: new validation namespace required"
        else:write(frozen,pin)
    rows=[];validations=[];started=time.monotonic()
    def checkpoint():
        write(args.output,{"schema":"pitgun.thermal-balance-sensitivity-results/v1","phase":args.phase,"plan":plan,
            "provenance":campaign.pin,"selection":selection,"selection_sha256":selection_hash,"rows":rows,"validations":validations,
            "complete":len(rows)==len(matrix) and (args.phase=="development" or len(validations)==plan["expected_fresh_repeats"])})
    for case in matrix:
        row=campaign.execute(case);rows.append(row);checkpoint()
        print(f'{len(rows)}/{len(matrix)} {case["id"]} {row["checks"] or "OK"} {time.monotonic()-started:.1f}s',flush=True)
    if selection:
        dev=json.load(gzip.open(args.development))
        baseline_rows=[r for r in dev["rows"] if r["case"]["policy_id"] in ["reference",selection["selected_policy"]]
            and r["case"]["fixture_id"]=="craft_engineering:era2-exit" and len(r["case"]["strategy"]["pit_laps"])==1]
        baseline_rows += [r for r in rows if r["case"]["fixture_id"]=="craft_engineering:era2-exit" and len(r["case"]["strategy"]["pit_laps"])==1]
        assert len(baseline_rows)==plan["expected_fresh_repeats"]
        for baseline in baseline_rows:
            fresh=campaign.execute(copy.deepcopy(baseline["case"]),validation="repeat")
            exact=(not baseline["checks"] and not fresh["checks"] and
                baseline["data"]["outcome"]["result_digest"]==fresh["data"]["outcome"].get("result_digest") and
                baseline["data"]["receipt"]["physical_input_digest"]==fresh["data"].get("receipt",{}).get("physical_input_digest"))
            validations.append({"baseline_id":baseline["case"]["id"],"fresh":fresh,"exact":exact});checkpoint()
            print(f'repeat {len(validations)}/6 exact={exact}',flush=True)
    campaign.verify();checkpoint()
    print("Finished; retained numerical failures:",sum(bool(r["checks"])for r in rows),flush=True)


if __name__=="__main__":main()
