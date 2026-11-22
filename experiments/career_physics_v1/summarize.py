#!/usr/bin/env python3
"""Read-only verification of frozen receipts, then deterministic summary export."""
import collections
import gzip
import hashlib
import json
import pathlib

EXP = pathlib.Path(__file__).resolve().parent
OUT = EXP / "results"
ROOT = EXP.parents[1]


def load(name):
    return json.loads(evidence_bytes(OUT / name))


def evidence_bytes(path):
    if path.exists():
        return path.read_bytes()
    return gzip.decompress(path.with_suffix(path.suffix + ".gz").read_bytes())


def sha(path):
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    campaign = load("campaign-v1.json")
    for name, digest in campaign["files"].items():
        if "sha256:" + hashlib.sha256(evidence_bytes(OUT / name)).hexdigest() != digest:
            raise RuntimeError(f"changed evidence:{name}")
    for name, digest in campaign["provenance"]["sources"].items():
        if sha(ROOT / name) != digest:
            raise RuntimeError(f"source differs from frozen campaign:{name}")
    base = load("baselines-v1.json")
    screen = load("screen-v1.json")
    confirm = load("confirmation-v1.json")
    all_rows = base + screen + confirm
    failures = [{"case":r["case"],"outcome":r["data"]["outcome"]} for r in all_rows if not r["data"]["outcome"]["ok"]]
    ledger_max = collections.defaultdict(float)
    energy_max = collections.defaultdict(float)
    carry = collections.defaultdict(float)
    fallback = cap_updates = 0
    for row in all_rows:
        d = row["data"]
        for key, value in d["maximum_carry_jumps"].items():
            carry[key] = max(carry[key], value)
        for lap in d["laps"]:
            ledger = lap["thermal"]["ledger"]
            for key, value in ledger.items():
                if key.startswith("maximum"):
                    ledger_max[key] = max(ledger_max[key],value)
        for lap in d["work"]["laps"]:
            if "reachable" in lap:
                reachable=lap["reachable"]
                fallback += int(reachable["conservative_recompute"])
                cap_updates = max(cap_updates,reachable["uncommitted_cap_updates"])
        if d["outcome"]["ok"]:
            for key,value in d["outcome"]["energy"].items():
                if key.startswith("maximum"):
                    energy_max[key]=max(energy_max[key],value)
    pairs={}
    for row in confirm:
        c=row["case"]
        category="concentrated" if c["allocation"][0]==0 and c["allocation"][3]==0 else "diversified"
        pairs.setdefault((c["fixture_id"],c["circuit"]),{}).setdefault(category,{})[c["forecast"]]=row
    comparisons=[]
    for (fixture,circuit), choices in sorted(pairs.items()):
        entry={"fixture":fixture,"circuit":circuit,"choices":{}}
        deltas={}
        for category, modes in sorted(choices.items()):
            c=modes["reachable_wear_v1"]["case"]
            entry["choices"][category]={"discretionary_allocation_aero_chassis_engine_cooling":c["allocation"],"gear":c["gear"],"downforce":c["downforce"],"full_gp_outcomes":{k:{name:value for name,value in v["data"]["outcome"].items() if name in ["ok","time_s","final_state","error","accepted_laps","result_digest"]} for k,v in modes.items()}}
        for mode in ["minimum_grip","reachable_wear_v1"]:
            a=choices.get("concentrated",{}).get(mode)
            b=choices.get("diversified",{}).get(mode)
            if a and b and a["data"]["outcome"]["ok"] and b["data"]["outcome"]["ok"]:
                deltas[mode]=a["data"]["outcome"]["time_s"]-b["data"]["outcome"]["time_s"]
        entry["concentrated_minus_diversified_time_s"]=deltas
        entry["forecast_reverses_pair_ranking"]=len(deltas)==2 and deltas["minimum_grip"]*deltas["reachable_wear_v1"]<0
        comparisons.append(entry)
    summary={"schema":"pitgun.career-physics-summary/v1","mapping_requests":len(load("mapping-v1.json")["rows"]),"baseline_count":len(base),"screen_count":len(screen),"confirmation_count":len(confirm),"failure_count":len(failures),"failures":failures,"maximum_carry_jumps":dict(carry),"thermal_maxima":dict(ledger_max),"energy_maxima":dict(energy_max),"conservative_recomputes":fallback,"maximum_uncommitted_cap_updates":cap_updates,"shortlisted_full_gp_comparisons":comparisons}
    (OUT/"summary-v1.json").write_text(json.dumps(summary,indent=2,allow_nan=False)+"\n")
    print(json.dumps({k:v for k,v in summary.items() if k not in ["failures","shortlisted_full_gp_comparisons"]},indent=2))
    for c in comparisons:
        print(c["fixture"],c["circuit"],c["concentrated_minus_diversified_time_s"],"ranking reversal",c["forecast_reverses_pair_ranking"])


if __name__ == "__main__":
    main()
