#!/usr/bin/env python3
"""Joint era-II field preparation and player comparison under one frozen matrix."""
import argparse
from copy import deepcopy
import fcntl
import hashlib
from pathlib import Path
import statistics
import sys

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(ROOT / 'experiments/fixed_nine_refinement_v2'))
from harness import BASE, encoded, execute_matrix, key, load, sha, write
from selection import choose, heavy_80


def semantic(case):
    return {k: v for k, v in case.items() if k != 'id'}


def source_rows(plan):
    path = ROOT / plan['reuse_archive']
    if sha(path) != plan['retained_files'][plan['reuse_archive']]:
        raise ValueError('source archive changed')
    archive = load(path)
    if not archive['complete'] or archive['provenance']['identity']['binary_sha256'] != plan['native_binary_sha256']:
        raise ValueError('incompatible historical executable or partial archive')
    index = {}
    for row in archive['rows']:
        if row['checks'] or row['binary_sha256'] != plan['native_binary_sha256']:
            continue
        index.setdefault(encoded(semantic(row['case'])), row)
    return index


def design(plan):
    old = source_rows(plan)
    actors = list(load(ROOT / plan['roster'])['drivers']) + ['balanced_reference']
    matrix, evaluations = [], []
    for ctx in plan['contexts']:
        for budget in plan['budgets']:
            for actor in actors:
                for pid, proposal in enumerate(ctx['proposals']):
                    career = actor == 'balanced_reference' and budget == 24
                    case = BASE.case(dict(ctx, era=2), proposal, actor, budget,
                        seed=plan['seed'], kind='career' if career else 'field',
                        stage='exit', label=f'joint-v1-proposal{pid}')
                    source = old.get(encoded(semantic(case)))
                    native_case = deepcopy(source['case'] if source else case)
                    matrix.append({'case': native_case, 'fresh': False})
                    evaluations.append({'circuit': ctx['circuit'], 'budget': budget,
                        'actor': actor, 'proposal': pid, 'logical_case': case,
                        'native_case_id': native_case['id'], 'reused': source is not None})
    if len({encoded(semantic(m['case'])) for m in matrix}) != len(matrix):
        raise ValueError('common pool contains duplicate physical proposals')
    for circuit, budget in plan['repeat_groups']:
        for actor in actors[:-1]:
            item = next(e for e in evaluations if e['circuit'] == circuit and e['budget'] == budget
                        and e['actor'] == actor and e['proposal'] == 0)
            case = next(m['case'] for m in matrix if m['case']['id'] == item['native_case_id'])
            matrix.append({'case': deepcopy(case), 'fresh': True})
    if len(evaluations) != plan['primary_gp'] or len(matrix) != plan['total_rows']:
        raise ValueError('matrix differs from frozen counts')
    return matrix, evaluations


FAIR_KEYS = ['bonus_offsets', 'budget', 'components', 'engine', 'initial_state',
             'laps', 'mechanical', 'pit_loss_s', 'points', 'policy_digest',
             'policy_id', 'resolved_compounds', 'stops']


