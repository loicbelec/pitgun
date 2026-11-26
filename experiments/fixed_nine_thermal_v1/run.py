#!/usr/bin/env python3
"""Bounded fixed-nine calibration, frozen selection, then unseen budgets/seeds."""
import argparse
import copy
from datetime import datetime,timezone
import fcntl
from fractions import Fraction
import gzip
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import time

from policy import allocate,preparation,project,scale
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[1]

def encoded(v):return json.dumps(v,sort_keys=True,separators=(',',':'),allow_nan=False).encode()
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def write(p,v):
    p=Path(p);p.parent.mkdir(parents=True,exist_ok=True)
    raw=json.dumps(v,indent=2,allow_nan=False).encode()+b'\n'
    if p.suffix=='.gz':raw=gzip.compress(raw,mtime=0)
    tmp=p.with_name(p.name+'.tmp');tmp.write_bytes(raw);tmp.replace(p)
def load(p):
    with (gzip.open(p) if str(p).endswith('.gz') else open(p)) as stream:
        return json.load(stream)
def fixture(era,stage):return f'craft_engineering:era{era}-{stage}'
def floors(era):return [0,0,0,0] if era==1 else [2,1,4,1]
def order(r):return (r['data']['outcome'].get('time_s',float('inf')),r['case']['id'])

def case(ctx,proposal,actor,budget,seed=73013,kind='field',stage=None,label=''):
    era=ctx['era'];f=floors(era)
    if kind=='career' and (era==1 or stage=='entry'):f=[0,1,0,0] if stage=='exit' or era==2 else [0,0,0,0]
    allocation=scale(proposal['points'],budget,f)
    if label in ['routine','entry-routine']:
        allocation=project([Fraction(budget-sum(f),4)+x for x in f],budget,f)
    return {'id':f'{era}/{ctx["circuit"]}/{actor}/{budget}/{label}', 'policy_id':'rolling-heat-065',
        'circuit':ctx['circuit'],'fixture_id':fixture(era,stage or ('entry' if era==1 else 'exit')),
        'actor':actor,'kind':kind,'effective_budget':budget,'effective_allocation':allocation,
        'gear':proposal['gear'],'downforce':.5 if kind=='career' and stage=='entry' else proposal['downforce'],
        'mode':'balanced','strategy':copy.deepcopy(proposal['strategy']),'seed':seed}

def checks(d):
    if not d['outcome']['ok']:return ['physical_or_resolution_failure']
    e=[];r=d['receipt'];c=d['case'];o=d['outcome'];ls=d['laps'];ledger=d['thermal_ledger']
    if len(ls)!=r['laps']:e.append('incomplete_finish')
    if max(d['maximum_carry_jumps_speed_engine_surface_carcass_wear'])>1e-8:e.append('boundary_jump')
    if not d['pit_resets_exact']:e.append('pit_preparation')
    if abs(o['service_time_s']-len(r['stops'])*r['pit_loss_s'])>1e-8:e.append('service_cost')
    if o['elapsed_accounting_residual_s']>1e-7 or abs(o['time_s']-sum(l['time_s'] for l in ls)-o['service_time_s'])>1e-7:e.append('elapsed_accounting')
    if ledger['maximum_tire_balance_residual_j']>1e-5:e.append('tire_balance')
    if abs(ledger['tire_generated_heat_j']+ledger['tire_direct_environment_heat_j']-ledger['tire_rolling_dissipation_j']-ledger['tire_deformation_dissipation_j'])>1e-5:e.append('dissipation_accounting')
    if r['budget']!=c['effective_budget'] or r['points']!=c['effective_allocation'] or sum(r['points'])!=r['budget']:e.append('budget_binding')
    if any(p<f or p>20 for p,f in zip(r['points'],r['bonus_offsets'])):e.append('floor_binding')
    if r['initial_state']['fuel_mass']!=100.:e.append('fuel_scope')
    if r['policy_id']!=c['policy_id']:e.append('policy_binding')
    if any(abs(l['hot_fraction']+l['cold_fraction']+l['peak_window_fraction']-1)>1e-8 for l in ls):e.append('window_accounting')
    if c['actor']!='balanced_reference' and r['driver']['lapse_profile']['noise_stream_id']!='fixed-field:'+c['actor']:e.append('identity_stream')
    if c['actor']!='balanced_reference':
        expected=load(ROOT/'experiments/fixed_opponents_v2/roster-v2.json')['drivers'][c['actor']]['resource']['traits']
        if r['driver']['controls']['driver_traits']!=expected:e.append('frozen_driver_traits')
    return e

