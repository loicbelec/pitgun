#!/usr/bin/env python3
"""Bounded native qualification; no cloud, provider, game or signed service calls."""
import argparse
import importlib.util
import json
import math
import pathlib
import platform
import statistics
import subprocess
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[2]
HERE = pathlib.Path(__file__).resolve().parent
HELPER = ROOT / 'experiments/physics_foundation/baseline_local.py'
spec = importlib.util.spec_from_file_location('baseline', HELPER)
b = importlib.util.module_from_spec(spec)
spec.loader.exec_module(b)
SOURCES = {
    'solver': 'crates/pitgun-racing-solver/src/lib.rs',
    'longitudinal': 'crates/pitgun-racing-solver/src/longitudinal_energy.rs',
    'simulator': 'crates/pitgun-racing-simulator/src/lib.rs',
    'probe': 'crates/pitgun-racing-simulator/examples/longitudinal_energy_probe.rs',
}


def stable(value):
    """Drop measurement fields only; never normalize physical floating values."""
    if isinstance(value, (list, tuple)):
        return [stable(v) for v in value]
    if isinstance(value, dict):
        return {k: stable(v) for k, v in value.items()
                if k not in ('solve_ms', 'measurements', 'performance')}
    return value


def validate_energy(output, criteria):
    e, checks = output['energy'], output['checks']
    tolerance = max(criteria['global_energy_abs_j'],
                    criteria['global_energy_rel'] * e['engine_shaft_work_j'])
    for name in ('vehicle_work_residual_j', 'mass_service_boundary_residual_j',
                 'shaft_work_residual_j', 'thermal_storage_residual_j'):
        if abs(checks[name]) > tolerance:
            raise ValueError(f'{name} exceeds {tolerance} J')
    if abs(checks['clock_residual_s']) > criteria['clock_abs_s']:
        raise ValueError('the motion/service/energy clocks differ')
    if abs(checks['fuel_mass_residual_kg']) > criteria['fuel_abs_kg']:
        raise ValueError('fuel consumption differs from shaft work and idle time')
    for name in ('maximum_vehicle_balance_residual_j', 'maximum_driveline_balance_residual_j'):
        if e[name] > criteria['segment_work_residual_j']:
            raise ValueError(f'{name} exceeds the segment tolerance')
    if e['maximum_thermal_balance_residual_j'] > criteria['segment_thermal_residual_j']:
        raise ValueError('segment thermal balance exceeds tolerance')
    if e['maximum_raw_contact_utilization'] > criteria['raw_contact_max']:
        raise ValueError('raw contact demand exceeds capacity')
    for name in ('drag_work_j', 'rolling_work_j', 'wheel_propulsion_work_j',
                 'friction_brake_work_j', 'driveline_loss_j', 'clutch_slip_loss_j'):
        if e[name] < 0:
            raise ValueError(f'negative dissipative/propulsive work: {name}')
    if e['segments'] != output['nodes'] - 1:
        raise ValueError('missing or duplicated driving segments')


def execute(probe, request, mode, catalog, identity, timeout, options=()):
    with tempfile.TemporaryDirectory(prefix='pitgun-energy-') as tmp:
        request_path = pathlib.Path(tmp) / 'request.json'
        request_path.write_bytes(b.encode(request))
        measured = b.run_process([str(probe), str(request_path), str(catalog), mode, *options], timeout)
    raw, error = measured.pop('stdout'), measured.pop('stderr').decode(errors='replace')
    if measured['returncode'] or measured['timed_out']:
        raise RuntimeError(error[:2000])
    data = json.loads(raw)
    b.encode(data)  # Reject NaN/Infinity, including permissive Python JSON values.
    if data['binary_digest'] != identity['probe_digest'] or data['sources'] != identity['compiled_sources']:
        raise ValueError('stale probe/source or substituted executable')
    if data['catalog'] != identity['catalog']:
        raise ValueError('unexpected immutable catalog identity')
    measured['solver_ms'] = sum(c['output']['solve_ms'] for c in data['competitors'].values())
    return data, measured


