#!/usr/bin/env python3
"""Read-only aero audit. No coefficient fit or assumed preferred slider."""
import argparse
import gzip
import json
from pathlib import Path

from run import ROOT, cases, numerical_checks, repeat_ids, sha, write


def metrics(row):
    d = row['data']; laps = d['laps']; bins = []
    for index in range(3):
        parts = [l['spatial_bins'][index] for l in laps]
        summed = {key: sum(p[key] for p in parts) for key in parts[0] if key != 'maximum_speed_mps'}
        summed['maximum_speed_mps'] = max(p['maximum_speed_mps'] for p in parts)
        bins.append(summed)
    drive = sum(b['time_s'] for b in bins)
    return {'total_s': d['outcome']['time_s'], 'drive_s': drive, 'service_s': d['outcome']['service_time_s'],
        'first_five_drive_s': sum(l['time_s'] for l in laps[:5]), 'bins': bins,
        'braking_time_s': sum(b['braking_time_s'] for b in bins),
        'brake_cap_time_s': sum(b['mechanical_brake_cap_time_s'] for b in bins),
        'near_redline_top_gear_time_s': sum(b['near_redline_top_gear_time_s'] for b in bins),
        'drag_work_j': sum(b['drag_work_midpoint_j'] for b in bins),
        'brake_work_j': sum(b['brake_work_j'] for b in bins),
        'maximum_speed_kph': max(b['maximum_speed_mps'] for b in bins) * 3.6,
        'surface_mean_c': sum(l['surface_mean_c'] * l['time_s'] for l in laps) / drive,
        'temperature_grip_mean': sum(l['temperature_grip_mean'] * l['time_s'] for l in laps) / drive,
        'maximum_wear': max(l['wear_end'] for l in laps),
        'drag_area_m2': d['receipt']['mechanical']['fixed_drag_area_m2'],
        'downforce_area_m2': d['receipt']['mechanical']['fixed_downforce_area_m2']}


def analyze(data):
    p = data['plan']; rows = data['rows']; validations = data['validations']
    if not data['complete'] or [r['case'] for r in rows] != cases(p): raise ValueError('incomplete/wrong matrix')
    if [v['baseline_id'] for v in validations] != repeat_ids(p): raise ValueError('repeat coverage changed')
    for row in rows + [v['fresh'] for v in validations]:
        if numerical_checks(row['data']) != row['checks']: raise ValueError('check mismatch')
    lookup = {r['case']['id']: r for r in rows}
    previous = json.load(gzip.open(ROOT / p['baseline_archive']))
    assert sha(ROOT / p['baseline_archive']) == p['baseline_archive_sha256']
    old = {r['case']['id']: r for r in previous['rows']}
    bridges = []
    for context in p['contexts']:
        circuit = context['circuit']
        for slider in [.2, .5, .8]:
            current = lookup[f'{circuit}/coupled/{slider}']['data']
            key = f'era2/{circuit}/exit/balanced/SM-50' + (f'/downforce-{slider}' if slider != .5 else '')
            before = old[key]['data']
            bridges.append({'case_id': current['case']['id'], 'physical_input_exact':
                current.get('receipt', {}).get('physical_input_digest') == before['receipt']['physical_input_digest'],
                'result_exact': current['outcome'].get('result_digest') == before['outcome']['result_digest']})
    exact = []
    for v in validations:
        original, fresh = lookup[v['baseline_id']], v['fresh']
        exact.append(not original['checks'] and not fresh['checks'] and v['exact'] and original['case'] == fresh['case'] and
            original['data']['outcome']['result_digest'] == fresh['data']['outcome'].get('result_digest') and
            original['data']['receipt']['physical_input_digest'] == fresh['data'].get('receipt', {}).get('physical_input_digest'))
    passed = not any(r['checks'] for r in rows + [v['fresh'] for v in validations]) and all(exact) and all(b['physical_input_exact'] and b['result_exact'] for b in bridges)
    result = {'schema': 'pitgun.aero-tradeoff-audit-decision/v1', 'numerical_pass': passed,
        'main_gp': len(rows), 'fresh_repeats': len(validations), 'exact_repeats': sum(exact),
        'accepted_laps': sum(len(r['data'].get('laps', [])) for r in rows + [v['fresh'] for v in validations]),
        'bridges': bridges, 'failures': [{'id': r['case']['id'], 'checks': r['checks']} for r in rows if r['checks']],
        'metrics': {}, 'sampled_coupled_best': {}, 'model_changed': False, 'release_admitted': False}
    if not passed: return result
    result['metrics'] = {r['case']['id']: metrics(r) for r in rows}
    for context in p['contexts']:
        circuit = context['circuit']
        ids = [f'{circuit}/coupled/{x}' for x in p['sliders']]
        result['sampled_coupled_best'][circuit] = min(ids, key=lambda key: result['metrics'][key]['total_s'])
    return result


