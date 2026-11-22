#!/usr/bin/env python3
"""Pre-registered bounded GP campaign. All selection rules precede results."""
import concurrent.futures as cf
import copy
import gzip
import hashlib
import json
import pathlib
import subprocess
import sys
import tempfile

EXP = pathlib.Path(__file__).resolve().parent
ROOT = EXP.parents[1]
PLAN = json.loads((EXP / "plan-v1.json").read_text())
FIXTURES = {f["id"]:f for f in json.loads((ROOT / PLAN["fixtures"]).read_text())["fixtures"]}
PRIOR = json.loads((ROOT / PLAN["prior_shortlist"]).read_text())
COHORTS = [("craft_engineering:"+s,c) for s in PLAN["stages"] for c in PLAN["fit_circuits"]]


def canonical(data):
    return json.dumps(data,sort_keys=True,separators=(",",":"),allow_nan=False).encode()


def sha(data):
    return "sha256:"+hashlib.sha256(data).hexdigest()


def write(path,data):
    raw=json.dumps(data,indent=2,allow_nan=False).encode()+b"\n"
    path.write_bytes(gzip.compress(raw,mtime=0) if path.suffix==".gz" else raw)


def load(path):
    raw=path.read_bytes()
    return json.loads(gzip.decompress(raw) if path.suffix==".gz" else raw)


def category(case):
    a=case["allocation"]
    return "concentrated" if a[0]==a[3]==0 else "diversified"


def identity(case):
    return hashlib.sha256(canonical({k:v for k,v in case.items() if k!="id"})).hexdigest()


def case_for(fid,circuit,allocation,gear=.5,downforce=.5,**extra):
    case={"fixture_id":fid,"circuit":circuit,"allocation":list(allocation),"gear":gear,"downforce":downforce,"forecast":"reachable_wear_v1","actor":"player",**extra}
    case["id"]=identity(case)
    return case


def authored(fid,circuit):
    f=FIXTURES[fid]
    prepared=next(p for p in f["preparedRequests"] if p["circuitId"]==circuit)
    p=next(p for p in prepared["request"]["input"]["competitors"] if p["is_player"])
    return tuple(int(p["tuning"][a+"_points"]-f["offsets"][a]) for a in PLAN["axes"])


def legal(fid,allocation):
    f=FIXTURES[fid]
    return sum(allocation)==f["discretionaryPoints"] and all(isinstance(x,int) and 0<=x and x+f["offsets"][a]<=20 for a,x in zip(PLAN["axes"],allocation))


def neighbors(fid,a):
    result=set()
    for donor in range(4):
        for receiver in range(4):
            if donor==receiver or a[donor]==0:continue
            b=list(a);b[donor]-=1;b[receiver]+=1
            if legal(fid,b):result.add(tuple(b))
    return result


def initial_cases():
    result=[]
    for fid,circuit in COHORTS:
        f=FIXTURES[fid];n=f["discretionaryPoints"]
        anchors={authored(fid,circuit)}
        prior=[r for r in PRIOR if r["fixture_id"]==fid and r["circuit"]==circuit and r["forecast"]=="reachable_wear_v1"]
        anchors.update(tuple(r["allocation"]) for r in prior)
        allocations=anchors|{(0,n-e,e,0) for e in range(n+1)}
        for a in anchors:allocations|=neighbors(fid,a)
        for r in prior:
            if r["allocation"][0] or r["allocation"][3]:continue
            a=r["allocation"];donor=max([1,2],key=lambda i:a[i])
            for receiver in [0,3]:
                for amount in [1,2,4]:
                    if a[donor]<amount:continue
                    b=list(a);b[donor]-=amount;b[receiver]+=amount
                    if legal(fid,b):allocations.add(tuple(b))
        result.extend(case_for(fid,circuit,a) for a in sorted(allocations))
    return result


def best(rows,fid,circuit,family):
    choices=[r for r in rows if r["case"]["fixture_id"]==fid and r["case"]["circuit"]==circuit and category(r["case"])==family and r["data"]["outcome"]["ok"]]
    if not choices:raise RuntimeError(f"no successful setup in {fid}/{circuit}/{family}; no silent substitution")
    return min(choices,key=lambda r:(r["data"]["outcome"]["time_s"],identity(r["case"])))


def slider_cases(rows):
    cases=[]
    for fid,circuit in COHORTS:
        f=FIXTURES[fid];n=f["discretionaryPoints"]
        allocations={(0,n-e,e,0) for e in range(n+1)}
        diversified=sorted([r for r in rows if r["case"]["fixture_id"]==fid and r["case"]["circuit"]==circuit and category(r["case"])=="diversified" and r["data"]["outcome"]["ok"]],key=lambda r:(r["data"]["outcome"]["time_s"],identity(r["case"])))
        allocations.update(tuple(r["case"]["allocation"]) for r in diversified[:3])
        dfs=PLAN["downforce_grid_when_unlocked"] if f["sliders"]["downforce"][0]!=f["sliders"]["downforce"][1] else [f["sliders"]["downforce"][0]]
        for a in sorted(allocations):
            for gear in PLAN["gear_grid"]:
                for df in dfs:cases.append(case_for(fid,circuit,a,gear,df))
    return cases


