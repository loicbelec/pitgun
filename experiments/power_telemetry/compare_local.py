#!/usr/bin/env python3
"""Compare opt-in power telemetry with the frozen lot0 physical outputs, offline."""
import argparse
import importlib.util
import json
import math
import pathlib
import tempfile

ROOT=pathlib.Path(__file__).resolve().parents[2]
HELPER=ROOT/'experiments/physics_foundation/baseline_local.py'
spec=importlib.util.spec_from_file_location('baseline',HELPER)
b=importlib.util.module_from_spec(spec);spec.loader.exec_module(b)

def near(actual,expected,label):
    if not math.isclose(actual,expected,rel_tol=1e-10,abs_tol=.01):
        raise ValueError(f'{label}: {actual} != {expected}')

def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--probe',type=pathlib.Path,required=True)
    p.add_argument('--output',type=pathlib.Path,default=pathlib.Path(__file__).parent/'results/local-power-v1.json')
    args=p.parse_args();probe=args.probe.resolve(strict=True)
    plan_path=ROOT/'experiments/physics_foundation/plan-v1.json'
    old_path=ROOT/'experiments/physics_foundation/results/local-baseline-v1.json'
    plan=b.load(plan_path);old=b.load(old_path);b.validate_plan(plan)
    old_by_id={c['case_id']:c for c in old['cases']};reports=[]
    source={}
    for c in ('pitgun-racing-solver','pitgun-racing-simulator'):
        for file in sorted((ROOT/'crates'/c).rglob('*.rs')):source[str(file.relative_to(ROOT))]=b.digest(file.read_bytes())
    identity={'probe_digest':b.digest(probe.read_bytes()),'source_files':source}
    for case in plan['cases']:
        with tempfile.TemporaryDirectory(prefix='pitgun-power-v1-') as tmp:
            path=pathlib.Path(tmp)/'request.json';path.write_bytes(b.encode(case['request']))
            outcomes=[];measurements=[]
            for flags in ([],['--power-v1'],['--power-v1']):
                measured=b.run_process([str(probe),str(path),str(ROOT/plan['catalog']),*flags],180)
                raw=measured.pop('stdout');error=measured.pop('stderr').decode(errors='replace')
                if measured['returncode'] or measured['timed_out']:raise RuntimeError(f"{case['id']}: {error}")
                data=json.loads(raw);b.encode(data);b.validate_probe_identity(data['identity'],identity)
                old_identity=old_by_id[case['id']]['outcome']['identity']
                for key in ('catalog','physical_core','request_digest'):
                    if data['identity'][key]!=old_identity[key]:raise ValueError(f"{case['id']}: {key} differs from the pinned baseline")
                if data['result']!=old_by_id[case['id']]['outcome']['result']:
                    raise ValueError(f"{case['id']}: legacy physical output or canonical digest changed")
                measurements.append({**measured,'power_projection_ms':data['timings']['power_projection_ms']})
                outcomes.append(data)
        if outcomes[0]['power_projection'] is not None:raise ValueError('projection must be explicitly selected')
        if outcomes[1]['power_projection']!=outcomes[2]['power_projection']:raise ValueError('power projection is not deterministic')
        laps=outcomes[1]['power_projection'];expected=outcomes[1]['result']
        if sorted(int(i) for i in laps)!=list(range(1,case['request']['input']['laps']+1)):
            raise ValueError('missing native lap, including service prefix')
        if any(x['pedal_command_pct'] is not None or x['brake_command_pct'] is not None for x in laps.values()):
            raise ValueError('projection invented a pedal command')
        if any(not 0<=x['maximum_equivalent_engine_load_pct']<=100 for x in laps.values()):
            raise ValueError('equivalent load is out of bounds')
        sums={key:sum(v['ledger'][key] for v in laps.values()) for key in laps['1']['ledger']}
        near(sums['solver_engine_work_j'],expected['fuel']['engine_output_work_kj']*1000,'native engine work')
        near(sums['solver_driveline_loss_j'],expected['mechanical']['driveline_loss_kj']*1000,'native driveline losses')
        near(sums['generated_engine_heat_j'],expected['mechanical']['generated_engine_heat_kj']*1000,'native engine heat')
        near(sums['solver_engine_work_j'],sums['solver_wheel_work_j']+sums['solver_driveline_loss_j']+sums['solver_launch_regularization_j'],'declared port balance')
        near(sums['trajectory_engine_work_j'],sums['trajectory_wheel_work_j']+sums['trajectory_driveline_loss_j']+sums['trajectory_launch_regularization_j'],'trajectory-clock declared port balance')
        if not math.isclose(sums['combustion_fuel_consumption_kg'],expected['fuel']['fuel_consumed_kg'],rel_tol=1e-10,abs_tol=1e-9):
            raise ValueError('native fuel accounting differs')
        if max(abs(x['ledger']['engine_thermal_residual_j']) for x in laps.values())>.01:
            raise ValueError('native thermal accounting residual exceeds0.01J')
        if max(x['maximum_port_residual_w'] for x in laps.values())>1e-6:
            raise ValueError('instantaneous declared-port residual exceeds1e-6W')
        reports.append({'case_id':case['id'],'legacy_output_digest':expected['output_digest'],
            'request_digest':outcomes[1]['identity']['request_digest'],
            'catalog':outcomes[1]['identity']['catalog'],'physical_core':outcomes[1]['identity']['physical_core'],
            'legacy_output_equal_with_projection_off_and_on':True,'new_projection_repeated_equal':True,
            'old_throttle_max_pct':expected['exported_channels']['5008']['maximum'],
            'maximum_equivalent_engine_load_pct':max(x['maximum_equivalent_engine_load_pct'] for x in laps.values()),
            'native_ledger_checks_passed':True,'ledger_sum':sums,'laps':laps,'measurements':measurements})
        print('PASS',case['id'],flush=True)
    report={'schema':'pitgun.power-telemetry-comparison/v1','passed':True,
        'identity':{**identity,'baseline_digest':b.digest(old_path.read_bytes()),'plan_digest':b.digest(plan_path.read_bytes()),
                    'runner_digest':b.digest(pathlib.Path(__file__).read_bytes()),'helper_digest':b.digest(HELPER.read_bytes())},
        'cases':reports,'criteria':{'energy_abs_tolerance_j':.01,'energy_rel_tolerance':1e-10,'thermal_residual_per_lap_j':.01,'power_port_residual_w':1e-6,'fuel_abs_tolerance_kg':1e-9},
        'scope':'Offline native, same existing physical branch. New versioned read-only power projection; historical channels/results preserved. Not a WASM/Authority/Verifier or physical-accuracy qualification.'}
    args.output.parent.mkdir(parents=True,exist_ok=True);data=b.encode(report);args.output.write_bytes(data)
    args.output.with_suffix('.sha256').write_text(b.digest(data)+'\n')

if __name__=='__main__':main()
