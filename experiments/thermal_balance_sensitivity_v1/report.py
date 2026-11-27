#!/usr/bin/env python3
"""Preregistered policy selection; held-out results never change the selection."""
import argparse
import gzip
import json
from pathlib import Path

from run import ROOT, cases, numerical_checks, sha, write


def metrics(row, warmup=3):
    if row['checks']: return None
    laps = row['data']['laps']
    post = []
    offset = 0
    for stint in row['case']['strategy']['stints']:
        post += laps[offset + warmup:offset + stint['laps']]
        offset += stint['laps']
    duration = sum(l['time_s'] for l in post)
    if not duration: raise ValueError('no post-warm-up exposure')
    def mean(field): return sum(l[field] * l['time_s'] for l in post) / duration
    return {'hot_fraction': mean('hot_fraction'), 'cold_fraction': mean('cold_fraction'),
        'window_fraction': mean('peak_window_fraction'), 'temperature_grip_mean': mean('temperature_grip_mean'),
        'wear_grip_mean': mean('wear_grip_mean'), 'surface_mean_c': mean('surface_mean_c'),
        'carcass_mean_c': mean('carcass_mean_c'), 'surface_max_c': max(l['surface_max_c'] for l in laps),
        'carcass_max_c': max(l['carcass_max_c'] for l in laps), 'maximum_wear': max(l['wear_end'] for l in laps),
        'race_time_s': row['data']['outcome']['time_s'], 'drive_s': row['data']['outcome']['drive_time_s'],
        'service_s': row['data']['outcome']['service_time_s'], 'post_warmup_duration_s': duration}


def checked(data, phase, selected=None):
    if not data['complete'] or data['phase'] != phase:
        raise ValueError('incomplete/wrong experiment phase')
    if [r['case'] for r in data['rows']] != cases(data['plan'], phase, selected):
        raise ValueError('matrix differs from preregistration')
    for r in data['rows'] + [v['fresh'] for v in data['validations']]:
        if numerical_checks(r['data']) != r['checks']:
            raise ValueError('retained numerical checks differ')
    return {r['case']['id']: r for r in data['rows']}


def compare_policy(plan, lookup, name, phase):
    reasons = []
    hot_ratios = []
    measurements = []
    criteria = plan['criteria']
    for context in plan[phase + '_contexts']:
        for stops in context['stops']:
            suffix = f"{context['fixture_id']}/{context['circuit']}/{stops}"
            reference = lookup['reference/' + suffix]
            trial = lookup[name + '/' + suffix]
            if reference['checks'] or trial['checks']:
                reasons.append(suffix + ':numerical_failure'); continue
            r = metrics(reference, criteria['warmup_laps_per_stint'])
            c = metrics(trial, criteria['warmup_laps_per_stint'])
            measurements.append({'context': suffix, 'reference': r, 'candidate': c})
            if c['surface_max_c'] > criteria['maximum_surface_c'] or c['carcass_max_c'] > criteria['maximum_carcass_c']:
                reasons.append(suffix + ':thermal_ceiling')
            if context['fixture_id'] == 'craft_engineering:era2-exit' and stops == 1:
                if r['hot_fraction'] <= 0:
                    reasons.append(suffix + ':reference_has_no_hot_exposure')
                else:
                    ratio = c['hot_fraction'] / r['hot_fraction']
                    hot_ratios.append(ratio)
                    if ratio > 1. - criteria['minimum_relative_hot_exposure_reduction_each_exit_context']:
                        reasons.append(suffix + ':insufficient_hot_reduction')
            if 'entry' in context['fixture_id'] and r['temperature_grip_mean'] - c['temperature_grip_mean'] > criteria['maximum_entry_mean_temperature_grip_loss']:
                reasons.append(suffix + ':entry_grip_loss')
    return {'eligible': not reasons, 'rejection_reasons': reasons,
        'score': sum(hot_ratios) if hot_ratios else None, 'measurements': measurements}