def markdown(data, result):
    lines = ['# Aero tradeoff audit V1', '',
        f"Numerical PASS: **{result['numerical_pass']}**. {result['main_gp']} main GP +{result['fresh_repeats']} fresh repeats; "
        f"{result['accepted_laps']} accepted laps; {result['exact_repeats']} exact repeats and nine reference input/result bridges.", '',
        'Developed Craft era-II V8_1970/basic aero; 16 discretionary points plus eight installed bonuses; gear .5, '
        'balanced reference driver, seed73013, SM midpoint stop, fuel100kg. Same thermal policy and solver throughout. '
        'Counterfactual coefficients are offline diagnostics only.', '']
    if not result['numerical_pass']: return '\n'.join(lines) + '\n'
    m = result['metrics']
    lines += ['## Full coupled slider response', '',
        '| Circuit | Slider | Drag/downforce area m² | Total s | Delta .5 s | First-five delta .5 s | Max km/h | Top-gear redline time s |',
        '|---|---:|---:|---:|---:|---:|---:|---:|']
    for context in data['plan']['contexts']:
        c = context['circuit']; ref = m[f'{c}/coupled/0.5']
        for slider in data['plan']['sliders']:
            r = m[f'{c}/coupled/{slider}']
            lines.append(f"| {c} | {slider} | {r['drag_area_m2']:.4f}/{r['downforce_area_m2']:.4f} | {r['total_s']:.3f} | {r['total_s']-ref['total_s']:+.3f} | {r['first_five_drive_s']-ref['first_five_drive_s']:+.3f} | {r['maximum_speed_kph']:.2f} | {r['near_redline_top_gear_time_s']:.1f} |")
    lines += ['', '## Controlled coefficient responses and geometry partitions', '',
        'Deltas compare against coupled .5. Geometry columns use identical supplied curvature bins, so their driving-time deltas sum to the total-time delta (service is unchanged). '
        'Drag/downforce controls retain live thermal/wear/fuel response: their seconds are not additive independent effects.', '',
        '| Circuit | Mode | Slider | Total delta s | Low/medium/high curvature delta s | Drag work delta MJ | Braking time delta s | Brake-cap time s |',
        '|---|---|---:|---:|---:|---:|---:|---:|']
    for row in data['rows']:
        c = row['case']; key = c['id']; r = m[key]; ref = m[f"{c['circuit']}/coupled/0.5"]
        delta = '/'.join(f"{a['time_s']-b['time_s']:+.3f}" for a, b in zip(r['bins'], ref['bins']))
        lines.append(f"| {c['circuit']} | {c['aero_mode']} | {c['downforce']} | {r['total_s']-ref['total_s']:+.3f} | {delta} | {(r['drag_work_j']-ref['drag_work_j'])/1e6:+.3f} | {r['braking_time_s']-ref['braking_time_s']:+.3f} | {r['brake_cap_time_s']:.1f} |")
    lines += ['', '## Limits and decision', '',
        'All six repeats and nine old results must match at the full-result/input level, not merely total time. Bin work is checked against accepted native energy ledgers. '
        'Braking-phase duration changes are not fixed spatial sectors; do not attribute their difference as isolated braking seconds. '
        'Fixed gear, driver, car/means and one tire strategy condition the response. No real aero identification or global optimum is claimed.', '',
        'See `INTERPRETATION.md` for the source polar convention, causal findings and proposed next step. No coefficient, game, runtime, catalog or deployment change.']
    return '\n'.join(lines) + '\n'


def main():
    parser = argparse.ArgumentParser(); parser.add_argument('input', type=Path); parser.add_argument('--output-dir', type=Path, required=True)
    args = parser.parse_args(); data = json.load(gzip.open(args.input)); result = analyze(data)
    args.output_dir.mkdir(parents=True, exist_ok=True); write(args.output_dir/'DECISION_V1.json', result)
    (args.output_dir/'RESULTS_V1.md').write_text(markdown(data, result))
    print(json.dumps({k: result[k] for k in ['numerical_pass','main_gp','fresh_repeats','exact_repeats','accepted_laps','sampled_coupled_best']}))
    if not result['numerical_pass']: raise SystemExit(1)


if __name__ == '__main__': main()
