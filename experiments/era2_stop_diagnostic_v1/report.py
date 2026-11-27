#!/usr/bin/env python3
"""Read-only report of the preregistered diagnostic. Never fits coefficients."""
import argparse
import gzip
import json
from pathlib import Path

from run import ROOT, cases, numerical_checks, sha, write


def summarize_stints(row):
    laps = row['data']['laps']
    result = []
    offset = 0
    for stint in row['case']['strategy']['stints']:
        part = laps[offset:offset + stint['laps']]
        offset += stint['laps']
        if not part: continue
        duration = sum(l['time_s'] for l in part)
        result.append({
            'first_lap': part[0]['lap'], 'last_lap': part[-1]['lap'], 'compound': stint['tire_id'],
            'drive_time_s': duration, 'wear_start': part[0]['wear_start'], 'wear_end': part[-1]['wear_end'],
            'wear_grip_end': part[-1]['wear_grip_end'],
            'surface_start_c': part[0]['surface_start_c'], 'surface_end_c': part[-1]['surface_end_c'],
            'surface_mean_c': sum(l['surface_mean_c'] * l['time_s'] for l in part) / duration,
            'surface_max_c': max(l['surface_max_c'] for l in part),
            'carcass_mean_c': sum(l['carcass_mean_c'] * l['time_s'] for l in part) / duration,
            'minimum_applied_temperature_grip': min(l['minimum_temperature_grip'] for l in part),
            'fuel_start_kg': part[0]['fuel_start_kg'], 'fuel_after_stint_kg': part[-1]['fuel_after_lap_kg'],
        })
    return result


def analyze(data):
    if not data['complete']:
        raise ValueError('incomplete campaign: no completed diagnostic report')
    plan = data['plan']
    expected = cases(plan)
    rows = data['rows']
    if [r['case'] for r in rows] != expected:
        raise ValueError('matrix order/content differs from preregistration')
    for row in rows + [v['fresh'] for v in data['validations']]:
        if numerical_checks(row['data']) != row['checks']:
            raise ValueError('retained check result differs from recomputation')
    lookup = {r['case']['id']: r for r in rows}
    old = json.load(gzip.open(ROOT / plan['baseline_archive']))
    if sha(ROOT / plan['baseline_archive']) != plan['baseline_archive_sha256']:
        raise ValueError('baseline archive hash mismatch')
    old_lookup = {r['case']['id']: r for r in old['rows']}
    bridges = []
    for context in plan['contexts']:
        circuit = context['circuit']
        for family in ['MH', 'SM']:
            new = lookup[f'{circuit}/{family}/one/nominal']['data']
            previous = old_lookup[f'{circuit}/exit/balanced/{family}-50']['data']
            bridges.append({'case_id': new['case']['id'], 'physical_input_exact':
                new['receipt']['physical_input_digest'] == previous['receipt']['physical_input_digest'],
                'result_exact': new['outcome'].get('result_digest') == previous['outcome'].get('result_digest')})
    repeats = data['validations']
    expected_repeats = [f"{c['circuit']}/MH/{r}/nominal" for c in plan['contexts'] for r in ['one', 'three']]
    if [v['baseline_id'] for v in repeats] != expected_repeats:
        raise ValueError('fresh repeat coverage differs from preregistration')
    repeat_exact = []
    for validation in repeats:
        baseline = lookup[validation['baseline_id']]
        fresh = validation['fresh']
        repeat_exact.append(not baseline['checks'] and not fresh['checks'] and
            baseline['case'] == fresh['case'] and
            baseline['data']['receipt']['physical_input_digest'] == fresh['data']['receipt']['physical_input_digest'] and
            baseline['data']['outcome'].get('result_digest') == fresh['data']['outcome'].get('result_digest') and validation['exact'])
    numerical_pass = (not any(r['checks'] for r in rows) and len(repeats) == plan['expected_repeats'] and
        all(repeat_exact) and all(b['physical_input_exact'] and b['result_exact'] for b in bridges))
    summary = {'schema': 'pitgun.era2-stop-diagnostic-decision/v1', 'numerical_pass': numerical_pass,
        'strategic_gate': 'open', 'production_policy_changed': False,
        'main_gp': len(rows), 'fresh_repeats': len(repeats), 'exact_repeats': sum(repeat_exact),
        'accepted_laps': sum(len(r['data'].get('laps', [])) for r in rows + [v['fresh'] for v in repeats]),
        'bridges': bridges, 'matched_comparisons': [], 'counterfactuals': [], 'best_sampled': [],
        'failures': [{'id': r['case']['id'], 'checks': r['checks']} for r in rows if r['checks']],
        'maximum_tire_balance_residual_j': max(r['data'].get('thermal_ledger', {}).get('maximum_tire_balance_residual_j', 0.) for r in rows),
        'maximum_elapsed_accounting_residual_s': max(r['data']['outcome'].get('elapsed_accounting_residual_s', 0.) for r in rows),
        'maximum_carry_jump': max(max(r['data'].get('maximum_carry_jumps_speed_engine_surface_carcass_wear', [0.])) for r in rows),
        'stints': {r['case']['id']: summarize_stints(r) for r in rows if not r['checks']}}
    if not numerical_pass:
        return summary
    def total(circuit, family, recipe, diagnostic='nominal'):
        return lookup[f'{circuit}/{family}/{recipe}/{diagnostic}']['data']['outcome']['time_s']
    for context in plan['contexts']:
        circuit = context['circuit']
        for family in ['MH', 'HM', 'SM', 'MS']:
            times = [total(circuit, family, p) for p in ['one', 'two', 'three']]
            summary['matched_comparisons'].append({'circuit': circuit, 'family': family,
                'one_s': times[0], 'two_s': times[1], 'three_s': times[2],
                'two_gain_vs_one_s': times[0] - times[1], 'three_gain_vs_one_s': times[0] - times[2],
                'winner_stops': min(range(3), key=times.__getitem__) + 1})
        for diagnostic in plan['diagnostics']:
            times = [total(circuit, 'MH', p, diagnostic) for p in ['one', 'two', 'three']]
            summary['counterfactuals'].append({'circuit': circuit, 'diagnostic': diagnostic,
                'one_s': times[0], 'two_s': times[1], 'three_s': times[2],
                'two_gain_vs_one_s': times[0] - times[1], 'three_gain_vs_one_s': times[0] - times[2]})
        for stops in [1, 2, 3]:
            sampled = [r for r in rows if r['case']['circuit'] == circuit and r['case']['diagnostic'] == 'nominal'
                and len(r['case']['strategy']['pit_laps']) == stops]
            best = min(sampled, key=lambda r: r['data']['outcome']['time_s'])
            outcome = best['data']['outcome']
            summary['best_sampled'].append({'circuit': circuit, 'stops': stops, 'id': best['case']['id'],
                'time_s': outcome['time_s'], 'drive_s': outcome['drive_time_s'], 'service_s': outcome['service_time_s']})
    return summary