def development(data, archive_sha):
    lookup = checked(data, 'development')
    plan = data['plan']
    old = json.load(gzip.open(ROOT / plan['baseline_archive']))
    if sha(ROOT / plan['baseline_archive']) != plan['baseline_archive_sha256']:
        raise ValueError('reference archive changed')
    previous = {r['case']['id']: r for r in old['rows']}
    bridges = []
    for circuit in ['MONZA', 'SAOPAULO']:
        new = lookup[f'reference/craft_engineering:era2-exit/{circuit}/1']['data']
        before = previous[f'{circuit}/MH/one/nominal']['data']
        bridges.append({'circuit': circuit, 'physical_input_exact':
            new['receipt']['physical_input_digest'] == before['receipt']['physical_input_digest'],
            'result_exact': new['outcome'].get('result_digest') == before['outcome'].get('result_digest')})
    comparisons = {n: compare_policy(plan, lookup, n, 'development') for n in plan['policies'] if n != 'reference'}
    reference_ok = not any(r['checks'] for r in data['rows'] if r['case']['policy_id'] == 'reference')
    bridge_ok = all(b['physical_input_exact'] and b['result_exact'] for b in bridges)
    eligible = sorted((v['score'], n) for n, v in comparisons.items() if v['eligible'])
    selected = eligible[0][1] if eligible and reference_ok and bridge_ok else None
    return {'schema': 'pitgun.thermal-balance-selection/v1', 'development_archive_sha256': archive_sha,
        'development_pass': bool(selected), 'reference_numerical_pass': reference_ok, 'bridges': bridges,
        'selected_policy': selected, 'policy_sha256': plan['policies'][selected]['sha256'] if selected else None,
        'comparisons': comparisons, 'release_admitted': False}


def validation(dev, result, selection):
    name = selection['selected_policy']
    if result['selection'] != selection:
        raise ValueError('validation did not use the frozen development selection')
    lookup = checked(result, 'validation', name)
    if result['plan'] != dev['plan']: raise ValueError('validation plan changed')
    comparison = compare_policy(result['plan'], lookup, name, 'validation')
    previous = {r['case']['id']: r for r in dev['rows']}
    previous.update(lookup)
    expected_ids = [r['case']['id'] for r in dev['rows'] if r['case']['policy_id'] in ['reference', name]
        and r['case']['fixture_id'] == 'craft_engineering:era2-exit' and len(r['case']['strategy']['pit_laps']) == 1]
    expected_ids += [r['case']['id'] for r in result['rows'] if r['case']['fixture_id'] == 'craft_engineering:era2-exit' and len(r['case']['strategy']['pit_laps']) == 1]
    if [v['baseline_id'] for v in result['validations']] != expected_ids:
        raise ValueError('fresh repeat coverage changed')
    exact = []
    for v in result['validations']:
        original, fresh = previous[v['baseline_id']], v['fresh']
        exact.append(not original['checks'] and not fresh['checks'] and original['case'] == fresh['case'] and
            original['data']['receipt']['physical_input_digest'] == fresh['data']['receipt']['physical_input_digest'] and
            original['data']['outcome'].get('result_digest') == fresh['data']['outcome'].get('result_digest') and v['exact'])
    repeat_pass = len(exact) == result['plan']['expected_fresh_repeats'] and all(exact)
    return {'candidate_screening_pass': comparison['eligible'] and repeat_pass, 'comparison': comparison,
        'fresh_repeats': len(exact), 'exact_repeats': sum(exact), 'release_admitted': False}


