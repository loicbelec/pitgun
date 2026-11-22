#!/usr/bin/env python3
"""Frozen bounded seam screen and serial advance-clock cost. No model tuning."""
import hashlib
import json
from pathlib import Path
import statistics
import subprocess
import sys
import tempfile

HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[1]
def read(p): return json.loads(p.read_bytes())
def digest(p): return 'sha256:'+hashlib.sha256(p.read_bytes()).hexdigest()

def main():
    action=sys.argv[1];binaries,dest=[Path(p).resolve() for p in sys.argv[2:]]
    if action not in ['screen','cost']: raise ValueError('screen|cost required')
    if subprocess.check_output(['git','status','--porcelain'],cwd=ROOT,text=True).strip(): raise RuntimeError('commit before qualification')
    plan=read(HERE/'plan-v1.json')
    identity={'source_commit':subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),'plan_digest':digest(HERE/'plan-v1.json'),'runner_digest':digest(Path(__file__)),'probe_digest':digest(binaries/'lap_boundary_probe')}
    if action=='screen':dest.mkdir(parents=True,exist_ok=False)
    def case(scenario,mode,service=False):
        return {**{k:v for k,v in scenario.items() if k not in ['group','compare_service']},'id':scenario['id']+':'+mode,'forecast':mode,'recompute_invariant':False,'service_boundaries':service,'tail_trace':True}
    with tempfile.TemporaryDirectory(prefix='pitgun-bounded-lap-') as tmp:
        scratch=Path(tmp)/'case.json'
        def invoke(scenario,mode,service=False):
            scratch.write_text(json.dumps(case(scenario,mode,service)))
            p=subprocess.run([str(binaries/'lap_boundary_probe'),str(ROOT),str(scratch)],capture_output=True,text=True,timeout=plan['timeout_s'])
            if p.returncode:raise RuntimeError(p.stderr[:1500])
            r=json.loads(p.stdout)
            if r['data']['binary_digest']!=identity['probe_digest']: raise RuntimeError('stale executable')
            sources={'probe':'crates/pitgun-racing-simulator/examples/lap_boundary_probe.rs','helper':'crates/pitgun-racing-simulator/examples/support/frozen_thermal_input.rs','solver':'crates/pitgun-racing-solver/src/lib.rs','boundary':'crates/pitgun-racing-solver/src/lap_boundary.rs','reachability':'crates/pitgun-racing-solver/src/tire_reachability.rs','energy':'crates/pitgun-racing-solver/src/longitudinal_energy.rs','thermal':'crates/pitgun-racing-solver/src/thermal_physics.rs'}
            if r['data']['sources']!={k:digest(ROOT/v) for k,v in sources.items()}:raise RuntimeError('stale compiled source')
            return r
        if action=='screen':
            passes=[];services=[]
            for repeat in range(plan['repeats']):
                rows={}
                for s in plan['scenarios']:
                    for mode in plan['modes']:
                        print(f"pass{repeat+1}: {s['id']} {mode}",flush=True)
                        r=invoke(s,mode);rows[r['data']['id']]=r['data']
                        (dest/f"pass-{repeat}-{s['id']}-{mode}.json").write_text(json.dumps(r)+'\n')
                passes.append(rows)
            if passes[0]!=passes[1]:raise RuntimeError('nonrepeatable data/errors/counters/prefixes')
            reference=read(ROOT/'experiments/lap_boundary_v1/results/screen-v1.json')['results']
            for s in plan['scenarios']:
                if s['id'].startswith('reference-'):
                    r=passes[0][s['id']+':minimum_grip']; old=reference[s['frozen_case']+':cached']
                    if any(r[k]!=old[k] for k in ['physical_input_digest','diagnostic_input_digest','laps','outcome','work']):raise RuntimeError('old lap reference changed: '+s['id'])
                if s.get('compare_service'):
                    for mode in plan['modes']:
                        print('service replay: '+s['id']+' '+mode,flush=True)
                        r=invoke(s,mode,True)['data'];normal=passes[0][s['id']+':'+mode]
                        if r!=normal:raise RuntimeError('unchanged pre-service continuation differs')
                        services.append({'id':r['id'],'exact':True})
            for row in passes[0].values():
                for k in ['maximum_speed_jump_mps','maximum_engine_temperature_jump_c','maximum_tire_surface_jump_c','maximum_tire_carcass_jump_c','maximum_wear_jump_fraction']:
                    if row[k]!=0:raise RuntimeError('continuity jump: '+row['id'])
                if row['forecast']=='reachable_wear_v1':
                    for lap in row['work']['laps']:
                        w=lap['reachable']
                        if not w['service_reset'] and row['outcome']['ok'] and w['committed_end_speed_mps']>w['certificate_cap_mps']+1e-9:raise RuntimeError('uncertified committed end')
            print('normal-constructor tire/driver/history/event regressions',flush=True)
            subprocess.run([sys.executable,str(ROOT/'experiments/tyre_operating_policy_v1/run_local.py'),str(binaries),str(dest/'normal-regressions')],check=True,cwd=ROOT)
            previous=read(ROOT/'experiments/tyre_operating_policy_v1/results/screen-v1.json')
            normal=read(dest/'normal-regressions/screen-v1.json')
            for a,b in zip(previous['results'],normal['results']):
                if any(a[k]!=b[k] for k in ['case','input_digest','diagnostic_input_digest','outcome','laps','events','chart','preparation','maximum_surface_jump_c','maximum_carcass_jump_c']):raise RuntimeError('old preparation result changed')
            report={'schema':'pitgun.bounded-lap-lookahead-screen/v1','identity':identity,'repeat_exact':True,'continuity_exact':True,'old_lap_references_equal':8,'service_replays':services,'normal_regressions':{'thermal_preparation_cases':len(normal['results']),'driver_cases':len(normal['driver_regressions']),'historical':len(normal['historical_regressions']),'event_cases':normal['old_event_cases_equal']},'all_bounded_complete':all(r['outcome']['ok'] for r in passes[0].values() if r['forecast']=='reachable_wear_v1'),'results':passes[0]}
            p=dest/'screen-v1.json';p.write_text(json.dumps(report,sort_keys=True,separators=(',',':'))+'\n');p.with_suffix('.sha256').write_text(digest(p)+'\n')
            print(json.dumps({'repeat_exact':True,'all_bounded_complete':report['all_bounded_complete'],'outcomes':[{'id':r['id'],'ok':r['outcome']['ok'],'error':r['outcome'].get('error')} for r in passes[0].values()]},indent=2))
        else:
            screen=read(dest/'screen-v1.json')
            if screen['identity']!=identity or not screen['all_bounded_complete']:raise RuntimeError('qualify the same source/binary first')
            summary=[]
            for id in plan['cost_scenarios']:
                scenario=next(s for s in plan['scenarios'] if s['id']==id)
                samples={m:[] for m in plan['modes']}
                for i in range(plan['cost_repetitions']):
                    for mode in plan['modes'] if i%2==0 else reversed(plan['modes']):
                        print('cost '+id+' '+mode,flush=True);r=invoke(scenario,mode)
                        if r['data']!=screen['results'][id+':'+mode]:raise RuntimeError('cost output differs')
                        samples[mode].append(r['performance']['solver_ms'])
                summary.append({'id':id,'samples_ms':samples,'medians_ms':{k:statistics.median(v) for k,v in samples.items()}})
            p=dest/'cost-v1.json';p.write_text(json.dumps({'schema':'pitgun.bounded-lap-lookahead-cost/v1','identity':identity,'summary':summary,'scope':'Serial alternating native advance calls. Different physical outputs: not an equal-output optimization, browser/WASM/VPS/RSS result. Metrics/digest/serialization outside timer.'},indent=2)+'\n');p.with_suffix('.sha256').write_text(digest(p)+'\n');print(json.dumps(summary,indent=2))

if __name__=='__main__':main()
