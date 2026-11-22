#!/usr/bin/env python3
"""Fresh native equivalence across the prior100 whole-GP control plans."""
import pathlib
import sys
from kernel import EXP, ROOT, Kernel, change, key, load, sha, write

def main():
 out=EXP/'results';out.mkdir(exist_ok=True);source=ROOT/'experiments/pit_strategy_audit_v1/results';receipt=load(source/'campaign-v1.json')
 for p,d in receipt['files'].items():
  if sha((source/p).read_bytes())!=d:raise RuntimeError('retained source receipt changed:'+p)
 old=[r for r in load(source/'paired-ablation-v1.json.gz') if r['case']['audit'] is None]
 cases=[change(r['case'],performance_profile=None,diagnostic_mode=len(r['case']['strategy']['pit_laps'])>2,diagnostic_laps=None,diagnostic_driving_mode=None) for r in old];expected={key(c):r['data'] for c,r in zip(cases,old)}
 kernel=Kernel(sys.argv[1],sys.argv[2]);rows,stats=kernel.batch('legacy-equivalence',cases,out)
 for r in rows:
  d=r['data'];prior=expected[key(r['case'])]
  for field in ['physical_input_digest','diagnostic_input_digest','lap_times_s','outcome','maximum_carry_jumps','thermal_ledger','metrics','lap_metrics']:
   if prior[field]!=d[field]:raise RuntimeError('legacy equality failed:'+field)
 kernel.verify();write(out/'legacy-equivalence-receipt-v1.json',{'schema':'pitgun.tire-performance-legacy-equivalence/v1','source_campaign_digest':sha((source/'campaign-v1.json').read_bytes()),'exact_controls':len(rows),'phase_stats':stats,'kernel_pin':kernel.pin});kernel.close();print('legacy source controls exact:',len(rows),flush=True)

if __name__=='__main__':main()