def local_cases(rows):
    cases=[]
    for fid,circuit in COHORTS:
        for family in ["concentrated","diversified"]:
            c=best(rows,fid,circuit,family)["case"]
            cases.extend(case_for(fid,circuit,a,c["gear"],c["downforce"]) for a in sorted(neighbors(fid,c["allocation"])))
    return cases


def strategy_for(fid,circuit,recipe):
    if recipe=="source":return None
    n=next(p for p in FIXTURES[fid]["preparedRequests"] if p["circuitId"]==circuit)["request"]["input"]["laps"]
    specs={"mh35":(["medium","hard"],[.35]),"mh50":(["medium","hard"],[.5]),"mh65":(["medium","hard"],[.65]),"sh50":(["soft","hard"],[.5]),"hs50":(["hard","soft"],[.5]),"ms55":(["medium","soft"],[.55]),"sms_thirds":(["soft","medium","soft"],[1/3,2/3])}
    tires,fractions=specs[recipe];pits=[int(n*f) for f in fractions];bounds=[0]+pits+[n]
    return {"stints":[{"tire_id":t,"laps":bounds[i+1]-bounds[i]} for i,t in enumerate(tires)],"pit_laps":pits}


def strategy_case(base,recipe,circuit=None,**extra):
    case=copy.deepcopy(base);case.update(extra)
    if circuit is not None:case["circuit"]=circuit
    strategy=strategy_for(case["fixture_id"],case["circuit"],recipe)
    case.pop("strategy",None)
    if strategy is not None:case["strategy"]=strategy
    case["id"]=identity(case)
    return case


