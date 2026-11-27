#!/usr/bin/env python3
"""Read-only qualification gates; no fitting or runtime activation."""
import argparse
import gzip
import json
from pathlib import Path

from run import ROOT, cases, numerical_checks, repeat_ids, sha, write


def case_key(case):
    return json.dumps({k: v for k, v in case.items() if k != 'id'}, sort_keys=True, separators=(',', ':'))


def labels(row):
    p = row['case']['id'].split('/')
    return (p[1], 'entry' if p[0] == 'era1' else p[2], p[2] if p[0] == 'era1' else p[3],
        p[3] if p[0] == 'era1' else p[4], p[4:] if p[0] == 'era1' else p[5:])


def metrics(row):
    if row['checks']: return None
    laps = row['data']['laps']; post = []; offset = 0
    for stint in row['case']['strategy']['stints']:
        post += laps[offset + 3:offset + stint['laps']]; offset += stint['laps']
    duration = sum(l['time_s'] for l in post)
    def mean(field): return sum(l[field] * l['time_s'] for l in post) / duration
    return {'time_s': row['data']['outcome']['time_s'], 'service_s': row['data']['outcome']['service_time_s'],
        'surface_mean_c': mean('surface_mean_c'), 'carcass_mean_c': mean('carcass_mean_c'),
        'hot_fraction': mean('hot_fraction'), 'cold_fraction': mean('cold_fraction'),
        'window_fraction': mean('peak_window_fraction'), 'temperature_grip_mean': mean('temperature_grip_mean'),
        'maximum_surface_c': max(l['surface_max_c'] for l in laps),
        'maximum_carcass_c': max(l['carcass_max_c'] for l in laps),
        'maximum_wear': max(l['wear_end'] for l in laps),
        'minimum_engine_capacity': min(l['minimum_power_fraction'] for l in laps),
        'derated_time_s': sum(l['derated_time_s'] for l in laps),
        'driver_lapses': sum(l['driver_lapses'] for l in laps),
        'final_fuel_kg': row['data']['outcome']['final_state']['fuel_mass']}


