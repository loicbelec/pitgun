#!/usr/bin/env python3
"""Bounded exploratory screen. Exposed anchors are never called held-out data."""
import copy,json,pathlib,sys
HERE=pathlib.Path(__file__).resolve().parent
old=json.loads((HERE.parent/'thermal_tire_gameplay/plan-v1.json').read_text())
cases=[]
for c in old['cases']:
 if c['group'] in ['equal_budget','compound_stint','pit_strategy','setup_response','full_distance','environment_diagnostic','initial_temperature_diagnostic']:
  if c['metadata'].get('series','').startswith(('hybrid','classic40','modern20-MONZA-chassis')):continue
  cases.append({'id':c['id'],'group':c['group'],'request':c['request'],'overrides':c['overrides']})
for prefix in ['budget-early8-monza-c0','budget-classic15-montreal-c0','budget-modern20-monza-c0','budget-modern20-monza-c4']:
 base=next(c for c in cases if c['id']==prefix)
 c=copy.deepcopy(base);c['id']='ablation-'+prefix;c['group']='disable_derating';c['overrides']={'disable_derating':True};c['reference']=prefix;cases.append(c)
base=next(c for c in cases if c['id']=='budget-early8-monza-c0')
for cooling in [1,2,4]:
 c=copy.deepcopy(base);c['id']='isolated-early-c'+str(cooling);c['group']='isolated_cooling';comp=c['request']['input']['competitors'][0];comp['tuning']['cooling_points']=cooling;comp['budget_cap']+=cooling;cases.append(c)
for n in ['franz_hermann','battery_voltas']:
 c=copy.deepcopy(next(c for c in cases if c['id']=='tires-medium'));c['id']='driver-'+n;c['group']='physical_driver';c['request']['input']['competitors'][0]['driver_id']=n;cases.append(c)
assert len({c['id'] for c in cases})==len(cases)
plan={'schema':'pitgun.thermal-physics-screen/v2','status':'EXPLORATORY_CALIBRATION_NOT_ACCEPTANCE','catalog':old['catalog'],'models':[False,True],'cases':cases,'timeout_s':180,'scope':'Native single car, synthetic effective budgets8/15/20; no earned career reconstruction, native/WASM admission or experimental physical validation. Default100kg preserved: fuel failures must remain explicit. All anchor conditions are exposed. V2 uses a conservative minimum-grip seam forecast, so cross-model time deltas are not pure thermal effects.','criteria':['Repeat every case/model exactly excluding process measurements.','Check chemical/thermal/mechanical ledgers and serialize every failure.','Quantify equal-budget cooling, fixed-engine cooling and causal derating ablations separately.','Show both surface heating/cooling and carcass memory, compound short/long tradeoffs.','Do not claim that zero cooling can never win or that all strategy/setup parameters are balanced.']}
text=json.dumps(plan,indent=2)+'\n'
if '--check' in sys.argv:assert (HERE/'plan-v1.json').read_text()==text
else:(HERE/'plan-v1.json').write_text(text)
print(len(cases),'cases,',len(cases)*2,'runs per pass')
