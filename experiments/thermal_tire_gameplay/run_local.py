#!/usr/bin/env python3
"""Audit mechanics and response lines; never fit or promote coefficients."""
import argparse,importlib.util,json,pathlib,platform,subprocess,tempfile
ROOT=pathlib.Path(__file__).resolve().parents[2];HERE=pathlib.Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('baseline',ROOT/'experiments/physics_foundation/baseline_local.py');b=importlib.util.module_from_spec(spec);spec.loader.exec_module(b)
SOURCES={'solver':'crates/pitgun-racing-solver/src/lib.rs','energy':'crates/pitgun-racing-solver/src/longitudinal_energy.rs','simulator':'crates/pitgun-racing-simulator/src/lib.rs','probe':'crates/pitgun-racing-simulator/examples/thermal_tire_gameplay_probe.rs'}
def stable(v):
 if isinstance(v,list):return [stable(x) for x in v]
 if isinstance(v,dict):return {k:stable(x) for k,x in v.items() if k!='measurements'}
 return v

def execute(probe,case,model,identity,timeout):
 with tempfile.TemporaryDirectory(prefix='pitgun-thermal-') as tmp:
  p=pathlib.Path(tmp)/'case.json';p.write_bytes(b.encode({'request':case['request'],'model':model,**case.get('overrides',{})}));m=b.run_process([str(probe),str(p),str(ROOT/'catalogs/racing/v1.8.1')],timeout)
 raw,err=m.pop('stdout'),m.pop('stderr')
 if m['returncode'] or m['timed_out']:raise ValueError(err.decode(errors='replace')[:2000])
 d=json.loads(raw);b.encode(d)
 if d['binary_digest']!=identity['binary_digest'] or d['sources']!=identity['compiled_sources'] or d['catalog']!=identity['catalog']:raise ValueError('stale source/binary/catalog')
 return {'case_id':case['id'],'model':model,'data':d,'measurements':m}

def analyze(plan,results):
 by_key={(r['case_id'],r['model']):r['data'] for r in results};groups={}
 for c in plan['cases']:
  if c['group']=='equal_budget':groups.setdefault(c['metadata']['series'],[]).append(c)
 cooling=[]
 for model in plan['models']:
  for series,cases in groups.items():
   runs=[(c,by_key[(c['id'],model)]['outcome']) for c in cases];finished=[(c,o) for c,o in runs if o['ok']];finished.sort(key=lambda x:x[1]['total_time_s']);zero=next(((c,o) for c,o in runs if c['raw_tuning']['cooling_points']==0),None)
   cooling.append({'model':model,'series':series,'sampled_cooling':[c['raw_tuning']['cooling_points'] for c in cases],'all_samples_complete':len(finished)==len(runs),'best_sampled_cooling':finished[0][0]['raw_tuning']['cooling_points'] if finished else None,'zero_cooling_completes':zero[1]['ok'],'zero_within_margin_of_best':bool(finished and zero[1]['ok'] and zero[1]['total_time_s']<=finished[0][1]['total_time_s']+plan['comparison_margin_s']),'zero_vs_best_delta_s':zero[1]['total_time_s']-finished[0][1]['total_time_s'] if finished and zero[1]['ok'] else None})
 ablations=[]
 for c in plan['cases']:
  if not c['group'].startswith('ablation'):continue
  refs=[x for x in plan['cases'] if not x['overrides'] and x['request']==c['request']]
  if not refs:raise ValueError('ablation missing an exact request reference: '+c['id'])
  for model in plan['models']:
   off=by_key[(c['id'],model)]['outcome'];on=by_key[(refs[0]['id'],model)]['outcome']
   ablations.append({'case_id':c['id'],'reference':refs[0]['id'],'model':model,'both_complete':off['ok'] and on['ok'],'diagnostic_removed_feedback_delta_s':off['total_time_s']-on['total_time_s'] if off['ok'] and on['ok'] else None})
 failures=[{'case_id':r['case_id'],'model':r['model'],'error':r['data']['outcome']['error']} for r in results if not r['data']['outcome']['ok']]
 return {'cooling_response_lines':cooling,'diagnostic_ablations':ablations,'failures':failures,'interpretation':'Response lines and ablations show mechanisms, not a global setup optimum, experimental physical validation, earned career progression or forced positive-cooling proof. Tire laws are unchanged by the energy candidate.'}

