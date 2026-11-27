#!/usr/bin/env python3
"""Bounded era-I player preparation audit against the immutable V1 field."""
import argparse
from copy import deepcopy
from fractions import Fraction
import fcntl
import hashlib
from pathlib import Path
import sys

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(ROOT/'experiments/fixed_nine_refinement_v2'))
from harness import BASE, encoded, execute_matrix, load, sha, write


def semantic(case):
    # Rust Case.id is only echoed; it never enters RunRaceRequest or solver input.
    return {k: v for k, v in case.items() if k != 'id'}


def offsets(budget): return [0,1,0,0] if budget == 12 else [0,0,0,0]


def permitted(case, family):
    a, b = case['effective_allocation'], case['effective_budget']; f=offsets(b)
    if sum(a)!=b or any(p<q or p>20 for p,q in zip(a,f)):return False
    if case['downforce']!=.5 or not 0<=case['gear']<=1:return False
    stints=case['strategy']['stints'];pits=case['strategy']['pit_laps']
    if len(stints)!=2 or len(pits)!=1 or stints[0]['laps']!=pits[0] or min(s['laps']for s in stints)<1:return False
    if family=='zero_extra':return a[0]==f[0] and a[3]==f[3]
    if family=='heavy_80':
        return Fraction(a[1]-f[1]+a[2]-f[2], b-sum(f))>=Fraction(4,5)
    return family=='free'


def base_case(plan, ctx, budget, family):
    c = deepcopy(ctx['entry'][family if family=='zero_extra' else 'free'] if budget==8
                 else ctx['exit'][family if family=='zero_extra' else 'free'])
    c['effective_budget']=budget;c['effective_allocation']=BASE.scale(c['effective_allocation'],budget,offsets(budget))
    c['kind']='field' if budget==10 else 'career'
    c['fixture_id']='craft_engineering:era1-entry' if budget!=12 else 'craft_engineering:era1-exit'
    c['seed']=plan['seed'];c['id']=f'era1-audit/{ctx["circuit"]}/{budget}/{family}/baseline'
    if family=='heavy_80':
        f=offsets(budget);discretionary=budget-sum(f);cooling=f[3]+discretionary//5
        remaining=budget-cooling;chassis,engine=c['effective_allocation'][1:3]
        c['effective_allocation']=BASE.project([0,Fraction(remaining*chassis,chassis+engine),
            Fraction(remaining*engine,chassis+engine),cooling],budget,[0,f[1],0,cooling])
    if not permitted(c,family):raise ValueError('illegal authored baseline')
    return c


def transform(base, op):
    c=deepcopy(base)
    if op=='gear015':c['gear']=.15
    elif op=='gear005':c['gear']=.05
    elif op in ['pit_early3','pit_late3']:
        delta=-3 if op=='pit_early3' else 3
        c['strategy']['pit_laps'][0]+=delta
        c['strategy']['stints'][0]['laps']+=delta;c['strategy']['stints'][1]['laps']-=delta
    else:
        donor,receiver={'cool_to_engine':(3,2),'cool_to_chassis':(3,1),'engine_to_aero':(2,0),
            'chassis_to_engine':(1,2),'engine_to_chassis':(2,1)}[op]
        c['effective_allocation'][donor]-=1;c['effective_allocation'][receiver]+=1;c['gear']=.15
    return c


def design(plan):
    matrix=[];evaluations=[];representatives={}
    for ctx in plan['contexts']:
        for budget in plan['budgets']:
            for family in plan['families']:
                base=base_case(plan,ctx,budget,family);proposals=[('baseline',base)];seen={encoded(semantic(base))}
                for slot in plan['slots']:
                    for op in slot:
                        c=transform(base,op);c['id']=base['id'].rsplit('/',1)[0]+'/'+op
                        key=encoded(semantic(c))
                        if permitted(c,family) and key not in seen:
                            proposals.append((op,c));seen.add(key);break
                    else:raise ValueError('no distinct legal candidate for slot')
                if len(proposals)!=6:raise ValueError('unfair family search allowance')
                for name,c in proposals:
                    key=encoded(semantic(c))
                    if key not in representatives:
                        representatives[key]=c['id'];matrix.append({'case':c,'fresh':False})
                    evaluations.append({'circuit':ctx['circuit'],'budget':budget,'family':family,
                        'variant':name,'logical_case':c,'native_case_id':representatives[key]})
    if len(evaluations)!=162:raise ValueError('changed logical matrix')
    repeats=[]
    for ctx in plan['contexts']:
        for family in ['zero_extra','free']:
            e=next(e for e in evaluations if e['circuit']==ctx['circuit'] and e['budget']==12 and e['family']==family and e['variant']=='baseline')
            c=next(r['case']for r in matrix if r['case']['id']==e['native_case_id'])
            repeats.append({'case':deepcopy(c),'fresh':True})
    matrix+=repeats
    if len(matrix)>168:raise ValueError('campaign ceiling exceeded')
    if len(matrix)!=plan['expected_native_gp'] or len(matrix)-6!=plan['expected_unique_primary_gp']:
        raise ValueError('deduplicated matrix differs from frozen counts')
    return matrix,evaluations