class Campaign:
    def __init__(self,binary,cache,plan):
        self.binary=binary.resolve();self.cache=cache.resolve();self.cache.mkdir(parents=True,exist_ok=True);self.plan=plan
        paths=list((ROOT/'crates').rglob('*.rs'))+list((ROOT/'crates').rglob('Cargo.toml'))
        paths += [ROOT/'Cargo.toml',ROOT/'Cargo.lock']+list(HERE.glob('*.py'))+[HERE/'ACCEPTANCE.md',HERE/'plan-v1.json']
        paths += [p for p in (ROOT/'catalogs/racing/v1.10.1').rglob('*') if p.is_file()]
        paths += [ROOT/p for p in plan['retained_files']]
        self.identity={'binary_sha256':sha(self.binary),'sources':{str(p.relative_to(ROOT)):sha(p) for p in sorted(set(paths))}}
        for p,h in plan['retained_files'].items():assert sha(ROOT/p)==h,'retained evidence changed'
        pin=self.cache/'pin-v1.json'
        if pin.exists():
            self.pin=load(pin);assert self.pin['identity']==self.identity,'changed sources: fresh namespace required'
        else:
            assert not subprocess.check_output(['git','status','--porcelain'],cwd=ROOT,text=True),'commit protocol before compute'
            self.pin={'identity':self.identity,'source_commit':subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),'utc':datetime.now(timezone.utc).isoformat()};write(pin,self.pin)
    def verify(self):
        assert sha(self.binary)==self.identity['binary_sha256']
        assert all(sha(ROOT/p)==h for p,h in self.identity['sources'].items()),'source changed'
    def execute(self,c,fresh=False,resolve=False):
        self.verify();key=hashlib.sha256(encoded(c)).hexdigest()+('.fresh' if fresh else '')+('.resolve' if resolve else '')
        path=self.cache/(key+'.json')
        if path.exists():
            row=load(path);assert row['case']==c and row['binary_sha256']==self.identity['binary_sha256']
            assert row['data']['case']==c
            if not resolve:assert row['checks']==checks(row['data'])
            return row
        request=self.cache/'current-case.json';write(request,c);start=time.monotonic()
        cmd=[str(self.binary),str(ROOT/self.plan['policy']),str(request)]+(['--resolve-only'] if resolve else [])
        run=subprocess.run(cmd,cwd=ROOT,capture_output=True,text=True);self.verify()
        d=json.loads(run.stdout) if run.returncode==0 else {'case':c,'outcome':{'ok':False,'error':run.stderr}}
        d['case']['strategy'].setdefault('pit_laps',[]);assert d['case']==c,'native echo mismatch'
        row={'case':c,'data':d,'wall_s':time.monotonic()-start,'checks':[] if resolve and d.get('resolved_only') else checks(d),
            'binary_sha256':self.identity['binary_sha256'],'returncode':run.returncode,'stderr':run.stderr}
        write(path,row);return row

def style_key(c,style):
    a=c['effective_allocation'];p=c['strategy']['pit_laps'];n=sum(s['laps'] for s in c['strategy']['stints']);first=p[0]/n
    if style=='tire_manager':return (len(p),abs(first-.5),-a[3])
    if style=='power':return (-a[2],abs(first-.28),-c['gear'])
    if style=='mechanical':return (-a[1],abs(first-.4),len(p))
    if style=='smooth':return (abs(first-1/3),len(p),-a[1])
    return (abs(first-1/3),abs(a[1]-a[3]),len(p))

def choose(groups,actor,roster):
    # Relative times across available endpoint budgets. No player result is used.
    scores=[]
    for index in range(2):
        rows=[g[index] for g in groups]
        if any(r['checks'] for r in rows):continue
        scores.append((sum(order(r)[0]/min(order(x)[0] for x in g) for r,g in zip(rows,groups))/len(groups),index))
    if not scores:raise RuntimeError('no valid actor proposal')
    best=min(scores)[0];eligible=[(v,i) for v,i in scores if v<=best*1.003]
    if actor in roster['front_runners']:return min(scores)[1]
    return min(eligible,key=lambda vi:(style_key(groups[-1][vi[1]]['case'],roster['drivers'][actor]['style']),vi))[1]

def neighbors(c,era,f):
    result=[copy.deepcopy(c)]
    transforms=[('gear',-.1),('gear',.1)] + ([('downforce',-.1),('downforce',.1)] if era==2 else [('transfer',(1,2)),('transfer',(2,1))])
    alternatives=[('transfer',(i,j)) for i in range(4) for j in range(4) if i!=j]+[('gear',-.2),('gear',.2)]
    seen={encoded({k:v for k,v in c.items() if k!='id'})}
    for field,delta in transforms+alternatives:
        n=copy.deepcopy(c)
        if field=='transfer':
            i,j=delta;n['effective_allocation'][i]-=1;n['effective_allocation'][j]+=1
            if any(v<floor or v>20 for v,floor in zip(n['effective_allocation'],f)):continue
        else:
            n[field]=round(n[field]+delta,8)
            if not 0<=n[field]<=1:continue
        key=encoded({k:v for k,v in n.items() if k!='id'})
        if key in seen:continue
        seen.add(key);n['id']=c['id']+f'/neighbor{len(result)}';result.append(n)
        if len(result)==5:return result
    raise RuntimeError('cannot construct equal bounded neighborhoods')

