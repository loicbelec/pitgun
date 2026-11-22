#!/usr/bin/env python3
"""Shared immutable native case/cache boundary for the physical screen and field fit."""
import concurrent.futures as cf
import copy
import gzip
import hashlib
import json
import os
import pathlib
import subprocess
import tempfile
import threading

EXP=pathlib.Path(__file__).resolve().parent;ROOT=EXP.parents[1]
DEFAULTS={'audit':None,'performance_profile':None,'diagnostic_mode':False,'diagnostic_laps':None,'diagnostic_driving_mode':None}
def enc(d):return json.dumps(d,sort_keys=True,separators=(',',':'),allow_nan=False,ensure_ascii=False).encode()
def sha(b):return 'sha256:'+hashlib.sha256(b).hexdigest()
def key(c):return hashlib.sha256(enc({k:v for k,v in c.items() if k!='id'})).hexdigest()
def load(p):
 p=pathlib.Path(p);b=p.read_bytes();return json.loads(gzip.decompress(b) if p.suffix=='.gz' else b)
def write(p,d):
 p=pathlib.Path(p);b=json.dumps(d,indent=2,ensure_ascii=False,allow_nan=False).encode()+b'\n';p.write_bytes(gzip.compress(b,mtime=0) if p.suffix=='.gz' else b)
def normalize(c):
 c=copy.deepcopy(c)
 for k,v in DEFAULTS.items():c.setdefault(k,v)
 for field in ['gear','downforce']:
  if type(c[field]) not in [int,float]:raise ValueError('slider must be numeric')
  c[field]=float(c[field])
 if c['environment'] is not None:
  if any(type(v) not in [int,float] for v in c['environment'].values()):raise ValueError('environment must be numeric')
  c['environment']={k:float(v) for k,v in c['environment'].items()}
 c['id']=key(c);return c
def change(c,**fields):
 c=copy.deepcopy(c);c.update(fields);return normalize(c)
def strategy(n,tires,pits):
 bounds=[0]+list(pits)+[n]
 if bounds!=sorted(set(bounds)):return None
 return {'stints':[{'tire_id':t,'laps':bounds[i+1]-bounds[i]} for i,t in enumerate(tires)],'pit_laps':list(pits)}
def rank(rows):
 return sorted(rows,key=lambda r:(not r['data']['outcome']['ok'],-len(r['data']['lap_times_s']),r['data']['outcome'].get('time_s',float('inf')),r['case']['actor']))

class Kernel:
 def __init__(self,binary,cache,workers=8):
  self.binary=pathlib.Path(binary).resolve();self.cache=pathlib.Path(cache).resolve();self.cache.mkdir(parents=True,exist_ok=True);self.workers=workers;self.tmp=tempfile.TemporaryDirectory(prefix='pitgun-tire-performance-')
  paths=['Cargo.toml','Cargo.lock','experiments/tire_performance_v1/kernel.py','experiments/tire_performance_v1/profile-candidates-v1.json','experiments/fixed_opponents_v2/roster-v2.json','experiments/career_physics_v1/profile-v1.json','experiments/opponent_physics_v2/results/career-fixtures-v1.json']
  paths+=sorted(str(p.relative_to(ROOT)) for p in (ROOT/'crates').rglob('*.rs'))
  paths+=sorted(str(p.relative_to(ROOT)) for p in (ROOT/'crates').rglob('Cargo.toml'))
  if (ROOT/'.cargo/config.toml').exists():paths.append('.cargo/config.toml')
  paths+=sorted(str(p.relative_to(ROOT)) for p in (ROOT/'catalogs/racing/v1.8.1').rglob('*.json'))
  self.pin={'schema':'pitgun.tire-performance-kernel-cache/v1','binary_digest':sha(self.binary.read_bytes()),'sources':{p:sha((ROOT/p).read_bytes()) for p in paths}}
  self.profile_digest=sha((EXP/'profile-candidates-v1.json').read_bytes())
  pin=self.cache/'kernel-pin-v1.json'
  if pin.exists() and load(pin)!=self.pin:raise RuntimeError('immutable kernel cache pin mismatch')
  write(pin,self.pin)

 def verify(self):
  if sha(self.binary.read_bytes())!=self.pin['binary_digest']:raise RuntimeError('native binary changed')
  if {p:sha((ROOT/p).read_bytes()) for p in self.pin['sources']}!=self.pin['sources']:raise RuntimeError('physical source changed')

 def execute(self,c,force=False):
  c=normalize(c);k=key(c);p=self.cache/(k+'.json.gz')
  if p.exists() and not force:
   d=load(p)
   if d['id']!=k or d['binary_digest']!=self.pin['binary_digest'] or d['receipt']['performance_profile_set_digest']!=self.profile_digest:raise RuntimeError('cached identity mismatch')
   return d
  f=pathlib.Path(self.tmp.name)/(k+'.json');write(f,c)
  try:d=json.loads(subprocess.check_output([str(self.binary),str(ROOT),str(f)],stderr=subprocess.PIPE))
  except subprocess.CalledProcessError as e:raise RuntimeError('native resolution failure:'+e.stderr.decode()) from e
  if d['id']!=k or d['binary_digest']!=self.pin['binary_digest'] or d['receipt']['performance_profile_set_digest']!=self.profile_digest:raise RuntimeError('native identity mismatch')
  if p.exists() and load(p)!=d:raise RuntimeError('exact fresh repeat differs')
  atomic=p.with_name(k+f'.{os.getpid()}.{threading.get_ident()}.tmp');atomic.write_bytes(gzip.compress(enc(d),mtime=0));atomic.replace(p)
  return d

 def batch(self,name,cases,out,force=False):
  cases=list({key(c):c for c in map(normalize,cases)}.values());cached=0 if force else sum((self.cache/(key(c)+'.json.gz')).exists() for c in cases);rows=[]
  with cf.ThreadPoolExecutor(max_workers=self.workers) as pool:
   for c,d in zip(cases,pool.map(lambda c:self.execute(c,force),cases)):
    rows.append({'case':c,'data':d,'origin':'native_current'})
    if len(rows)%100==0:print(f'{name}: {len(rows)}/{len(cases)}',flush=True)
  out=pathlib.Path(out);out.mkdir(exist_ok=True);write(out/(name+'-v1.json.gz'),rows);stats={'requests':len(cases),'cached':cached,'fresh':len(cases)-cached};print(name+' complete:'+str(stats),flush=True);return rows,stats

 def close(self):self.tmp.cleanup()