def field_sources(plan):
    for name in ['field_archive','field_policy']:
        if sha(ROOT/plan[name])!=plan['retained_files'][plan[name]]:
            raise ValueError('frozen field artifact changed')
    previous=load(ROOT/plan['field_archive']);policy=load(ROOT/plan['field_policy'])
    if previous['provenance']['identity']['binary_sha256']!=plan['native_binary_sha256']:
        raise ValueError('cannot reuse field under a different executable')
    index={}
    for row in previous['rows']:
        if row['case']['actor']!='balanced_reference' and not row['checks']:
            index.setdefault(encoded(semantic(row['case'])),row)
    result=[]
    for ctx in plan['contexts']:
        for budget in plan['budgets']:
            for actor in policy['contexts'][0]['actors']:
                prep=BASE.preparation(policy,1,ctx['circuit'],budget,actor)
                c=BASE.case({'era':1,'circuit':ctx['circuit']},
                    {'points':prep['effective_allocation'],'gear':prep['gear'],'downforce':prep['downforce'],'strategy':prep['strategy']},
                    actor,budget,seed=plan['seed'],label='field-reuse');c.update(prep)
                old=index.get(encoded(semantic(c)))
                if (old is None or BASE.checks(old['data']) or old['binary_sha256']!=plan['native_binary_sha256']):
                    raise ValueError('no exact checked field source')
                meta={'actor':actor,'circuit':ctx['circuit'],'budget':budget,'source_case_id':old['case']['id'],
                    'physical_input_digest':old['data']['receipt']['physical_input_digest'],
                    'result_digest':old['data']['outcome']['result_digest'],'time_s':old['data']['outcome']['time_s'],'reused':True}
                result.append((c,meta))
    if len(result)!=81:raise ValueError('incomplete frozen field')
    return result


def preflight(worker, plan, matrix, cache):
    evidence=[]
    for c,meta in field_sources(plan):
        resolved=worker.execute(c,resolve=True)
        receipt=resolved['data'].get('receipt',{})
        if not resolved['data'].get('resolved_only') or receipt.get('physical_input_digest')!=meta['physical_input_digest']:
            raise ValueError('field physical input does not bridge')
        evidence.append(meta)
    matrix_again,evals=design(plan)
    if matrix_again!=matrix:raise ValueError('matrix regeneration mismatch')
    # An alias differs only in Case.id, which is not consumed by the frozen solver.
    lookup={r['case']['id']:r['case']for r in matrix if not r['fresh']}
    if any(semantic(e['logical_case'])!=semantic(lookup[e['native_case_id']])for e in evals):
        raise ValueError('invalid logical alias')
    metadata={'field_archive':plan['field_archive'],'field_archive_sha256':sha(ROOT/plan['field_archive']),
        'field_reuse':evidence,'logical_evaluations':len(evals),'unique_primary_gp':len(matrix)-6,
        'id_only_aliases':len(evals)-(len(matrix)-6),'alias_basis':'exact typed case equality except non-physical Case.id'}
    write(cache/'field-preflight.json',metadata)
    return metadata


