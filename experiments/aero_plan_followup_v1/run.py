#!/usr/bin/env python3
"""Separate post-audit conditional strategy diagnostic; immutable physical inputs."""
import argparse
import copy
import fcntl
import gzip
import importlib.util
import json
from pathlib import Path
import subprocess

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
spec = importlib.util.spec_from_file_location('frozen_aero_audit', ROOT/'experiments/aero_tradeoff_audit_v1/run.py')
AERO = importlib.util.module_from_spec(spec); spec.loader.exec_module(AERO)
sha, write = AERO.sha, AERO.write


def cases(plan):
    rows = []
    for c in plan['contexts']:
        n = c['laps']; a = n//2; b = n-a
        stints = [('medium', a//2), ('hard', b), ('medium', a-a//2)]
        for mode, slider in [('coupled', x) for x in plan['sliders']] + [('downforce_only', .8)]:
            rows.append({'id':f"MH2/{c['circuit']}/{mode}/{slider}", 'policy_id':plan['policy_id'], 'aero_mode':mode,
                'circuit':c['circuit'], 'fixture_id':plan['fixture_id'], 'discretionary_allocation':plan['allocation'],
                'gear':.5,'downforce':slider,'mode':'balanced','seed':plan['seed'],
                'strategy':{'stints':[{'tire_id':t,'laps':l}for t,l in stints], 'pit_laps':[a//2,a//2+b]}})
    assert len(rows)==plan['expected_cases']
    return rows


def metric(row):
    d=row['data']; ls=d['laps']; drive=sum(l['time_s']for l in ls)
    return {'total_s':d['outcome']['time_s'],'first_five_s':sum(l['time_s']for l in ls[:5]),
        'surface_mean_c':sum(l['surface_mean_c']*l['time_s']for l in ls)/drive,
        'temperature_grip_mean':sum(l['temperature_grip_mean']*l['time_s']for l in ls)/drive,
        'wear_grip_mean':sum(l['wear_grip_mean']*l['time_s']for l in ls)/drive}


def analyze(data):
    assert data['complete']; p=data['plan']; rows=data['rows']; lookup={r['case']['id']:r for r in rows}
    assert [r['case']for r in rows]==cases(p)
    all_rows=rows+[v['fresh']for v in data['validations']]
    assert all(AERO.numerical_checks(r['data'])==r['checks']for r in all_rows)
    if any(r['checks']for r in all_rows):
        return {'schema':'pitgun.aero-plan-followup-decision/v1','numerical_pass':False,'failures':[{'id':r['case']['id'],'checks':r['checks']}for r in all_rows if r['checks']],'model_changed':False}
    old=json.load(gzip.open(ROOT/p['baseline_archive'])); previous={r['case']['id']:r for r in old['rows']}
    bridges=[]; summaries=[]
    for c in p['contexts']:
        circuit=c['circuit']; current=lookup[f'MH2/{circuit}/coupled/0.5']; before=previous[f'era2/{circuit}/exit/balanced/MH-matched-2']
        bridges.append({'circuit':circuit,'physical_input_exact':current['data']['receipt']['physical_input_digest']==before['data']['receipt']['physical_input_digest'],
            'result_exact':current['data']['outcome'].get('result_digest')==before['data']['outcome']['result_digest']})
        summaries.append({'circuit':circuit,'baseline':metric(current),'extra_downforce':metric(lookup[f'MH2/{circuit}/downforce_only/0.8']),
            'coupled':{str(x):metric(lookup[f'MH2/{circuit}/coupled/{x}'])for x in p['sliders']}})
    repeats=data['validations']; assert [v['baseline_id']for v in repeats]==[f"MH2/{c['circuit']}/coupled/0.5"for c in p['contexts']]
    exact=[]
    for v in repeats:
        before=lookup[v['baseline_id']]; fresh=v['fresh']
        exact.append(not before['checks'] and not fresh['checks'] and before['case']==fresh['case'] and
            before['data']['outcome']['result_digest']==fresh['data']['outcome'].get('result_digest') and
            before['data']['receipt']['physical_input_digest']==fresh['data']['receipt']['physical_input_digest'] and v['exact'])
    all_rows=rows+[v['fresh']for v in repeats]
    assert all(AERO.numerical_checks(r['data'])==r['checks']for r in all_rows)
    passed=not any(r['checks']for r in all_rows) and all(exact) and all(b['physical_input_exact']and b['result_exact']for b in bridges)
    return {'schema':'pitgun.aero-plan-followup-decision/v1','numerical_pass':passed,'main_gp':len(rows),'fresh_repeats':len(repeats),
        'exact_repeats':sum(exact),'accepted_laps':sum(len(r['data']['laps'])for r in all_rows),'bridges':bridges,'summaries':summaries,
        'model_changed':False,'release_admitted':False}


def main():
    parser=argparse.ArgumentParser();parser.add_argument('--binary',type=Path,required=True);parser.add_argument('--cache',type=Path,required=True);parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args();p=json.loads((HERE/'plan-v1.json').read_text());assert sha(args.binary)==p['native_binary_sha256'],'not the frozen aero probe';matrix=cases(p)
    old=json.load(gzip.open(ROOT/p['baseline_archive']));lookup={r['case']['id']:r['case']for r in old['rows']}
    for c in matrix:
        if c['aero_mode']=='coupled' and c['downforce']==.5:
            before=lookup[f"era2/{c['circuit']}/exit/balanced/MH-matched-2"]
            assert {k:v for k,v in c.items()if k not in ['id','aero_mode']}=={k:v for k,v in before.items()if k!='id'}
    args.cache.parent.mkdir(parents=True,exist_ok=True);lock=(args.cache.parent/'aero-audit-worker.lock').open('a');fcntl.flock(lock.fileno(),fcntl.LOCK_EX|fcntl.LOCK_NB)
    campaign=AERO.Campaign(p,args.binary,args.cache)
    sources=dict(campaign.identity['sources']);sources.update({str((HERE/f).relative_to(ROOT)):sha(HERE/f)for f in ['run.py','plan-v1.json','ACCEPTANCE.md']})
    identity={'binary_sha256':sha(args.binary),'sources':sources};pin={'identity':identity,'source_commit':subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),'parent':campaign.pin}
    pin_file=args.cache/'followup-pin-v1.json'
    if pin_file.exists():
        retained=json.loads(pin_file.read_text());assert retained['identity']==identity;pin=retained
    else:write(pin_file,pin)
    def verify():
        campaign.verify();assert sha(args.binary)==identity['binary_sha256'];assert all(sha(ROOT/f)==h for f,h in sources.items())
    rows=[];validations=[]
    def checkpoint():
        write(args.output,{'schema':'pitgun.aero-plan-followup-results/v1','plan':p,'provenance':pin,'rows':rows,'validations':validations,
            'complete':len(rows)==p['expected_cases']and len(validations)==p['expected_repeats']})
    for c in matrix:
        verify();r=campaign.execute(c);verify();rows.append(r);checkpoint();print(f"{len(rows)}/12 {c['id']} {r['checks']or 'OK'}",flush=True)
    for c in p['contexts']:
        key=f"MH2/{c['circuit']}/coupled/0.5";before=next(r for r in rows if r['case']['id']==key);verify();fresh=campaign.execute(copy.deepcopy(before['case']),repeat=True);verify()
        exact=not before['checks']and not fresh['checks']and before['data']['outcome']['result_digest']==fresh['data']['outcome'].get('result_digest')and before['data']['receipt']['physical_input_digest']==fresh['data']['receipt']['physical_input_digest']
        validations.append({'baseline_id':key,'fresh':fresh,'exact':exact});checkpoint();print('repeat',len(validations),'/3',exact,flush=True)
    result=analyze(json.load(gzip.open(args.output)));write(args.output.parent/'mh2-decision-v1.json',result);print(json.dumps({'pass':result['numerical_pass'],'exact_repeats':result['exact_repeats'],'laps':result['accepted_laps']}),flush=True)


if __name__=='__main__':main()
