#!/usr/bin/env python3
"""Finite era-III pilot with the shared durable journal and unchanged solver."""
import argparse
from copy import deepcopy
import fcntl
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import time

HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[1]
sys.path.insert(0,str(ROOT/'experiments/fixed_nine_refinement_v2'))
from harness import BASE, Journal, advance_batch, encoded, load, sha, write, key


def fixtures():return load(HERE/'results/career-fixtures-v1.json')
def fixture(stage,doctrine='craft_engineering'):
    return next(f for f in fixtures()['fixtures'] if f['id']==f'{doctrine}:era3-{stage}')
def floors(f):return [f['offsets'][a] for a in ['aero','chassis','engine','cooling']]
def cap(f):return f['discretionaryPoints']+sum(floors(f))


def strategy(laps,two):
    if two:
        a,b=laps//3,2*laps//3
        return {'stints':[{'tire_id':t,'laps':n}for t,n in zip(['medium','hard','medium'],[a,b-a,laps-b])],'pit_laps':[a,b]}
    a=laps//2
    return {'stints':[{'tire_id':'medium','laps':a},{'tire_id':'hard','laps':laps-a}],'pit_laps':[a]}


def make_case(actor,recipe,two,stage='developed',budget=None,circuit='BARCELONA'):
    f=fixture(stage);available=cap(f) if budget is None else budget
    weights,downforce,gear={'neutral':([1,1,1,1],.5,.5),'cooling':([2,2,3,3],.65,.64),'concentrated':([1,4,4,1],.35,.72)}[recipe]
    points=BASE.scale(weights,min(available,80),floors(f))
    laps=next(p['request']['input']['laps'] for p in f['preparedRequests'] if p['circuitId']==circuit)
    return {'id':f'III/{circuit}/{stage}/{available}/{actor}/{recipe}/{1+int(two)}stop','policy_id':'rolling-heat-065','circuit':circuit,'seed':83017,
        'fixture_id':f['id'],'actor':actor,'kind':'career' if actor=='balanced_reference' and budget is None else 'field','effective_budget':available,'effective_allocation':points,
        'downforce':downforce,'gear':gear,'mode':'balanced','strategy':strategy(laps,two)}


def pilot_matrix():
    matrix=[{'case':make_case(actor,recipe,two),'fresh':False}for actor in ['franz_hermann','charles_leclair','balanced_reference']for recipe in ['neutral','cooling','concentrated']for two in [False,True]]
    matrix += [{'case':make_case('balanced_reference',recipe,False,stage='entry'),'fresh':False}for recipe in ['neutral','concentrated']]
    matrix += [{'case':make_case('balanced_reference','neutral',True,budget=82),'fresh':False}]
    matrix += [dict(deepcopy(matrix[0]),fresh=True)]
    return matrix


def checks(data):
    errors=BASE.checks(data)
    if not data['outcome']['ok']:return errors
    r,c=data['receipt'],data['case'];spent=sum(c['effective_allocation']);available=c['effective_budget']
    valid=(r['budget']==available and r['points']==c['effective_allocation']and spent<=available
        and r['spent_budget']==spent and r['unused_budget']==available-spent)
    errors=[e for e in errors if e!='budget_binding']
    if not valid:errors.append('available_spent_budget_binding')
    errors=[e for e in errors if e!='fuel_scope']
    if r['initial_state']['fuel_mass']!=c.get('initial_fuel_kg',100.):errors.append('explicit_initial_fuel_binding')
    return errors


class Worker(BASE.Campaign):
    def execute(self,case,fresh=False,resolve=False):
        self.verify();path=self.cache/key(case,fresh,resolve)
        if path.exists():
            row=load(path)
            if row['case']!=case or row['binary_sha256']!=self.identity['binary_sha256']or row['data']['case']!=case:raise ValueError('changed native cache binding')
            if not resolve and row['checks']!=checks(row['data']):raise ValueError('changed native checks')
            return row
        request=self.cache/'current-case.json';write(request,case);started=time.monotonic()
        run=subprocess.run([str(self.binary),str(ROOT/self.plan['policy']),str(request)]+(['--resolve-only']if resolve else[]),cwd=ROOT,capture_output=True,text=True,timeout=1800)
        self.verify()
        data=json.loads(run.stdout)if run.returncode==0 else {'case':case,'outcome':{'ok':False,'error':run.stderr}}
        if data['case']!=case:raise ValueError('native case echo differs')
        row={'case':case,'data':data,'wall_s':time.monotonic()-started,'checks':[]if resolve and data.get('resolved_only')else checks(data),'binary_sha256':self.identity['binary_sha256'],'returncode':run.returncode,'stderr':run.stderr}
        write(path,row);return row