def preflight(worker, plan, matrix, cache):
    old = source_rows(plan)
    _, evaluations = design(plan)
    groups, reuse = {}, []
    for i, item in enumerate(matrix[:plan['primary_gp']]):
        case = item['case']
        resolved = worker.execute(case, resolve=True)
        if not resolved['data'].get('resolved_only') or resolved['returncode']:
            raise ValueError('native resolution failed')
        receipt = resolved['data']['receipt']
        e = evaluations[i]
        group = (e['circuit'], e['budget'], e['proposal'])
        fair = {k: receipt[k] for k in FAIR_KEYS}
        if groups.setdefault(group, fair) != fair:
            raise ValueError(f'common preparation differs across actors: {group}')
        if e['reused']:
            source = old[encoded(semantic(case))]
            if (source['case'] != case or source['data']['case'] != case or BASE.checks(source['data'])
                    or receipt['physical_input_digest'] != source['data']['receipt']['physical_input_digest']):
                raise ValueError('source does not bridge exact current native input')
            meta = {'source_archive': plan['reuse_archive'],
                'source_archive_sha256': plan['retained_files'][plan['reuse_archive']],
                'source_case_id': case['id'],
                'physical_input_digest': receipt['physical_input_digest'],
                'result_digest': source['data']['outcome']['result_digest'],
                'source_row_sha256': hashlib.sha256(encoded(source)).hexdigest()}
            imported = dict(source, reuse_provenance=meta)
            path = worker.cache / key(case)
            if path.exists() and load(path) != imported:
                raise ValueError('refusing to replace a different native cache row')
            if not path.exists():
                write(path, imported)
            reuse.append(meta)
        if (i + 1) % 100 == 0:
            print(f'preflight {i+1}/{plan["primary_gp"]}', flush=True)
    metadata = {'resolved_primary': len(evaluations), 'equal_preparation_groups': len(groups),
                'reused_gp': len(reuse), 'new_gp_expected': len(matrix) - len(reuse), 'reuse': reuse}
    if len(reuse) != plan['expected_reused_gp']:
        raise ValueError('reuse count differs from preregistered source matrix')
    write(cache / 'preflight.json', metadata)
    return metadata


