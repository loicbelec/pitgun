#!/usr/bin/env python3
"""Bounded diagnostic using the existing fuel preparation, not new coefficients."""
import argparse
from pathlib import Path
import subprocess
import sys
import time
import json
from datetime import datetime,timezone

HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[1]
sys.path.insert(0,str(HERE));from run import encoded,load,sha,write


def main():
 p=argparse.ArgumentParser();p.add_argument('--binary',type=Path,required=True);p.add_argument('--cache',type=Path,required=True);p.add_argument('--output',type=Path,required=True);args=p.parse_args()
 plan=load(HERE/'fuel-diagnostic-plan-v1.json');archive=load(HERE/plan['pilot_archive']);source_files={str(x.relative_to(ROOT)):sha(x)for x in (ROOT/'crates').rglob('*.rs')}
 if sha(args.binary)!=plan['binary_sha256']or sha(HERE/plan['pilot_archive'])!=plan['pilot_archive_sha256']:raise ValueError('wrong binary or pilot archive')
 if sha(ROOT/plan['fuel_policy'])!=plan['fuel_policy_sha256']:raise ValueError('fuel policy changed')
 args.cache.mkdir(parents=True,exist_ok=True)
 identity={'source_commit':subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),'sources':source_files,'script_sha256':sha(Path(__file__)),'plan':plan}
 pin=args.cache/'pin.json'
 if pin.exists():
  if load(pin)!=identity:raise ValueError('changed diagnostic source identity')
 else:
  if subprocess.check_output(['git','status','--porcelain'],cwd=ROOT,text=True).strip():raise ValueError('freeze clean diagnostic sources before compute')
  write(pin,identity)
 rows=[]
 indices=plan['pilot_row_indices']+[plan['repeat_row_index']]
 for order,index in enumerate(indices):
  case=archive['rows'][index]['case'];expected=archive['rows'][index]['data']['receipt']['physical_input_digest'];output=args.cache/f'{order:02d}-native.json';case_path=args.cache/f'{order:02d}-case.json'
  if case_path.exists():
   if load(case_path)!=case:raise ValueError('changed diagnostic input')
  else:write(case_path,case)
  if output.exists():row=load(output)
  else:
   before=time.monotonic();r=subprocess.run([str(args.binary.resolve()),str(ROOT/plan['tire_policy']),str(case_path.resolve()),'--fuel-estimate',str(ROOT/plan['fuel_policy'])],cwd=ROOT,capture_output=True,text=True,check=True,timeout=180)
   data=json.loads(r.stdout);row={'case_index':index,'data':data,'wall_s':time.monotonic()-before};write(output,row)
  d=row['data']
  if d.get('original_physical_input_digest')!=expected or d['preparation']['source_input_digest']!=expected:raise ValueError('diagnostic changed original physical input')
  rows.append(row);print(case['id'],d['preparation']['outcome'],flush=True)
 if rows[0]['data']!=rows[-1]['data']:raise ValueError('fuel estimate repeat differs')
 if any(sha(ROOT/f)!=v for f,v in source_files.items())or sha(args.binary)!=plan['binary_sha256']:raise ValueError('diagnostic sources changed')
 result={'schema':'pitgun.era3-fuel-estimate-review/v1','identity':identity,'rows':rows,'exactRepeat':True,'uniqueInputs':len(plan['pilot_row_indices']),'pilotLapsIncludingRepeat':sum(x['completed_laps']for r in rows for x in r['data']['preparation']['pilots']),'fullGpComputed':0,'scope':'Existing110kg operational-limit policy diagnostic; not proof of tank capacity or physical impossibility. No prepared GP or game admission.'}
 if args.output.exists():
  if load(args.output)!=result:raise ValueError('do not replace another diagnostic result')
 else:write(args.output,result)

if __name__=='__main__':main()
