#!/usr/bin/env python3
"""Deterministic preregistered scenarios; physical point resolution stays in Rust."""
import copy,json,pathlib,sys
HERE=pathlib.Path(__file__).resolve().parent
cases=[]
def add(id,group,vehicle='modern_v6t',era=5,track='MONZA',laps=5,engine=8,cooling=4,aero=4,chassis=4,tire='medium',seed=42,df=.5,gear=.5,pits=None,stints=None,driver='default',overrides=None,offset=0,meta=None):
    raw={'engine_points':engine,'cooling_points':cooling,'aero_points':aero,'chassis_points':chassis,'downforce_slider':df,'gear_ratio_slider':gear}
    effective=copy.deepcopy(raw);effective['cooling_points']+=offset
    budget=sum(effective[k] for k in ['engine_points','cooling_points','aero_points','chassis_points'])
    assert all(0<=effective[k]<=20 for k in ['engine_points','cooling_points','aero_points','chassis_points'])
    strategy={'stints':stints or [{'tire_id':tire,'laps':laps}],'pit_laps':pits or []}
    assert sum(s['laps'] for s in strategy['stints'])==laps
    request={'input':{'track_id':track,'laps':laps,'vehicle_id':vehicle,'era':era,'hz':5.,'pit_strategy':{'player_pit_laps':pits or []},'competitors':[{'id':'player','name':'Audit driver','driver_id':driver,'team_id':'pitgun-lab','is_player':True,'tuning':effective,'budget_cap':budget,'stint_strategy':strategy}]},'seed':seed,'era':era,'hz':5.}
    cases.append({'id':id,'group':group,'raw_tuning':raw,'cooling_offset':offset,'metadata':meta or {},'request':request,'overrides':overrides or {}})
# Finite response lines at equal point budget. These are not an exhaustive optimum search.
for name,vehicle,era,track,aero,chassis,engine,cs in [
 ('early8','classic_v8_1960',1,'MONZA',1,1,6,[0,1,2]),
 *[('classic15','classic_v8_1970',2,t,0,8,7,[0,1,2,4]) for t in ['MONZA','MONTREAL']],
 *[('modern20','modern_v6t',5,t,4,4,12,[0,2,4,6]) for t in ['MONZA','SILVERSTONE','BARCELONA']],
 *[('hybrid40','f1_2026',5,t,10,10,20,[0,2,5,10]) for t in ['MONZA','BARCELONA']],
 ('classic40','classic_v8_1970',2,'MONZA',10,10,20,[0,2,5,10])]:
 for c in cs:add(f'budget-{name}-{track.lower()}-c{c}','equal_budget',vehicle,era,track,engine=engine-c,cooling=c,aero=aero,chassis=chassis,meta={'series':name+'-'+track,'donor':'engine'})
for c in [0,1,2,4]:add(f'budget-modern20-chassis-c{c}','equal_budget',engine=12,cooling=c,aero=4,chassis=4-c,meta={'series':'modern20-MONZA-chassis','donor':'chassis'})
for c in [0,2,4,6]:add(f'isolate-cooling-c{c}','isolated_cooling',engine=12,cooling=c,meta={'series':'fixed-engine-MONZA','not_equal_budget':True})
for vehicle,era,name in [('classic_v8_1970',2,'classic15'),('modern_v6t',5,'modern20')]:
 for c in [0,2]:add(f'full-{name}-c{c}','full_distance',vehicle,era,laps=53,engine=(7 if era==2 else 12)-c,cooling=c,aero=0 if era==2 else 4,chassis=8 if era==2 else 4,pits=[26],stints=[{'tire_id':'medium','laps':26},{'tire_id':'hard','laps':27}],meta={'fuel_failure_must_remain_explicit':True})
for c in [0,4]:add(f'reserved-silverstone-c{c}','reserved',track='SILVERSTONE',engine=12-c,cooling=c,seed=17)
for c in [0,2]:add(f'bonus-classic15-c{c}','hq_offset',vehicle='classic_v8_1970',era=2,track='MONTREAL',engine=7-c,cooling=c,aero=0,chassis=8,offset=1)
for c in [0,4]:add(f'hot-modern-c{c}','environment_diagnostic',engine=12-c,cooling=c,overrides={'air_temperature_c':45.})
for name,vehicle,era,a,ch,e in [('classic','classic_v8_1970',2,0,8,7),('modern','modern_v6t',5,4,4,12),('hybrid','f1_2026',5,10,10,20)]:
 for c in ([0,5] if name=='hybrid' else [0,4]):add(f'no-derating-{name}-c{c}','ablation_derating',vehicle,era,engine=e-c,cooling=c,aero=a,chassis=ch,overrides={'disable_derating':True})
for t in ['soft','medium','hard']:add('tires-'+t,'compound_stint',track='BARCELONA',laps=20,tire=t)
for d in ['conservative','default','aggressive']:add('driver-'+d,'driver_stint',track='BARCELONA',laps=20,driver=d)
for t in [35.,110.]:add('tire-start-'+str(int(t)),'initial_temperature_diagnostic',track='BARCELONA',laps=20,overrides={'initial_tire_temperature_c':t})
add('no-tire-temperature-feedback','ablation_tires',track='BARCELONA',laps=20,overrides={'disable_tire_temperature_feedback':True})
add('no-tire-wear-grip','ablation_tires',track='BARCELONA',laps=20,overrides={'disable_tire_wear_grip':True})
add('soft-medium-pit10','pit_strategy',track='BARCELONA',laps=20,pits=[10],stints=[{'tire_id':'soft','laps':10},{'tire_id':'medium','laps':10}])
for lap in [5,10,15]:add(f'medium-soft-pit{lap}','pit_strategy',track='BARCELONA',laps=20,pits=[lap],stints=[{'tire_id':'medium','laps':lap},{'tire_id':'soft','laps':20-lap}])
for name,changes in [('base',{}),('aero',{'engine':6,'aero':6}),('chassis',{'engine':6,'chassis':6}),('cooling',{'engine':6,'cooling':6}),('engine',{'engine':10,'cooling':2}),('df02',{'df':.2}),('df08',{'df':.8}),('gear03',{'gear':.3}),('gear074',{'gear':.74})]:add('setup-'+name,'setup_response',track='BARCELONA',**changes)
plan={'schema':'pitgun.thermal-tire-gameplay-plan/v1','catalog':'catalogs/racing/v1.8.1','models':['historical','energy'],'seed_note':'42 exposed response lines;17 reserved Silverstone check, not used to select coefficients. No fitting authorized by this audit.','timeout_s':180,'comparison_margin_s':.05,'cases':cases,'acceptance':'Audit reproducibility is distinct from gameplay PASS. Report zero-cooling dominance per sampled equal-budget response line, resolved derating and diagnostic on/off causality, tire thermal-floor occupancy/temperature/wear/compound/stop-order sensitivity and explicit failures. Do not declare a global optimum or force positive cooling in every scenario. No equation/catalog/profile modification.','scope':'Single native car, actual career-V9 physical resolution, historical and unregistered energy. Synthetic effective budgets8/15/20/40, no claim of earned HQ state or deployed binary replay. +1 cooling offset is a labeled synthetic bonus, not a full career reconstruction. Ablations/air/initial-temperature overrides are diagnostic, not playable controls.'}
assert len({c['id'] for c in cases})==len(cases)
text=json.dumps(plan,indent=2)+'\n'
if '--check' in sys.argv:
 assert (HERE/'plan-v1.json').read_text()==text,'changed scenario generator/plan'
else:(HERE/'plan-v1.json').write_text(text)
print(len(cases),'cases',len(cases)*2,'native runs per pass')
