#!/usr/bin/env python3
"""Frozen paired full-GP physical screen; profile selection precedes field fitting."""
import collections
import itertools
import math
import pathlib
import subprocess
import sys
from kernel import EXP, ROOT, Kernel, change, enc, key, load, normalize, sha, strategy, write

def recipes(n):
 legal={};extra={}
 for tires in itertools.product(['soft','medium','hard'],repeat=2):
  if tires[0]==tires[1]:continue
  for lap in [int(n*f) for f in [.25,.35,.5,.65,.75]]+[1,n-1]:
   s=strategy(n,tires,[lap])
   if s:legal[sha(enc(s))]=s
 for tires in itertools.product(['soft','medium','hard'],repeat=3):
  if tires[0]==tires[1]:continue
  for pits in [(int(n*.25),int(n*.55)),(n//3,2*n//3),(int(n*.4),int(n*.75)),(n//2,n//2+1)]:
   s=strategy(n,tires,pits)
   if s:legal[sha(enc(s))]=s
 for stops in [3,4,5,6,7,8,12]:
  for first,second in itertools.permutations(['soft','medium','hard'],2):
   tires=[first]+[second]*stops
   for pits in [tuple(n*i//(stops+1) for i in range(1,stops+1)),tuple(1+(n-1)*i//stops for i in range(stops))]:
    s=strategy(n,tires,pits)
    if s:extra[sha(enc(s))]=s
 return list(legal.values()),list(extra.values())

def main():
 plan=load(EXP/'plan-v1.json');source_path=ROOT/'experiments/fixed_opponents_v2/results/strategy-search-v2.json.gz';source=load(source_path);fixtures={f['id']:f for f in load(ROOT/'experiments/opponent_physics_v2/results/career-fixtures-v1.json')['fixtures']};out=EXP/'results';out.mkdir(exist_ok=True)
 if (out/'screen-campaign-v1.json').exists():raise RuntimeError('refuse overwrite completed physical screen')
 anchors={}
 for era,circuit in plan['training_contexts']:
  rs=[r for r in source if fixtures[r['case']['fixture_id']]['era']==era and r['case']['circuit']==circuit and r['data']['outcome']['ok']];anchors[(era,circuit)]=min(rs,key=lambda r:(r['data']['outcome']['time_s'],key(r['case'])))['case']
 if sys.argv[1:]==['--preview']:
  print({str(ck):{'legal':len(recipes(sum(s['laps'] for s in c['strategy']['stints']))[0]),'diagnostic':len(recipes(sum(s['laps'] for s in c['strategy']['stints']))[1])} for ck,c in anchors.items()});return
 if not (out/'legacy-equivalence-receipt-v1.json').exists():raise RuntimeError('100fresh legacy-equivalence controls required first')
 kernel=Kernel(sys.argv[1],sys.argv[2],plan['workers']);sources={str(p.relative_to(ROOT)):sha(p.read_bytes()) for p in [EXP/'plan-v1.json',EXP/'run_screen.py',source_path]};provenance={'source_commit':subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),'kernel_pin':kernel.pin,'screen_sources':sources};write(out/'screen-provenance-v1.json',provenance)
 if load(out/'legacy-equivalence-receipt-v1.json')['kernel_pin']!=kernel.pin:raise RuntimeError('legacy equivalence belongs to a different physical kernel')
 cases=[]
 for ck,c in anchors.items():
  legal,extra=recipes(sum(s['laps'] for s in c['strategy']['stints']))
  for profile in plan['profiles']:
   cases += [change(c,strategy=s,performance_profile=profile,diagnostic_mode=False) for s in legal]
   cases += [change(c,strategy=s,performance_profile=profile,diagnostic_mode=True) for s in extra]
 rows,stats=kernel.batch('profile-screen',cases,out);groups=collections.defaultdict(list)
 for r in rows:
  c=r['case'];groups[(c['performance_profile'],fixtures[c['fixture_id']]['era'],c['circuit'])].append(r)
 decisions=[]
 profiles=load(EXP/'profile-candidates-v1.json')['profiles'];prior_heat=load(ROOT/'experiments/career_physics_v1/profile-v1.json')['compounds']
 for profile in plan['profiles']:
  invariants={}
  for name,compound in profiles[profile]['compounds'].items():
   curve=compound['wear_curve'];wear20=1-curve['linear_loss']*.2-curve['late_life_loss']*.2**curve['late_life_power'];end=1-curve['linear_loss']-curve['late_life_loss'];temp20=math.exp(-(20/compound['temperature_width_c'])**2);same_wear_temperature=math.isclose(compound['thermal_wear_gain']/compound['temperature_width_c']**2,prior_heat[name]['heat']['thermal_wear_gain']/prior_heat[name]['temperature_width_c']**2,rel_tol=1e-12)
   invariants[name]={'direct_wear_grip_at_raw20percent':wear20,'direct_wear_grip_at_raw100percent':end,'direct_temperature_grip_at20C_offset':temp20,'wear_temperature_function_preserved':same_wear_temperature,'passed':wear20>=.98 and abs(end-.8)<1e-12 and temp20>=.97 and same_wear_temperature}
  contexts=[]
  for ck in anchors:
   rs=groups[(profile,)+ck];good=[r for r in rs if r['data']['outcome']['ok']];legal=[r for r in good if not r['case']['diagnostic_mode']];extra=[r for r in good if r['case']['diagnostic_mode']];carry=max(max(r['data']['maximum_carry_jumps']) for r in rs)
   if not legal or not extra:
    contexts.append({'context':list(ck),'eligible':False,'failed':len(rs)-len(good),'maximum_carry_jump':carry,'reason':'no successful legal/extra-stop comparison'});continue
   best=min(legal,key=lambda r:r['data']['outcome']['time_s']);shadow=min(extra,key=lambda r:r['data']['outcome']['time_s']);passed=len(good)==len(rs) and carry==0 and best['data']['outcome']['time_s']<=shadow['data']['outcome']['time_s'];contexts.append({'context':list(ck),'eligible':passed,'failed':len(rs)-len(good),'maximum_carry_jump':carry,'best_legal_case':best['case'],'best_legal_time_s':best['data']['outcome']['time_s'],'best_extra_case':shadow['case'],'extra_stops_margin_s':shadow['data']['outcome']['time_s']-best['data']['outcome']['time_s']})
  decisions.append({'profile':profile,'eligible':all(c['eligible'] for c in contexts) and all(c['passed'] for c in invariants.values()),'curve_invariants':invariants,'contexts':contexts})
 write(out/'profile-decisions-v1.json',decisions);feasible=[d for d in decisions if d['eligible']]
 if not feasible:
  kernel.verify();write(out/'screen-refine-v1.json',{'status':'REFINE','phase_stats':stats,'no_profile_selected':True,'provenance':provenance});raise RuntimeError('REFINE: no global profile meets physical stop-incentive targets')
 selected=feasible[0];write(out/'profile-selection-lock-v1.json',{'status':'NATIVE_PHYSICAL_SCREEN_CANDIDATE','profile':selected['profile'],'training_choice':selected,'profiles_digest':kernel.profile_digest,'before_field_reserved_results':True})
 modes=[]
 for c in selected['contexts']:
  base=c['best_legal_case']
  modes += [change(base,diagnostic_mode=True,diagnostic_driving_mode=mode) for mode in ['manage','balanced','attack']]
 mode_rows,mode_stats=kernel.batch('driving-modes',modes,out)
 for r in mode_rows:
  if r['case']['diagnostic_driving_mode']!='balanced':continue
  control=kernel.execute(change(r['case'],diagnostic_mode=False,diagnostic_driving_mode=None))
  if r['data']['physical_input_digest']!=control['physical_input_digest'] or r['data']['outcome']!=control['outcome']:raise RuntimeError('explicit/default Balanced differs')
 base=anchors[(1,'MONZA')];short=[]
 for profile in [None,selected['profile']]:
  for allocation in [[0,4,0,0],[0,4,0,1],[0,4,4,0],[0,4,4,1]]:
   short.append(change(base,performance_profile=profile,allocation=allocation,budget=sum(allocation),gear=.5,downforce=.5,diagnostic_mode=True,diagnostic_laps=5,strategy={'stints':[{'tire_id':'medium','laps':5}],'pit_laps':[]}))
 short_rows,short_stats=kernel.batch('five-lap-cooling',short,out)
 repeats=[c['best_legal_case'] for c in selected['contexts']];repeated,repeat_stats=kernel.batch('selected-exact-repeat',repeats,out,True)
 kernel.verify()
 if {p:sha((ROOT/p).read_bytes()) for p in sources}!=sources:raise RuntimeError('screen protocol/source data changed')
 write(out/'screen-campaign-v1.json',{'schema':'pitgun.tire-performance-screen-receipt/v1','profile':selected['profile'],'phase_stats':{'profile-screen':stats,'driving-modes':mode_stats,'five-lap-cooling':short_stats,'selected-exact-repeat':repeat_stats},'provenance':provenance,'files':{p.name:sha(p.read_bytes()) for p in out.glob('*-v1.*') if p.name not in ['screen-campaign-v1.json']}});kernel.close();print('physical screen complete; native candidate only',flush=True)

if __name__=='__main__':main()