def report(data, selection, final=None, val=None):
    lines = ['# Thermal balance sensitivity V1', '',
        f"Development: {len(data['rows'])} full GP. Selected policy: **{selection['selected_policy'] or 'none — reject this bounded family'}**.", '',
        'All cases start with 100 kg fuel and tires at 70°C. Nominal operating windows, grip/wear laws, '
        'service costs, engine and sampling remain unchanged. Exposure/grip means exclude the first three laps of each stint; '
        'all laps remain in the archived race totals. These are reduced-model screening hypotheses, not measured tire coefficients.', '',
        '| Candidate | Eligible | Relative hot-exposure score (lower better) | Rejections |', '|---|---|---:|---|']
    for n, c in selection['comparisons'].items():
        score = f"{c['score']:.4f}" if c['score'] is not None else 'n/a'
        lines.append(f"| {n} | {c['eligible']} | {score} | {'; '.join(c['rejection_reasons']) or 'none'} |")
    lines += ['', '## Development thermal measurements', '',
        '| Candidate | Context | Hot exposure ref → trial % | Temperature grip ref → trial | Surface mean ref → trial °C |',
        '|---|---|---:|---:|---:|']
    for n, c in selection['comparisons'].items():
        for m in c['measurements']:
            if m['context'].endswith('/1'):
                r, t = m['reference'], m['candidate']
                lines.append(f"| {n} | {m['context']} | {100*r['hot_fraction']:.1f} → {100*t['hot_fraction']:.1f} | {r['temperature_grip_mean']:.3f} → {t['temperature_grip_mean']:.3f} | {r['surface_mean_c']:.1f} → {t['surface_mean_c']:.1f} |")
    lines += ['', '## Strategy comparisons', '', 'Complete elapsed times include every 22 s service. '
        'These centered recipes have equal compound exposure within each context; they are not global optima.', '',
        '| Phase | Policy | Circuit | Stops | Driving s | Service s | Total s |', '|---|---|---|---:|---:|---:|---:|']
    for phase, campaign in [('development', data), ('reserved', val)]:
        if campaign is None: continue
        for row in campaign['rows']:
            c = row['case']
            if c['fixture_id'] != 'craft_engineering:era2-exit' or row['checks']: continue
            o = row['data']['outcome']
            lines.append(f"| {phase} | {c['policy_id']} | {c['circuit']} | {len(c['strategy']['pit_laps'])} | {o['drive_time_s']:.3f} | {o['service_time_s']:.3f} | {o['time_s']:.3f} |")
    if final:
        lines += ['', '## Frozen reserved validation', '',
            f"Screening PASS: **{final['candidate_screening_pass']}**; exact repeats {final['exact_repeats']}/{final['fresh_repeats']}. "
            f"Rejection reasons: {final['comparison']['rejection_reasons'] or 'none'}.", '',
            'The candidate is not admitted to a runtime or deployed game. Broader era-I/II qualification, '
            'AI fitting, WASM/recovery and release admission remain separate gates.']
    lines += ['', f"Native development source `{data['provenance']['source_commit']}`; "
        f"binary `{data['provenance']['identity']['binary_sha256']}`. "
        'Reference bridges and all candidate acceptance/rejection reasons are in `selection-v1.json`; '
        'raw receipts, scalar trajectories and conservation checks are retained in the phase archives.']
    return '\n'.join(lines) + '\n'


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('development', type=Path)
    parser.add_argument('--validation', type=Path)
    parser.add_argument('--output-dir', type=Path, required=True)
    args = parser.parse_args()
    data = json.load(gzip.open(args.development))
    selection = development(data, sha(args.development))
    final = val = None
    if args.validation:
        val = json.load(gzip.open(args.validation)); final = validation(data, val, selection)
    args.output_dir.mkdir(parents=True, exist_ok=True)
    write(args.output_dir / 'selection-v1.json', selection)
    if final: write(args.output_dir / 'validation-v1.json', final)
    (args.output_dir / 'RESULTS_V1.md').write_text(report(data, selection, final, val))
    print(json.dumps({'selected': selection['selected_policy'], 'development_pass': selection['development_pass'],
        'reserved_pass': final['candidate_screening_pass'] if final else None}))


if __name__ == '__main__': main()
