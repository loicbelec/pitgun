#!/usr/bin/env python3
"""Bounded aerodynamic tradeoff audit; no production law changes."""
import argparse
import copy
import fcntl
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import time

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]


def module(name, relative):
    spec = importlib.util.spec_from_file_location(name, ROOT / relative)
    loaded = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(loaded)
    return loaded


NATIVE = module('frozen_thermal_harness', 'experiments/thermal_balance_sensitivity_v1/run.py')
sha, write = NATIVE.sha, NATIVE.write


def numerical_checks(data):
    errors=NATIVE.numerical_checks(data)
    if not data["outcome"]["ok"]: return errors
    for lap in data["laps"]:
        bins=lap["spatial_bins"]; energy=lap["energy_ledger"]
        for key,target,tolerance in [("time_s",lap["time_s"],1e-7),("distance_m",energy["distance_m"],1e-7),
            ("drag_work_midpoint_j",energy["drag_work_j"],1e-3),("brake_work_j",energy["friction_brake_work_j"],1e-3)]:
            if abs(sum(b[key]for b in bins)-target)>tolerance: errors.append("bin_"+key)
    receipt=data["receipt"]; mode=data["case"]["aero_mode"]
    ref=receipt["reference_mechanical_drag_downforce_and_legacy_drag_xy"]
    nominal=receipt["nominal_aero_areas_drag_downforce"]; m=receipt["mechanical"]
    expected={"coupled":nominal,"drag_only":[nominal[0],ref[1]],"downforce_only":[ref[0],nominal[1]],"no_downforce":[ref[0],0.]}[mode]
    if [m["fixed_drag_area_m2"],m["fixed_downforce_area_m2"]]!=expected: errors.append("aero_control_binding")
    if mode=="coupled" and receipt["nominal_input_digest"]!=receipt["physical_input_digest"]: errors.append("nominal_input_changed")
    return errors


def cases(plan):
    rows=[]
    for context in plan["contexts"]:
        circuit,laps=context["circuit"],context["laps"]
        def add(mode,slider):
            rows.append({"id":f"{circuit}/{mode}/{slider}","policy_id":plan["policy_id"],"aero_mode":mode,
                "circuit":circuit,"fixture_id":plan["fixture_id"],"discretionary_allocation":plan["allocation"],
                "gear":.5,"downforce":slider,"mode":"balanced","seed":plan["seed"],
                "strategy":NATIVE.strategy(laps,["soft","medium"],[[1,2]])})
        for slider in plan["sliders"]: add("coupled",slider)
        for mode in ["drag_only","downforce_only"]:
            for slider in plan["controls"]: add(mode,slider)
        add("no_downforce",.5)
    assert len(rows)==plan["expected_cases"] and len({c["id"]for c in rows})==len(rows)
    return rows


def repeat_ids(plan):
    return [f'{c["circuit"]}/coupled/0.5'for c in plan["contexts"]]+[
        "SILVERSTONE/drag_only/0.8","SILVERSTONE/downforce_only/0.8","SILVERSTONE/no_downforce/0.5"]


class Campaign:
    """Reuse the frozen native cache/checker, add a pin for this orchestration."""
    def __init__(self, plan, binary, cache):
        self.native = NATIVE.Campaign(plan, binary, cache / 'native')
        extra = [HERE / f for f in ['run.py', 'report.py', 'test_run.py', 'ACCEPTANCE.md', 'plan-v1.json']]
        extra += [ROOT / f for f in plan['retained_files']]

        for path, expected in plan['retained_files'].items():
            assert sha(ROOT / path) == expected, 'retained reference changed'
        sources = dict(self.native.identity['sources'])
        sources.update({str(p.relative_to(ROOT)): sha(p) for p in extra})
        self.identity = {'binary_sha256': sha(binary), 'sources': sources}
        pin_path = cache / 'qualification-pin-v1.json'
        self.pin = {'schema': 'pitgun.aero-tradeoff-audit-pin/v1', 'identity': self.identity,
            'source_commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
            'native_harness': self.native.pin}
        if pin_path.exists():
            retained = json.loads(pin_path.read_text())
            if retained['identity'] != self.identity: raise ValueError('new namespace required: source/policy changed')
            self.pin = retained
        else: write(pin_path, self.pin)

    def verify(self):
        self.native.verify()
        assert all(sha(ROOT / p) == expected for p, expected in self.identity['sources'].items()), 'qualification sources changed'

    def execute(self, case, repeat=False):
        self.verify()
        result = self.native.execute(case, validation='repeat' if repeat else None)
        result["checks"] = numerical_checks(result["data"])
        self.verify()
        return result


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--limit', type=int)
    parser.add_argument('--target-dir', type=Path, required=True)
    parser.add_argument('--cache', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    plan = json.loads((HERE / 'plan-v1.json').read_text())
    matrix = cases(plan)
    args.cache.parent.mkdir(parents=True, exist_ok=True)
    lock = (args.cache.parent / 'aero-audit-worker.lock').open('a')
    fcntl.flock(lock.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
    env = dict(os.environ, CARGO_BUILD_JOBS='2', CARGO_TARGET_DIR=str(args.target_dir.resolve()))
    subprocess.run(['cargo', 'build', '--offline', '--release', '-p', 'pitgun-racing-simulator', '--example',
        'aero_tradeoff_probe'], cwd=ROOT, env=env, check=True)
    campaign = Campaign(plan, args.target_dir / 'release/examples/aero_tradeoff_probe', args.cache)
    rows, validations = [], []
    started = time.monotonic()
    def checkpoint():
        write(args.output, {'schema': 'pitgun.aero-tradeoff-audit-results/v1',
            'plan': plan, 'provenance': campaign.pin, 'rows': rows, 'validations': validations,
            'complete': len(rows) == len(matrix) and len(validations) == plan['expected_repeats']})
    for case in matrix:
        if args.limit is not None and len(rows) >= args.limit:
            checkpoint(); print('Partial pilot retained; not a completed audit.', flush=True); return
        row = campaign.execute(case); rows.append(row); checkpoint()
        elapsed = time.monotonic() - started
        print(f'{len(rows)}/{len(matrix)} {case["id"]} {row["checks"] or "OK"} elapsed={elapsed:.1f}s', flush=True)
    lookup = {r['case']['id']: r for r in rows}
    for key in repeat_ids(plan):
        baseline = lookup[key]; fresh = campaign.execute(copy.deepcopy(baseline['case']), repeat=True)
        exact = not baseline['checks'] and not fresh['checks'] and (
            baseline['data']['outcome']['result_digest'] == fresh['data']['outcome'].get('result_digest') and
            baseline['data']['receipt']['physical_input_digest'] == fresh['data'].get('receipt', {}).get('physical_input_digest'))
        validations.append({'baseline_id': key, 'fresh': fresh, 'exact': exact}); checkpoint()
        print(f'repeat {len(validations)}/6 exact={exact}', flush=True)
    campaign.verify()
    print(f'Completed audit: {len(rows)} main, {len(validations)} fresh repeats; '
        f'{sum(bool(r["checks"]) for r in rows)} main failures.', flush=True)


if __name__ == '__main__': main()
