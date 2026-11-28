#!/usr/bin/env python3
"""Paired common140kg stress screen; one worker, no setup fitting."""
import argparse
from functools import cache
from copy import deepcopy
import fcntl
import hashlib
from pathlib import Path
import sys
import json

HERE=Path(__file__).resolve().parent;ROOT=HERE.parents[1]
sys.path.insert(0,str(ROOT/'experiments/era3_joint_preparation_v1'))
import run as legacy


@cache
def corpus():return legacy.load(ROOT/'experiments/era3_common_fuel_v1/results/career-fixtures-v2.json')
@cache
def fixture(stage):return next(f for f in corpus()['fixtures']if f['id']==f'craft_engineering:era3-{stage}')
def context_ids():return [r['circuitId']for r in fixture('developed')['preparedRequests']]


def strategy(laps,count):
 if count==1:
  first=laps//2
  return {'stints':[{'tire_id':'hard','laps':first},{'tire_id':'medium','laps':laps-first}],'pit_laps':[first]}
 a,b=laps//3,2*laps//3
 return {'stints':[{'tire_id':t,'laps':n}for t,n in zip(['soft','medium','hard'],[a,b-a,laps-b])],'pit_laps':[a,b]}


# Authored before timing this screen. No per-circuit optimization.
RECIPES=[
 {'id':'actual-entry','stage':'entry','actor':'balanced_reference','budget':24,'weights':[1,1,1,1],'downforce':.5,'gear':.5,'stops':1,'kind':'career'},
 {'id':'actual-developed-power','stage':'developed','actor':'balanced_reference','budget':34,'weights':[1,2,5,1],'downforce':1.,'gear':.75,'stops':2,'kind':'career'},
 {'id':'actual-exit-balanced','stage':'exit','actor':'balanced_reference','budget':43,'weights':[3,2,4,3],'downforce':.75,'gear':.65,'stops':1,'kind':'career'},
 {'id':'field-low-cooling','stage':'developed','actor':'franz_hermann','budget':24,'weights':[6,2,15,1],'downforce':1.,'gear':.9,'stops':2,'kind':'field'},
 {'id':'field-mid-power','stage':'developed','actor':'charles_leclair','budget':43,'weights':[1,2,5,2],'downforce':1.,'gear':.9,'stops':2,'kind':'field'},
 {'id':'field-high-balanced','stage':'developed','actor':'franz_hermann','budget':60,'weights':[2,2,3,3],'downforce':.7,'gear':.8,'stops':1,'kind':'field'},
 {'id':'field-max79','stage':'developed','actor':'charles_leclair','budget':79,'weights':[20,19,20,20],'downforce':1.,'gear':1.,'stops':2,'kind':'field'},
 {'id':'field-max82','stage':'developed','actor':'franz_hermann','budget':82,'weights':[20,20,20,20],'downforce':1.,'gear':.8,'stops':2,'kind':'field'},
]
REPEATS=[('BARCELONA','field-max82'),('MONACO','field-max82'),('SPA','actual-developed-power'),('SAOPAULO','field-max79')]


def design(plan):
 matrix=[]
 for circuit in context_ids():
  for recipe in RECIPES:
   f=fixture(recipe['stage']);floors=legacy.floors(f if recipe['kind']=='career'else fixture('developed'))
   laps=next(p['request']['input']['laps']for p in f['preparedRequests']if p['circuitId']==circuit)
   c={'id':f'III/common140/{circuit}/{recipe["id"]}','policy_id':'rolling-heat-065','circuit':circuit,'seed':plan['seed'],'fixture_id':f['id'],'fixture_revision':2,
     'actor':recipe['actor'],'kind':recipe['kind'],'effective_budget':recipe['budget'],'effective_allocation':legacy.BASE.scale(recipe['weights'],min(recipe['budget'],80),floors),
     'downforce':recipe['downforce'],'gear':recipe['gear'],'mode':'balanced','strategy':strategy(laps,recipe['stops']),'initial_fuel_kg':plan['initial_fuel_kg']}
   matrix.append({'case':c,'fresh':False})
 for circuit,recipe in REPEATS:
  m=next(m for m in matrix if m['case']['id']==f'III/common140/{circuit}/{recipe}')
  matrix.append(dict(deepcopy(m),fresh=True))
 return matrix