def analyze(data):
    plan = data['plan']
    matrix, evaluations = design(plan)
    if (not data['complete'] or len(data['rows']) != len(matrix)
            or data['source_matrix_sha256'] != hashlib.sha256(encoded(matrix)).hexdigest()):
        raise ValueError('incomplete or changed matrix')
    rows = data['rows']
    if any(row['case'] != item['case'] or row['data']['case'] != item['case']
           or row['binary_sha256'] != plan['native_binary_sha256']
           or row['checks'] != BASE.checks(row['data']) for row, item in zip(rows, matrix)):
        raise ValueError('native evidence binding changed')
    lookup = {row['case']['id']: row for row in rows[:plan['primary_gp']]}
    repeats = []
    for row in rows[plan['primary_gp']:]:
        prior = lookup[row['case']['id']]
        repeats.append({'case_id': row['case']['id'], 'exact': all(
            prior['data'][section][field] == row['data'][section][field]
            for section, field in [('receipt', 'physical_input_digest'), ('outcome', 'result_digest')])})
    numerical = not any(row['checks'] for row in rows) and all(r['exact'] for r in repeats)
    roster = load(ROOT / plan['roster'])
    policy = {'schema': 'pitgun.joint-budget-preparation-policy/v1', 'admitted': False,
        'physics_sha256': sha(ROOT / plan['policy']), 'roster_sha256': sha(ROOT / plan['roster']),
        'selector': 'within-own-best-0.3pct-then-style-midpoint-time/v1', 'contexts': []}
    summaries = []
    if numerical:
        for ctx in plan['contexts']:
            actors = {actor: {'anchors': {}} for actor in roster['drivers']}
            for budget in plan['budgets']:
                relevant = [e for e in evaluations if e['circuit'] == ctx['circuit'] and e['budget'] == budget]
                selected = {}
                for actor, driver in roster['drivers'].items():
                    options = [(e['proposal'], lookup[e['native_case_id']]) for e in relevant if e['actor'] == actor]
                    pid, row = choose(options, driver['style'], BASE)
                    selected[actor] = row
                    case = row['case']
                    actors[actor]['anchors'][str(budget)] = {'proposal': pid,
                        'preparation': {k: deepcopy(case[k]) for k in ['effective_allocation', 'effective_budget', 'downforce', 'gear', 'strategy', 'mode']},
                        'native_case_id': case['id'], 'physical_input_digest': row['data']['receipt']['physical_input_digest'],
                        'result_digest': row['data']['outcome']['result_digest'],
                        'time_s': row['data']['outcome']['time_s'],
                        'regret_s': row['data']['outcome']['time_s'] - min(r['data']['outcome']['time_s'] for _, r in options)}
                players = [lookup[e['native_case_id']] for e in relevant if e['actor'] == 'balanced_reference']
                pools = {'free': players, 'zero_extra': [r for r in players if r['case']['effective_allocation'][0] == 2 and r['case']['effective_allocation'][3] == 1],
                         'heavy_80': [r for r in players if heavy_80(r['case'])]}
                best = {}
                for family, options in pools.items():
                    if not options:
                        best[family] = None
                        continue
                    row = min(options, key=BASE.order)
                    total = row['data']['outcome']['time_s']
                    best[family] = {'time_s': total, 'rank': 1 + sum(BASE.order(r)[0] < total for r in selected.values()),
                        'gap_to_best_ai_s': total - min(BASE.order(r)[0] for r in selected.values()), 'case_id': row['case']['id']}
                summaries.append({'circuit': ctx['circuit'], 'budget': budget,
                    'scope': 'actual career exit' if budget == 24 else 'controlled upgraded equal equipment',
                    'best': best, 'field': {a: {'time_s': BASE.order(r)[0],
                        'rank': 1 + sum(BASE.order(other)[0] < BASE.order(r)[0] for other in selected.values())}
                        for a, r in selected.items()}})
            policy['contexts'].append({'era': 2, 'circuit': ctx['circuit'], 'floors': [2, 1, 4, 1], 'actors': actors})
    gates = {'numerical_and_exact_repeats': numerical}
    if numerical:
        for budget in plan['budgets']:
            group = [s for s in summaries if s['budget'] == budget]
            ranks = [s['best']['free']['rank'] for s in group]
            gains = sum(s['best']['free']['time_s'] < s['best']['zero_extra']['time_s'] - 1e-8 for s in group)
            gates[f'cap{budget}_free_median'] = statistics.median(ranks) <= (3 if budget == 24 else 5)
            gates[f'cap{budget}_shortcut_not_sweep'] = sum(s['best']['zero_extra']['rank'] == 1 for s in group) <= 5
            gates[f'cap{budget}_unrestricted_time_gains'] = gains >= (2 if budget == 24 else 1)
            if budget == 24:
                gates['exit_at_least_one_prepared_win'] = any(rank == 1 for rank in ranks)
    reused = sum('reuse_provenance' in r for r in rows)
    if reused != plan['expected_reused_gp'] or reused != data['preflight']['reused_gp']:
        raise ValueError('final reuse count differs')
    result = {'schema': 'pitgun.joint-era2-preparation-decision/v1', 'complete': True,
        'numerical_pass': numerical, 'development_review_pass': all(gates.values()), 'gates': gates,
        'rows': len(rows), 'reused_gp': reused, 'fresh_native_gp': len(rows) - reused,
        'fresh_native_laps': sum(len(r['data'].get('laps', [])) for r in rows if 'reuse_provenance' not in r),
        'fresh_repeats': repeats, 'summaries': summaries, 'release_admitted': False,
        'scope': 'opened seed 73013, four common preparations; no global optimum or fresh reserve claim'}
    return result, policy


def main():
    parser = argparse.ArgumentParser()
    for name in ['binary', 'cache', 'output']:
        parser.add_argument('--' + name, required=True, type=Path)
    parser.add_argument('--max-new-gp', type=int, default=858)
    parser.add_argument('--max-wall-seconds', type=float, default=7200.)
    args = parser.parse_args()
    args.cache.parent.mkdir(parents=True, exist_ok=True)
    with (args.cache.parent / 'fixed-nine-worker.lock').open('a') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        path = HERE / 'plan-v1.json'
        matrix, _ = design(load(path))
        data = execute_matrix(path, args.binary, args.cache, args.output, matrix, preflight=preflight,
            max_new_gp=args.max_new_gp, max_wall_seconds=args.max_wall_seconds)
        if not data['complete']:
            print(data, flush=True)
            return
        result, policy = analyze(data)
        result['results_sha256'] = sha(args.output)
        write(args.output.parent / 'decision-v1.json', result)
        write(args.output.parent / 'policy-v1.json', policy)
        print({k: result[k] for k in ['numerical_pass', 'development_review_pass', 'fresh_native_gp', 'gates']}, flush=True)


if __name__ == '__main__':
    main()
