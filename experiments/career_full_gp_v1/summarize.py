#!/usr/bin/env python3
"""Verify frozen campaign receipts and derive contextual response summaries."""
import collections
import gzip
import hashlib
import json
import pathlib

EXP=pathlib.Path(__file__).resolve().parent
ROOT=EXP.parents[1]
OUT=EXP/"results"


def load(name):
    p=OUT/name;raw=p.read_bytes()
    return json.loads(gzip.decompress(raw) if p.suffix==".gz" else raw)


def key(case):
    return hashlib.sha256(json.dumps({k:v for k,v in case.items() if k!="id"},sort_keys=True,separators=(",",":"),allow_nan=False).encode()).hexdigest()


def measures(data):
    laps=data["laps"];outcome=data["outcome"]
    if not outcome["ok"]:return {"ok":False,"error":outcome["error"],"accepted_laps":outcome["accepted_laps"]}
    return {"ok":True,"time_s":outcome["time_s"],"engine_peak_c":max(l["engine_min_max_c"][1] for l in laps),"surface_peak_c":max(l["surface_min_max_c"][1] for l in laps),"derated_time_s":sum(l["derated_time_s"] for l in laps),"minimum_power_fraction":min(l["minimum_power_fraction"] for l in laps),"maximum_post_service_end_lap_wear":max(l["state_after"]["tire_wear"] for l in laps),"fuel_used_kg":data["initial_fuel_kg"]-outcome["final_state"]["fuel_mass"],"engine_shaft_work_j":outcome["energy"]["engine_shaft_work_j"]}


def diff_paths(a,b,prefix=""):
    if isinstance(a,dict) and isinstance(b,dict):
        result=[]
        for k in sorted(a.keys()|b.keys()):result+=diff_paths(a.get(k),b.get(k),prefix+"."+k if prefix else k)
        return result
    if isinstance(a,list) and isinstance(b,list):
        if a==b:return []
        if len(a)!=len(b):return [prefix]
        return [x for i,(u,v) in enumerate(zip(a,b)) for x in diff_paths(u,v,f"{prefix}[{i}]")]
    return [] if a==b else [prefix]