def analyze(data):
    plan=data['plan'];matrix,evals=design(plan)
    if not data['complete'] or len(data['rows'])!=len(matrix):raise ValueError('partial audit cannot be decided')
    if data['source_matrix_sha256']!=hashlib.sha256(encoded(matrix)).hexdigest():raise ValueError('changed matrix')
    if plan!=load(HERE/'plan-v1.json'):raise ValueError('unexpected frozen protocol')
    if data['preflight']['field_reuse']!=[meta for _,meta in field_sources(plan)]:
        raise ValueError('reused field evidence differs from frozen source')
    rows=data['rows'];n=len(matrix)-6
    if [r['case']for r in rows]!=[r['case']for r in matrix]:raise ValueError('native case echo mismatch')
    if any(r['checks']!=BASE.checks(r['data'])for r in rows):raise ValueError('checks do not regenerate')
    lookup={r['case']['id']:r for r in rows[:n]};repeats=[]
    for fresh in rows[n:]:
        before=lookup[fresh['case']['id']]
        exact=not before['checks'] and not fresh['checks'] and all(before['data'][s][k]==fresh['data'][s].get(k)
            for s,k in [('receipt','physical_input_digest'),('outcome','result_digest')])
        repeats.append({'case_id':fresh['case']['id'],'exact':exact})
    numerical=not any(r['checks']for r in rows) and all(r['exact']for r in repeats)
    summaries=[]
    if numerical:
        for ctx in plan['contexts']:
            for budget in plan['budgets']:
                field=[e for e in data['preflight']['field_reuse']if e['circuit']==ctx['circuit'] and e['budget']==budget]
                if len(field)!=9:raise ValueError('missing frozen field')
                best={}
                for family in plan['families']:
                    options=[(e,lookup[e['native_case_id']])for e in evals if e['circuit']==ctx['circuit']and e['budget']==budget and e['family']==family]
                    e,r=min(options,key=lambda pair:(pair[1]['data']['outcome']['time_s'],pair[0]['logical_case']['id']))
                    total=r['data']['outcome']['time_s']
                    best[family]={'time_s':total,'rank':1+sum(f['time_s']<total for f in field),
                        'gap_to_best_ai_s':total-min(f['time_s']for f in field),'logical_case':e['logical_case'],'native_case_id':e['native_case_id']}
                summaries.append({'circuit':ctx['circuit'],'budget':budget,'kind':'controlled intermediate'if budget==10 else'actual career fixture', 'best':best})
    exits=[s for s in summaries if s['budget']==12]
    wins=sum(s['best']['free']['rank']==1 for s in exits)
    ranks=sorted(s['best']['free']['rank']for s in exits)
    zero_wins=sum(s['best']['zero_extra']['rank']==1 for s in summaries)
    passed=numerical and wins>=1 and ranks[len(ranks)//2]<=3 and zero_wins<len(summaries)
    return {'schema':'pitgun.era1-attainability-audit-decision/v1','complete':True,'numerical_pass':numerical,
        'development_review_pass':passed,'fresh_native_gp':len(rows),'logical_evaluations':len(evals),
        'unique_primary_gp':n,'fresh_repeats':repeats,'reused_field_gp':81,'summaries':summaries,
        'exit_free_wins':wins,'zero_extra_wins':zero_wins,
        'accepted_fresh_laps':sum(len(r['data'].get('laps',[]))for r in rows),'release_admitted':False,
        'scope':'opened-seed development audit; broad heavy allocations may legitimately win; no global optimum or new holdout claim'}


def main():
    parser=argparse.ArgumentParser();parser.add_argument('--binary',type=Path,required=True)
    parser.add_argument('--cache',type=Path,required=True);parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--max-new-gp',type=int,default=20);parser.add_argument('--max-wall-seconds',type=float,default=300.)
    args=parser.parse_args();args.cache.parent.mkdir(parents=True,exist_ok=True)
    with (args.cache.parent/'fixed-nine-worker.lock').open('a')as lock:
        fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
        path=HERE/'plan-v1.json';plan=load(path);matrix,_=design(plan)
        data=execute_matrix(path,args.binary,args.cache,args.output,matrix,preflight=preflight,
            max_new_gp=args.max_new_gp,max_wall_seconds=args.max_wall_seconds)
        if not data['complete']:print(data,flush=True);return
        result=analyze(data);result['results_sha256']=sha(args.output)
        write(args.output.parent/'decision-v1.json',result)
        print({k:result[k]for k in ['numerical_pass','development_review_pass','exit_free_wins']},flush=True)


if __name__=='__main__':main()