def review(data):
 if not data['complete']:raise ValueError('cannot review partial screen')
 expected=design(data['plan']);rows=data['rows']
 if len(rows)!=len(expected)or data['source_matrix_sha256']!=hashlib.sha256(legacy.encoded(expected)).hexdigest():raise ValueError('screen matrix differs')
 if any(r['case']!=m['case']or r['data']['case']!=m['case']or r['binary_sha256']!=data['plan']['native_binary_sha256']or r['checks']!=legacy.checks(r['data'])for r,m in zip(rows,expected)):raise ValueError('screen row binding changed')
 repeats=[]
 for r in rows[-len(REPEATS):]:
  old=next(o for o in rows[:-len(REPEATS)]if o['case']==r['case'])
  repeats.append({'id':r['case']['id'],'exact':r['data']==old['data']})
 summaries=[];failures=[]
 for r in rows:
  d=r['data'];outcome=d['outcome'];entry={'id':r['case']['id'],'actor':r['case']['actor'],'circuit':r['case']['circuit'],'ok':outcome['ok'],'acceptedLaps':len(d.get('laps',[])),'nativeWallSeconds':r['wall_s'],'checks':r['checks']}
  if outcome['ok']:
   entry.update(finalFuelKg=outcome['final_state']['fuel_mass'],timeSeconds=outcome['time_s'],minimumPowerFraction=min(l['minimum_power_fraction']for l in d['laps']),maximumTireSurfaceC=max(l['surface_max_c']for l in d['laps']))
   if entry['finalFuelKg']<data['plan']['minimum_finish_reserve_kg']:failures.append({'id':entry['id'],'reason':'finish_reserve','fuelKg':entry['finalFuelKg']})
  else:entry['error']=outcome.get('error');failures.append({'id':entry['id'],'reason':'not_completed','error':entry['error']})
  if r['checks']:failures.append({'id':entry['id'],'reason':'native_checks','checks':r['checks']})
  summaries.append(entry)
 passed=not failures and all(r['exact']for r in repeats)
 return {'schema':'pitgun.era3-common-fuel-review/v2','status':'PASS'if passed else'REFINE','chargeKg':data['plan']['initial_fuel_kg'],'minimumReserveKg':data['plan']['minimum_finish_reserve_kg'],'attempts':len(rows),'completedGP':sum(r['data']['outcome']['ok']for r in rows),'acceptedLaps':sum(len(r['data'].get('laps',[]))for r in rows),'contexts':context_ids(),'nativeWallSeconds':sum(r['wall_s']for r in rows),'minimumObservedFinishFuelKg':min((s['finalFuelKg']for s in summaries if s['ok']),default=None),'failures':failures,'repeats':repeats,'rows':summaries,'scope':'Finite common-charge operability screen only; not exhaustive all-legal-input coverage, field calibration, reserve seed qualification or runtime admission.'}


def validate_pair(case,receipt,old):
    previous=deepcopy(old['case']);expected=deepcopy(case)
    previous.pop('id');expected.pop('id');previous.pop('initial_fuel_kg');expected.pop('initial_fuel_kg')
    if previous!=expected:raise ValueError('paired screen changed preparation/seed/driver')
    if old['case']['initial_fuel_kg']!=160. or case['initial_fuel_kg']!=140.:raise ValueError('wrong paired fuel charges')
    prior=old['data']['receipt']
    for key in ['bonus_offsets','budget','spent_budget','unused_budget','points','components','driver','engine','mechanical','stops','resolved_compounds','policy_digest','pit_loss_s','laps']:
        if receipt[key]!=prior[key]:raise ValueError('paired physical receipt changed '+key)
    if receipt['initial_state']!=dict(prior['initial_state'],fuel_mass=140.):raise ValueError('paired initial state changed beyond fuel mass')


def main():
 p=argparse.ArgumentParser();p.add_argument('--binary',type=Path,required=True);p.add_argument('--cache',type=Path,required=True);p.add_argument('--output',type=Path,required=True);p.add_argument('--max-new-gp',type=int,default=100);p.add_argument('--max-wall-seconds',type=float,default=3600);args=p.parse_args()
 plan_path=HERE/'plan-v1.json';plan=legacy.load(plan_path);plan['retained_files']=dict(plan['retained_files'])
 for f in list(HERE.glob('*.py'))+[plan_path]:plan['retained_files'][str(f.relative_to(ROOT))]=legacy.sha(f)
 matrix=design(plan)
 if len(matrix)!=plan['gp_count']:raise ValueError('frozen screen count differs')
 args.cache.parent.mkdir(parents=True,exist_ok=True)
 with(args.cache.parent/'native-worker.lock').open('w')as lock:
  fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB);worker=legacy.Worker(args.binary,args.cache/'native',plan)
  if worker.identity['binary_sha256']!=plan['native_binary_sha256']:raise ValueError('wrong frozen binary')
  digest=hashlib.sha256(legacy.encoded(matrix)).hexdigest()
  if args.output.exists():
   old=legacy.load(args.output)
   if old['source_matrix_sha256']!=digest or old['provenance']['identity']!=worker.identity:raise ValueError('do not overwrite another archive')
  baseline=legacy.load(ROOT/plan['paired_archive'])
  if legacy.sha(ROOT/plan['paired_archive'])!=plan['paired_archive_sha256']:raise ValueError('paired160 archive changed')
  for i,m in enumerate(matrix):
   row=worker.execute(m['case'],resolve=True);r=row['data'].get('receipt',{})
   if not row['data'].get('resolved_only')or r['initial_state']['fuel_mass']!=plan['initial_fuel_kg']:raise ValueError('common fuel input failed resolution')
   validate_pair(m['case'],r,baseline['rows'][i])
   if(i+1)%25==0:print(f'preflight {i+1}/{len(matrix)}',flush=True)
  journal=legacy.EraJournal(args.cache/'journal',worker.cache,worker.identity,matrix)
  progress=legacy.advance_batch(worker,journal,matrix,args.cache/'progress.json',args.max_new_gp,args.max_wall_seconds);worker.verify()
  if not progress['complete']:
   print(json.dumps({k:v for k,v in progress.items()if k!='timings'},indent=2));return
  if not args.output.exists():journal.export(args.output,{'schema':'pitgun.era3-common-fuel-screen/v2','plan':legacy.load(plan_path),'provenance':worker.pin,'source_matrix_sha256':digest})
  elif legacy.load(args.output)['journal_sha256']!=legacy.sha(journal.path):raise ValueError('archive journal differs')
  summary=review(legacy.load(args.output));dest=args.output.with_suffix('.review.json')
  if not dest.exists()or legacy.load(dest)!=summary:legacy.write(dest,summary)
  print(json.dumps({k:v for k,v in summary.items()if k!='rows'},indent=2),flush=True)

if __name__=='__main__':main()
