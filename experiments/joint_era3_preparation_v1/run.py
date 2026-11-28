#!/usr/bin/env python3
"""Finite joint era-III preparation; frozen physics and durable native evidence."""
import argparse
from copy import deepcopy
from functools import cache
import fcntl
import hashlib
import importlib.util
from pathlib import Path
import statistics
import sys

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
spec = importlib.util.spec_from_file_location('era3_frozen_pilot', ROOT / 'experiments/era3_joint_preparation_v1/run.py')
legacy = importlib.util.module_from_spec(spec)
spec.loader.exec_module(legacy)
sys.path.insert(0, str(ROOT / 'experiments/joint_era2_preparation_v1'))
from selection import choose


@cache
def corpus():
    return legacy.load(ROOT / 'experiments/era3_common_fuel_v1/results/career-fixtures-v2.json')


def fixture(stage):
    return next(f for f in corpus()['fixtures'] if f['id'] == f'craft_engineering:era3-{stage}')


def strategy(laps, stops):
    boundaries = [i * laps // (stops + 1) for i in range(stops + 2)]
    tires = {1: ['hard', 'medium'], 2: ['soft', 'medium', 'hard'],
             3: ['hard', 'medium', 'hard', 'medium']}[stops]
    return {'stints': [{'tire_id': t, 'laps': b - a}
                       for t, a, b in zip(tires, boundaries, boundaries[1:])],
            'pit_laps': boundaries[1:-1]}


def make_case(plan, ctx, budget, actor, proposal, family='common', stage='developed'):
    f = fixture(stage)
    floors = legacy.floors(f if family == 'career' else fixture('developed'))
    spec = plan['career_preparation'] if family == 'career' else plan['proposals'][proposal]
    points = legacy.BASE.scale(spec['weights'], min(budget, 80), floors)
    if proposal == 'shortcut':
        # All discretionary spending goes to chassis/engine. Axis caps may
        # leave points unused; available budget must remain unchanged.
        points = list(floors)
        for _ in range(budget - sum(points)):
            axis = min([i for i in (1, 2) if points[i] < 20],
                       key=lambda i: (points[i], i), default=None)
            if axis is None:
                break
            points[axis] += 1
    if family == 'career':
        downforce, gear = spec['downforce'], spec['gear']
        if stage == 'entry':
            downforce, gear = .5, .5
    else:
        downforce = min(1., ctx['downforce'] + spec['downforce_delta'])
        gear = min(1., ctx['gear'] + spec['gear_delta'])
    stops = spec['stops']
    if proposal == 'three_stop':
        stops = 3
    return {'id': f'III/joint-v1/{ctx["circuit"]}/{budget}/{actor}/{family}-{stage}-{proposal}',
            'policy_id': 'rolling-heat-065', 'circuit': ctx['circuit'], 'seed': plan['seed'],
            'fixture_id': f['id'], 'fixture_revision': 2, 'actor': actor,
            'kind': 'career' if family == 'career' else 'field',
            'effective_budget': budget, 'effective_allocation': points,
            'downforce': downforce, 'gear': gear, 'mode': 'balanced',
            'strategy': strategy(ctx['laps'], stops), 'initial_fuel_kg': plan['initial_fuel_kg']}


def design(plan):
    archive = legacy.load(ROOT / plan['bridge_archive'])
    matrix = [{'case': deepcopy(archive['rows'][i]['case']), 'fresh': False}
              for i in plan['bridge_indices']]
    evaluations = []
    actors = list(legacy.load(ROOT / plan['roster'])['drivers']) + ['balanced_reference']
    for ctx in plan['contexts']:
        for budget in plan['budgets']:
            for actor in actors:
                for proposal in ['balanced', 'power']:
                    c = make_case(plan, ctx, budget, actor, proposal)
                    evaluations.append({'index': len(matrix), 'family': 'common', 'proposal': proposal})
                    matrix.append({'case': c, 'fresh': False})
            c = make_case(plan, ctx, budget, 'balanced_reference', 'shortcut')
            evaluations.append({'index': len(matrix), 'family': 'shortcut', 'proposal': 'shortcut'})
            matrix.append({'case': c, 'fresh': False})
        for stage, budget in [('entry', 24), ('exit', 43)]:
            for proposal in ['balanced', 'shortcut']:
                c = make_case(plan, ctx, budget, 'balanced_reference', proposal, 'career', stage)
                evaluations.append({'index': len(matrix), 'family': 'career', 'proposal': proposal})
                matrix.append({'case': c, 'fresh': False})
        c = make_case(plan, ctx, 43, 'balanced_reference', 'three_stop', 'career', 'exit')
        evaluations.append({'index': len(matrix), 'family': 'three_stop', 'proposal': 'three_stop'})
        matrix.append({'case': c, 'fresh': False})
    for circuit, budget in plan['repeat_groups']:
        for actor in actors[:-1]:
            case = next(m['case'] for m in matrix if m['case']['circuit'] == circuit
                        and m['case']['effective_budget'] == budget and m['case']['actor'] == actor
                        and m['case']['id'].endswith('/common-developed-balanced'))
            matrix.append({'case': deepcopy(case), 'fresh': True})
    if len(matrix) != plan['gp_count'] or len(evaluations) != plan['evaluation_gp_count']:
        raise ValueError('matrix differs from frozen counts')
    return matrix, evaluations


FAIR_KEYS = ['bonus_offsets', 'budget', 'spent_budget', 'unused_budget', 'components',
             'engine', 'initial_state', 'laps', 'mechanical', 'pit_loss_s', 'points',
             'policy_digest', 'policy_id', 'resolved_compounds', 'stops']


def fair_receipt(receipt):
    return {k: receipt[k] for k in FAIR_KEYS}


def preflight(worker, plan, matrix, evaluations):
    groups = {}
    common = {e['index'] for e in evaluations if e['family'] == 'common'}
    for i, item in enumerate(matrix):
        if item['fresh']:
            continue
        row = worker.execute(item['case'], resolve=True)
        if row['returncode'] or not row['data'].get('resolved_only'):
            raise ValueError('native resolution failed: ' + item['case']['id'])
        receipt = row['data']['receipt']
        if receipt['initial_state']['fuel_mass'] != plan['initial_fuel_kg']:
            raise ValueError('common charge differs')
        if i in common:
            c = item['case']
            group = (c['circuit'], c['effective_budget'], c['id'].rsplit('-', 1)[-1])
            fair = fair_receipt(receipt)
            if groups.setdefault(group, fair) != fair:
                raise ValueError('unequal preparation across actors: ' + str(group))
        if (i + 1) % 100 == 0:
            print(f'preflight {i+1}/{len(matrix)}', flush=True)
    return {'resolved_unique_cases': sum(not m['fresh'] for m in matrix),
            'equal_actor_groups': len(groups), 'minimum_budget': min(plan['budgets']),
            'common_charge_kg': plan['initial_fuel_kg']}


def analyze(data):
    plan = data['plan']
    matrix, evaluations = design(plan)
    rows = data['rows']
    if not data['complete'] or len(rows) != len(matrix):
        raise ValueError('partial evidence cannot produce a policy')
    if data['source_matrix_sha256'] != hashlib.sha256(legacy.encoded(matrix)).hexdigest():
        raise ValueError('changed matrix')
    for row, item in zip(rows, matrix):
        if (row['case'] != item['case'] or row['data']['case'] != item['case']
                or row['binary_sha256'] != plan['native_binary_sha256']
                or row['checks'] != legacy.checks(row['data'])):
            raise ValueError('native evidence binding changed')
    previous = legacy.load(ROOT / plan['bridge_archive'])['rows']
    bridges = [rows[i]['data'] == previous[j]['data'] for i, j in enumerate(plan['bridge_indices'])]
    originals = {r['case']['id']: r for r, m in zip(rows, matrix) if not m['fresh']}
    repeats = [{'id': r['case']['id'], 'exact': r['data'] == originals[r['case']['id']]['data']}
               for r, m in zip(rows, matrix) if m['fresh']]
    failures = [{'id': r['case']['id'], 'checks': r['checks'], 'outcome': r['data']['outcome']}
                for r in rows if r['checks']]
    low_reserve = [{'id': r['case']['id'], 'kg': r['data']['outcome']['final_state']['fuel_mass']}
                   for r in rows if r['data']['outcome']['ok'] and
                   r['data']['outcome']['final_state']['fuel_mass'] < plan['minimum_finish_reserve_kg']]
    gates = {'native_checks': not failures, 'finish_reserve': not low_reserve,
             'exact_bridges': all(bridges), 'exact_repeats': all(r['exact'] for r in repeats)}
    numerical = all(gates.values())
    policy = {'schema': 'pitgun.joint-era3-preparation-policy/v1', 'admitted': False,
              'selector': 'within-own-best-0.3pct-then-style-midpoint-time/v1',
              'fuel_kg': plan['initial_fuel_kg'], 'contexts': []}
    summaries, career, witnesses = [], [], []
    roster = legacy.load(ROOT / plan['roster'])['drivers']
    if numerical:
        for ctx in plan['contexts']:
            circuit = ctx['circuit']
            actors = {actor: {'anchors': {}} for actor in roster}
            selected_by_budget = {}
            for budget in plan['budgets']:
                pool = [(e, rows[e['index']]) for e in evaluations
                        if rows[e['index']]['case']['circuit'] == circuit
                        and rows[e['index']]['case']['effective_budget'] == budget]
                selected = {}
                for actor, driver in roster.items():
                    options = [(e['proposal'], r) for e, r in pool
                               if e['family'] == 'common' and r['case']['actor'] == actor]
                    pid, row = choose(options, driver['style'], legacy.BASE)
                    selected[actor] = row
                    actors[actor]['anchors'][str(budget)] = {
                        'proposal': pid, 'case_id': row['case']['id'],
                        'preparation': {k: deepcopy(row['case'][k]) for k in
                                        ['effective_allocation', 'effective_budget', 'downforce', 'gear', 'strategy', 'mode']},
                        'physical_input_digest': row['data']['receipt']['physical_input_digest'],
                        'result_digest': row['data']['outcome']['result_digest'],
                        'time_s': row['data']['outcome']['time_s']}
                selected_by_budget[budget] = selected
                players = [r for e, r in pool if e['family'] == 'common'
                           and r['case']['actor'] == 'balanced_reference']
                shortcut = next(r for e, r in pool if e['family'] == 'shortcut')
                best = min(players, key=legacy.BASE.order)
                summaries.append({'circuit': circuit, 'budget': budget,
                                  'free': comparison(best, selected), 'shortcut': comparison(shortcut, selected),
                                  'field': {a: comparison(r, {k: v for k, v in selected.items() if k != a})
                                            for a, r in selected.items()}})
            for stage, budget in [('entry', 24), ('exit', 43)]:
                for e in evaluations:
                    r = rows[e['index']]
                    if (e['family'] == 'career' and r['case']['circuit'] == circuit
                            and r['case']['fixture_id'] == fixture(stage)['id']):
                        career.append({'circuit': circuit, 'stage': stage, 'proposal': e['proposal'],
                                       **comparison(r, selected_by_budget[budget])})
            witness = next(rows[e['index']] for e in evaluations if e['family'] == 'three_stop'
                           and rows[e['index']]['case']['circuit'] == circuit)
            baseline = next(c for c in career if c['circuit'] == circuit and c['stage'] == 'exit' and c['proposal'] == 'balanced')
            witnesses.append({'circuit': circuit, 'three_stop_time_s': witness['data']['outcome']['time_s'],
                              'one_stop_time_s': baseline['time_s']})
            policy['contexts'].append({'era': 3, 'circuit': circuit, 'floors': [6, 2, 9, 1], 'actors': actors})
        for budget in plan['budgets']:
            group = [s for s in summaries if s['budget'] == budget]
            gates[f'cap{budget}_free_median_top5'] = statistics.median(s['free']['rank'] for s in group) <= 5
            gates[f'cap{budget}_shortcut_not_sweep'] = not all(s['shortcut']['rank'] == 1 for s in group)
            gates[f'cap{budget}_free_gains_2_contexts'] = sum(s['free']['time_s'] < s['shortcut']['time_s'] - 1e-8 for s in group) >= 2
        exit_rows = [c for c in career if c['stage'] == 'exit' and c['proposal'] == 'balanced']
        gates['actual_exit_median_top5'] = statistics.median(c['rank'] for c in exit_rows) <= 5
        gates['actual_exit_one_win'] = any(c['rank'] == 1 for c in exit_rows)
        gates['three_stop_not_universal'] = not all(w['three_stop_time_s'] < w['one_stop_time_s'] for w in witnesses)
    return {'schema': 'pitgun.joint-era3-review/v1', 'status': 'PASS' if all(gates.values()) else 'REFINE',
            'numerical_pass': numerical, 'gates': gates, 'attempts': len(rows),
            'completed_gp': sum(r['data']['outcome']['ok'] for r in rows),
            'accepted_laps': sum(len(r['data'].get('laps', [])) for r in rows),
            'native_wall_seconds': sum(r['wall_s'] for r in rows), 'failures': failures,
            'low_reserve': low_reserve, 'bridges': bridges, 'repeats': repeats,
            'comparisons': summaries, 'actual_career': career, 'three_stop_witnesses': witnesses,
            'runtime_admitted': False, 'scope': plan['scope']}, policy


def comparison(row, field):
    time = row['data']['outcome']['time_s']
    return {'case_id': row['case']['id'], 'time_s': time,
            'rank': 1 + sum(r['data']['outcome']['time_s'] < time for r in field.values()),
            'gap_to_best_ai_s': time - min(r['data']['outcome']['time_s'] for r in field.values())}


def main():
    p = argparse.ArgumentParser()
    for name in ['binary', 'cache', 'output']:
        p.add_argument('--' + name, required=True, type=Path)
    p.add_argument('--max-new-gp', type=int, default=836)
    p.add_argument('--max-wall-seconds', type=float, default=14400.)
    args = p.parse_args()
    path = HERE / 'plan-v1.json'
    plan = legacy.load(path)
    plan['retained_files'] = dict(plan['retained_files'])
    for f in list(HERE.glob('*.py')) + [path]:
        plan['retained_files'][str(f.relative_to(ROOT))] = legacy.sha(f)
    matrix, evaluations = design(plan)
    args.cache.parent.mkdir(parents=True, exist_ok=True)
    with (args.cache.parent / 'native-worker.lock').open('a') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        worker = legacy.Worker(args.binary, args.cache / 'native', plan)
        if worker.identity['binary_sha256'] != plan['native_binary_sha256']:
            raise ValueError('wrong frozen binary')
        digest = hashlib.sha256(legacy.encoded(matrix)).hexdigest()
        if args.output.exists():
            old = legacy.load(args.output)
            if old['source_matrix_sha256'] != digest or old['provenance']['identity'] != worker.identity:
                raise ValueError('refusing to replace another archive')
        checked = preflight(worker, plan, matrix, evaluations)
        legacy.write(args.cache / 'preflight.json', checked)
        journal = legacy.EraJournal(args.cache / 'journal', worker.cache, worker.identity, matrix)
        progress = legacy.advance_batch(worker, journal, matrix, args.cache / 'progress.json', args.max_new_gp, args.max_wall_seconds)
        worker.verify()
        if not progress['complete']:
            print({k: v for k, v in progress.items() if k != 'timings'}, flush=True)
            return
        if not args.output.exists():
            journal.export(args.output, {'schema': 'pitgun.joint-era3-campaign/v1',
                'plan': legacy.load(path), 'provenance': worker.pin, 'preflight': checked,
                'source_matrix_sha256': digest})
        elif legacy.load(args.output)['journal_sha256'] != legacy.sha(journal.path):
            raise ValueError('archive journal differs')
        review, policy = analyze(legacy.load(args.output))
        for name, value in [('review-v1.json', review), ('candidate-v1.json', policy)]:
            dest = args.output.parent / name
            if not dest.exists() or legacy.load(dest) != value:
                legacy.write(dest, value)
        print({k: review[k] for k in ['status', 'numerical_pass', 'attempts', 'completed_gp', 'gates']}, flush=True)


if __name__ == '__main__':
    main()
