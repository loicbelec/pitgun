#!/usr/bin/env python3
"""Materialize frozen preparations, never simulation results or fresh fitting.

The immutable table eliminates duplicated interpolation in Rust/TypeScript.
Both runtimes select the same exact effective-cap rows before a weekend.
"""
import json
import importlib.util
import math
from pathlib import Path
import shutil
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT/'experiments/fixed_nine_fresh_reserve_v1'))
import candidate


def profile():
    frozen = candidate.load(ROOT/'experiments/fixed_nine_fresh_reserve_v1/candidate-v1.json')
    review = candidate.load(ROOT/'experiments/fixed_nine_fresh_reserve_v1/results/review-v1.json')
    digest = candidate.sha(ROOT/'experiments/fixed_nine_fresh_reserve_v1/candidate-v1.json')
    if not review['native_reserved_review_pass'] or review['candidate_sha256'] != digest:
        raise ValueError('frozen native candidate has not passed reserve')
    p = candidate.load(ROOT/'catalogs/racing/v1.10.2/simulation/qualified-career/profile-v1.json')
    envelope=candidate.load(ROOT/'experiments/validated_runtime_v1/game-budget-envelope-v1.json')
    projection={'schema':'pitgun.local-frozen-field-budget-projection/v1','candidate_sha256':digest,
        'game_envelope_sha256':candidate.sha(ROOT/'experiments/validated_runtime_v1/game-budget-envelope-v1.json'),
        'outside_reserve_anchors':'nearest endpoint preparation, exact integer point projection; no fitting',
        'scope':'reserve-qualified original range; extended game caps are local projection/technical-smoke coverage, not additional balance calibration'}
    p['source_policy_digest'] = 'sha256:'+__import__('hashlib').sha256(candidate.encoded(projection)).hexdigest()
    p['budget_projection']=projection
    p['preparation_basis'] = 'effective-cap-table/v1'
    p['tire_operating'] = candidate.load(ROOT/frozen['policy'])
    p['tires'] = p['tire_operating']['performance']
    p['qualification_status'] = 'native I/II fresh reserve PASS; local runtime qualification'
    p['calibration_status'] = 'frozen I interpolation / II complete anchors; no runtime fitting'
    p['budgets_by_era'] = {era:list(range(e['minimum'],e['maximum']+1))for era,e in envelope['coverage'].items()}
    p['contexts'] = []
    for ctx in frozen['contexts']:
        cases=[]
        for budget in p['budgets_by_era'][str(ctx['era'])]:
            for actor in p['roster']:
                lo,hi=(8,12)if ctx['era']==1 else(12,24)
                anchor=min(hi,max(lo,budget))
                prep = candidate.field(frozen,ctx['era'],ctx['circuit'],anchor,actor)
                if budget!=anchor:
                    prep['effective_allocation']=candidate.BASE.scale(prep['effective_allocation'],budget,candidate.BASE.floors(ctx['era']))
                cases.append({'actor':actor,'allocation':prep['effective_allocation'],
                    'budget':budget,'gear':prep['gear'],'downforce':prep['downforce'],
                    'strategy':prep['strategy']})
        p['contexts'].append({'era':ctx['era'],'circuit':ctx['circuit'],'cases':cases})
    return p