def analyze(data):
    era, plan, rows = data['era'], data['plan'], data['rows']
    if not data['complete'] or [r['case'] for r in rows] != cases(plan, era):
        raise ValueError('incomplete or mismatched qualification matrix')
    lookup = {r['case']['id']: r for r in rows}
    validations = data['validations']
    if [v['baseline_id'] for v in validations] != repeat_ids(plan, era):
        raise ValueError('fresh repeat coverage mismatch')
    all_rows = rows + [v['fresh'] for v in validations]
    for r in all_rows:
        if numerical_checks(r['data']) != r['checks']: raise ValueError('retained numerical checks changed')
    exact = []
    for v in validations:
        before, fresh = lookup[v['baseline_id']], v['fresh']
        exact.append(not before['checks'] and not fresh['checks'] and before['case'] == fresh['case'] and v['exact'] and
            before['data']['receipt']['physical_input_digest'] == fresh['data']['receipt']['physical_input_digest'] and
            before['data']['outcome'].get('result_digest') == fresh['data']['outcome'].get('result_digest'))
    by_case = {case_key(r['case']): r for r in rows}
    bridges = []
    for path in [p for p in plan['retained_files'] if p.endswith('.json.gz')]:
        if sha(ROOT / path) != plan['retained_files'][path]: raise ValueError('screen archive changed')
        screen = json.load(gzip.open(ROOT / path))
        for r in screen['rows']:
            c = r['case']
            if c['policy_id'] != plan['policy_id'] or (c['fixture_id'] == 'craft_engineering:era1-entry') != (era == 'era1'): continue
            current = by_case[case_key(c)]
            bridges.append({'screen_id': c['id'], 'case_id': current['case']['id'], 'physical_input_exact':
                r['data']['receipt']['physical_input_digest'] == current['data'].get('receipt', {}).get('physical_input_digest'),
                'result_exact': r['data']['outcome'].get('result_digest') == current['data']['outcome'].get('result_digest')})
    numerical_pass = (not any(r['checks'] for r in all_rows) and all(exact) and
        len(bridges) == (3 if era == 'era1' else 11) and all(b['physical_input_exact'] and b['result_exact'] for b in bridges))
    result = {'schema': 'pitgun.thermal-candidate-qualification-decision/v1', 'era': era,
        'policy_id': plan['policy_id'], 'numerical_pass': numerical_pass, 'qualified_for_ai_preparation': False,
        'main_gp': len(rows), 'fresh_repeats': len(validations), 'exact_repeats': sum(exact),
        'laps': sum(len(r['data'].get('laps', [])) for r in all_rows), 'bridges': bridges,
        'failures': [{'id': r['case']['id'], 'checks': r['checks']} for r in all_rows if r['checks']],
        'review_flags': [], 'cooling_comparisons': [], 'stop_comparisons': [], 'sliders': [], 'modes': [],
        'thermal': [], 'release_admitted': False,
        'maximum_tire_balance_residual_j': max(r['data'].get('thermal_ledger', {}).get('maximum_tire_balance_residual_j', 0.) for r in all_rows),
        'maximum_carry_jump': max(max(r['data'].get('maximum_carry_jumps_speed_engine_surface_carcass_wear', [0.])) for r in all_rows)}
    if not numerical_pass: return result
    def time(row): return row['data']['outcome']['time_s']
    def best(eligible): return min(eligible, key=time)
    circuits = list(dict.fromkeys(r['case']['circuit'] for r in rows))
    stages = ['entry'] if era == 'era1' else ['entry', 'exit']
    normal = (['SM-50', 'MH-50', 'HS-50', 'SMS-thirds', 'MHM-thirds'] if era == 'era1' else ['SM-50', 'MH-50', 'SMS-thirds'])
    profiles = (['balanced', 'engine_chassis', 'cooling'] if era == 'era1' else ['balanced', 'no_discretionary_cooling', 'cooling'])
    for circuit in circuits:
        for stage in stages:
            core = [r for r in rows if labels(r)[:2] == (circuit, stage) and not labels(r)[4]]
            winners = {p: best([r for r in core if labels(r)[2] == p and labels(r)[3] in normal]) for p in profiles}
            concentrated = winners[profiles[1]]
            alternative = best([winners[profiles[0]], winners[profiles[2]]])
            gap = time(concentrated) - time(alternative)
            cooling = {'circuit': circuit, 'stage': stage, 'profile_winners': {p: {'id': r['case']['id'], 'time_s': time(r)} for p, r in winners.items()},
                'concentrated_minus_best_other_s': gap}
            if gap <= 0 and (era == 'era1' or stage == 'entry'):
                result['review_flags'].append(f'{circuit}/{stage}:concentrated_preparation_leads_sampled_normal_plans')
            if era == 'era1':
                gear_winners = {p: best([r for r in rows if labels(r)[:3] == (circuit, stage, p) and labels(r)[3] == 'SM-50'
                    and (not labels(r)[4] or labels(r)[4][0].startswith('gear-'))]) for p in profiles}
                gear_gap = time(gear_winners['engine_chassis']) - min(time(gear_winners['balanced']), time(gear_winners['cooling']))
                cooling['legal_SM_gear_concentrated_minus_other_s'] = gear_gap
                if gear_gap <= 0: result['review_flags'].append(f'{circuit}:concentrated_preparation_leads_legal_SM_gears')
            result['cooling_comparisons'].append(cooling)
            balanced = [r for r in core if labels(r)[2] == 'balanced']
            stops = {}
            for n in [0, 1, 2, 3]:
                eligible = [r for r in balanced if len(r['case']['strategy']['pit_laps']) == n]
                if eligible: stops[n] = best(eligible)
            result['stop_comparisons'].append({'circuit': circuit, 'stage': stage,
                'best_by_stops': {str(n): {'id': r['case']['id'], 'time_s': time(r), 'service_s': r['data']['outcome']['service_time_s']} for n, r in stops.items()},
                'three_beats_one_two': time(stops[3]) < min(time(stops[1]), time(stops[2]))})
            base = next(r for r in balanced if labels(r)[3] == 'SM-50')
            m = metrics(base); result['thermal'].append({'circuit': circuit, 'stage': stage, **m})
            # Test warming within an actual stint, not across a fresh-set reset.
            lap_data = base['data']['laps']; offset = 0; warming = False
            for stint in base['case']['strategy']['stints']:
                segment = lap_data[offset:offset + stint['laps']]; offset += stint['laps']
                warming |= max(l['surface_mean_c'] for l in segment) > segment[0]['surface_mean_c'] + 2.
            if not warming: result['review_flags'].append(f'{circuit}/{stage}:no_balanced_SM_warming_response')
            variants = [r for r in rows if labels(r)[:4] == (circuit, stage, 'balanced', 'SM-50') and labels(r)[4]]
            for r in variants:
                suffix = '/'.join(labels(r)[4]); detail = {'circuit': circuit, 'stage': stage, 'variant': suffix,
                    'delta_s': time(r) - time(base), 'driver_lapses': metrics(r)['driver_lapses']}
                if suffix in ['manage', 'attack']: result['modes'].append(detail)
                else: result['sliders'].append(detail)
    for stage in stages:
        group = [r for r in result['stop_comparisons'] if r['stage'] == stage]
        if all(r['three_beats_one_two'] for r in group):
            result['review_flags'].append(f'{stage}:universal_sampled_three_stop_advantage')
    result['qualified_for_ai_preparation'] = not result['review_flags']
    return result


