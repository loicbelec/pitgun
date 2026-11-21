#!/usr/bin/env python3
"""Exact-output performance comparison plus independent observation-rate audit."""
import argparse
import importlib.util
import json
import pathlib
import platform
import statistics
import subprocess
import tempfile

ROOT=pathlib.Path(__file__).resolve().parents[2]
HERE=pathlib.Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('baseline',ROOT/'experiments/physics_foundation/baseline_local.py')
b=importlib.util.module_from_spec(spec);spec.loader.exec_module(b)

def stable(v):
    if isinstance(v,(list,tuple)):return [stable(x) for x in v]
    if isinstance(v,dict):return {k:stable(x) for k,x in v.items() if k not in ('solve_ms','measurements','performance','timings_ms','resolve_ms')}
    return v

def run(binary,request,args,timeout):
    with tempfile.TemporaryDirectory(prefix='pitgun-cost-') as tmp:
        path=pathlib.Path(tmp)/'request.json';path.write_bytes(b.encode(request))
        m=b.run_process([str(binary),str(path),str(ROOT/'catalogs/racing/v1.8.1'),*args],timeout)
    out,err=m.pop('stdout'),m.pop('stderr')
    if m['returncode'] or m['timed_out']:raise ValueError(err.decode(errors='replace')[:2000])
    data=json.loads(out);b.encode(data)
    if data.get('binary_digest',data.get('identity',{}).get('native_binary_digest'))!=b.digest(binary.read_bytes()):raise ValueError('binary identity mismatch')
    return data,m

def outcomes(data):return {k:stable(v['output']) for k,v in data['competitors'].items()}

def summary(rows):
    return {'median_wall_ms':statistics.median(r['wall_ms'] for r in rows),
            'median_cpu_ms':statistics.median(r['user_cpu_ms']+r['system_cpu_ms'] for r in rows),
            'max_rss_bytes':max(r['process_lifetime_max_rss_bytes'] for r in rows),
            'median_stages_ms':{k:statistics.median(r['timings_ms'][k] for r in rows) for k in rows[0]['timings_ms']}}

