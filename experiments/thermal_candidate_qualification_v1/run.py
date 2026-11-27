#!/usr/bin/env python3
"""Serial, per-era qualification of the already selected immutable policy."""
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
ERA1 = module('frozen_era1_harness', 'experiments/era1_strategy_qualification_v1/run.py')
ERA2 = module('frozen_era2_harness', 'experiments/era2_strategy_qualification_v1/run.py')
sha, write, numerical_checks = NATIVE.sha, NATIVE.write, NATIVE.numerical_checks


def matched(laps, stops):
    a, b = laps // 2, laps - laps // 2
    parts = ([('medium', a // 2), ('hard', b), ('medium', a - a // 2)] if stops == 2 else
        [('medium', a // 2), ('hard', b // 2), ('medium', a - a // 2), ('hard', b - b // 2)])
    boundaries, count = [], 0
    for _, length in parts[:-1]:
        count += length; boundaries.append(count)
    return {'stints': [{'tire_id': tire, 'laps': length} for tire, length in parts], 'pit_laps': boundaries}


def cases(plan, era):
    p = json.loads((ROOT / f'experiments/{era}_strategy_qualification_v1/plan-v1.json').read_text())
    rows = []
    if era == 'era1':
        for old in ERA1.cases(p):
            if old['downforce'] != .5: continue  # replace unavailable paired sliders by legal gearing
            c = copy.deepcopy(old)
            c['discretionary_allocation'] = c.pop('allocation')
            assert c.pop('recompute_invariant') is False
            c['fixture_id'] = 'craft_engineering:era1-entry'
            rows.append(c)
        for context in p['contexts']:
            for setup, profile in p['setups'].items():
                base = next(c for c in rows if c['id'] == f"{context['circuit']}/{setup}/SM-50")
                for gear in [.35, .85]:
                    c = copy.deepcopy(base); c['id'] += f'/gear-{gear}'; c['gear'] = gear; rows.append(c)
    else:
        rows = ERA2.cases(p)
    # Add exact-exposure MH two/three-stop controls, retaining every old recipe.
    for context in p['contexts']:
        stages = [''] if era == 'era1' else ['entry/', 'exit/']
        for stage in stages:
            base = next(c for c in rows if c['id'] == f"{context['circuit']}/{stage}balanced/MH-50")
            for stops in [2, 3]:
                c = copy.deepcopy(base)
                c['id'] = f"{context['circuit']}/{stage}balanced/MH-matched-{stops}"
                c['strategy'] = matched(context['laps'], stops); rows.append(c)
            if era == 'era2':
                c = copy.deepcopy(base); c['id'] = f"{context['circuit']}/{stage}balanced/HS-50"
                c['strategy'] = NATIVE.strategy(context['laps'], ['hard', 'soft'], [[1, 2]])
                rows.append(c)
    for c in rows:
        c['id'] = era + '/' + c['id']; c['policy_id'] = plan['policy_id']
    assert len(rows) == plan['expected_cases'][era]
    assert len({c['id'] for c in rows}) == len(rows)
    return rows


def repeat_ids(plan, era):
    if era == 'era1':
        return [f'era1/{c}/{s}/{r}' for c in plan['era1_repeat_contexts']
            for s, r in [('balanced', 'SM-50'), ('engine_chassis', 'MH-50')]]
    return [f'era2/{c}/{stage}/balanced/{recipe}' for c in plan['era2_repeat_contexts']
        for stage, recipe in [('entry', 'SM-50'), ('exit', 'SMS-thirds')]]


class Campaign:
    """Reuse the frozen native cache/checker, add a pin for this orchestration."""
    def __init__(self, plan, binary, cache):
        self.native = NATIVE.Campaign(plan, binary, cache / 'native')
        extra = [HERE / f for f in ['run.py', 'report.py', 'test_run.py', 'ACCEPTANCE.md', 'plan-v1.json']]
        extra += [ROOT / f for f in plan['retained_files']]
        extra += [ROOT / f'experiments/{e}_strategy_qualification_v1/run.py' for e in ['era1', 'era2']]
        for path, expected in plan['retained_files'].items():
            assert sha(ROOT / path) == expected, 'retained reference changed'
        sources = dict(self.native.identity['sources'])
        sources.update({str(p.relative_to(ROOT)): sha(p) for p in extra})
        self.identity = {'binary_sha256': sha(binary), 'sources': sources}
        pin_path = cache / 'qualification-pin-v1.json'
        self.pin = {'schema': 'pitgun.thermal-candidate-qualification-pin/v1', 'identity': self.identity,
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
        self.verify()
        return result


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--era', choices=['era1', 'era2'], required=True)
    parser.add_argument('--target-dir', type=Path, required=True)
    parser.add_argument('--cache', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    plan = json.loads((HERE / 'plan-v1.json').read_text())
    matrix = cases(plan, args.era)
    args.cache.parent.mkdir(parents=True, exist_ok=True)
    lock = (args.cache.parent / 'qualification-worker.lock').open('a')
    fcntl.flock(lock.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
    env = dict(os.environ, CARGO_BUILD_JOBS='2', CARGO_TARGET_DIR=str(args.target_dir.resolve()))
    subprocess.run(['cargo', 'build', '--offline', '--release', '-p', 'pitgun-racing-simulator', '--example',
        'thermal_balance_sensitivity_probe'], cwd=ROOT, env=env, check=True)
    campaign = Campaign(plan, args.target_dir / 'release/examples/thermal_balance_sensitivity_probe', args.cache)
    rows, validations = [], []
    started = time.monotonic()
    def checkpoint():
        write(args.output, {'schema': 'pitgun.thermal-candidate-qualification-results/v1', 'era': args.era,
            'plan': plan, 'provenance': campaign.pin, 'rows': rows, 'validations': validations,
            'complete': len(rows) == len(matrix) and len(validations) == plan['expected_repeats'][args.era]})
    for case in matrix:
        row = campaign.execute(case); rows.append(row); checkpoint()
        elapsed = time.monotonic() - started
        print(f'{len(rows)}/{len(matrix)} {case["id"]} {row["checks"] or "OK"} elapsed={elapsed:.1f}s', flush=True)
    lookup = {r['case']['id']: r for r in rows}
    for key in repeat_ids(plan, args.era):
        baseline = lookup[key]; fresh = campaign.execute(copy.deepcopy(baseline['case']), repeat=True)
        exact = not baseline['checks'] and not fresh['checks'] and (
            baseline['data']['outcome']['result_digest'] == fresh['data']['outcome'].get('result_digest') and
            baseline['data']['receipt']['physical_input_digest'] == fresh['data'].get('receipt', {}).get('physical_input_digest'))
        validations.append({'baseline_id': key, 'fresh': fresh, 'exact': exact}); checkpoint()
        print(f'repeat {len(validations)}/6 exact={exact}', flush=True)
    campaign.verify()
    print(f'Completed {args.era}: {len(rows)} main, {len(validations)} fresh repeats; '
        f'{sum(bool(r["checks"]) for r in rows)} main failures.', flush=True)


if __name__ == '__main__': main()