def markdown(data, summary):
    lines = ['# Era-II stop-count diagnostic V1', '',
        f"Numerical/bridge checks: **{'PASS' if summary['numerical_pass'] else 'FAIL — inspect retained failures'}**. "
        f"{summary['main_gp']} main GP +{summary['fresh_repeats']} fresh repeats, {summary['accepted_laps']} accepted laps. "
        f"{summary['exact_repeats']} exact repeats; four prior input/result bridges checked. Strategic gate remains **open**.", '',
        'Scope: developed era-II Craft fixture; 16 discretionary points +8 installed bonuses; balanced reference driver; sliders 0.5; '
        '**100 kg initial fuel**, 70°C initial/replacement tires. Monza and São Paulo only. All nominal coefficients remain frozen. '
        'Complete receipts, per-lap scalars and counterfactual inputs are archived. No AI fitting, live activation or deployment.', '']
    if not summary['numerical_pass']: return '\n'.join(lines) + '\n'
    lines += ['## Matched compound exposure', '',
        'Within each row the exact number of laps on each compound is unchanged across stop counts; sequence and stint age change. '
        'M/H/S mean medium/hard/soft; the first letter receives floor(N/2) laps. Positive gain means faster than one stop. '
        'These centered schedules are samples, not optimized strategies.', '',
        '| Circuit | Family | One stop s | Two stops s | Three stops s | Two-stop gain s | Three-stop gain s | Best stops |',
        '|---|---|---:|---:|---:|---:|---:|---:|']
    for r in summary['matched_comparisons']:
        lines.append(f"| {r['circuit']} | {r['family']} | {r['one_s']:.3f} | {r['two_s']:.3f} | {r['three_s']:.3f} | {r['two_gain_vs_one_s']:.3f} | {r['three_gain_vs_one_s']:.3f} | {r['winner_stops']} |")
    lines += ['', '## Best sampled nominal plans, including timing controls', '',
        '| Circuit | Stops | Plan | Driving s | Service s | Total s |', '|---|---:|---|---:|---:|---:|']
    for r in summary['best_sampled']:
        lines.append(f"| {r['circuit']} | {r['stops']} | {r['id']} | {r['drive_s']:.3f} | {r['service_s']:.3f} | {r['time_s']:.3f} |")
    lines += ['', 'Each stop still costs 22 s, including the diagnostic counterfactuals. Early/late two-stop controls retain exact compound totals; '
        'their individual records and stint boundaries are in `DECISION_V1.json` and the archive.', '',
        '## One-factor counterfactuals: MH family', '',
        'Only the selected multiplicative grip penalty is removed. Heat laws, wear generation, mount preparation and service cost remain. '
        'Trajectories respond to the new grip: differences cannot be added as independent causal seconds.', '',
        '| Circuit | Diagnostic | Two-stop gain vs one s | Three-stop gain vs one s |', '|---|---|---:|---:|']
    for r in summary['counterfactuals']:
        lines.append(f"| {r['circuit']} | {r['diagnostic']} | {r['two_gain_vs_one_s']:.3f} | {r['three_gain_vs_one_s']:.3f} |")
    lines += ['', '## Nominal MH stint trajectories', '',
        'Surface/carcass means are weighted by driving time. Wear/grip are model fractions, not measured tire identification. '
        'Minimum temperature grip is the applied surface-temperature factor; carcass temperatures retain thermal memory.', '',
        '| Circuit | Stops | Laps | Tire | Surface mean/max °C | Carcass mean °C | End wear % | End wear grip | Min temp grip |',
        '|---|---:|---|---|---:|---:|---:|---:|---:|']
    for context in data['plan']['contexts']:
        circuit = context['circuit']
        for stops, recipe in enumerate(['one', 'two', 'three'], 1):
            for r in summary['stints'][f'{circuit}/MH/{recipe}/nominal']:
                lines.append(f"| {circuit} | {stops} | {r['first_lap']}–{r['last_lap']} | {r['compound']} | {r['surface_mean_c']:.1f}/{r['surface_max_c']:.1f} | {r['carcass_mean_c']:.1f} | {100*r['wear_end']:.1f} | {r['wear_grip_end']:.3f} | {r['minimum_applied_temperature_grip']:.3f} |")
    lines += ['', '## Evidence and limits', '',
        f"- Native source `{data['provenance']['source_commit']}`; executable SHA256 `{data['provenance']['identity']['binary_sha256']}`.",
        '- Four nominal MH-half/SM-half runs reproduce the archived physical-input and complete-result digests exactly. '
        'Four repeated runs match too. This checks experiment continuity and deterministic execution, not physical realism.',
        f"- Maximum tire balance residual {summary['maximum_tire_balance_residual_j']:.3g} J; "
        f"elapsed residual {summary['maximum_elapsed_accounting_residual_s']:.3g} s; "
        f"lap-boundary carry jump {summary['maximum_carry_jump']:.3g}.",
        '- No coefficient fit, hard stop limit or final nine-AI calibration. No assertion about untested circuits, budgets or global optima.',
        '- A separate interpretation and next hypothesis are documented in `INTERPRETATION.md`. Numerical pass does not close the strategic gate.']
    return '\n'.join(lines) + '\n'


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('input', type=Path)
    parser.add_argument('--output-dir', type=Path, required=True)
    args = parser.parse_args()
    data = json.load(gzip.open(args.input))
    summary = analyze(data)
    args.output_dir.mkdir(parents=True, exist_ok=True)
    write(args.output_dir / 'DECISION_V1.json', summary)
    (args.output_dir / 'RESULTS_V1.md').write_text(markdown(data, summary))
    print(json.dumps({k: summary[k] for k in ['numerical_pass', 'main_gp', 'fresh_repeats', 'exact_repeats', 'accepted_laps']}))
    if not summary['numerical_pass']: raise SystemExit(1)


if __name__ == '__main__': main()
