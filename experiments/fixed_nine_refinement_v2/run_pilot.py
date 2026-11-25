#!/usr/bin/env python3
"""Opened-case equal-preparation Mexico diagnostic; no field admission."""
import argparse
from collections import defaultdict
from copy import deepcopy
import fcntl
from pathlib import Path
import hashlib
from harness import BASE, HERE, ROOT, encoded, execute_matrix, load, sha, write


def cases(plan):
    rows = []
    context = {'era': 2, 'circuit': 'MEXICO'}
    for budget in plan['budgets']:
        for actor in plan['actors']:
            for proposal in plan['proposals']:
                c = BASE.case(context, proposal, actor, budget, seed=plan['seed'], label=proposal['id'])
                rows.append({'case': c, 'fresh': False})
    assert len(rows) == 24 and len({r['case']['id'] for r in rows}) == 24
    lookup = {r['case']['id']: r for r in rows}
    for actor in plan['actors']:
        for budget, proposal in plan['repeats']:
            original = f'2/MEXICO/{actor}/{budget}/{proposal}'
            rows.append({'case': deepcopy(lookup[original]['case']), 'fresh': True})
    assert len(rows) == plan['maximum_gp'] == 30
    return rows


def fairness_checks(rows, plan):
    errors = []
    groups = defaultdict(dict)
    common = ['bonus_offsets', 'budget', 'components', 'engine', 'fixture_digest',
              'initial_state', 'laps', 'mechanical', 'pit_loss_s', 'points',
              'policy_digest', 'resolved_compounds', 'stops']
    roster = load(ROOT / 'experiments/fixed_opponents_v2/roster-v2.json')['drivers']
    for row in rows:
        c, d = row['case'], row['data']
        if 'receipt' not in d:
            errors.append('missing_resolution:' + c['id'])
            continue
        r = d['receipt']
        if (r['points'] != c['effective_allocation'] or r['budget'] != c['effective_budget']
            or r['bonus_offsets'] != [2, 1, 4, 1] or r['initial_state']['fuel_mass'] != 100.
            or r['laps'] != 71 or r['stops'] != [35]
            or r['components'] != {'schema_version': 'pitgun.racing-vehicle-components/v1', 'engine_id': 'v8_1970', 'aero_id': 'basic'}):
            errors.append('case_binding:' + c['id'])
        expected_traits = ({'limit_exploitation': .84, 'consistency': .84, 'tire_management': .84}
                           if c['actor'] == 'balanced_reference' else roster[c['actor']]['resource']['traits'])
        stream = 'career-driver:player' if c['actor'] == 'balanced_reference' else 'fixed-field:' + c['actor']
        if (r['driver']['controls']['driver_traits'] != expected_traits
            or r['driver']['controls']['driver_id'] != c['actor']
            or r['driver']['controls']['driving_mode'] != 'balanced'
            or r['driver']['lapse_profile']['noise_stream_id'] != stream):
            errors.append('driver_binding:' + c['id'])
        groups[(c['effective_budget'], c['id'].split('/')[-1])][c['actor']] = {k: r[k] for k in common}
    for group, actors in groups.items():
        if set(actors) != set(plan['actors']):
            errors.append('incomplete_actor_group:' + str(group))
        elif any(value != actors['balanced_reference'] for value in actors.values()):
            errors.append('unequal_physical_preparation:' + str(group))
    if len(groups) != 8:
        errors.append('incomplete_preparation_groups')
    return errors


def preflight(worker, plan, matrix, cache):
    rows = [worker.execute(item['case'], resolve=True) for item in matrix[:24]]
    errors = fairness_checks(rows, plan)
    if not all(r['data'].get('resolved_only') for r in rows):
        errors.append('not_resolve_only')
    write(cache / 'preflight.json', {'errors': errors, 'rows': rows})
    if errors:
        raise ValueError('pilot preflight failed: ' + str(errors))


