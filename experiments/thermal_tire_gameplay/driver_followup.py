#!/usr/bin/env python3
"""Separate diagnostic after discovering strategy labels resolve to default V9."""
import argparse,importlib.util,json,pathlib
HERE=pathlib.Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('audit',HERE/'run_local.py');r=importlib.util.module_from_spec(spec);spec.loader.exec_module(r)
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--probe',type=pathlib.Path,required=True);p.add_argument('--check',action='store_true');a=p.parse_args()
 plan_path=HERE/'driver-supplement-plan-v1.json';plan=r.b.load(plan_path);base=r.b.load(HERE/'results/local-gameplay-v1.json');identity=base['identity'];rows=[]
 for c in plan['cases']:
  for model in ['historical','energy']:rows.append(r.execute(a.probe.resolve(strict=True),c,model,identity,180))
 report={'schema':plan['schema'],'plan_digest':r.b.digest(plan_path.read_bytes()),'runner_digest':r.b.digest(pathlib.Path(__file__).read_bytes()),'base_report_digest':r.b.digest((HERE/'results/local-gameplay-v1.json').read_bytes()),'identity':identity,'results':rows,'reason':plan['reason']}
 path=HERE/'results/driver-followup-v1.json'
 if a.check:
  if r.stable(report)!=r.stable(r.b.load(path)):raise ValueError('followup repeat differs')
  print('PASS driver followup repeated exactly')
 else:
  raw=r.b.encode(report);path.write_bytes(raw);path.with_suffix('.sha256').write_text(r.b.digest(raw)+'\n')
if __name__=='__main__':main()
