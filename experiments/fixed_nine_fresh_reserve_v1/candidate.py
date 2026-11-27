"""Frozen I/II candidate: retained I interpolation, II complete-anchor policy.

Reference preparations come only from completed development archives. No fresh
race result, player tuning or current standings can enter these helpers.
"""
from copy import deepcopy
from fractions import Fraction
import importlib.util
from pathlib import Path
import sys

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(ROOT / 'experiments/fixed_nine_refinement_v2'))
from harness import BASE, encoded, load, sha, write

JOINT = ROOT / 'experiments/joint_era2_preparation_v1'
spec = importlib.util.spec_from_file_location('joint_era2_frozen_selection', JOINT / 'selection.py')
joint = importlib.util.module_from_spec(spec)
spec.loader.exec_module(joint)
PREP_KEYS = ['effective_budget', 'effective_allocation', 'downforce', 'gear', 'strategy', 'mode']


def build():
    paths = {
        '1': {'policy': 'experiments/fixed_nine_thermal_v1/policy-v1.json',
              'decision': 'experiments/era1_attainability_audit_v1/results/decision-v1.json',
              'archive': 'experiments/era1_attainability_audit_v1/results/era1-v1.json.gz',
              'method': 'retained-point-interpolation/v1'},
        '2': {'policy': 'experiments/joint_era2_preparation_v1/results/policy-v1.json',
              'decision': 'experiments/joint_era2_preparation_v1/results/decision-v1.json',
              'archive': 'experiments/joint_era2_preparation_v1/results/era2-v1.json.gz',
              'method': 'nearest-complete-anchor/v1'}}
    contexts = []
    for era, source in paths.items():
        for k in ['policy', 'decision', 'archive']:
            source[k + '_sha256'] = sha(ROOT / source[k])
        decision = load(ROOT / source['decision'])
        archive = load(ROOT / source['archive'])
        if not decision['development_review_pass'] or not archive['complete']:
            raise ValueError('candidate requires complete passing development evidence')
        rows = {r['case']['id']: r for r in archive['rows']}
        for summary in decision['summaries']:
            circuit, budget = summary['circuit'], summary['budget']
            ctx = next((c for c in contexts if c['era'] == int(era) and c['circuit'] == circuit), None)
            if ctx is None:
                ctx = {'era': int(era), 'circuit': circuit, 'references': {'free': {}, 'zero_extra': {}}}
                contexts.append(ctx)
            for family in ctx['references']:
                best = summary['best'][family]
                source_id = best['native_case_id'] if era == '1' else best['case_id']
                row = rows[source_id]
                if row['checks'] or row['checks'] != BASE.checks(row['data']):
                    raise ValueError('unchecked development source')
                ctx['references'][family][str(budget)] = {
                    'preparation': {k: deepcopy(row['case'][k]) for k in PREP_KEYS},
                    'source_case_id': source_id,
                    'physical_input_digest': row['data']['receipt']['physical_input_digest'],
                    'result_digest': row['data']['outcome']['result_digest']}
    return {'schema': 'pitgun.fixed-nine-consolidated-candidate/v1', 'admitted': False,
        'policy': 'experiments/thermal_balance_sensitivity_v1/rolling-heat-065.json',
        'policy_sha256': sha(ROOT / 'experiments/thermal_balance_sensitivity_v1/rolling-heat-065.json'),
        'roster': 'experiments/fixed_opponents_v2/roster-v2.json',
        'roster_sha256': sha(ROOT / 'experiments/fixed_opponents_v2/roster-v2.json'),
        'reference_scaling': 'nearest-preparation-family-preserving-integer-projection/v1',
        'field_sources': paths, 'contexts': contexts}


def reference(candidate, era, circuit, budget, family):
    ctx = next(c for c in candidate['contexts'] if c['era'] == era and c['circuit'] == circuit)
    anchors = ctx['references'][family]
    keys = sorted(int(k) for k in anchors)
    if isinstance(budget, bool) or not isinstance(budget, int) or not keys[0] <= budget <= keys[-1]:
        raise ValueError('reference outside frozen integer budget range')
    anchor = min(keys, key=lambda k: (abs(k - budget), k))
    prep = deepcopy(anchors[str(anchor)]['preparation'])
    floors = [0, 1, 0, 0] if era == 1 and budget == 12 else BASE.floors(era)
    if family == 'zero_extra':
        # Keep this control family literal between anchors: no discretionary A/K.
        _, chassis, engine, _ = prep['effective_allocation']
        remaining = budget - floors[0] - floors[3]
        prep['effective_allocation'] = BASE.project([floors[0],
            Fraction(remaining*chassis,chassis+engine),
            Fraction(remaining*engine,chassis+engine),floors[3]],budget,floors)
    else:
        prep['effective_allocation'] = BASE.scale(prep['effective_allocation'], budget, floors)
    prep['effective_budget'] = budget
    return prep


def field(candidate, era, circuit, budget, actor):
    if era not in [1, 2]:
        raise ValueError('candidate only supports eras I/II')
    if isinstance(budget, bool) or not isinstance(budget, int):
        raise ValueError('field requires an integer effective budget')
    source = candidate['field_sources'][str(era)]
    path = ROOT / source['policy']
    if sha(path) != source['policy_sha256']:
        raise ValueError('frozen field source changed')
    policy = load(path)
    if era == 1:
        prep = BASE.preparation(policy, era, circuit, budget, actor)
    else:
        prep = joint.preparation(policy, circuit, budget, actor, BASE.scale)
    return {k: deepcopy(prep[k]) for k in PREP_KEYS}


if __name__ == '__main__':
    write(HERE / 'candidate-v1.json', build())
