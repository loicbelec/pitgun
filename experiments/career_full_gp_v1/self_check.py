#!/usr/bin/env python3
"""Qualify the new probe against retained native results and strict boundaries."""
import gzip
import json
import pathlib
import subprocess
import sys
import tempfile

EXP=pathlib.Path(__file__).resolve().parent
ROOT=EXP.parents[1]


def main():
    if len(sys.argv)!=2:raise SystemExit("usage: self_check.py BINARY")
    binary=str(pathlib.Path(sys.argv[1]).resolve())
    old=json.loads(gzip.decompress((ROOT/"experiments/career_physics_v1/results/confirmation-v1.json.gz").read_bytes()))
    retained=[];rejected=[]
    with tempfile.TemporaryDirectory(prefix="pitgun-gp-qualification-") as td:
        p=pathlib.Path(td)/"case.json"
        def execute(case):
            p.write_text(json.dumps(case))
            return subprocess.run([binary,str(ROOT),str(p)],capture_output=True,text=True)
        controls=[next(r for r in old if r["case"]["fixture_id"].endswith(stage) and r["case"]["forecast"]=="reachable_wear_v1" and r["case"]["circuit"]=="MONZA") for stage in ["era1-entry","era2-exit"]]
        for row in controls:
            c={k:v for k,v in row["case"].items() if k!="five_laps"}
            run=execute(c)
            if run.returncode:raise RuntimeError(run.stderr)
            new=json.loads(run.stdout)
            for key in ["physical_input_digest","diagnostic_input_digest","outcome","laps","work","maximum_carry_jumps"]:
                if new[key]!=row["data"][key]:raise RuntimeError(f"retained output changed:{key}")
            retained.append({"case":c["id"],"input_digest":new["physical_input_digest"],"result_digest":new["outcome"]["result_digest"]})
        base={k:v for k,v in controls[0]["case"].items() if k!="five_laps"}
        late={k:v for k,v in controls[1]["case"].items() if k!="five_laps"}
        invalid=[("wrong_budget",dict(base,allocation=[0,1,0,0])),("gear_bound",dict(base,gear=1.01)),("locked_downforce",dict(base,downforce=0)),("axis_cap",dict(late,allocation=[0,0,16,0],diagnostic_increment="engine")),("unknown_axis",dict(base,diagnostic_increment="magic")),("bad_stint",dict(base,strategy={"stints":[{"tire_id":"medium","laps":1}],"pit_laps":[]})),("environment_bound",dict(base,environment={"air_c":99,"road_c":42})),("actor_scope",dict(base,actor="ai_1"))]
        for label,case in invalid:
            result=execute(case)
            if result.returncode==0:raise RuntimeError(f"unexpected admission:{label}")
            rejected.append(label)
    report={"retained_exact_controls":retained,"strict_rejections":rejected,"scope":"probe qualification; no numerical coefficient/runtime change"}
    print(json.dumps(report,indent=2))


if __name__=="__main__":main()
