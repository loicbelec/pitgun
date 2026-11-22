#!/usr/bin/env python3
"""Verify fixed-field evidence; summarize persistent profiles and player progress."""
import collections
import gzip
import hashlib
import importlib.util
import json
import pathlib

EXP=pathlib.Path(__file__).resolve().parent;ROOT=EXP.parents[1];OUT=EXP/'results'

def enc(d):return json.dumps(d,sort_keys=True,separators=(',',':'),allow_nan=False,ensure_ascii=False).encode()
def key(c):return hashlib.sha256(enc({k:v for k,v in c.items() if k!='id'})).hexdigest()
def sha(b):return 'sha256:'+hashlib.sha256(b).hexdigest()
def load(p):
 b=p.read_bytes();return json.loads(gzip.decompress(b) if p.suffix=='.gz' else b)
def rank(rows):
 return sorted(rows,key=lambda r:(not r['data']['outcome']['ok'],-r['data']['outcome'].get('accepted_laps',len(r['data']['lap_times_s'])),r['data']['outcome'].get('time_s',float('inf')),r['case']['actor']))
def envkey(c):return json.dumps(c['environment'],sort_keys=True)

def main():
 prov=load(OUT/'provenance-v1.json')
 for p,d in prov['sources'].items():
  if sha((ROOT/p).read_bytes())!=d:raise RuntimeError('source pin differs:'+p)
 complete=(OUT/'campaign-v1.json').exists()
 if complete:
  campaign=load(OUT/'campaign-v1.json')
  for p,d in campaign['files'].items():
   if sha((OUT/p).read_bytes())!=d:raise RuntimeError('receipt pin differs:'+p)
 unique={}
 for p in sorted(OUT.glob('*-v1.json.gz')):
  if p.name=='native-field-audit-v1.json.gz':continue
  for r in load(p):unique[key(r['case'])]=r
 numeric={r['data']['diagnostic_input_digest']:r for r in unique.values()};failures=[];carry=[0.]*5;thermal=collections.defaultdict(float);fallback=0;updates=0
 for r in numeric.values():
  d=r['data']
  if not d['outcome']['ok']:failures.append({'case':r['case'],'outcome':d['outcome']})
  for i,v in enumerate(d['maximum_carry_jumps']):carry[i]=max(carry[i],v)
  for k,v in d['thermal_ledger'].items():
   if k.startswith('maximum'):thermal[k]=max(thermal[k],v)
  fallback+=d['metrics']['conservative_recomputes'];updates=max(updates,d['metrics']['maximum_cap_updates'])
 summary={'schema':'pitgun.fixed-field-summary/v1','status':'COMPLETE' if complete else 'IN_PROGRESS_OR_REFINE','source_commit':prov['source_commit'],'unique_semantic_requests':len(unique),'unique_diagnostic_inputs':len(numeric),'failed_simulations':failures,'maximum_carry_jumps_speed_engine_surface_carcass_wear':carry,'thermal_maxima':dict(thermal),'conservative_recompute_laps':fallback,'maximum_cap_updates':updates}
 if (OUT/'calibration-candidates-v1.json').exists():summary['eligible_training_calibrations']=[c for c in load(OUT/'calibration-candidates-v1.json') if c['eligible']]
 if not complete:
  (OUT/'progress-summary-v1.json').write_text(json.dumps(summary,indent=2,ensure_ascii=False)+'\n');print(json.dumps({k:v for k,v in summary.items() if k!='failed_simulations'},indent=2));return
 seal=load(OUT/'selection-lock-v1.json');policy=seal['policy']
 if sha(enc(policy))!=seal['policy_digest']:raise RuntimeError('selected policy changed')
 reserved=load(OUT/'reserved-field-and-player-v1.json.gz');base={key(r['case']):r for r in reserved}
 for r in load(OUT/'reserved-exact-repeat-v1.json.gz'):
  if base[key(r['case'])]!=r:raise RuntimeError('published fresh repeat differs')
 actors=policy['roster']['drivers'];hashes=collections.defaultdict(set);groups={};player_rows={}
 era_by_fixture={f['id']:f['era'] for f in load(ROOT/'experiments/opponent_physics_v2/results/career-fixtures-v1.json')['fixtures']}
 for r in reserved:
  c=r['case'];fixture=c['fixture_id'];era=era_by_fixture[fixture]
  group=(era,c['circuit'],c['seed'],envkey(c))
  if c['actor'] in actors:
   if c['budget']!=policy['budgets_by_era'][str(era)]:raise RuntimeError('field budget is not constant by era')
   actual=r['data']['receipt']['driver'];hashes[c['actor']].add(actual['driver_resource_digest'])
   if actual['controls']['driver_id']!=c['actor'] or actual['controls']['requested_commitment']!=.8:raise RuntimeError('wrong identity or hidden driving mode')
   groups.setdefault(group,{})[c['actor']]=r
  elif c['actor']=='player':player_rows[key(c)]=r
 for actor in actors:
  if len(hashes[actor])!=1:raise RuntimeError('persistent physical traits changed across races:'+actor)
 scores={a:{'points':0,'rank_sum':0,'wins':0,'podiums':0,'races':0,'finishes':0} for a in actors}
 era_scores={}
 for group,field in sorted(groups.items()):
  if set(field)!=set(actors):raise RuntimeError('not exactly9persistent opponents')
  for i,r in enumerate(rank(list(field.values()))):
   a=r['case']['actor'];s=scores[a];s['races']+=1;s['rank_sum']+=i+1;s['wins']+=i==0;s['podiums']+=i<3;s['finishes']+=r['data']['outcome']['ok'];points=[25,18,15,12,10,8,6,4,2][i] if r['data']['outcome']['ok'] else 0;s['points']+=points
   es=era_scores.setdefault(str(group[0]),{});es[a]=es.get(a,0)+points
 hierarchy=sorted(scores,key=lambda a:(-scores[a]['points'],a));summary['elite_hierarchy_passed']=set(hierarchy[:2])==set(policy['roster']['front_runners']);summary['hierarchy']=hierarchy;summary['field_only_scores']=scores;summary['field_only_points_by_era']=era_scores;summary['constant_driver_resource_digests']={a:next(iter(hashes[a])) for a in actors};summary['budgets_by_era']=policy['budgets_by_era'];summary['elite_calibration_level']=policy['elite_level'];summary['fixed_roster']=actors
 progress={};races=[]
 for s in policy['player_cases']:
  c=s['case'];era=s['context'][0];stage=s['fixture'].split(':')[1]
  for group,field in groups.items():
   if group[0]!=era or group[1]!=c['circuit']:continue
   candidate=dict(c,seed=group[2],environment=json.loads(group[3]));candidate['id']=key(candidate);p=player_rows[key(candidate)];rr=rank(list(field.values())+[p]);position=next(i+1 for i,r in enumerate(rr) if r['case']['actor']=='player');row={'fixture':s['fixture'],'style':s['style'],'circuit':c['circuit'],'seed':group[2],'environment':json.loads(group[3]),'position':position,'finished':p['data']['outcome']['ok'],'player_time_s':p['data']['outcome'].get('time_s'),'standings':[r['case']['actor'] for r in rr]};races.append(row)
   stat=progress.setdefault(stage,{}).setdefault(s['style'],{'positions':[],'wins':0,'podiums':0,'finishes':0});stat['positions'].append(position);stat['wins']+=position==1;stat['podiums']+=position<=3;stat['finishes']+=p['data']['outcome']['ok']
 for styles in progress.values():
  for stat in styles.values():stat['mean_position']=sum(stat['positions'])/len(stat['positions']);stat['races']=len(stat['positions'])
 summary['player_progression']=progress;summary['player_races']=races;summary['phase_stats']=campaign['phase_stats'];summary['native_process_gp_executions']=sum(s['fresh'] for s in campaign['phase_stats'].values())+campaign['native_field_extra_calculations'];summary['selected_policy_digest']=seal['policy_digest'];summary['selected_field_all_finish']=all(r['data']['outcome']['ok'] for g in groups.values() for r in g.values())
 advanced_ok=all(progress[stage]['informed']['wins']>=1 and progress[stage]['informed']['mean_position']<progress[stage]['routine']['mean_position'] for stage in ['era1-exit','era2-exit']);summary['progression_passed']=advanced_ok;summary['verdict']='NATIVE_I_II_CANDIDATE' if summary['elite_hierarchy_passed'] and summary['selected_field_all_finish'] and advanced_ok else 'REFINE_NO_ADMISSION'
 audit=load(OUT/'native-field-audit-v1.json.gz');summary['native_ten_car_fields_checked']=len(audit['fields']);summary['reorder_and_player_independence_checked']=len(audit['invariance'])
 (OUT/'summary-v1.json').write_text(json.dumps(summary,indent=2,ensure_ascii=False,allow_nan=False)+'\n')
 public={'schema':policy['schema'],'not_admitted_to_runtime':True,'source_policy_digest':seal['policy_digest'],'roster':policy['roster'],'budgets_by_era':policy['budgets_by_era'],'field_contexts':policy['field_contexts'],'profile_derivation':'cases use pinned base roster plus one globally selected elite_level; roster snapshot expanded once; future admission must compile expanded resources without re-applying calibration'}
 (EXP/'policy-candidate-v1.json').write_text(json.dumps(public,indent=2,ensure_ascii=False)+'\n')
 print(json.dumps({k:summary[k] for k in ['verdict','native_process_gp_executions','unique_diagnostic_inputs','budgets_by_era','elite_calibration_level','hierarchy','elite_hierarchy_passed','selected_field_all_finish','progression_passed','maximum_carry_jumps_speed_engine_surface_carcass_wear']},indent=2))
 for stage,styles in progress.items():print(stage,{s:(round(d['mean_position'],2),d['wins'],d['races']) for s,d in styles.items()})

if __name__=='__main__':main()