def main():
    manifest=load("campaign-v1.json")
    for name,digest in manifest["files"].items():
        if "sha256:"+hashlib.sha256((OUT/name).read_bytes()).hexdigest()!=digest:raise RuntimeError(f"receipt changed:{name}")
    for name,digest in manifest["provenance"]["sources"].items():
        if "sha256:"+hashlib.sha256((ROOT/name).read_bytes()).hexdigest()!=digest:raise RuntimeError(f"source changed:{name}")
    phases={name:load(name+"-v1.json.gz") for name in manifest["phase_stats"]}
    originals={key(r["case"]):r for r in phases["reserved"]}
    repeats={key(r["case"]):r for r in phases["reserved-repeat"]}
    if originals!=repeats:raise RuntimeError("published reserved repeat differs")
    unique={key(r["case"]):r for name,rows in phases.items() if name not in ["confirmation","reserved-repeat"] for r in rows}
    numerical_unique={r["data"]["diagnostic_input_digest"]:r for r in unique.values()}
    failures=[];carry=collections.defaultdict(float);thermal=collections.defaultdict(float);energy=collections.defaultdict(float);fallback=0;updates=0
    for row in numerical_unique.values():
        d=row["data"]
        if not d["outcome"]["ok"]:failures.append({"case":row["case"],"outcome":d["outcome"]})
        for k,v in d["maximum_carry_jumps"].items():carry[k]=max(carry[k],v)
        for lap in d["laps"]:
            for k,v in lap["thermal"]["ledger"].items():
                if k.startswith("maximum"):thermal[k]=max(thermal[k],v)
        for lap in d["work"]["laps"]:
            if "reachable" in lap:
                fallback+=int(lap["reachable"]["conservative_recompute"]);updates=max(updates,lap["reachable"]["uncommitted_cap_updates"])
        if d["outcome"]["ok"]:
            for k,v in d["outcome"]["energy"].items():
                if k.startswith("maximum"):energy[k]=max(energy[k],v)
    diagnostics=[];drows=phases["axis-diagnostics"]
    for base in drows:
        if base["case"].get("diagnostic_increment"):continue
        fid=base["case"]["fixture_id"];circuit=base["case"]["circuit"];b=measures(base["data"])
        for inc in drows:
            c=inc["case"];axis=c.get("diagnostic_increment")
            if not axis or c["fixture_id"]!=fid or c["circuit"]!=circuit:continue
            if base["data"]["driver"]!=inc["data"]["driver"]:raise RuntimeError("axis pair changes driver")
            if base["data"]["physical_parameters"]["config"]!=inc["data"]["physical_parameters"]["config"]:raise RuntimeError("axis pair changes config/noise/environment")
            m=measures(inc["data"]);changes=diff_paths(base["data"]["physical_parameters"],inc["data"]["physical_parameters"])
            diagnostics.append({"fixture":fid,"circuit":circuit,"axis":axis,"base":b,"incremented":m,"deltas":{k:m[k]-b[k] for k in b if k!="ok"} if b["ok"] and m["ok"] else {},"changed_physical_parameter_paths":changes,"off_budget":True})
    selected=load("selection-lock-v1.json");conf={key(r["case"]):r for r in phases["confirmation"]}
    encoded=json.dumps(selected["selections"],sort_keys=True,separators=(",",":"),allow_nan=False).encode()
    if "sha256:"+hashlib.sha256(encoded).hexdigest()!=selected["selection_digest"]:raise RuntimeError("selection seal differs")
    setup_rows=phases["allocations"]+phases["sliders"]+phases["local"]
    for s in selected["selections"]:
        if s["case"].get("diagnostic_increment"):raise RuntimeError("off-budget diagnostic selected competitively")
        same=[r for r in setup_rows if r["case"]["fixture_id"]==s["fixture_id"] and r["case"]["circuit"]==s["circuit"] and (r["case"]["allocation"][0]==r["case"]["allocation"][3]==0)==(s["family"]=="concentrated") and r["data"]["outcome"]["ok"]]
        expected=min(same,key=lambda r:(r["data"]["outcome"]["time_s"],key(r["case"])))["case"]
        if any(expected[k]!=s["case"][k] for k in ["allocation","gear","downforce"]):raise RuntimeError("selected setup violates declared GP selection")
        if conf[key(s["case"])]["data"]["outcome"]!=s["outcome"]:raise RuntimeError("selected strategy/fresh confirmation differs")
    selection_data=[]
    for s in selected["selections"]:
        row=conf[key(s["case"])];entry={k:s[k] for k in ["fixture_id","circuit","family","recipe","case","diagnostic_input_digest"]};entry["measures"]=measures(row["data"]);entry["effective_allocation"]={a:s["case"]["allocation"][i]+row["data"]["offsets"][a] for i,a in enumerate(["aero","chassis","engine","cooling"])};selection_data.append(entry)
        same=[r for r in setup_rows if r["case"]["fixture_id"]==s["fixture_id"] and r["case"]["circuit"]==s["circuit"] and (r["case"]["allocation"][0]==r["case"]["allocation"][3]==0)==(s["family"]=="concentrated") and r["data"]["outcome"]["ok"]]
        original=min(same,key=lambda r:(r["data"]["outcome"]["time_s"],key(r["case"])))
        entry["fixed_source_strategy_measures"]=measures(original["data"])
        entry["strategy_gain_s"]=entry["fixed_source_strategy_measures"]["time_s"]-entry["measures"]["time_s"]
    pairs=[]
    for fid,circuit in sorted({(s["fixture_id"],s["circuit"]) for s in selection_data}):
        choices={s["family"]:s for s in selection_data if s["fixture_id"]==fid and s["circuit"]==circuit}
        pairs.append({"fixture":fid,"circuit":circuit,"concentrated":choices["concentrated"],"diversified":choices["diversified"],"fixed_source_strategy_concentrated_minus_diversified_s":choices["concentrated"]["fixed_source_strategy_measures"]["time_s"]-choices["diversified"]["fixed_source_strategy_measures"]["time_s"],"concentrated_minus_diversified_s":choices["concentrated"]["measures"]["time_s"]-choices["diversified"]["measures"]["time_s"]})
    labels=load("validation-plan-v1.json");reserved={key(r["case"]):r for r in phases["reserved"]};validation=[]
    for label in labels:
        row=reserved[label["key"]];validation.append({**label,"case":row["case"],"measures":measures(row["data"])})
    groups={}
    for row in validation:
        c=row["case"];k=(row["kind"],c["fixture_id"],c["circuit"],c["seed"],json.dumps(c.get("environment"),sort_keys=True))
        groups.setdefault(k,{})[row["family"]]=row
    reserved_pairs=[]
    for k,choices in sorted(groups.items()):
        a=choices["concentrated"]["measures"];b=choices["diversified"]["measures"]
        reserved_pairs.append({"kind":k[0],"fixture":k[1],"circuit":k[2],"seed":k[3],"environment":json.loads(k[4]),"concentrated":a,"diversified":b,"concentrated_minus_diversified_s":a["time_s"]-b["time_s"] if a["ok"] and b["ok"] else None})
    refs={key(r["case"]):r for r in phases["reference"]}
    for pair in pairs:
        times={}
        for family in ["concentrated","diversified"]:
            c=dict(pair[family]["case"],forecast="minimum_grip");m=measures(refs[key(c)]["data"]);pair[family]["minimum_reference_measures"]=m
            if m["ok"]:times[family]=m["time_s"]
        gap=times["concentrated"]-times["diversified"] if len(times)==2 else None
        pair["minimum_reference_concentrated_minus_diversified_s"]=gap
        pair["boundary_forecast_reverses_pair_order"]=gap is not None and gap*pair["concentrated_minus_diversified_s"]<0
    summary={"schema":"pitgun.full-gp-response-summary/v1","source_commit":manifest["provenance"]["source_commit"],"phase_stats":manifest["phase_stats"],"fresh_process_executions":sum(v["fresh"] for v in manifest["phase_stats"].values()),"unique_semantic_requests":len(unique),"unique_diagnostic_inputs":len(numerical_unique),"unique_physical_inputs":len({r["data"]["physical_input_digest"] for r in unique.values()}),"simulation_failures":failures,"maximum_carry_jumps":dict(carry),"thermal_maxima":dict(thermal),"energy_maxima":dict(energy),"conservative_recomputes":fallback,"maximum_uncommitted_cap_updates":updates,"axis_diagnostics":diagnostics,"selected_pairs":pairs,"reserved":validation,"reserved_pairs":reserved_pairs,"note":"Raw differences under unchanged numerical tolerances; no accuracy/global-optimum/opponent-field acceptance claim. Axis increments are explicitly off-budget. Numerical work aggregates deduplicate diagnostic input (physical input plus forecast), not merely case labels."}
    (OUT/"summary-v1.json").write_text(json.dumps(summary,indent=2,allow_nan=False)+"\n")
    lines=["# Lecture chiffrée de la campagne GP", "", "Différences brutes, coefficients et tolérances inchangés. Les diagnostics +1 sont hors budget.","", "## Configurations retenues et stratégies optimisées dans la grille", "", "Écart = concentré − diversifié. Positif : configuration diversifiée plus rapide.","", "| Étape | Circuit | Écart GP, stratégie source fixe | Écart GP, recettes sélectionnées | Allocation concentrée A/C/E/K | Allocation diversifiée A/C/E/K |", "| --- | --- | ---: | ---: | --- | --- |"]
    for p in pairs:lines.append(f"| {p['fixture'].split(':')[1]} | {p['circuit']} | {p['fixed_source_strategy_concentrated_minus_diversified_s']:+.3f} s | {p['concentrated_minus_diversified_s']:+.3f} s | {p['concentrated']['case']['allocation']} | {p['diversified']['case']['allocation']} |")
    lines += ["", "## Réponse isolée à +1 point, au-delà du budget carrière", "", "| Étape | Circuit | Axe | Δ temps GP | Δ pic moteur | Δ durée derating |", "| --- | --- | --- | ---: | ---: | ---: |"]
    for d in diagnostics:
        delta=d["deltas"]
        if delta:lines.append(f"| {d['fixture'].split(':')[1]} | {d['circuit']} | {d['axis']} | {delta['time_s']:+.3f} s | {delta['engine_peak_c']:+.3f} °C | {delta['derated_time_s']:+.3f} s |")
        else:lines.append(f"| {d['fixture'].split(':')[1]} | {d['circuit']} | {d['axis']} | échec conservé | — | — |")
    lines += ["", f"{len(unique)} requêtes sémantiques, {len(numerical_unique)} entrées de diagnostic distinctes (physique + anticipation), {summary['unique_physical_inputs']} entrées physiques distinctes ; {summary['fresh_process_executions']} processus natifs de calcul, confirmations/répétitions incluses.",f"Échecs de simulation conservés : {len(failures)}. Aucun échec de résolution n'est assimilé à un DNF.","", "Les témoins de sélection et conditions réservées sont détaillés dans summary-v1.json. Le champ de neuf adversaires n'est pas exécuté.",""]
    (EXP/"MEASURED_TABLES_V1.md").write_text("\n".join(lines))
    print(json.dumps({"requests":len(unique),"physical_inputs":summary["unique_physical_inputs"],"executions":summary["fresh_process_executions"],"failures":len(failures),"maximum_carry_jumps":dict(carry),"axis_pairs":len(diagnostics)},indent=2))
    for p in pairs:print(p["fixture"],p["circuit"],p["concentrated_minus_diversified_s"])


if __name__=="__main__":main()
