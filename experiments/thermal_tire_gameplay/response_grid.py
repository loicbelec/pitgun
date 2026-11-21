#!/usr/bin/env python3
"""Coarse constrained allocation audit; does not optimize model coefficients."""
import argparse,copy,importlib.util,itertools,json,pathlib
HERE=pathlib.Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('audit',HERE/'run_local.py');r=importlib.util.module_from_spec(spec);spec.loader.exec_module(r)
def cases(plan,template):
 for group in plan['groups']:
  for a,ch,e,c in itertools.product(range(0,min(group['budget'],plan['axis_cap'])+1,group['step']),repeat=4):
   if a+ch+e+c!=group['budget']:continue
   case=copy.deepcopy(template);case['id']=f"{group['id']}-a{a}-ch{ch}-e{e}-c{c}";req=case['request'];req['seed']=plan['seed'];req['era']=group['era'];req['input'].update({'vehicle_id':group['vehicle'],'era':group['era'],'track_id':group['track'],'laps':plan['laps']});car=req['input']['competitors'][0];car['budget_cap']=group['budget'];car['stint_strategy']={'stints':[{'tire_id':'medium','laps':plan['laps']}],'pit_laps':[]};car['tuning']={'aero_points':a,'chassis_points':ch,'engine_points':e,'cooling_points':c,'downforce_slider':plan['downforce'],'gear_ratio_slider':plan['gearing']};case['overrides']={};case['group_id']=group['id'];yield case

def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--probe',type=pathlib.Path,required=True);p.add_argument('--check',action='store_true');a=p.parse_args();probe=a.probe.resolve(strict=True)
 path=HERE/'results/budget-grid-v1.json';plan_path=HERE/'grid-plan-v1.json';plan=r.b.load(plan_path);base_path=HERE/'results/local-gameplay-v1.json';base=r.b.load(base_path);identity=base['identity'];template=next(c for c in r.b.load(HERE/'plan-v1.json')['cases'] if c['id']=='setup-base');all_cases=list(cases(plan,template));rows=[];best={};winners=[]
 for index,c in enumerate(all_cases):
  for model in plan['models']:
   data=r.execute(probe,c,model,identity,180)['data'];o=data['outcome'];tuning=c['request']['input']['competitors'][0]['tuning'];row={'case_id':c['id'],'group':c['group_id'],'model':model,'tuning':tuning,'applied_input_digest':data['applied_input_digest'],'request_digest':data['request_digest'],'outcome':o};rows.append(row)
   if o['ok']:
    category='zero' if tuning['cooling_points']==0 else 'positive';key=(c['group_id'],model,category)
    if key not in best or o['total_time_s']<best[key][1]['outcome']['total_time_s']:best[key]=(c,data)
  if index%20==0:print('GRID',index+1,'/',len(all_cases),flush=True)
 for key,(c,data) in best.items():
  repeated=r.execute(probe,c,key[1],identity,180)['data']
  if data!=repeated:raise ValueError('selected full native output changed on repetition')
  winners.append({'group':key[0],'model':key[1],'category':key[2],'case_id':c['id'],'request':c['request'],'full_data':data,'exact_winner_repeated':True})
 comparisons=[]
 for group in plan['groups']:
  for model in plan['models']:
   z=best.get((group['id'],model,'zero'));positive=best.get((group['id'],model,'positive'));comparisons.append({'group':group['id'],'model':model,'best_zero_case':z[0]['id'] if z else None,'best_positive_case':positive[0]['id'] if positive else None,'positive_minus_zero_s':positive[1]['outcome']['total_time_s']-z[1]['outcome']['total_time_s'] if z and positive else None})
 report={'schema':plan['schema'],'plan':plan,'plan_digest':r.b.digest(plan_path.read_bytes()),'runner_digest':r.b.digest(pathlib.Path(__file__).read_bytes()),'base_report_digest':r.b.digest(base_path.read_bytes()),'identity':identity,'allocations':len(all_cases),'rows':rows,'winners':winners,'comparisons':comparisons,'failures':[{'case_id':v['case_id'],'model':v['model'],'error':v['outcome']['error']} for v in rows if not v['outcome']['ok']]}
 if a.check:
  if report!=r.b.load(path):raise ValueError('grid changed; archive untouched')
  print('PASS full grid recheck')
 else:
  raw=r.b.encode(report);path.write_bytes(raw);path.with_suffix('.sha256').write_text(r.b.digest(raw)+'\n')
 print('PASS all selected minima repeated exactly')
if __name__=='__main__':main()