def markdown(results):
    complete = len(results) == 2
    lines = ['# Selected thermal policy — era-I/II qualification', '',
        f"Scope complete: **{complete}**. Policy `rolling-heat-065`; 100 kg initial fuel, unchanged coefficients, no AI fit or runtime adoption.", '',
        '| Era | Main GP | Fresh repeats exact | Laps | Numerical pass | Review flags |', '|---|---:|---:|---:|---|---|']
    for r in results:
        lines.append(f"| {r['era']} | {r['main_gp']} | {r['exact_repeats']}/{r['fresh_repeats']} | {r['laps']} | {r['numerical_pass']} | {'; '.join(r['review_flags']) or 'none'} |")
    for r in results:
        if not r['numerical_pass']: continue
        lines += ['', f"## {r['era']} — cooling and concentrated preparation", '',
            'Same recipe set and means for each profile. Positive gap means the concentrated engine/chassis or zero-extra-cooling profile is slower. '
            'Exit Cooling1 is retained; a negative exit gap is legitimate. These are sampled alternatives, not global setup optima.', '',
            '| Circuit | Stage | Concentrated minus best other s | Legal SM gears comparison s |', '|---|---|---:|---:|']
        for c in r['cooling_comparisons']:
            gear = f"{c['legal_SM_gear_concentrated_minus_other_s']:+.3f}" if 'legal_SM_gear_concentrated_minus_other_s' in c else 'n/a'
            lines.append(f"| {c['circuit']} | {c['stage']} | {c['concentrated_minus_best_other_s']:+.3f} | {gear} |")
        lines += ['', f"## {r['era']} — sampled balanced stop schedules", '',
            'Service-inclusive totals; zero stops are diagnostic only. Recipes and their exact boundaries are retained in the archive. '
            'One/two/three-stop winners may use different compound families; matched-MH controls are additionally present.', '',
            '| Circuit | Stage | Best 0-stop s | Best 1-stop s | Best 2-stop s | Best 3-stop s | Three beats one/two |', '|---|---|---:|---:|---:|---:|---|']
        for c in r['stop_comparisons']:
            t = [f"{c['best_by_stops'][str(n)]['time_s']:.3f}" if str(n) in c['best_by_stops'] else 'not sampled' for n in range(4)]
            lines.append(f"| {c['circuit']} | {c['stage']} | {' | '.join(t)} | {c['three_beats_one_two']} |")
        lines += ['', f"## {r['era']} — balanced SM thermal response", '',
            'Driving-time-weighted means/exposure exclude the first three laps of each stint. Peaks and engine minima include every lap. '
            'Surface and carcass are separate model states, not identified tire measurements.', '',
            '| Circuit | Stage | Surface/carcass mean °C | Hot/cold/window % | Mean temp grip | Max wear % | Min engine capacity |',
            '|---|---|---:|---:|---:|---:|---:|']
        for c in r['thermal']:
            lines.append(f"| {c['circuit']} | {c['stage']} | {c['surface_mean_c']:.1f}/{c['carcass_mean_c']:.1f} | {100*c['hot_fraction']:.1f}/{100*c['cold_fraction']:.1f}/{100*c['window_fraction']:.1f} | {c['temperature_grip_mean']:.3f} | {100*c['maximum_wear']:.1f} | {c['minimum_engine_capacity']:.3f} |")
        lines += ['', f"## {r['era']} — legal slider, seed and driver contrasts", '',
            '| Circuit | Stage | Variant | Delta balanced s | Recoverable lapses |', '|---|---|---|---:|---:|']
        for c in r['sliders'] + r['modes']:
            lines.append(f"| {c['circuit']} | {c['stage']} | {c['variant']} | {c['delta_s']:+.3f} | {c['driver_lapses']} |")
    qualified = complete and all(r['numerical_pass'] and r['qualified_for_ai_preparation'] for r in results)
    lines += ['', '## Decision and limits', '', f'Candidate supports proceeding to final AI preparation: **{qualified}**.', '',
        'No nine-AI win rate, global optimum, all-budget fairness, broad aggression-risk calibration, industrial accuracy or native/WASM proof is claimed. '
        'Point reallocations move multiple axes; early aero can be unavailable. Old results retain their own policy identity. '
        'All high-temperature/cold/zero-stop extremes and raw scalar trajectories remain archived; no coefficient change or asymmetric stop limit hides a failure. '
        'The game/runtime/catalog and deployed services remain unchanged.']
    return '\n'.join(lines) + '\n'


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('inputs', nargs='+', type=Path)
    parser.add_argument('--output-dir', type=Path, required=True)
    args = parser.parse_args()
    results = [analyze(json.load(gzip.open(p))) for p in args.inputs]
    if len({r['era'] for r in results}) != len(results): raise ValueError('duplicate era')
    args.output_dir.mkdir(parents=True, exist_ok=True)
    write(args.output_dir / 'DECISION_V1.json', {'phases': results, 'scope_complete': len(results) == 2,
        'qualified_for_ai_preparation': len(results) == 2 and all(r['qualified_for_ai_preparation'] for r in results), 'release_admitted': False})
    (args.output_dir / 'RESULTS_V1.md').write_text(markdown(results))
    print(json.dumps([{'era': r['era'], 'numerical_pass': r['numerical_pass'], 'review_flags': r['review_flags']} for r in results]))


if __name__ == '__main__': main()
