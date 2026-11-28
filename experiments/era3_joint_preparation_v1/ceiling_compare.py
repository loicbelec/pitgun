#!/usr/bin/env python3
"""Actual GP at the existing operational ceiling; no fuel/physics refit."""
import argparse
from copy import deepcopy
from pathlib import Path
import fcntl
import hashlib
import sys
import json

HERE=Path(__file__).resolve().parent;ROOT=HERE.parents[1]
sys.path.insert(0,str(HERE));import run


def matrix(plan):
 archive=run.load(HERE/plan['pilot_archive']);items=[]
 for index in plan['pilot_row_indices']:
  case=deepcopy(archive['rows'][index]['case']);case['id']+='/explicit-start110';case['initial_fuel_kg']=plan['initial_fuel_kg'];items.append({'case':case,'fresh':False})
 return items+[dict(deepcopy(items[0]),fresh=True)]


def main():
 p=argparse.ArgumentParser();p.add_argument('--binary',type=Path,required=True);p.add_argument('--cache',type=Path,required=True);p.add_argument('--output',type=Path,required=True);args=p.parse_args()
 path=HERE/'ceiling-plan-v1.json';plan=run.load(path);plan['retained_files']=dict(plan['retained_files'])
 for f in list(HERE.glob('*.py'))+[path]:plan['retained_files'][str(f.relative_to(ROOT))]=run.sha(f)
 items=matrix(plan);digest=hashlib.sha256(run.encoded(items)).hexdigest()
 args.cache.parent.mkdir(parents=True,exist_ok=True)
 with (args.cache.parent/'native-worker.lock').open('w')as lock:
  fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
  worker=run.Worker(args.binary,args.cache/'native',plan)
  if worker.identity['binary_sha256']!=plan['native_binary_sha256']:raise ValueError('not the frozen ceiling diagnostic binary')
  archive=run.load(HERE/plan['pilot_archive'])
  for item,index in zip(items,plan['pilot_row_indices']+[plan['pilot_row_indices'][0]]):
   resolved=worker.execute(item['case'],resolve=True)['data'];prior=archive['rows'][index]['data']['receipt'];receipt=resolved['receipt']
   if not resolved.get('resolved_only'):raise ValueError('ceiling input did not resolve')
   for key in ['bonus_offsets','budget','points','components','driver','engine','mechanical','stops','resolved_compounds','policy_digest']:
    if receipt[key]!=prior[key]:raise ValueError('counterfactual changed '+key)
   expected=dict(prior['initial_state'],fuel_mass=plan['initial_fuel_kg'])
   if receipt['initial_state']!=expected:raise ValueError('counterfactual must change starting fuel only')
  if args.output.exists():
   old=run.load(args.output)
   if old['source_matrix_sha256']!=digest or old['provenance']['identity']!=worker.identity:raise ValueError('different diagnostic archive')
  journal=run.EraJournal(args.cache/'journal',worker.cache,worker.identity,items)
  progress=run.advance_batch(worker,journal,items,args.cache/'progress.json',max_new_gp=4,max_wall_seconds=600)
  if not progress['complete']:raise ValueError('bounded comparison incomplete; durable progress retained')
  worker.verify()
  if not args.output.exists():journal.export(args.output,{'schema':'pitgun.era3-ceiling-comparison/v1','plan':run.load(path),'provenance':worker.pin,'source_matrix_sha256':digest})
  rows=run.load(args.output)['rows'];assert rows[0]['data']==rows[-1]['data']
  print(json.dumps({'complete':True,'attempts':len(rows),'exactRepeat':True,'rows':[{'id':r['case']['id'],'outcome':r['data']['outcome'],'checks':r['checks'],'laps':len(r['data']['laps'])}for r in rows]},indent=2))

if __name__=='__main__':main()
