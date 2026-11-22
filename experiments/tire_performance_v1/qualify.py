#!/usr/bin/env python3
"""Physical parity against prior full-GP receipts and strict boundary checks."""
import gzip
import json
import pathlib
import subprocess
import sys
import tempfile

EXP=pathlib.Path(__file__).resolve().parent;ROOT=EXP.parents[1]

def main():
 binary=str(pathlib.Path(sys.argv[1]).resolve());old=json.loads(gzip.decompress((ROOT/'experiments/career_physics_v1/results/confirmation-v1.json.gz').read_bytes()));fixtures={f['id']:f for f in json.loads((ROOT/'experiments/opponent_physics_v2/results/career-fixtures-v1.json').read_text())['fixtures']};controls=[]
 with tempfile.TemporaryDirectory(prefix='pitgun-field-qualification-') as td:
  p=pathlib.Path(td)/'case.json'
  def run(c):
   p.write_text(json.dumps(c));return subprocess.run([binary,str(ROOT),str(p)],capture_output=True,text=True)
  for stage in ['era1-entry','era2-exit']:
   row=next(r for r in old if r['case']['fixture_id'].endswith(stage) and r['case']['circuit']=='MONZA' and r['case']['forecast']=='reachable_wear_v1');a=row['case'];f=fixtures[a['fixture_id']];prep=next(x for x in f['preparedRequests'] if x['circuitId']=='MONZA')['request'];s=next(x['stint_strategy'] for x in prep['input']['competitors'] if x['is_player']);case={k:a[k] for k in ['id','fixture_id','circuit','allocation','gear','downforce']};case.update(actor='player',kind='career_player',budget=f['discretionaryPoints'],strategy=s,seed=prep['seed'],environment=None,elite_level=0)
   r=run(case)
   if r.returncode:raise RuntimeError(r.stderr)
   d=json.loads(r.stdout)
   assert d['physical_input_digest']==row['data']['physical_input_digest']
   assert d['diagnostic_input_digest']==row['data']['diagnostic_input_digest']
   assert d['outcome']['result_digest']==row['data']['outcome']['result_digest']
   assert d['lap_times_s']==[l['time_s'] for l in row['data']['laps']]
   assert max(d['maximum_carry_jumps'])==0
   controls.append({'input_digest':d['physical_input_digest'],'result_digest':d['outcome']['result_digest']})
   if stage=='era1-entry':base=case
  bad=[dict(base,budget=1),dict(base,actor='unknown',kind='field'),dict(base,downforce=0),dict(base,gear=1.01),dict(base,elite_level=3),dict(base,kind='unknown'),dict(base,strategy={'stints':[{'tire_id':'soft','laps':26},{'tire_id':'soft','laps':27}],'pit_laps':[26]})]
  bad += [dict(base,audit=a) for a in [
   {'kind':'wear_grip_scale','factor':-.1},
   {'kind':'wear_energy_scale','factor':0},
   {'kind':'mount_temperature','celsius':111},
   {'kind':'pit_penalty','seconds':181},
   {'kind':'unknown'}]]
  n=sum(x['laps'] for x in base['strategy']['stints'])
  bad += [dict(base,diagnostic_driving_mode='attack'),dict(base,performance_profile='unknown'),dict(base,strategy={'stints':[{'tire_id':'soft','laps':1},{'tire_id':'medium','laps':1},{'tire_id':'soft','laps':1},{'tire_id':'hard','laps':n-3}],'pit_laps':[1,2,3]}),dict(base,diagnostic_laps=5,strategy={'stints':[{'tire_id':'soft','laps':2},{'tire_id':'medium','laps':3}],'pit_laps':[2]})]
  for c in bad:
   r=run(c)
   if r.returncode==0:raise RuntimeError('invalid case accepted')
  print(json.dumps({'retained_full_gp_controls_exact':controls,'strict_rejections':len(bad),'legacy_controls_exact_optional_curve_not_admitted':True},indent=2))

if __name__=='__main__':main()
