#!/usr/bin/env python3
"""Pinned local thermal screen, exact repeat and historical/energy golden preflight."""
import argparse,hashlib,importlib.util,json,pathlib,platform,subprocess,tempfile
ROOT=pathlib.Path(__file__).resolve().parents[2];HERE=pathlib.Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('baseline',ROOT/'experiments/physics_foundation/baseline_local.py');b=importlib.util.module_from_spec(spec);spec.loader.exec_module(b)
SOURCES={'solver':'crates/pitgun-racing-solver/src/lib.rs','energy':'crates/pitgun-racing-solver/src/longitudinal_energy.rs','thermal':'crates/pitgun-racing-solver/src/thermal_physics.rs','simulator':'crates/pitgun-racing-simulator/src/lib.rs','probe':'crates/pitgun-racing-simulator/examples/thermal_physics_v2_probe.rs'}
def run(probe,payload,extra,timeout):
 with tempfile.TemporaryDirectory(prefix='pitgun-thermal-v2-') as tmp:
  f=pathlib.Path(tmp)/'case.json';f.write_bytes(b.encode(payload));m=b.run_process([str(probe),str(f),*map(str,extra)],timeout)
 out=m.pop('stdout');err=m.pop('stderr')
 if m['returncode'] or m['timed_out']:raise RuntimeError(err.decode(errors='replace')[:2000])
 return json.loads(out),m
def stable(v):
 if isinstance(v,list):return [stable(x) for x in v]
 if isinstance(v,dict):return {k:stable(x) for k,x in v.items() if k!='measurements'}
 return v
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--probe',type=pathlib.Path,required=True);p.add_argument('--legacy-probe',type=pathlib.Path,required=True);p.add_argument('--output',type=pathlib.Path,default=HERE/'results/screen-v1.json');p.add_argument('--check',action='store_true');a=p.parse_args()
 plan=b.load(HERE/'plan-v1.json');profile=HERE/'profile-v1.json';probe=a.probe.resolve(strict=True)
 subprocess.run(['python3',str(HERE/'make_plan.py'),'--check'],cwd=ROOT,check=True,stdout=subprocess.DEVNULL)
 paths=[*map(lambda s:ROOT/s,SOURCES.values()),HERE/'run_local.py',HERE/'make_plan.py',HERE/'plan-v1.json',profile,ROOT/'Cargo.lock']
 rel=[str(x.relative_to(ROOT)) for x in paths]
 subprocess.run(['git','diff','--exit-code','HEAD','--',*rel],cwd=ROOT,check=True,stdout=subprocess.DEVNULL)
 for path in rel:subprocess.run(['git','ls-files','--error-unmatch',path],cwd=ROOT,check=True,stdout=subprocess.DEVNULL)
 identity={'source_commit':subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),'source_files':{str(x.relative_to(ROOT)):b.digest(x.read_bytes()) for x in paths},'binary_digest':b.digest(probe.read_bytes()),'profile_digest':b.digest(profile.read_bytes()),'plan_digest':b.digest((HERE/'plan-v1.json').read_bytes()),'platform':platform.platform()}
 compiled={k:identity['source_files'][v] for k,v in SOURCES.items()}
 old=b.load(ROOT/'experiments/longitudinal_energy/results/local-energy-v1.json');preflight=[]
 for c in old['cases']:
  if len(c['request']['input']['competitors'])!=1:continue
  for mode in ['historical','energy']:
   data,_=run(a.legacy_probe.resolve(strict=True),{'request':c['request'],'model':mode},[ROOT/plan['catalog']],180)
   ref=c['outcomes']['legacy' if mode=='historical' else 'energy']['competitors']['player']['output'];o=data['outcome']
   if o['ok']!=ref['ok'] or (o['ok'] and o['result_digest']!=ref['result_digest']) or (not o['ok'] and o['error']!=ref['error']):raise RuntimeError('changed frozen output '+c['id'])
   preflight.append({'case':c['id'],'model':mode,'exact_frozen_outcome':True})
 print('PASS',len(preflight),'frozen preflight outcomes',flush=True)
 results=[]
 for c in plan['cases']:
  for candidate in plan['models']:
   data,m=run(probe,{'request':c['request'],'candidate':candidate,**c['overrides']},[ROOT/plan['catalog'],profile],plan['timeout_s'])
   if data['compiled_sources']!=compiled or data['binary_digest']!=identity['binary_digest'] or data['profile_digest']!=identity['profile_digest']:raise RuntimeError('stale compiled source/binary/profile')
   if candidate and data['outcome']['ok']:
    e=data['outcome']['energy'];t=data['outcome']['thermal']['ledger']
    if max(e['maximum_vehicle_balance_residual_j'],e['maximum_driveline_balance_residual_j'],t['maximum_engine_balance_residual_j'],t['maximum_tire_balance_residual_j'],t['maximum_fuel_balance_residual_j'])>1e-3:raise RuntimeError('physical balance failure '+c['id'])
   results.append({'case':c['id'],'candidate':candidate,'data':data,'measurements':m})
   print(c['id'],'V2' if candidate else 'energy-V1','OK' if data['outcome']['ok'] else data['outcome']['error'],flush=True)
 report={'schema':'pitgun.thermal-physics-screen-results/v2','identity':identity,'preflight':preflight,'results':results,'status':'EXPLORATORY_REFINE_NOT_PHYSICAL_OR_GAMEPLAY_ACCEPTANCE','scope':plan['scope']}
 if a.check:
  old=b.load(a.output);report['identity']['source_commit']=old['identity']['source_commit']
  if stable(old)!=stable(report):raise RuntimeError('repeat differs; archive untouched')
  print('PASS exact full repeat; archive untouched')
 else:
  a.output.parent.mkdir(exist_ok=True,parents=True);raw=b.encode(report);a.output.write_bytes(raw);a.output.with_suffix('.sha256').write_text(b.digest(raw)+'\n')
if __name__=='__main__':main()
