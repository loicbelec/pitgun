#!/usr/bin/env python3
"""New seeds/budgets after candidate freeze; no refitting during evaluation."""
import argparse
from copy import deepcopy
import fcntl
import hashlib
from pathlib import Path
import statistics

from candidate import BASE, HERE, ROOT, encoded, field, load, reference, sha, write
from harness import execute_matrix, key


def semantic(case):
    return {k: v for k, v in case.items() if k != 'id'}


def sources(plan):
    path = ROOT / plan['candidate']
    if sha(path) != plan['retained_files'][plan['candidate']]:
        raise ValueError('candidate changed after freeze')
    candidate = load(path)
    roster = load(ROOT / candidate['roster'])
    return candidate, roster


def design(plan, era):
    candidate, roster = sources(plan)
    matrix, evaluations, representatives = [], [], {}
    for ctx in [c for c in candidate['contexts'] if c['era'] == era]:
        for group in plan['groups'][str(era)]:
            budget, seed = group['budget'], group['seed']
            actors = [(a, a) for a in roster['drivers']] + [('free', 'balanced_reference'), ('zero_extra', 'balanced_reference')]
            for role, actor in actors:
                prep = (field(candidate, era, ctx['circuit'], budget, actor) if actor != 'balanced_reference'
                        else reference(candidate, era, ctx['circuit'], budget, role))
                career = actor == 'balanced_reference' and budget == (12 if era == 1 else 24)
                case = BASE.case(ctx, {'points': prep['effective_allocation'], 'downforce': prep['downforce'],
                    'gear': prep['gear'], 'strategy': prep['strategy']}, actor, budget, seed=seed,
                    kind='career' if career else 'field', stage='exit' if career or era == 2 else 'entry',
                    label=f'fresh-reserve-v1/{role}/{seed}')
                expected = {k: case[k] for k in prep}
                if expected != prep:
                    raise ValueError('native case builder altered frozen preparation')
                digest = encoded(semantic(case))
                if digest not in representatives:
                    representatives[digest] = case['id']
                    matrix.append({'case': case, 'fresh': False})
                evaluations.append({'era': era, 'circuit': ctx['circuit'], 'budget': budget,
                    'seed': seed, 'role': role, 'logical_case': case,
                    'native_case_id': representatives[digest]})
    for circuit in plan['repeat_circuits'][str(era)]:
        for actor in ['franz_hermann', 'charles_leclair']:
            e = next(e for e in evaluations if e['circuit'] == circuit and e['role'] == actor
                     and e['budget'] == (12 if era == 1 else 24))
            matrix.append({'case': deepcopy(e['logical_case']), 'fresh': True})
    expected = plan['expected'][str(era)]
    if len(evaluations) != expected['logical'] or len(matrix) != expected['native']:
        raise ValueError('changed frozen matrix counts')
    if len({key(m['case'], fresh=m['fresh']) for m in matrix}) != len(matrix):
        raise ValueError('duplicate native jobs')
    return matrix, evaluations


def seen_seeds(value):
    if isinstance(value, dict):
        result = {value['seed']} if isinstance(value.get('seed'), int) else set()
        for child in value.values(): result |= seen_seeds(child)
        return result
    if isinstance(value, list):
        result = set()
        for child in value: result |= seen_seeds(child)
        return result
    return set()


def preflight(worker, plan, matrix, cache):
    candidate, roster = sources(plan)
    fresh_seeds = {m['case']['seed'] for m in matrix}
    prior_seeds = set()
    for path in plan['prior_archives']:
        prior_seeds |= seen_seeds(load(ROOT / path))
    if fresh_seeds & prior_seeds:
        raise ValueError('fresh seed was already opened in retained development/validation evidence')
    resolved = []
    for item in [m for m in matrix if not m['fresh']]:
        c = item['case']; r = worker.execute(c, resolve=True)
        if r['returncode'] or not r['data'].get('resolved_only'):
            raise ValueError('native resolution failed')
        receipt = r['data']['receipt']
        era = 1 if ':era1-' in c['fixture_id'] else 2
        expected_components = None if era == 1 else {'schema_version': 'pitgun.racing-vehicle-components/v1', 'engine_id': 'v8_1970', 'aero_id': 'basic'}
        if (receipt['components'] != expected_components or receipt['initial_state']['fuel_mass'] != 100.
                or receipt['initial_state']['tire_temp'] != 70. or receipt['points'] != c['effective_allocation']
                or receipt['budget'] != c['effective_budget']):
            raise ValueError('hardware/initial state/budget binding changed')
        driver = receipt['driver']
        expected_traits = ({'limit_exploitation': .84, 'consistency': .84, 'tire_management': .84}
                           if c['actor'] == 'balanced_reference' else roster['drivers'][c['actor']]['resource']['traits'])
        stream = 'career-driver:player' if c['actor'] == 'balanced_reference' else 'fixed-field:' + c['actor']
        if (driver['controls']['driver_traits'] != expected_traits
                or driver['controls']['driving_mode'] != 'balanced'
                or driver['lapse_profile']['noise_stream_id'] != stream):
            raise ValueError('driver identity/traits/mode/stream changed')
        resolved.append({'case_id': c['id'], 'physical_input_digest': receipt['physical_input_digest']})
    result = {'candidate_sha256': sha(ROOT / plan['candidate']), 'fresh_seeds': sorted(fresh_seeds),
        'prior_seeds': sorted(prior_seeds), 'prior_seed_scope': plan['prior_archives'],
        'resolved_primary': len(resolved), 'resolved_inputs': resolved,
        'field_floors': {'1': [0,0,0,0], '2': [2,1,4,1]},
        'actual_exit_reference_floors': {'1': [0,1,0,0], '2': [2,1,4,1]}}
    write(cache / 'preflight.json', result)
    return result


