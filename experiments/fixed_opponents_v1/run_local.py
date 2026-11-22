#!/usr/bin/env python3
"""Extended full-GP fixed-field fitting, then sealed independent validation."""
import collections
import concurrent.futures as cf
import copy
import gzip
import hashlib
import itertools
import json
import pathlib
import subprocess
import sys
import tempfile

EXP=pathlib.Path(__file__).resolve().parent
ROOT=EXP.parents[1]
PLAN=json.loads((EXP/"plan-v1.json").read_text())
ROSTER=json.loads((ROOT/PLAN["roster"]).read_text())
FIXTURES={f["id"]:f for f in json.loads((ROOT/"experiments/opponent_physics_v2/results/career-fixtures-v1.json").read_text())["fixtures"] if f["doctrine"]=="craft_engineering"}
CONTEXTS={(c["era"],c["circuit"]):c for c in PLAN["contexts"]}
AXES=["aero","chassis","engine","cooling"]

def enc(d):return json.dumps(d,sort_keys=True,separators=(",",":"),allow_nan=False,ensure_ascii=False).encode()
def sha(b):return "sha256:"+hashlib.sha256(b).hexdigest()
def key(c):return hashlib.sha256(enc({k:v for k,v in c.items() if k!="id"})).hexdigest()
def write(p,d):
 b=json.dumps(d,indent=2,ensure_ascii=False,allow_nan=False).encode()+b"\n";p.write_bytes(gzip.compress(b,mtime=0) if p.suffix==".gz" else b)
def load(p):
 b=p.read_bytes();return json.loads(gzip.decompress(b) if p.suffix==".gz" else b)
def strategy(n,tires,pits):
 bounds=[0]+list(pits)+[n]
 if len(set(bounds))!=len(bounds) or bounds!=sorted(bounds):return None
 return {"stints":[{"tire_id":t,"laps":bounds[i+1]-bounds[i]} for i,t in enumerate(tires)],"pit_laps":list(pits)}