def main():
    p=argparse.ArgumentParser(description=__doc__)
    for name in ('before','after','observations','legacy'):p.add_argument('--'+name,type=pathlib.Path,required=True)
    p.add_argument('--output',type=pathlib.Path,default=HERE/'results/local-cost-v1.json')
    p.add_argument('--check',action='store_true');a=p.parse_args()
    plan_path=HERE/'plan-v1.json';plan=b.load(plan_path);baseline_path=ROOT/plan['baseline'];base=b.load(baseline_path)
    if b.digest(baseline_path.read_bytes())!=plan['baseline_digest']:raise ValueError('changed frozen baseline')
    binaries={k:getattr(a,k).resolve(strict=True) for k in ('before','after','observations','legacy')}
    if b.digest(binaries['before'].read_bytes())!=base['identity']['probe_digest']:raise ValueError('before binary is not the frozen qualified candidate')
    paths=[*sorted((ROOT/'crates/pitgun-racing-solver').rglob('*.rs')),*sorted((ROOT/'crates/pitgun-racing-simulator').rglob('*.rs')),ROOT/'Cargo.lock',ROOT/'Cargo.toml',ROOT/'crates/pitgun-racing-solver/Cargo.toml',ROOT/'crates/pitgun-racing-simulator/Cargo.toml',ROOT/'experiments/physics_foundation/baseline_local.py',plan_path,pathlib.Path(__file__)]
    rel=[str(x.relative_to(ROOT)) for x in paths]
    subprocess.run(['git','diff','--exit-code','HEAD','--',*rel],cwd=ROOT,check=True,stdout=subprocess.DEVNULL)
    for path in rel:subprocess.run(['git','ls-files','--error-unmatch',path],cwd=ROOT,check=True,stdout=subprocess.DEVNULL)
    sources={str(x.relative_to(ROOT)):b.digest(x.read_bytes()) for x in paths}
    identity={'source_commit':subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),'source_files':sources,'binaries':{k:b.digest(v.read_bytes()) for k,v in binaries.items()},'baseline_digest':plan['baseline_digest'],'plan_digest':b.digest(plan_path.read_bytes()),'runner_digest':b.digest(pathlib.Path(__file__).read_bytes())}
    checks=[];paired=[]
    # The unchanged default must reproduce full lot0 canonical summaries.
    old=b.load(ROOT/'experiments/physics_foundation/results/local-baseline-v1.json')
    old_cases={c['case_id']:c for c in old['cases']}
    for case in b.load(ROOT/'experiments/physics_foundation/plan-v1.json')['cases']:
        data,_=run(binaries['legacy'],case['request'],[],plan['timeout_s'])
        b.validate_probe_identity(data['identity'],{'probe_digest':identity['binaries']['legacy'],'source_files':sources})
        if data['result']!=old_cases[case['id']]['outcome']['result']:raise ValueError('historical default changed')
        checks.append({'id':case['id'],'exact_frozen_output_equal':True})
    for case in base['cases']:
        expected=outcomes(case['outcomes']['energy']);measurements={'before':[],'after':[]}
        for mode in plan['paired_order']:
            data,m=run(binaries[mode],case['request'],['energy'],plan['timeout_s'])
            expected_sources=(base['identity']['source_files'] if mode=='before' else sources)
            for name,path in [('solver','crates/pitgun-racing-solver/src/lib.rs'),('longitudinal','crates/pitgun-racing-solver/src/longitudinal_energy.rs'),('simulator','crates/pitgun-racing-simulator/src/lib.rs'),('probe','crates/pitgun-racing-simulator/examples/longitudinal_energy_probe.rs')]:
                if data['sources'][name]!=expected_sources[path]:raise ValueError('stale compiled source')
            if data['catalog']!=base['identity']['catalog'] or outcomes(data)!=expected:raise ValueError('complete physical outcome changed: '+case['id'])
            m['timings_ms']={'solve':sum(v['output']['solve_ms'] for v in data['competitors'].values())};measurements[mode].append(m)
        paired.append({'id':case['id'],'exact_full_physical_outputs_equal':True,'result_digests':{k:v.get('result_digest') for k,v in expected.items()},'failures':{k:v['error'] for k,v in expected.items() if not v['ok']},'measurements':measurements,'performance':{k:summary(v) for k,v in measurements.items()}})
        print('EXACT',case['id'],flush=True)
    by_id={c['id']:c for c in base['cases']};sens=[]
    for item in base['sensitivities']:
        data,_=run(binaries['after'],by_id[item['base']]['request'],['energy',*item['options']],plan['timeout_s'])
        if outcomes(data)!=outcomes(item['outcome']):raise ValueError('sensitivity outcome changed')
        sens.append({'group':item['group'],'base':item['base'],'options':item['options'],'exact_full_physical_outputs_equal':True})
    cadence=[]
    for name in plan['cadence_cases']:
        case=by_id[name];expected=outcomes(case['outcomes']['energy']);seen={};measurements={str(hz):[] for hz in plan['cadence_order_hz']}
        for hz in plan['cadence_order_hz']:
            data,m=run(binaries['observations'],case['request'],[str(hz)],plan['timeout_s'])
            if data['sources']['solver']!=sources['crates/pitgun-racing-solver/src/lib.rs'] or data['sources']['longitudinal']!=sources['crates/pitgun-racing-solver/src/longitudinal_energy.rs'] or data['sources']['probe']!=sources['crates/pitgun-racing-simulator/examples/energy_cost_probe.rs']:raise ValueError('stale observation source')
            for car,v in data['cars'].items():
                if v['result_digest']!=expected[car]['result_digest'] or v['energy']!=expected[car]['energy'] or v['final_state']!=expected[car]['final_state']:raise ValueError('observation rate changed physics')
            key=str(hz)
            if key in seen and stable(data)!=seen[key]:raise ValueError('observation repetition changed')
            seen[key]=stable(data)
            m['timings_ms']={k:sum(v['timings_ms'][k] for v in data['cars'].values()) for k in next(iter(data['cars'].values()))['timings_ms']};m['timings_ms']['resolve']=data['resolve_ms'];measurements[key].append(m)
        cadence.append({'id':name,'physics_exact_at_all_rates':True,'outcomes':seen,'measurements':measurements,'performance':{k:summary(v) for k,v in measurements.items()}})
        print('CADENCE',name,flush=True)
    report={'schema':'pitgun.energy-cost-report/v1','identity':identity,'plan':plan,'historical_parity':checks,'paired':paired,'sensitivities':sens,'cadence':cadence,'environment':{'platform':platform.platform(),'rustc':subprocess.check_output(['rustc','--version'],text=True).strip()},'qualified':True}
    if a.check:
        previous=b.load(a.output);report['identity']['source_commit']=previous['identity']['source_commit']
        if stable(report)!=stable(previous):raise ValueError('deterministic evidence changed; archive untouched')
        print('PASS full recheck; archive untouched')
    else:
        a.output.parent.mkdir(exist_ok=True,parents=True);raw=b.encode(report);a.output.write_bytes(raw);a.output.with_suffix('.sha256').write_text(b.digest(raw)+'\n')
if __name__=='__main__':main()