class EraJournal(Journal):
    """Retain the shared durable format; validate the explicit unspent-cap rule.

    The historical journal/checker is left byte-for-byte unchanged so old
    source-pinned campaigns can still resume from their original checkout.
    """
    def _validate(self,index,record):
        if index>=len(self.matrix):raise ValueError('too many journal rows')
        item=self.matrix[index];filename=key(item['case'],fresh=item['fresh'])
        expected={'index':index,'case_sha256':hashlib.sha256(encoded(item['case'])).hexdigest(),'cache_file':filename,'fresh':item['fresh']}
        if any(record.get(k)!=v for k,v in expected.items()):raise ValueError('journal sequence or case binding changed')
        path=self.native_cache/filename
        if not path.exists()or sha(path)!=record.get('row_sha256'):raise ValueError('journal row missing or changed')
        row=load(path)
        if row['case']!=item['case']or row['data']['case']!=item['case']:raise ValueError('journal/native case echo mismatch')
        if row['binary_sha256']!=self.identity['binary_sha256']:raise ValueError('journal binary binding changed')
        if row['checks']!=checks(row['data']):raise ValueError('journal native checks changed')
        return row


def summarize_rows(rows):
    repeat=rows[0]['data']==rows[-1]['data']
    summary={'complete':True,'attemptedGP':len(rows),'completedGP':sum(r['data']['outcome']['ok']for r in rows),'laps':sum(len(r['data']['laps'])for r in rows),'failedCases':[{'id':r['case']['id'],'checks':r['checks']}for r in rows if r['checks']],
        'exactFullOrPartialRepeat':repeat,'nativeWallSeconds':sum(r['wall_s']for r in rows),'rows':[{'id':r['case']['id'],'ok':r['data']['outcome']['ok'],'timeSeconds':r['data']['outcome'].get('time_s'),'nativeWallSeconds':r['wall_s'],'minimumPowerFraction':min((l['minimum_power_fraction']for l in r['data'].get('laps',[])),default=None),'availableCap':r['data'].get('receipt',{}).get('budget'),'spent':r['data'].get('receipt',{}).get('spent_budget')}for r in rows],
        'qualification':'Pilot diagnostics only; no fixed-nine candidate, reserved review, runtime or model-quality admission.'}
    return summary


def run(plan_path,binary,cache,output,max_new_gp=22,max_wall_seconds=1800):
    plan=load(plan_path);plan['retained_files']=dict(plan['retained_files'])
    for path in list(HERE.glob('*.py'))+[Path(plan_path),HERE/'ACCEPTANCE.md',HERE/'export_career_fixtures.mjs']:
        plan['retained_files'][str(path.resolve().relative_to(ROOT))]=sha(path)
    matrix=pilot_matrix()
    if len(matrix)!=plan['gp_count']:raise ValueError('pilot matrix count differs')
    worker=Worker(Path(binary),Path(cache)/'native',plan)
    if worker.identity['binary_sha256']!=plan['native_binary_sha256']:raise ValueError('wrong native executable')
    digest=hashlib.sha256(encoded(matrix)).hexdigest()
    if Path(output).exists():
        old=load(output)
        if old['source_matrix_sha256']!=digest or old['provenance']['identity']!=worker.identity:raise ValueError('do not overwrite another matrix/archive')
    # Resolve all unique vectors before any GP, including the explicit unused cap82 boundary.
    for item in matrix:
        resolved=worker.execute(item['case'],resolve=True)
        if not resolved['data'].get('resolved_only')or resolved['returncode']:raise ValueError('native resolution failed: '+item['case']['id'])
    journal=EraJournal(Path(cache)/'journal',worker.cache,worker.identity,matrix)
    progress=advance_batch(worker,journal,matrix,Path(cache)/'progress.json',max_new_gp,max_wall_seconds)
    worker.verify();write(Path(cache)/'progress.json',{k:v for k,v in progress.items()if k!='timings'})
    if progress['complete']:
        if not Path(output).exists():
            journal.export(output,{'schema':'pitgun.era3-native-pilot/v1','plan':load(plan_path),'provenance':worker.pin,'source_matrix_sha256':digest})
        elif load(output)['journal_sha256']!=sha(journal.path):
            raise ValueError('retained archive journal differs')
        summary=summarize_rows(load(output)['rows'])
        summary_path=Path(output).with_suffix('.summary.json')
        if not summary_path.exists() or load(summary_path)!=summary:write(summary_path,summary)
        return summary
    return progress


def main():
    parser=argparse.ArgumentParser();parser.add_argument('--binary',required=True);parser.add_argument('--cache',required=True);parser.add_argument('--output',required=True);parser.add_argument('--max-new-gp',type=int,default=22);parser.add_argument('--max-wall-seconds',type=float,default=1800)
    args=parser.parse_args();cache=Path(args.cache);cache.parent.mkdir(parents=True,exist_ok=True)
    with (cache.parent/'native-worker.lock').open('w')as lock:
        fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
        print(json.dumps(run(HERE/'pilot-plan-v1.json',args.binary,cache,args.output,args.max_new_gp,args.max_wall_seconds),indent=2),flush=True)

if __name__=='__main__':main()
