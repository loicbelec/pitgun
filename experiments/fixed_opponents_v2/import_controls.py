#!/usr/bin/env python3
"""Audited V1 reference/player reuse; named-driver calculations are never imported."""
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
OLD=ROOT/'experiments/fixed_opponents_v1/results'

def enc(d):return json.dumps(d,sort_keys=True,separators=(',',':'),allow_nan=False,ensure_ascii=False).encode()
def sha(b):return 'sha256:'+hashlib.sha256(b).hexdigest()
def key(c):return hashlib.sha256(enc({k:v for k,v in c.items() if k!='id'})).hexdigest()
def load(p):
 b=p.read_bytes();return json.loads(gzip.decompress(b) if p.suffix=='.gz' else b)

def main():
 binary=pathlib.Path(sys.argv[1]).resolve();cache=pathlib.Path(sys.argv[2]).resolve();cache.mkdir(parents=True,exist_ok=True)
 if list(cache.iterdir()):raise RuntimeError('import requires an empty V2 cache')
 prov=load(OLD/'provenance-v1.json')
 for p,d in prov['sources'].items():
  if sha((ROOT/p).read_bytes())!=d:raise RuntimeError('V1 frozen source changed:'+p)
 rows={};source_files={}
 for name in ['setup-search','reference-local','strategy-search','player-training','player-local-1','player-local-2']:
  p=OLD/(name+'-v1.json.gz');source_files[str(p.relative_to(ROOT))]=sha(p.read_bytes())
  for r in load(p):
   if r['case']['actor'] not in ['reference','player']:raise RuntimeError('refuse named-driver import')
   rows[key(r['case'])]=r
 binary_digest=sha(binary.read_bytes());roster_digest=sha((EXP/'roster-v2.json').read_bytes())
 def rebound(r):
  d=copy.deepcopy(r['data']);d['binary_digest']=binary_digest;d['receipt']['roster_digest']=roster_digest;return d
 # One complete reference GP per context, plus actual player stages on Monza.
 controls={}
 for k,r in sorted(rows.items()):
  c=r['case'];group=(c['actor'],c['fixture_id'],c['circuit'])
  if c['actor']=='reference' or c['circuit']=='MONZA':controls.setdefault(group,r)
 with tempfile.TemporaryDirectory(prefix='pitgun-field-import-') as td:
  def check(r):
   c=r['case'];p=pathlib.Path(td)/(key(c)+'.json');p.write_bytes(enc(c))
   actual=json.loads(subprocess.check_output([str(binary),str(ROOT),str(p)],stderr=subprocess.PIPE))
   if actual!=rebound(r):raise RuntimeError('V2 native control differs from rebound V1 receipt:'+key(c))
   return {'case':c,'physical_input_digest':actual['physical_input_digest'],'result_digest':actual['outcome'].get('result_digest'),'all_non_wrapper_fields_exact':True}
  with cf.ThreadPoolExecutor(max_workers=8) as pool:checked=list(pool.map(check,controls.values()))
 imported=[]
 for k,r in sorted(rows.items()):
  original=enc(r['data']);d=rebound(r)
  (cache/(k+'.json.gz')).write_bytes(gzip.compress(enc(d),mtime=0))
  imported.append({'case_key':k,'actor':r['case']['actor'],'source_data_digest':sha(original),'rebound_data_digest':sha(enc(d)),'physical_input_digest':d['physical_input_digest'],'diagnostic_input_digest':d['diagnostic_input_digest'],'result_digest':d['outcome'].get('result_digest')})
 manifest={'schema':'pitgun.fixed-field-control-import/v1','source_provenance':prov,'source_files':source_files,'new_binary_digest':binary_digest,'new_roster_digest':roster_digest,'wrapper_fields_rebound':['binary_digest','receipt.roster_digest'],'new_native_control_gp_executions':len(checked),'controls':checked,'imported':imported,'named_driver_imports':0}
 (cache/'import-manifest.json').write_text(json.dumps(manifest,indent=2,ensure_ascii=False)+'\n')
 print(json.dumps({'native_controls':len(checked),'imported_unchanged_physical_cases':len(imported),'named_driver_imports':0},indent=2))

if __name__=='__main__':main()
