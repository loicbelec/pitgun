#!/usr/bin/env python3
"""Extended full-GP fixed-field fitting, then sealed independent validation."""
import itertools
import json
import pathlib
import sys

EXP=pathlib.Path(__file__).resolve().parent
ROOT=EXP.parents[1]
sys.path.insert(0,str(ROOT/"experiments/tire_performance_v1"))
from kernel import Kernel, enc, sha, key, load, write, strategy, change
from run_screen import recipes as physical_recipes
PLAN=load(EXP/"plan-v3.json")
PROFILE_LOCK=ROOT/"experiments/tire_performance_v1/results/profile-selection-lock-v1.json"
PROFILE=load(PROFILE_LOCK)["profile"] if PROFILE_LOCK.exists() else None
ROSTER=json.loads((ROOT/PLAN["roster"]).read_text())
FIXTURES={f["id"]:f for f in json.loads((ROOT/"experiments/opponent_physics_v2/results/career-fixtures-v1.json").read_text())["fixtures"] if f["doctrine"]=="craft_engineering"}
CONTEXTS={(c["era"],c["circuit"]):c for c in PLAN["contexts"]}
AXES=["aero","chassis","engine","cooling"]

def recipes(n):return physical_recipes(n)[0]
def legal(a,n,era):return sum(a)==n and all(x>=0 and x+(0 if era==1 else [2,1,4,1][i])<=20 for i,x in enumerate(a))
def case(ctx,actor,a,gear,df,s,budget=None,kind="field",fixture=None,seed=None,env=None,level=0):
 c={"fixture_id":fixture or ctx["fixture_id"],"circuit":ctx["circuit"],"actor":actor,"kind":kind,"budget":budget if budget is not None else ctx["base_budget"],"allocation":list(a),"gear":gear,"downforce":df,"strategy":s,"seed":seed if seed is not None else PLAN["training_seed"],"environment":env,"elite_level":level,"performance_profile":PROFILE};return change(c)
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
def permitted(c):
 era=FIXTURES[c["fixture_id"]]["era"]
 if c["kind"]=="career_player":
  f=FIXTURES[c["fixture_id"]];offset=[f["offsets"][k] for k in AXES];df=f["sliders"]["downforce"];gear=f["sliders"]["gearRatio"]
 else:offset=[0,0,0,0] if era==1 else [2,1,4,1];df=[.5,.5] if era==1 else [0,1];gear=[0,1]
 return sum(c["allocation"])==c["budget"] and all(0<=x and x+offset[i]<=20 for i,x in enumerate(c["allocation"])) and df[0]<=c["downforce"]<=df[1] and gear[0]<=c["gear"]<=gear[1]
def point_neighbors(c,era):
 rows=[]
 for donor,receiver in itertools.permutations(range(4),2):
  if c["allocation"][donor]==0:continue
  a=c["allocation"].copy();a[donor]-=1;a[receiver]+=1;n=change(c,allocation=a)
  if permitted(n):rows.append(n)
 return rows
def local(c,era,sliders=True):
 result=point_neighbors(c,era)
 if sliders:
  for field in ["gear","downforce"]:
   for d in [-.25,.25]:
    n=change(c,**{field:c[field]+d})
    if permitted(n):result.append(n)
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

