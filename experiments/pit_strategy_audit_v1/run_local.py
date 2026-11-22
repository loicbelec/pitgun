#!/usr/bin/env python3
"""Paired native tire/stand ablations; never fits or admits an opponent policy."""
import collections
import concurrent.futures as cf
import copy
import gzip
import hashlib
import json
import pathlib
import subprocess
import sys
import tempfile

EXP=pathlib.Path(__file__).resolve().parent;ROOT=EXP.parents[1]
def enc(d):return json.dumps(d,sort_keys=True,separators=(',',':'),allow_nan=False,ensure_ascii=False).encode()
def sha(b):return 'sha256:'+hashlib.sha256(b).hexdigest()
def load(p):
 b=p.read_bytes();return json.loads(gzip.decompress(b) if p.suffix=='.gz' else b)
def write(p,d):
 b=json.dumps(d,indent=2,ensure_ascii=False,allow_nan=False).encode()+b'\n';p.write_bytes(gzip.compress(b,mtime=0) if p.suffix=='.gz' else b)
def key(c):return hashlib.sha256(enc({k:v for k,v in c.items() if k!='id'})).hexdigest()

def main():
 binary=pathlib.Path(sys.argv[1]).resolve();cache=pathlib.Path(sys.argv[2]).resolve();cache.mkdir(parents=True,exist_ok=True);out=EXP/'results';out.mkdir(exist_ok=True)
 if (out/'campaign-v1.json').exists():raise RuntimeError('refuse overwrite completed audit')
 plan=load(EXP/'plan-v1.json');source=load(ROOT/plan['source']);groups=collections.defaultdict(list)
 for r in source:groups[(r['case']['fixture_id'],r['case']['circuit'])].append(r)
 controls=[]
 for ck,rows in sorted(groups.items()):
  for stops in list(range(1,9))+[12]:
   good=[r for r in rows if len(r['case']['strategy']['pit_laps'])==stops and r['data']['outcome']['ok']]
   controls.append(min(good,key=lambda r:(r['data']['outcome']['time_s'],key(r['case']))))
  b=copy.deepcopy(controls[-1]['case']);n=sum(s['laps'] for s in b['strategy']['stints']);b['strategy']={'stints':[{'tire_id':'medium','laps':1},{'tire_id':'soft','laps':n-1}],'pit_laps':[1]};b['id']=key(b)
  original=next((r for r in rows if key(r['case'])==key(b)),None)
  controls.append(original or {'case':b,'data':None})
 requests={};originals={}
 for r in controls:
  for intervention in plan['interventions']:
   c=copy.deepcopy(r['case']);c['audit']=intervention;c['id']=key(c);requests[key(c)]=c
   if intervention is None and r['data'] is not None:originals[key(c)]=r['data']
 paths=['experiments/pit_strategy_audit_v1/plan-v1.json','experiments/pit_strategy_audit_v1/run_local.py','experiments/pit_strategy_audit_v1/qualify.py','crates/pitgun-racing-simulator/examples/pit_strategy_audit.rs',plan['source'],'experiments/fixed_opponents_v2/roster-v2.json','experiments/career_physics_v1/profile-v1.json','experiments/opponent_physics_v2/results/career-fixtures-v1.json']
 paths+=sorted(str(p.relative_to(ROOT)) for base in ['crates/pitgun-racing-solver/src','crates/pitgun-racing-simulator/src','catalogs/racing/v1.8.1'] for p in (ROOT/base).rglob('*') if p.is_file() and p.suffix in ['.rs','.json'])
 prov={'source_commit':subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),'binary_digest':sha(binary.read_bytes()),'sources':{p:sha((ROOT/p).read_bytes()) for p in paths},'native_workers':plan['workers'],'game_maximum_voluntary_stops':2}
 pin=cache/'provenance.json'
 if pin.exists() and load(pin)!=prov:raise RuntimeError('audit cache pin mismatch')
 write(pin,prov);write(out/'provenance-v1.json',prov)
 with tempfile.TemporaryDirectory(prefix='pitgun-pit-audit-') as td:
  def execute(c,force=False):
   k=key(c);p=cache/(k+'.json.gz')
   if p.exists() and not force:return load(p)
   f=pathlib.Path(td)/(k+'.json');write(f,c)
   try:d=json.loads(subprocess.check_output([str(binary),str(ROOT),str(f)],stderr=subprocess.PIPE))
   except subprocess.CalledProcessError as e:raise RuntimeError('native resolution failure:'+e.stderr.decode()) from e
   if d['binary_digest']!=prov['binary_digest']:raise RuntimeError('binary changed during audit')
   if k in originals:
    old=originals[k]
    for field in ['physical_input_digest','diagnostic_input_digest','lap_times_s','outcome','maximum_carry_jumps','thermal_ledger','metrics']:
     if old[field]!=d[field]:raise RuntimeError('unchanged source control differs:'+field)
   if p.exists() and load(p)!=d:raise RuntimeError('exact repeat differs')
   write(p,d);return d
  results=[]
  with cf.ThreadPoolExecutor(max_workers=plan['workers']) as pool:
   cases=list(requests.values())
   for c,d in zip(cases,pool.map(execute,cases)):
    results.append({'case':c,'data':d})
    if len(results)%100==0:print(f'audit: {len(results)}/{len(cases)}',flush=True)
  write(out/'paired-ablation-v1.json.gz',results)
  repeat=[r['case'] for r in results if r['case']['circuit']=='MONZA' and len(r['case']['strategy']['pit_laps'])==2]
  with cf.ThreadPoolExecutor(max_workers=plan['workers']) as pool:repeated=list(pool.map(lambda c:execute(c,True),repeat))
  write(out/'exact-repeat-v1.json.gz',[{'case':c,'data':d} for c,d in zip(repeat,repeated)])
 if {p:sha((ROOT/p).read_bytes()) for p in paths}!=prov['sources']:raise RuntimeError('audit source changed')
 write(out/'campaign-v1.json',{'schema':'pitgun.pit-strategy-audit-receipt/v1','requests':len(results),'unchanged_source_controls':len(originals),'exact_fresh_repeats':len(repeated),'native_gp_executions':len(results)+len(repeated),'provenance':prov,'files':{p.name:sha(p.read_bytes()) for p in out.glob('*-v1.*') if p.name!='campaign-v1.json'}})
 print('audit complete; no opponent policy selected',flush=True)

if __name__=='__main__':main()