def analyze(data, era):
    plan = data['plan']; matrix, evaluations = design(plan, era)
    if (not data['complete'] or len(data['rows']) != len(matrix)
            or data['source_matrix_sha256'] != hashlib.sha256(encoded(matrix)).hexdigest()):
        raise ValueError('incomplete or changed evaluation matrix')
    rows = data['rows']
    if any(r['case'] != m['case'] or r['data']['case'] != m['case'] or r['checks'] != BASE.checks(r['data'])
           or r['binary_sha256'] != plan['native_binary_sha256'] for r,m in zip(rows,matrix)):
        raise ValueError('native evidence binding changed')
    primary = {r['case']['id']: r for r,m in zip(rows,matrix) if not m['fresh']}
    repeats = [{'case_id': r['case']['id'], 'exact': all(
        primary[r['case']['id']]['data'][s][k] == r['data'][s][k]
        for s,k in [('receipt','physical_input_digest'),('outcome','result_digest')])}
        for r,m in zip(rows,matrix) if m['fresh']]
    numerical = not any(r['checks'] for r in rows) and all(r['exact'] for r in repeats)
    _, roster = sources(plan)
    summaries = []
    if numerical:
        groups = sorted({(e['circuit'],e['budget'],e['seed']) for e in evaluations})
        for circuit,budget,seed in groups:
            ev = [e for e in evaluations if (e['circuit'],e['budget'],e['seed']) == (circuit,budget,seed)]
            field_rows = {e['role']: primary[e['native_case_id']] for e in ev if e['role'] in roster['drivers']}
            if len(field_rows) != 9: raise ValueError('incomplete field')
            best = {}
            for family in ['free','zero_extra']:
                e = next(e for e in ev if e['role'] == family); row = primary[e['native_case_id']]
                total = row['data']['outcome']['time_s']
                best[family] = {'time_s': total, 'rank': 1+sum(BASE.order(r)[0] < total for r in field_rows.values()),
                    'gap_to_best_ai_s': total-min(BASE.order(r)[0] for r in field_rows.values()),
                    'native_case_id': row['case']['id']}
            summaries.append({'era': era, 'circuit': circuit, 'budget': budget, 'seed': seed,
                'scope': 'actual career exit' if budget == (12 if era == 1 else 24) else 'controlled common-era equipment',
                'reference': best, 'field': {a: {'time_s': BASE.order(r)[0],
                    'rank': 1+sum(BASE.order(other)[0] < BASE.order(r)[0] for other in field_rows.values())}
                    for a,r in field_rows.items()}})
    gates = {'numerical_and_exact_repeats': numerical}
    if numerical:
        for group in plan['groups'][str(era)]:
            budget = group['budget']; ss = [s for s in summaries if s['budget'] == budget]
            is_exit = budget == (12 if era == 1 else 24)
            gates[f'cap{budget}_prepared_median'] = statistics.median(s['reference']['free']['rank'] for s in ss) <= (3 if is_exit else 5)
            gates[f'cap{budget}_shortcut_not_sweep'] = sum(s['reference']['zero_extra']['rank'] == 1 for s in ss) <= (2 if era == 1 else 5)
            gains = sum(s['reference']['free']['time_s'] < s['reference']['zero_extra']['time_s']-1e-8 for s in ss)
            gates[f'cap{budget}_prepared_time_gain'] = gains >= (2 if era == 2 and is_exit else 1)
            if is_exit:
                gates['actual_exit_prepared_can_win'] = any(s['reference']['free']['rank'] == 1 for s in ss)
    return {'schema': 'pitgun.fixed-nine-fresh-reserve-decision/v1', 'era': era, 'complete': True,
        'candidate_sha256': sha(ROOT/plan['candidate']), 'numerical_pass': numerical,
        'reserved_review_pass': all(gates.values()), 'gates': gates, 'fresh_native_gp': len(rows),
        'logical_evaluations': len(evaluations), 'id_only_reference_aliases': len(evaluations)-len(primary),
        'fresh_native_laps': sum(len(r['data'].get('laps',[])) for r in rows),
        'fresh_repeats': repeats, 'summaries': summaries, 'release_admitted': False,
        'scope': 'one frozen preparation per role; new seeds and budget probes, no refit or global-optimum claim'}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--era', type=int, choices=[1,2], required=True)
    for name in ['binary','cache','output']: parser.add_argument('--'+name, type=Path, required=True)
    parser.add_argument('--max-new-gp', type=int, default=339)
    parser.add_argument('--max-wall-seconds', type=float, default=7200.)
    args = parser.parse_args(); args.cache.parent.mkdir(parents=True,exist_ok=True)
    with (args.cache.parent/'fixed-nine-worker.lock').open('a') as lock:
        fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
        path = HERE/'plan-v1.json'; matrix,_ = design(load(path),args.era)
        data = execute_matrix(path,args.binary,args.cache,args.output,matrix,preflight=preflight,
            max_new_gp=args.max_new_gp,max_wall_seconds=args.max_wall_seconds)
        if not data['complete']:
            print(data,flush=True); return
        result = analyze(data,args.era); result['results_sha256'] = sha(args.output)
        write(args.output.parent/f'era{args.era}-decision-v1.json',result)
        print({k:result[k] for k in ['era','numerical_pass','reserved_review_pass','gates']},flush=True)


if __name__ == '__main__': main()
