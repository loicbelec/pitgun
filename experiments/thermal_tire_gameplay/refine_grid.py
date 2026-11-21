#!/usr/bin/env python3
import argparse,copy,importlib.util,pathlib
HERE=pathlib.Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('audit',HERE/'run_local.py');r=importlib.util.module_from_spec(spec);spec.loader.exec_module(r)
def main():
 p=argparse.ArgumentParser();p.add_argument('--probe',type=pathlib.Path,required=True);p.add_argument('--check',action='store_true');a=p.parse_args();probe=a.probe.resolve(strict=True)
 plan_path=HERE/'refinement-plan-v1.json';plan=r.b.load(plan_path);grid_path=HERE/'results/budget-grid-v1.json';grid=r.b.load(grid_path);assert r.b.digest(grid_path.read_bytes())==plan['coarse_grid_digest'];identity=grid['identity'];cases={};rows=[];best={}
 for w in grid['winners']:
  if w['category']!='zero':continue
  for axis in plan['donor_axes']:
   for cooling in plan['cooling_points']:
    req=copy.deepcopy(w['request']);t=req['input']['competitors'][0]['tuning']
    if t[axis]<cooling:continue
    t[axis]-=cooling;t['cooling_points']=cooling;id=w['group']+'-'+axis+f'-c{cooling}';cases[(id,w['model'])]={'id':id,'request':req,'overrides':{},'group':w['group']}
 for (id,model),c in cases.items():
  d=r.execute(probe,c,model,identity,180)['data'];rows.append({'case_id':id,'group':c['group'],'model':model,'request':c['request'],'data':d});o=d['outcome'];key=(c['group'],model)
  if o['ok'] and (key not in best or o['total_time_s']<best[key]['data']['outcome']['total_time_s']):best[key]={'case':c,'data':d}
 comparisons=[]
 for z in grid['winners']:
  if z['category']!='zero':continue
  key=(z['group'],z['model']);again=r.execute(probe,{'id':z['case_id'],'request':z['request']},z['model'],identity,180)['data'];assert again==z['full_data']
  winner=best[key];repeated=r.execute(probe,winner['case'],z['model'],identity,180)['data'];assert repeated==winner['data']
  comparisons.append({'group':z['group'],'model':z['model'],'zero_request':z['request'],'best_refined_request':winner['case']['request'],'best_refined_cooling':winner['case']['request']['input']['competitors'][0]['tuning']['cooling_points'],'positive_minus_zero_s':winner['data']['outcome']['total_time_s']-z['full_data']['outcome']['total_time_s'],'zero_and_selected_positive_repeated_exactly':True})
 report={'schema':plan['schema'],'plan':plan,'plan_digest':r.b.digest(plan_path.read_bytes()),'runner_digest':r.b.digest(pathlib.Path(__file__).read_bytes()),'identity':identity,'rows':rows,'comparisons':comparisons,'failures':[{'case_id':x['case_id'],'model':x['model'],'error':x['data']['outcome']['error']} for x in rows if not x['data']['outcome']['ok']]};path=HERE/'results/grid-refinement-v1.json'
 if a.check:
  assert report==r.b.load(path),'refinement differs; archive untouched';print('PASS full refinement recheck')
 else:
  raw=r.b.encode(report);path.write_bytes(raw);path.with_suffix('.sha256').write_text(r.b.digest(raw)+'\n')
 print('PASS',len(rows),'fine response runs; all zero/selected-positive minima repeat exactly')
if __name__=='__main__':main()