def bridge():
    spec=importlib.util.spec_from_file_location('fresh_reserve_vectors',ROOT/'experiments/fixed_nine_fresh_reserve_v1/run.py')
    reserve=importlib.util.module_from_spec(spec);spec.loader.exec_module(reserve)
    plan=candidate.load(ROOT/'experiments/fixed_nine_fresh_reserve_v1/plan-v1.json')
    fixtures=candidate.load(ROOT/'experiments/opponent_physics_v2/results/career-fixtures-v1.json')['fixtures']
    vectors=[]
    for era in [1,2]:
        archive=candidate.load(ROOT/f'experiments/fixed_nine_fresh_reserve_v1/results/era{era}-v1.json.gz')
        _,evaluations=reserve.design(plan,era)
        rows={r['case']['id']:r for r in archive['rows']}
        for circuit,budget,seed in sorted({(e['circuit'],e['budget'],e['seed'])for e in evaluations}):
            group=[e for e in evaluations if (e['circuit'],e['budget'],e['seed'])==(circuit,budget,seed)]
            for role in ['free','zero_extra']:
                ref=next(e for e in group if e['role']==role)
                if role=='zero_extra' and ref['native_case_id']==next(e['native_case_id']for e in group if e['role']=='free'):continue
                case=ref['logical_case'];fixture=next(f for f in fixtures if f['id']==case['fixture_id'])
                wire=json.loads(json.dumps(fixture['preparedRequests'][0]['request']))
                a=case['effective_allocation'];tuning=dict(zip(['aero_points','chassis_points','engine_points','cooling_points'],a))
                tuning.update(downforce_slider=case['downforce'],gear_ratio_slider=case['gear'])
                wire['seed']=seed;inp=wire['input'];inp['seed']=seed;inp['track_id']=circuit
                inp['laps']=sum(s['laps']for s in case['strategy']['stints'])
                inp['pit_strategy']={'player_pit_laps':case['strategy']['pit_laps']};inp['player_tuning']=tuning
                inp['competitor_vehicle_components']=({'player':{'schema_version':'pitgun.racing-vehicle-components/v1','engine_id':'v8_1970','aero_id':'basic'}}if era==2 else{})
                player=next(c for c in inp['competitors']if c['is_player']);player['budget_cap']=budget
                player['tuning']=tuning;player['stint_strategy']=case['strategy'];player.pop('points',None)
                selected=([e for e in group if e['role']not in ['free','zero_extra']]+[ref])if role=='free'else[ref]
                expected={('player'if e['role']in ['free','zero_extra']else e['role']):{
                    'physical_input_digest':rows[e['native_case_id']]['data']['receipt']['physical_input_digest'],
                    'time_s':rows[e['native_case_id']]['data']['outcome']['time_s'],
                    'lap_times_ms':[math.floor(l['time_s']*1000+.5)+(round(rows[e['native_case_id']]['data']['receipt']['pit_loss_s']*1000)if l['lap']in rows[e['native_case_id']]['case']['strategy']['pit_laps']else 0)
                        for l in rows[e['native_case_id']]['data']['laps']]}
                    for e in selected}
                vectors.append({'id':f'{era}/{circuit}/{budget}/{role}','request':wire,'expected':expected,
                    'full_race_qa':role=='free'and((era==1 and circuit=='MONZA'and budget==12)or(era==2 and circuit=='MEXICO'and budget==24))})
    return {'schema':'pitgun.validated-runtime-input-bridge/v1','vectors':vectors}


if __name__ == '__main__':
    target=ROOT/'catalogs/racing/v1.10.3'
    if not target.exists(): shutil.copytree(ROOT/'catalogs/racing/v1.10.2',target)
    candidate.write(target/'simulation/qualified-career/profile-v1.json',profile())
    ratios=candidate.load(ROOT/'experiments/validated_runtime_v1/gear-ratios-native-v1.json')
    for engine,values in ratios['ratios'].items():
        engine_path=target/f'simulation/engines/{engine}.json'
        resource=candidate.load(engine_path);resource['gearbox']['ratios']=values
        candidate.write(engine_path,resource)
    (target/'README.md').write_text('# Local validated I/II candidate\n\nCatalog1.10.3/model0.16.3 binds rolling-heat-065 and the frozen I/II field.\nEffective-cap preparation table is generated from the native reserve-qualified\ncandidate; no results or live player tuning enter opponent selection.\nLocal unranked admission only; I–V coverage is required before full staging.\nOlder catalog/model bytes and the public LATEST pointer are preserved.\n')
    candidate.write(ROOT/'experiments/validated_runtime_v1/bridge-inputs-v1.json',bridge())
