#!/usr/bin/env python3
"""Rerun the frozen campaign in a new directory without overwriting evidence."""
import argparse
import gzip
import hashlib
import importlib.util
import json
import pathlib
import sys

EXP=pathlib.Path(__file__).resolve().parent
ROOT=EXP.parents[1]


def sha(path):return "sha256:"+hashlib.sha256(path.read_bytes()).hexdigest()


def load(path):
    raw=path.read_bytes()
    return json.loads(gzip.decompress(raw) if path.suffix==".gz" else raw)


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary",type=pathlib.Path)
    parser.add_argument("--output",required=True,type=pathlib.Path)
    parser.add_argument("--cache",required=True,type=pathlib.Path)
    parser.add_argument("--allow-rebuilt-binary",action="store_true",help="require numeric/physical equality while separately recording a rebuilt executable identity")
    parser.add_argument("--preview",action="store_true",help="verify pins and preview matrix without creating files or simulating")
    args=parser.parse_args();binary=args.binary.resolve();target=args.output.resolve();cache=args.cache.resolve()
    old=load(EXP/"results/campaign-v1.json")
    for name,digest in old["provenance"]["sources"].items():
        if sha(ROOT/name)!=digest:raise RuntimeError(f"frozen source changed:{name}")
    for name,digest in old["files"].items():
        if sha(EXP/"results"/name)!=digest:raise RuntimeError(f"frozen receipt changed:{name}")
    actual=sha(binary)
    if actual!=old["provenance"]["binary_digest"] and not args.allow_rebuilt_binary:raise RuntimeError("different binary; explicitly allow a rebuilt binary to qualify it numerically")
    if target==EXP or EXP in target.parents:raise RuntimeError("output must be outside frozen experiment")
    spec=importlib.util.spec_from_file_location("frozen_full_gp_campaign",EXP/"run_local.py");module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
    if args.preview:
        print(json.dumps({"pins_verified":True,"phase_a_cases":len(module.initial_cases()),"binary_digest":actual,"output":str(target),"simulation_executed":False},indent=2));return
    target.mkdir(parents=True,exist_ok=False)
    module.EXP=target
    sys.argv=[str(EXP/"run_local.py"),str(binary),str(cache)]
    module.main()
    for phase in old["phase_stats"]:
        name=phase+"-v1.json.gz";expected=load(EXP/"results"/name);got=load(target/"results"/name)
        if args.allow_rebuilt_binary:
            for row in expected+got:row["data"].pop("binary_digest",None)
        if expected!=got:raise RuntimeError(f"reproduced numeric/physical data differ:{phase}")
    if load(EXP/"results/selection-lock-v1.json")["selection_digest"]!=load(target/"results/selection-lock-v1.json")["selection_digest"]:raise RuntimeError("reproduced selection differs")
    (target/"reproduction-check.json").write_text(json.dumps({"phases_exact":list(old["phase_stats"]),"binary_digest":actual,"recorded_binary_digest":old["provenance"]["binary_digest"],"rebuilt_identity_explicitly_allowed":args.allow_rebuilt_binary},indent=2)+"\n")


if __name__=="__main__":main()