def compare_case(case, probe, catalog, plan, identity):
    outcomes, measurements = {}, {'legacy': [], 'energy': []}
    for mode in plan['order']:
        data, measured = execute(probe, case['request'], mode, catalog, identity, plan['timeout_s'])
        for car, value in data['competitors'].items():
            output = value['output']
            if not output['ok']:
                if case.get('required_completion', True):
                    raise ValueError(f"{case['id']}/{car}/{mode}: {output['error']}")
                if 'fuel depleted' not in output['error']:
                    raise ValueError(f"unclassified feasibility failure: {output['error']}")
            elif mode == 'energy':
                validate_energy(output, plan['criteria'])
        if mode in outcomes and stable(data) != stable(outcomes[mode]):
            raise ValueError(f"{case['id']}/{mode}: same-binary repetition changed")
        outcomes[mode] = data
        measurements[mode].append(measured)
    performance = {mode: {
        'median_solver_ms': statistics.median(m['solver_ms'] for m in rows),
        'median_process_wall_ms': statistics.median(m['wall_ms'] for m in rows),
        'maximum_process_rss_bytes': max(m['process_lifetime_max_rss_bytes'] for m in rows),
    } for mode, rows in measurements.items()}
    print('PASS', case['id'], {m: all(c['output']['ok'] for c in d['competitors'].values())
                              for m, d in outcomes.items()}, flush=True)
    return {'id': case['id'], 'required_completion': case.get('required_completion', True),
            'request': case['request'], 'same_binary_repetition_equal': True,
            'outcomes': {m: stable(v) for m, v in outcomes.items()},
            'measurements': measurements, 'performance': performance}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--probe', type=pathlib.Path, required=True)
    parser.add_argument('--legacy-probe', type=pathlib.Path, required=True)
    parser.add_argument('--output', type=pathlib.Path, default=HERE/'results/local-energy-v1.json')
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    plan_path, baseline_path = HERE/'plan-v1.json', ROOT/'experiments/physics_foundation/plan-v1.json'
    plan, baseline = b.load(plan_path), b.load(baseline_path)
    if plan['schema'] != 'pitgun.longitudinal-energy-qualification-plan/v1' or b.digest(baseline_path.read_bytes()) != plan['baseline_plan_digest']:
        raise ValueError('qualification baseline/plan identity changed')
    probe, legacy_probe = args.probe.resolve(strict=True), args.legacy_probe.resolve(strict=True)
    source_files = {str(p.relative_to(ROOT)): b.digest(p.read_bytes())
                    for crate in ('pitgun-racing-solver', 'pitgun-racing-simulator')
                    for p in sorted((ROOT/'crates'/crate).rglob('*.rs'))}
    for path in ('Cargo.lock','Cargo.toml','crates/pitgun-racing-solver/Cargo.toml','crates/pitgun-racing-simulator/Cargo.toml'):
        source_files[path]=b.digest((ROOT/path).read_bytes())
    critical = [*source_files, str(plan_path.relative_to(ROOT)), str(pathlib.Path(__file__).relative_to(ROOT))]
    subprocess.run(['git', 'diff', '--exit-code', 'HEAD', '--', *critical], cwd=ROOT,
                   check=True, stdout=subprocess.DEVNULL)
    for path in critical:
        subprocess.run(['git', 'ls-files', '--error-unmatch', path], cwd=ROOT,
                       check=True, stdout=subprocess.DEVNULL)
    identity = {'source_commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
                'probe_digest': b.digest(probe.read_bytes()), 'legacy_probe_digest': b.digest(legacy_probe.read_bytes()),
                'compiled_sources': {name: source_files[path] for name, path in SOURCES.items()},
                'source_files': source_files, 'plan_digest': b.digest(plan_path.read_bytes()),
                'runner_digest': b.digest(pathlib.Path(__file__).read_bytes()),
                'helper_digest': b.digest(HELPER.read_bytes()),
                'catalog': b.load(ROOT/plan['catalog']/'release.json')}
    by_id = {c['id']: c for c in baseline['cases']}
    cases = [by_id[i] for i in plan['reference_cases']] + plan['extra_cases']
    catalog = ROOT / plan['catalog']
    frozen = b.load(ROOT/'experiments/physics_foundation/results/local-baseline-v1.json')
    old = {c['case_id']: c for c in frozen['cases']}
    historical = []
    for case in baseline['cases']:
        with tempfile.TemporaryDirectory(prefix='pitgun-energy-legacy-') as tmp:
            request_path = pathlib.Path(tmp)/'request.json'
            request_path.write_bytes(b.encode(case['request']))
            measured = b.run_process([str(legacy_probe), str(request_path), str(catalog)], plan['timeout_s'])
        if measured['returncode'] or measured['timed_out']:
            raise ValueError(f"legacy {case['id']} failed")
        data = json.loads(measured['stdout'])
        b.validate_probe_identity(data['identity'], {'probe_digest': identity['legacy_probe_digest'], 'source_files': source_files})
        if data['result'] != old[case['id']]['outcome']['result']:
            raise ValueError(f"legacy {case['id']}: canonical output or diagnostic changed")
        historical.append({'id': case['id'], 'output_digest': data['result']['output_digest'], 'exact_frozen_output_equal': True})
    reports = [compare_case(case, probe, catalog, plan, identity) for case in cases]
    sensitivities = []
    for group, config in [('mesh', plan['mesh_sensitivity']), ('cooling', plan['cooling_sensitivity'])]:
        case = by_id[config['base']]
        settings = ([(str(f), '1', str(config['ambient_c']), 'smooth-driver') for f in config['factors']]
                    if group == 'mesh' else [('1', str(s), str(a), 'smooth-driver')
                    for a in config['engine_ambient_c'] for s in config['scales']])
        for options in settings:
            data, measured = execute(probe, case['request'], 'energy', catalog, identity,
                                     plan['timeout_s'], options)
            for value in data['competitors'].values():
                if not value['output']['ok']:
                    raise ValueError(f'{group}/{options}: {value["output"]["error"]}')
                validate_energy(value['output'], plan['criteria'])
            sensitivities.append({'group': group, 'base': case['id'], 'options': options,
                                  'outcome': stable(data), 'measurements': [measured]})
            print('PASS', group, options, flush=True)
    report = {'schema': 'pitgun.longitudinal-energy-qualification/v1', 'passed': True,
              'identity': identity, 'criteria': plan['criteria'], 'historical_canonical_parity': historical,
              'cases': reports, 'sensitivities': sensitivities,
              'environment': {'platform': platform.platform(), 'machine': platform.machine(),
                'rustc':subprocess.check_output(['rustc','--version'],cwd=ROOT,text=True).strip(),
                'cargo':subprocess.check_output(['cargo','--version'],cwd=ROOT,text=True).strip()},
              'scope': plan['scope'], 'performance_scope': plan['performance']}
    if args.check:
        previous = b.load(args.output)
        # A documentation/report commit can follow the frozen source commit.
        report['identity']['source_commit'] = previous['identity']['source_commit']
        if stable(report) != stable(previous):
            raise ValueError('repeated qualified report differs; no archive overwritten')
        print('PASS complete deterministic recheck; archive untouched')
    else:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        data = b.encode(report)
        args.output.write_bytes(data)
        args.output.with_suffix('.sha256').write_text(b.digest(data)+'\n')

if __name__ == '__main__':
    main()