def main():
    a=initial_cases()
    if sys.argv[1:]==["--preview"]:
        print(json.dumps({"phase_a_cases":len(a),"concentrated_slider_grid":sum((FIXTURES[f]["discretionaryPoints"]+1)*len(PLAN["gear_grid"])*(3 if f.endswith("era2-exit") else 1) for f,c in COHORTS),"cohorts":COHORTS},indent=2));return
    if len(sys.argv)!=3:raise SystemExit("usage: run_local.py BINARY PRIVATE_CACHE (or --preview)")
    binary=pathlib.Path(sys.argv[1]).resolve();cache=pathlib.Path(sys.argv[2]).resolve();cache.mkdir(parents=True,exist_ok=True)
    out=EXP/"results";out.mkdir(exist_ok=True)
    if (out/"campaign-v1.json").exists():raise SystemExit("refusing to overwrite completed campaign")
    paths=["experiments/career_full_gp_v1/plan-v1.json","experiments/career_full_gp_v1/run_local.py","experiments/career_full_gp_v1/self_check.py","crates/pitgun-racing-simulator/examples/career_full_gp_probe.rs","crates/pitgun-racing-simulator/src/thermal_career.rs","crates/pitgun-racing-simulator/src/thermal_driver.rs","crates/pitgun-racing-simulator/src/lib.rs","crates/pitgun-racing-solver/src/lib.rs","crates/pitgun-racing-solver/src/thermal_physics.rs","crates/pitgun-racing-solver/src/longitudinal_energy.rs","crates/pitgun-racing-solver/src/lap_boundary.rs","crates/pitgun-racing-solver/src/tire_reachability.rs",PLAN["fixtures"],PLAN["thermal_policy"],PLAN["prior_shortlist"]]
    provenance={"source_commit":subprocess.check_output(["git","rev-parse","HEAD"],cwd=ROOT,text=True).strip(),"sources":{p:sha((ROOT/p).read_bytes()) for p in paths},"binary_digest":sha(binary.read_bytes()),"execution":"up to4concurrent native processes; no timing/RSS claim"}
    pin=cache/"provenance.json"
    if pin.exists() and load(pin)!=provenance:raise RuntimeError("cache belongs to different sources/binary")
    write(pin,provenance);write(out/"provenance-v1.json",provenance)
    with tempfile.TemporaryDirectory(prefix="pitgun-full-gp-") as td:
        tmp=pathlib.Path(td)
        def execute(case,force=False):
            key=identity(case);path=cache/(key+".json.gz")
            if path.exists() and not force:return load(path)
            p=tmp/(key+".json");write(p,case)
            try:data=json.loads(subprocess.check_output([str(binary),str(ROOT),str(p)],stderr=subprocess.PIPE))
            except subprocess.CalledProcessError as e:raise RuntimeError(f"probe infrastructure/resolution error {key}: {e.stderr.decode()}") from e
            if data["binary_digest"]!=provenance["binary_digest"]:raise RuntimeError("binary changed during run")
            if path.exists() and load(path)!=data:raise RuntimeError("fresh native repeat differs")
            write(path,data);return data
        phase_stats={}
        def batch(name,cases,force=False):
            cases=list({identity(c):c for c in cases}.values())
            cached=sum((cache/(identity(c)+".json.gz")).exists() for c in cases) if not force else 0
            rows=[]
            with cf.ThreadPoolExecutor(max_workers=PLAN["workers"]) as pool:
                for c,data in zip(cases,pool.map(lambda c:execute(c,force),cases)):
                    rows.append({"case":c,"data":data})
                    if len(rows)%40==0:print(f"{name}: {len(rows)}/{len(cases)}",flush=True)
            write(out/(name+"-v1.json.gz"),rows)
            phase_stats[name]={"cases":len(cases),"cached":cached,"fresh":len(cases)-cached}
            print(f"{name}: complete {len(cases)}, fresh {len(cases)-cached}",flush=True)
            return rows
        rows_a=batch("allocations",a)
        rows_b=batch("sliders",slider_cases(rows_a))
        rows_c=batch("local",local_cases(rows_a+rows_b))
        setup_rows=rows_a+rows_b+rows_c
        diagnostic=[]
        for fid,circuit in COHORTS:
            base=case_for(fid,circuit,authored(fid,circuit))
            diagnostic.append(base)
            for axis in PLAN["axes"]:diagnostic.append(case_for(fid,circuit,base["allocation"],diagnostic_increment=axis))
        batch("axis-diagnostics",diagnostic)
        recipes=["source","mh35","mh50","mh65","sh50","hs50","ms55","sms_thirds"]
        trials=[];choices=[]
        for fid,circuit in COHORTS:
            for family in ["concentrated","diversified"]:
                base=best(setup_rows,fid,circuit,family)["case"]
                for recipe in recipes:
                    case=strategy_case(base,recipe);trials.append(case);choices.append((fid,circuit,family,recipe,identity(case)))
        strategy_rows=batch("strategies",trials);by_key={identity(r["case"]):r for r in strategy_rows}
        selected=[]
        for fid,circuit in COHORTS:
            for family in ["concentrated","diversified"]:
                options=[(recipe,by_key[key]) for f,c,k,recipe,key in choices if (f,c,k)==(fid,circuit,family) and by_key[key]["data"]["outcome"]["ok"]]
                if not options:raise RuntimeError("all strategy recipes failed; no silent selected substitute")
                recipe,row=min(options,key=lambda pair:(pair[1]["data"]["outcome"]["time_s"],pair[0]))
                selected.append({"fixture_id":fid,"circuit":circuit,"family":family,"recipe":recipe,"case":row["case"],"outcome":row["data"]["outcome"],"diagnostic_input_digest":row["data"]["diagnostic_input_digest"]})
        seal={"schema":"pitgun.full-gp-selection-lock/v1","selection_digest":sha(canonical(selected)),"selections":selected,"source_commit":provenance["source_commit"],"opened_reserved_results":False}
        seal_path=out/"selection-lock-v1.json";write(seal_path,seal)
        batch("confirmation",[s["case"] for s in selected],True)
        minimum=[]
        for s in selected:
            c=dict(s["case"],forecast="minimum_grip");c["id"]=identity(c);minimum.append(c)
        batch("reference",minimum)
        validation=[];validation_labels=[]
        for s in selected:
            for seed in PLAN["reserved"]["seeds"]:
                for env in PLAN["reserved"]["environments"]:
                    c=strategy_case(s["case"],s["recipe"],seed=seed,environment=env);validation.append(c);validation_labels.append({"kind":"same_circuit","key":identity(c),"family":s["family"],"source":s["diagnostic_input_digest"]})
                if s["circuit"]=="MONZA":
                    target=PLAN["reserved"]["transfer_circuits"][s["fixture_id"].split(":")[1]]
                    c=strategy_case(s["case"],s["recipe"],circuit=target,seed=seed);validation.append(c);validation_labels.append({"kind":"circuit_transfer","key":identity(c),"family":s["family"],"source":s["diagnostic_input_digest"]})
        write(out/"validation-plan-v1.json",validation_labels)
        batch("reserved",validation)
        batch("reserved-repeat",validation,True)
        if load(seal_path)!=seal:raise RuntimeError("selected configurations changed after opening reserved results")
        current={p:sha((ROOT/p).read_bytes()) for p in paths}
        if current!=provenance["sources"]:raise RuntimeError("sources changed during campaign")
        write(out/"campaign-v1.json",{"schema":"pitgun.career-full-gp-campaign/v1","provenance":provenance,"phase_stats":phase_stats,"selection_digest":seal["selection_digest"],"files":{p.name:sha(p.read_bytes()) for p in sorted(out.glob("*-v1.*")) if p.name!="campaign-v1.json"}})
        print("campaign complete; reserved results not used to retune",flush=True)


if __name__=="__main__":main()