def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--probe',type=pathlib.Path,required=True);p.add_argument('--output',type=pathlib.Path,default=HERE/'results/local-gameplay-v1.json');p.add_argument('--check',action='store_true');a=p.parse_args();probe=a.probe.resolve(strict=True)
 plan_path=HERE/'plan-v1.json';plan=b.load(plan_path)
 subprocess.run(['python3',str(HERE/'make_plan.py'),'--check'],cwd=ROOT,check=True,stdout=subprocess.DEVNULL)
 paths=[*sorted((ROOT/'crates/pitgun-racing-solver').rglob('*.rs')),*sorted((ROOT/'crates/pitgun-racing-simulator').rglob('*.rs')),ROOT/'Cargo.lock',ROOT/'Cargo.toml',ROOT/'crates/pitgun-racing-solver/Cargo.toml',ROOT/'crates/pitgun-racing-simulator/Cargo.toml',ROOT/'experiments/physics_foundation/baseline_local.py',HERE/'make_plan.py',plan_path,pathlib.Path(__file__)]
 rel=[str(x.relative_to(ROOT)) for x in paths];subprocess.run(['git','diff','--exit-code','HEAD','--',*rel],cwd=ROOT,check=True,stdout=subprocess.DEVNULL)
 for path in rel:subprocess.run(['git','ls-files','--error-unmatch',path],cwd=ROOT,check=True,stdout=subprocess.DEVNULL)
 sources={str(x.relative_to(ROOT)):b.digest(x.read_bytes()) for x in paths};identity={'source_commit':subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),'source_files':sources,'compiled_sources':{k:sources[v] for k,v in SOURCES.items()},'binary_digest':b.digest(probe.read_bytes()),'plan_digest':b.digest(plan_path.read_bytes()),'catalog':b.load(ROOT/plan['catalog']/'release.json')}
 old_path=ROOT/'experiments/longitudinal_energy/results/local-energy-v1.json';old=b.load(old_path);identity['energy_baseline_digest']=b.digest(old_path.read_bytes());preflight=[]
 for c in old['cases']:
  if len(c['request']['input']['competitors'])!=1:continue
  for model in plan['models']:
   v=execute(probe,c,model,identity,plan['timeout_s'])['data']['outcome'];ref=c['outcomes']['legacy' if model=='historical' else 'energy']['competitors']['player']['output']
   if v['ok']!=ref['ok'] or (v['ok'] and v['result_digest']!=ref['result_digest']) or (not v['ok'] and v['error']!=ref['error']):raise ValueError('read-only audit changed frozen result '+c['id'])
   preflight.append({'id':c['id'],'model':model,'frozen_result_equal':True})
 results=[]
 for c in plan['cases']:
  for model in plan['models']:
   row=execute(probe,c,model,identity,plan['timeout_s']);results.append(row)
   print(c['id'],model,'OK' if row['data']['outcome']['ok'] else row['data']['outcome']['error'],flush=True)
 report={'schema':'pitgun.thermal-tire-gameplay-audit/v1','identity':identity,'preflight':preflight,'results':results,'analysis':analyze(plan,results),'status':'AUDIT_COMPLETE_NOT_A_GAMEPLAY_ACCEPTANCE','environment':{'platform':platform.platform(),'rustc':subprocess.check_output(['rustc','--version'],text=True).strip()},'scope':plan['scope']}
 if a.check:
  previous=b.load(a.output);report['identity']['source_commit']=previous['identity']['source_commit']
  if stable(report)!=stable(previous):raise ValueError('deterministic audit differs; archive untouched')
  print('PASS deterministic full recheck; archive untouched')
 else:
  a.output.parent.mkdir(parents=True,exist_ok=True);raw=b.encode(report);a.output.write_bytes(raw);a.output.with_suffix('.sha256').write_text(b.digest(raw)+'\n')
if __name__=='__main__':main()