def ranked(field,player):return 1+sum(order(r)[0]<order(player)[0] for r in field)

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--binary',type=Path,required=True);parser.add_argument('--cache',type=Path,required=True);parser.add_argument('--output',type=Path,required=True);parser.add_argument('--phase',choices=['training','validation'],required=True);parser.add_argument('--selection',type=Path);parser.add_argument('--training',type=Path)
    args=parser.parse_args();p=load(HERE/'plan-v1.json');roster=load(ROOT/p['roster']);args.cache.mkdir(parents=True,exist_ok=True)
    lock=(args.cache.parent/'fixed-nine-worker.lock').open('a');fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
    worker=Campaign(args.binary,args.cache,p);rows=[];repeats=[];bridges=[];policy={'schema':'pitgun.fixed-nine-budget-policy/v1','admitted':False,'roster_sha256':sha(ROOT/p['roster']),'physics_sha256':sha(ROOT/p['policy']),'contexts':[]};summaries=[]
    def save(complete=False):
        write(args.output,{'schema':'pitgun.fixed-nine-thermal-results/v1','phase':args.phase,'plan':p,'provenance':worker.pin,'rows':rows,'repeats':repeats,'bridges':bridges,'policy':policy,'summaries':summaries,'complete':complete})
    def run(c):
        r=worker.execute(c);rows.append(r);save();print(f'{args.phase} {len(rows)} {c["id"]} {r["checks"] or "OK"}',flush=True);return r
    if args.phase=='training':
        for ctx in p['contexts']:
            era=ctx['era'];budget=p['anchor_budgets'][str(era)];actors={};selected={}
            for i,prop in enumerate(ctx['proposals']):
                c=case(ctx,prop,'balanced_reference',8 if era==1 else 24,label=f'bridge{i}')
                r=worker.execute(c,resolve=True);exact=r['data'].get('receipt',{}).get('physical_input_digest')==prop['source_physical_input_digest']
                bridges.append({'context':[era,ctx['circuit']],'proposal':i,'exact_input':exact})
                if not exact:raise RuntimeError('reference input bridge failed')
                if i==0 and ctx['circuit']=='MONZA':
                    fresh=run(c);bridges[-1]['exact_result']=fresh['data']['outcome'].get('result_digest')==prop['source_result_digest']
                    if not bridges[-1]['exact_result']:raise RuntimeError('reference full bridge failed')
            for actor in roster['drivers']:
                endpoint_groups=[]
                for b in ([8,12] if era==1 else [24]):
                    endpoint_groups.append([run(case(ctx,prop,actor,b,label=f'proposal{i}')) for i,prop in enumerate(ctx['proposals'])])
                pick=choose(endpoint_groups,actor,roster);prop=ctx['proposals'][pick]
                anchors={str(b):scale(prop['points'],b,floors(era)) for b in budget}
                actors[actor]={'proposal_index':pick,'anchors':anchors,'gear':prop['gear'],'downforce':prop['downforce'],'strategy':prop['strategy']}
                selected[actor]={b:next((g[pick] for g in endpoint_groups if g[pick]['case']['effective_budget']==b),None) for b in budget}
                for b in budget:
                    if selected[actor][b] is None:selected[actor][b]=run(case(ctx,prop,actor,b,label='anchor'))
            policy['contexts'].append({'era':era,'circuit':ctx['circuit'],'floors':floors(era),'components':None if era==1 else {'schema_version':'pitgun.racing-vehicle-components/v1','engine_id':'v8_1970','aero_id':'basic'},'actors':actors})
            exit_budget=12 if era==1 else 24;field=[selected[a][exit_budget] for a in actors]
            samples={}
            for family in ['informed','concentrated']:
                c=case(ctx,ctx[family], 'balanced_reference',exit_budget,kind='career',stage='exit',label=family)
                samples[family]=[run(n) for n in neighbors(c,era,[0,1,0,0] if era==1 else floors(era))]
            routine=run(case(ctx,ctx['routine'],'balanced_reference',exit_budget,kind='career',stage='exit',label='routine'))
            best={k:min(v,key=order) for k,v in samples.items()}
            entry=[]
            for family in ['informed','routine']:
                entry.append(run(case(ctx,ctx[family],'balanced_reference',8 if era==1 else 12,kind='career',stage='entry',label='entry-'+family)))
            summaries.append({'era':era,'circuit':ctx['circuit'],'exit_budget':exit_budget,'field_times_s':{a:order(selected[a][exit_budget])[0] for a in actors},
                'player':{k:{'rank':ranked(field,r),'time_s':order(r)[0],'case_id':r['case']['id']} for k,r in best.items()},'routine':{'rank':ranked(field,routine),'time_s':order(routine)[0]},
                'entry_player':{r['case']['id'].split('/')[-1]:{'rank':ranked([selected[a][budget[0]] for a in actors],r),'time_s':order(r)[0]} for r in entry},
                'player_templates':{k:r['case'] for k,r in best.items()}})
            if ctx['circuit']=='MONZA':
                for actor in actors:
                    before=selected[actor][exit_budget];fresh=worker.execute(before['case'],fresh=True)
                    exact=not before['checks'] and not fresh['checks'] and before['data']['outcome']['result_digest']==fresh['data']['outcome'].get('result_digest') and before['data']['receipt']['physical_input_digest']==fresh['data'].get('receipt',{}).get('physical_input_digest')
                    repeats.append({'baseline_id':before['case']['id'],'fresh':fresh,'exact':exact});save();print('repeat',len(repeats),exact,flush=True)
            save()
        numerical=not any(r['checks'] for r in rows) and all(r['exact'] for r in repeats)
        gates={}
        for era in [1,2]:
            s=[x for x in summaries if x['era']==era]
            ranks=sorted(x['player']['informed']['rank'] for x in s)
            gates[str(era)]={'informed_wins':sum(x['player']['informed']['rank']==1 for x in s),'median_informed_rank':ranks[len(ranks)//2],
                'concentrated_wins':sum(x['player']['concentrated']['rank']==1 for x in s),
                'informed_faster_than_routine':all(x['player']['informed']['time_s']<x['routine']['time_s'] for x in s)}
        passed=numerical and all(g['informed_wins']>=1 and g['median_informed_rank']<=3 and g['concentrated_wins']<(3 if e=='1' else 7) and g['informed_faster_than_routine'] for e,g in gates.items())
        save(True)
        selection={'schema':'pitgun.fixed-nine-thermal-selection/v1','training_pass':passed,'numerical_pass':numerical,'gates':gates,'training_archive_sha256':sha(args.output),'policy':policy,'player_templates':summaries}
        write(args.output.parent/'selection-v1.json',selection);print(json.dumps({'training_pass':passed,'gates':gates}),flush=True)
    else:
        selection=load(args.selection);assert sha(args.training)==selection['training_archive_sha256'],'training evidence changed';assert selection['training_pass'],'no eligible frozen training candidate'
        policy=selection['policy'];selection_hash=sha(args.selection);pin=args.cache/'selection-pin.json'
        if pin.exists():assert load(pin)=={'selection_sha256':selection_hash}
        else:write(pin,{'selection_sha256':selection_hash})
        for v in p['validation_contexts']:
            ctx=next(c for c in p['contexts'] if c['era']==v['era'] and c['circuit']==v['circuit']);field=[]
            for actor in roster['drivers']:
                prep=preparation(policy,v['era'],v['circuit'],v['budget'],actor)
                c=case(ctx,ctx['proposals'][0],actor,v['budget'],seed=v['seed'],label='reserved');c.update(prep);field.append(run(c))
            source=next(s for s in selection['player_templates'] if s['era']==v['era'] and s['circuit']==v['circuit']);bench={}
            for family in ['informed','concentrated']:
                c=copy.deepcopy(source['player_templates'][family]);c['seed']=v['seed'];c['kind']='field';c['effective_budget']=v['budget'];c['effective_allocation']=scale(c['effective_allocation'],v['budget'],floors(v['era']));c['id']=f'{v["era"]}/{v["circuit"]}/player/{v["budget"]}/reserved-{family}'
                r=run(c);bench[family]={'rank':ranked(field,r),'time_s':order(r)[0]}
            summaries.append({'era':v['era'],'circuit':v['circuit'],'budget':v['budget'],'seed':v['seed'],'player':bench,'field_times_s':{r['case']['actor']:order(r)[0] for r in field}})
        save(True);numerical=not any(r['checks'] for r in rows)
        review={}
        for era in [1,2]:
            group=[s for s in summaries if s['era']==era]
            review[str(era)]={'informed_wins':sum(s['player']['informed']['rank']==1 for s in group),
                'concentrated_wins':sum(s['player']['concentrated']['rank']==1 for s in group),'contexts':len(group)}
        passed=numerical and all(g['informed_wins']>=1 and g['concentrated_wins']<g['contexts'] for g in review.values())
        write(args.output.parent/'validation-decision-v1.json',{'numerical_pass':numerical,'validation_pass':passed,'review':review,'selection_sha256':selection_hash,'gp':len(rows),'summaries':summaries,'release_admitted':False});print('validation_pass',passed,'numerical_pass',numerical,flush=True)


if __name__=='__main__':main()