def analyze(data):
    p = data['plan']; rows = data['rows']
    if not data['complete'] or len(rows) != 30 or [r['case'] for r in rows] != [r['case'] for r in cases(p)]:
        raise ValueError('incomplete or changed matrix')
    if data['source_matrix_sha256'] != hashlib.sha256(encoded(cases(p))).hexdigest():
        raise ValueError('matrix identity changed')
    if any(r['checks'] != BASE.checks(r['data']) for r in rows):
        raise ValueError('native checks do not regenerate')
    fairness = fairness_checks(rows[:24], p)
    numerical = not any(r['checks'] for r in rows) and not fairness
    lookup = {r['case']['id']: r for r in rows[:24]}
    repeats = []
    for r in rows[24:]:
        a = lookup[r['case']['id']]
        exact = not a['checks'] and not r['checks'] and all(
            a['data'][section][field] == r['data'][section].get(field)
            for section, field in [('receipt', 'physical_input_digest'), ('outcome', 'result_digest')])
        repeats.append({'case_id': r['case']['id'], 'exact': exact})
    comparisons = []; grouped = defaultdict(dict)
    for r in rows[:24]:
        c = r['case']; grouped[(c['effective_budget'], c['actor'])][c['id'].split('/')[-1]] = r
    if numerical:
        for (budget, actor), proposals in sorted(grouped.items()):
            times = {name: r['data']['outcome']['time_s'] for name, r in proposals.items()}
            comparisons.append({'budget': budget, 'actor': actor, 'times_s': times,
                'best_proposal': min(times, key=lambda name: (times[name], name)),
                'cooling_advantage_at_df04_s': times['concentrated-df04'] - times['cooling-df04'],
                'cooling_advantage_at_df05_s': times['concentrated-df05'] - times['cooling-df05']})
    bridges = []
    previous = load(ROOT / p['previous_validation_archive'])
    for name, suffix in [('cooling-df04', 'informed'), ('concentrated-df04', 'concentrated')]:
        current = lookup[f'2/MEXICO/balanced_reference/16/{name}']
        before = next(r for r in previous['rows'] if r['case']['id'] == f'2/MEXICO/player/16/reserved-{suffix}')
        exact = not current['checks'] and all(current['data'][s].get(k) == before['data'][s][k]
            for s, k in [('receipt', 'physical_input_digest'), ('outcome', 'result_digest')])
        bridges.append({'case_id': current['case']['id'], 'prior_case_id': before['case']['id'], 'exact': exact})
    numerical &= all(r['exact'] for r in repeats) and all(b['exact'] for b in bridges)
    diagnostic = []
    if numerical:
        for budget in p['budgets']:
            group = [c for c in comparisons if c['budget'] == budget]
            reference = next(c for c in group if c['actor'] == 'balanced_reference')
            best_reference_concentrated = min(reference['times_s'][n] for n in ['concentrated-df04', 'concentrated-df05'])
            best_elite = min(min(c['times_s'].values()) for c in group if c['actor'] != 'balanced_reference')
            diagnostic.append({'budget': budget, 'best_elite_time_s': best_elite,
                'best_reference_concentrated_time_s': best_reference_concentrated,
                'elite_lead_over_concentrated_s': best_reference_concentrated - best_elite,
                'cooling_tradeoff_faster_for_all_three_at_both_sliders': all(
                    c['cooling_advantage_at_df04_s'] > 1e-6 and c['cooling_advantage_at_df05_s'] > 1e-6 for c in group)})
    supports = numerical and all(d['elite_lead_over_concentrated_s'] > 1e-6 and
        d['cooling_tradeoff_faster_for_all_three_at_both_sliders'] for d in diagnostic)
    return {'schema': 'pitgun.fixed-nine-mexico-pilot-decision/v2', 'complete': True,
        'numerical_pass': numerical, 'fairness_errors': fairness, 'main_gp': 24, 'fresh_repeats': repeats,
        'prior_result_bridges': bridges, 'accepted_laps': sum(len(r['data'].get('laps', [])) for r in rows),
        'comparisons': comparisons, 'diagnostic': diagnostic,
        'preparation_hypothesis_supported': supports, 'release_admitted': False,
        'scope': 'opened-case diagnosis; not fresh validation or universal physical balance'}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--cache', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    args.cache.parent.mkdir(parents=True, exist_ok=True)
    lock = (args.cache.parent / 'fixed-nine-worker.lock').open('a')
    fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    plan_path = HERE / 'pilot-plan-v2.json'; plan = load(plan_path)
    data = execute_matrix(plan_path, args.binary, args.cache, args.output, cases(plan), preflight=preflight)
    result = analyze(data)
    result['results_sha256'] = sha(args.output)
    write(args.output.parent / 'pilot-decision-v2.json', result)
    print({k: result[k] for k in ['numerical_pass', 'preparation_hypothesis_supported', 'accepted_laps']}, flush=True)


if __name__ == '__main__': main()