def recipes(n):
 result={}
 for stops,ratios in [(1,[(.25,),(.35,),(.5,),(.65,),(.75,)]),(2,[(.25,.55),(1/3,2/3),(.4,.75)]),(3,[(.2,.45,.7),(.25,.5,.75)])]:
  for tires in itertools.product(["soft","medium","hard"],repeat=stops+1):
   if tires[0]==tires[1]:continue
   timings=[tuple(int(n*f) for f in r) for r in ratios]
   if stops==1:timings += [(1,),(n-1,)]
   if stops==2:timings += [(n//2,n//2+1)]
   for pits in timings:
    s=strategy(n,tires,pits)
    if s:result[sha(enc(s))]=s
 return [result[k] for k in sorted(result)]
def legal(a,n,era):return sum(a)==n and all(x>=0 and x+(0 if era==1 else [2,1,4,1][i])<=20 for i,x in enumerate(a))
def case(ctx,actor,a,gear,df,s,budget=None,kind="field",fixture=None,seed=None,env=None,level=0):
 c={"fixture_id":fixture or ctx["fixture_id"],"circuit":ctx["circuit"],"actor":actor,"kind":kind,"budget":budget if budget is not None else ctx["base_budget"],"allocation":list(a),"gear":gear,"downforce":df,"strategy":s,"seed":seed if seed is not None else PLAN["training_seed"],"environment":env,"elite_level":level};c["id"]=key(c);return c
def change(c,**fields):
 c=copy.deepcopy(c);c.update(fields);c["id"]=key(c);return c
def ok(r):return r["data"]["outcome"]["ok"]
def order(r):return (r["data"]["outcome"].get("time_s",float("inf")),key(r["case"]))
def cohort(r):
 return (FIXTURES[r["case"]["fixture_id"]]["era"],r["case"]["circuit"])
def best(rows):
 good=[r for r in rows if ok(r)]
 if not good:raise RuntimeError("all candidate simulations failed; no silent selection")
 return min(good,key=order)
def near_style(rows,actor):
 b=best(rows)
 if actor in ROSTER["front_runners"]:return b
 choices=[r for r in rows if ok(r) and order(r)[0]<=order(b)[0]*1.0015]
 style=ROSTER["drivers"][actor]["style"]
 def preference(r):
  c=r["case"];a=c["allocation"];pits=c["strategy"]["pit_laps"];n=sum(t["laps"] for t in c["strategy"]["stints"]);first=pits[0]/n
  if style=="tire_manager":p=(len(pits),abs(first-.5),-a[3])
  elif style=="power":p=(-a[2],abs(first-.28),-c["gear"])
  elif style=="mechanical":p=(-a[1],abs(first-.4),len(pits))
  elif style=="smooth":p=(abs(first-1/3),len(pits),-a[1])
  else:p=(abs(first-1/3),abs(a[1]-a[3]),len(pits))
  return p+order(r)
 return min(choices,key=preference)
def allocations(ctx):
 n=ctx["base_budget"];result={(0,n-e,e,0) for e in range(n+1)}
 for k in [0,2,4]:
  for e in [0,int(n*.25+.5),int(n*.5+.5)]:
   for a in [0,int(n*.125+.5)]:
    row=(a,n-a-e-k,e,k)
    if legal(row,n,ctx["era"]):result.add(row)
 return sorted(result)
def point_neighbors(c,era):
 a=c["allocation"];donor=max(range(4),key=lambda i:a[i]);rows=[]
 for receiver in range(4):
  if receiver==donor:continue
  b=a.copy();b[donor]-=1;b[receiver]+=1
  if legal(b,c["budget"],era):rows.append(change(c,allocation=b))
 if a[3]:
  b=a.copy();b[3]-=1;b[1]+=1
  if legal(b,c["budget"],era):rows.append(change(c,allocation=b))
 return rows
def local(c,era,sliders=True):
 result=point_neighbors(c,era)
 if sliders:
  for d in [-.25,.25]:
   if 0<=c["gear"]+d<=1:result.append(change(c,gear=c["gear"]+d))
   if era==2 and 0<=c["downforce"]+d<=1:result.append(change(c,downforce=c["downforce"]+d))
 tires=[s["tire_id"] for s in c["strategy"]["stints"]];n=sum(s["laps"] for s in c["strategy"]["stints"])
 for i in range(len(c["strategy"]["pit_laps"])):
  for d in [-2,2]:
   pits=c["strategy"]["pit_laps"].copy();pits[i]+=d;s=strategy(n,tires,pits)
   if s:result.append(change(c,strategy=s))
 return result
def scale(c,n,era):
 a=c["allocation"];old=sum(a);raw=[x*n/old for x in a];b=[int(x) for x in raw]
 for i in sorted(range(4),key=lambda i:(-(raw[i]-b[i]),i))[:n-sum(b)]:b[i]+=1
 if not legal(b,n,era):raise RuntimeError("scaled budget exceeds legal axis cap")
 return change(c,budget=n,allocation=b)
def rank(rows):
 def k(r):
  d=r["data"];o=d["outcome"];return (not o["ok"],-o.get("accepted_laps",len(d["lap_times_s"])),o.get("time_s",float("inf")),r["case"]["actor"])
 return sorted(rows,key=k)

def main():
 if sys.argv[1:]==["--preview"]:
  print(json.dumps({"contexts":len(CONTEXTS),"setup_cases":sum(len(allocations(c))*3*(1 if c["era"]==1 else 2) for c in CONTEXTS.values()),"strategy_cases":sum(len(recipes(c["laps"])) for c in CONTEXTS.values()),"workers":PLAN["workers"]},indent=2));return
 if len(sys.argv)!=3:raise SystemExit("usage: run_local.py BINARY PRIVATE_CACHE, or --preview")
 binary=pathlib.Path(sys.argv[1]).resolve();cache=pathlib.Path(sys.argv[2]).resolve();cache.mkdir(parents=True,exist_ok=True);out=EXP/"results";out.mkdir(exist_ok=True)
 if (out/"campaign-v1.json").exists():raise RuntimeError("refusing overwrite completed campaign")
 paths=["experiments/fixed_opponents_v1/roster-v1.json","experiments/fixed_opponents_v1/plan-v1.json","experiments/fixed_opponents_v1/run_local.py","experiments/fixed_opponents_v1/qualify.py","crates/pitgun-racing-simulator/examples/fixed_field_probe.rs","crates/pitgun-racing-simulator/src/lib.rs","crates/pitgun-racing-simulator/src/thermal_career.rs","crates/pitgun-racing-simulator/src/thermal_driver.rs","crates/pitgun-racing-solver/src/lib.rs","crates/pitgun-racing-solver/src/thermal_physics.rs","crates/pitgun-racing-solver/src/longitudinal_energy.rs","crates/pitgun-racing-solver/src/tire_reachability.rs","experiments/career_physics_v1/profile-v1.json","experiments/opponent_physics_v2/results/career-fixtures-v1.json"]
 provenance={"source_commit":subprocess.check_output(["git","rev-parse","HEAD"],cwd=ROOT,text=True).strip(),"sources":{p:sha((ROOT/p).read_bytes()) for p in paths},"binary_digest":sha(binary.read_bytes()),"execution":"up to8native workers, no clocks/RSS performance claim"}
 pin=cache/"provenance.json"
 if pin.exists() and load(pin)!=provenance:raise RuntimeError("cache pin mismatch")
 write(pin,provenance);write(out/"provenance-v1.json",provenance);stats={}
 with tempfile.TemporaryDirectory(prefix="pitgun-fixed-field-") as td:
  tmp=pathlib.Path(td)
  def execute(c,force=False):
   k=key(c);p=cache/(k+".json.gz")
   if p.exists() and not force:return load(p)
   f=tmp/(k+".json");write(f,c)
   try:d=json.loads(subprocess.check_output([str(binary),str(ROOT),str(f)],stderr=subprocess.PIPE))
   except subprocess.CalledProcessError as e:raise RuntimeError(f"native infrastructure/resolution failure {k}: {e.stderr.decode()}") from e
   if d["binary_digest"]!=provenance["binary_digest"]:raise RuntimeError("binary changed")
   if p.exists() and load(p)!=d:raise RuntimeError("fresh repeat differs")
   write(p,d);return d
  def batch(name,cases,force=False):
   cases=list({key(c):c for c in cases}.values());cached=0 if force else sum((cache/(key(c)+".json.gz")).exists() for c in cases);rows=[]
   with cf.ThreadPoolExecutor(max_workers=PLAN["workers"]) as pool:
    for c,d in zip(cases,pool.map(lambda c:execute(c,force),cases)):
     rows.append({"case":c,"data":d})
     if len(rows)%100==0:print(f"{name}: {len(rows)}/{len(cases)}",flush=True)
   write(out/(name+"-v1.json.gz"),rows);stats[name]={"requests":len(cases),"cached":cached,"fresh":len(cases)-cached};print(f"{name} complete:{stats[name]}",flush=True);return rows
  setup=[]
  for ctx in CONTEXTS.values():
   s=strategy(ctx["laps"],["medium","hard"],[ctx["laps"]//2])
   for a in allocations(ctx):
    for g in [0,.5,1]:
     for df in ([.5] if ctx["era"]==1 else [0,.5]):setup.append(case(ctx,"reference",a,g,df,s))
  setups=batch("setup-search",setup)
  refinement=[]
  for ck,ctx in CONTEXTS.items():
   top=sorted([r for r in setups if cohort(r)==ck and ok(r)],key=order)[:2]
   for r in top:refinement += [c for c in local(r["case"],ctx["era"]) if c["strategy"]==r["case"]["strategy"]]
  refined=batch("reference-local",refinement);setups=list({key(r["case"]):r for r in setups+refined}.values())
  strat=[]
  for ck,ctx in CONTEXTS.items():
   base=best([r for r in setups if cohort(r)==ck])["case"]
   strat += [change(base,strategy=s) for s in recipes(ctx["laps"])]
  strategies=batch("strategy-search",strat)
  actor_candidates=[]
  for ck,ctx in CONTEXTS.items():
   top_setups=sorted([r for r in setups if cohort(r)==ck and ok(r)],key=order)[:3];top_strategies=sorted([r for r in strategies if cohort(r)==ck and ok(r)],key=order)[:3]
   for actor in ROSTER["drivers"]:
    for a,s in itertools.product(top_setups,top_strategies):actor_candidates.append(change(a["case"],actor=actor,strategy=s["case"]["strategy"]))
  adapted=batch("named-driver-search",actor_candidates)
  actor_local=[]
  for ck,ctx in CONTEXTS.items():
   for actor in ROSTER["drivers"]:
    b=best([r for r in adapted if cohort(r)==ck and r["case"]["actor"]==actor]);actor_local+=local(b["case"],ctx["era"],False)
  extra=batch("named-driver-local",actor_local);adapted+=extra
  selected={}
  for ck in CONTEXTS:
   for actor in ROSTER["drivers"]:selected[(ck,actor)]=near_style([r for r in adapted if cohort(r)==ck and r["case"]["actor"]==actor],actor)
  tiers=[]
  for (ck,actor),row in selected.items():
   for n in PLAN["budget_tiers"][str(ck[0])]:
    for level in ([0,1,2] if actor in ROSTER["front_runners"] else [0]):tiers.append(change(scale(row["case"],n,ck[0]),elite_level=level))
  tier_rows=batch("fixed-budget-calibration",tiers);tier_map={key(r["case"]):r for r in tier_rows}
  player_candidates=[];player_labels=[]
  for fid,f in FIXTURES.items():
   for prep in f["preparedRequests"]:
    ck=(f["era"],prep["circuitId"]);ctx=CONTEXTS[ck];n=f["discretionaryPoints"];base=best([r for r in setups if cohort(r)==ck])["case"];recipes_top=sorted([r for r in strategies if cohort(r)==ck and ok(r)],key=order)[:6]
    normal=[n//4]*4;normal[2]+=n-sum(normal)
    default=strategy(ctx["laps"],["medium","hard"],[ctx["laps"]//2])
    for label,a in [("routine",normal),("fixed_mc",[0,n-n//2,n//2,0])]:
     c=case(ctx,"player",a,.5,.5,default,n,"career_player",fid);player_candidates.append(c);player_labels.append((fid,ck,label,key(c)))
    mc=best([r for r in setups if cohort(r)==ck and r["case"]["allocation"][0]==r["case"]["allocation"][3]==0])["case"]
    for label,source in [("informed",base),("sampled_mc",mc)]:
     scaled=scale(source,n,1 if f["era"]==1 else 2);df=scaled["downforce"] if f["sliders"]["downforce"][0]!=f["sliders"]["downforce"][1] else .5
     for st in recipes_top:
      c=case(ctx,"player",scaled["allocation"],scaled["gear"],df,st["case"]["strategy"],n,"career_player",fid);player_candidates.append(c);player_labels.append((fid,ck,label,key(c)))
  players=batch("player-training",player_candidates);player_map={key(r["case"]):r for r in players}
  informed_groups=sorted({(fid,ck) for fid,ck,label,_ in player_labels if label=="informed"})
  for round_index in range(2):
   candidates=[];extra_labels=[]
   for fid,ck in informed_groups:
    b=best([player_map[k] for f,c,label,k in player_labels if (f,c,label)==(fid,ck,"informed")])["case"]
    neighbors=point_neighbors(b,ck[0]);a=b["allocation"];donor=max(range(4),key=lambda i:a[i])
    if round_index==0 and a[donor]>=2:
     for receiver in range(4):
      if receiver==donor:continue
      aa=a.copy();aa[donor]-=2;aa[receiver]+=2
      if legal(aa,b["budget"],ck[0]):neighbors.append(change(b,allocation=aa))
    for c in neighbors:candidates.append(c);extra_labels.append((fid,ck,"informed",key(c)))
   rr=batch("player-local-"+str(round_index+1),candidates);players+=rr;player_map.update({key(r["case"]):r for r in rr});player_labels+=extra_labels
  player_selected=[]
  for fid,ck,label,_ in player_labels:
   if any(s["fixture"]==fid and s["style"]==label and s["context"]==list(ck) for s in player_selected):continue
   options=[player_map[k] for f,c,l,k in player_labels if (f,c,l)==(fid,ck,label)];chosen=best(options);player_selected.append({"fixture":fid,"context":list(ck),"style":label,"case":chosen["case"]})
  def field_rows(ck,n,level):
   return [tier_map[key(change(scale(selected[(ck,a)]["case"],n,ck[0]),elite_level=level if a in ROSTER["front_runners"] else 0))] for a in ROSTER["drivers"]]
  choices=[]
  for level in [0,1,2]:
   for n1,n2 in itertools.product(PLAN["budget_tiers"]["1"],PLAN["budget_tiers"]["2"]):
    points=collections.defaultdict(float);difficulty={};total=0
    for ck in CONTEXTS:
     rr=rank(field_rows(ck,n1 if ck[0]==1 else n2,level));total+=1
     for i,r in enumerate(rr):points[r["case"]["actor"]]+=[25,18,15,12,10,8,6,4,2][i] if ok(r) else 0
    elites=sorted(points,key=lambda a:(-points[a],a))[:2];elite_ok=set(elites)==set(ROSTER["front_runners"])
    for era in [1,2]:
     positions=[];improvements=[]
     for s in player_selected:
      if s["style"]!="informed" or (era==1 and not s["fixture"].endswith("era1-exit")) or (era==2 and not s["fixture"].endswith("era2-exit")):continue
      ck=tuple(s["context"]);p=player_map[key(s["case"])];rr=rank(field_rows(ck,n1 if era==1 else n2,level)+[p]);positions.append(next(i+1 for i,r in enumerate(rr) if r["case"]["actor"]=="player"))
      routine=next(x for x in player_selected if x["fixture"]==s["fixture"] and x["context"]==s["context"] and x["style"]=="routine");improvements.append(player_map[key(routine["case"])]["data"]["outcome"].get("time_s",1e99)-p["data"]["outcome"].get("time_s",1e99))
     difficulty[str(era)]={"positions":positions,"wins":positions.count(1),"median":sorted(positions)[len(positions)//2],"mean_routine_gain_s":sum(improvements)/len(improvements)}
    eligible=elite_ok and all(d["wins"]>=1 and d["median"]<=3 and d["mean_routine_gain_s"]>0 for d in difficulty.values())
    choices.append({"elite_level":level,"budgets":{"1":n1,"2":n2},"points":dict(points),"top_two":elites,"difficulty":difficulty,"eligible":eligible})
  feasible=[c for c in choices if c["eligible"]];write(out/"calibration-candidates-v1.json",choices)
  if not feasible:raise RuntimeError("REFINE: no declared fixed calibration meets hierarchy/progression targets; retain results, do not fabricate a field")
  chosen=max(feasible,key=lambda c:(min(c["budgets"]["1"]/8,c["budgets"]["2"]/16),c["budgets"]["1"]/8+c["budgets"]["2"]/16,-c["elite_level"],c["budgets"]["2"],c["budgets"]["1"]))
  policy={"schema":"pitgun.fixed-opponent-policy-candidate/v1","roster":copy.deepcopy(ROSTER),"elite_level":chosen["elite_level"],"budgets_by_era":chosen["budgets"],"field_contexts":[],"training_choice":chosen,"player_cases":player_selected,"not_admitted_to_runtime":True}
  for a in ROSTER["front_runners"]:
   t=policy["roster"]["drivers"][a]["resource"]["traits"]
   for k in t:t[k]=min(.99,t[k]+.02*chosen["elite_level"])
  for ck,ctx in CONTEXTS.items():policy["field_contexts"].append({"era":ck[0],"circuit":ck[1],"cases":[r["case"] for r in field_rows(ck,chosen["budgets"][str(ck[0])],chosen["elite_level"])]})
  seal={"schema":"pitgun.fixed-field-selection-lock/v1","policy_digest":sha(enc(policy)),"policy":policy,"reserved_results_opened_at_seal":False};write(out/"selection-lock-v1.json",seal)
  validation=[]
  for group in policy["field_contexts"]:
   for seed,environment in itertools.product(PLAN["validation_seeds"],PLAN["validation_environments"]):validation += [change(c,seed=seed,environment=environment) for c in group["cases"]]
  validation += [change(p["case"],seed=seed,environment=environment) for p in player_selected for seed,environment in itertools.product(PLAN["validation_seeds"],PLAN["validation_environments"])]
  reserved=batch("reserved-field-and-player",validation)
  # Independent native ten-car assembly, then order/player-change invariance.
  audit=[]
  for group in policy["field_contexts"]:
   if group["circuit"] not in ["MONZA","ZANDVOORT","MONTREAL"]:continue
   p=next(x for x in player_selected if x["context"]==[group["era"],group["circuit"]] and x["style"]=="informed")
   seed=PLAN["validation_seeds"][0];cases=[change(p["case"],seed=seed)]+[change(c,seed=seed) for c in group["cases"]]
   f=tmp/"field.json";write(f,cases);native=json.loads(subprocess.check_output([str(binary),str(ROOT),str(f),"--field"]))
   for row in native["standings"]:
    c=next(c for c in cases if c["actor"]==row["actor"])
    if row["data"]!=execute(c):raise RuntimeError("ten-car native/independent outcome mismatch")
   audit.append({"era":group["era"],"circuit":group["circuit"],"cases":cases,"result":native})
  invariance=[]
  for record in audit[:2]:
   original={r["actor"]:r["data"] for r in record["result"]["standings"]}
   reversed_cases=list(reversed(record["cases"]));f=tmp/"field-invariance.json";write(f,reversed_cases)
   result=json.loads(subprocess.check_output([str(binary),str(ROOT),str(f),"--field"]))
   if {r["actor"]:r["data"] for r in result["standings"]}!=original:raise RuntimeError("roster reordering changes physical outcome")
   altered=copy.deepcopy(record["cases"]);player=next(c for c in altered if c["actor"]=="player");replacement=point_neighbors(player,record["era"])[0];altered=[replacement if c["actor"]=="player" else c for c in altered];write(f,altered)
   result=json.loads(subprocess.check_output([str(binary),str(ROOT),str(f),"--field"]))
   if any(r["data"]!=original[r["actor"]] for r in result["standings"] if r["actor"]!="player"):raise RuntimeError("player setup changes a rival")
   invariance.append({"era":record["era"],"circuit":record["circuit"],"reorder_exact":True,"player_change_preserves9rivals":True,"altered_cases":altered,"altered_result":result})
  write(out/"native-field-audit-v1.json.gz",{"fields":audit,"invariance":invariance})
  repeats=[r["case"] for r in reserved if r["case"]["seed"]==PLAN["validation_seeds"][0] and r["case"]["environment"] is None]
  batch("reserved-exact-repeat",repeats,True)
  if load(out/"selection-lock-v1.json")!=seal:raise RuntimeError("policy changed after reserved results")
  if {p:sha((ROOT/p).read_bytes()) for p in paths}!=provenance["sources"]:raise RuntimeError("source changed during calculation")
  write(out/"campaign-v1.json",{"schema":"pitgun.fixed-field-campaign-receipt/v1","provenance":provenance,"phase_stats":stats,"native_field_extra_calculations":sum(len(a["cases"]) for a in audit)+40,"policy_digest":seal["policy_digest"],"files":{p.name:sha(p.read_bytes()) for p in sorted(out.glob("*-v1.*")) if p.name!="campaign-v1.json"}})
  print("campaign complete; reserved results not used for retuning",flush=True)

if __name__=="__main__":main()
