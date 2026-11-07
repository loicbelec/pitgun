"""Run isolated, alternating macOS native/WASM trials, with exact shared input.

Usage: python3 SCRIPT INPUT NATIVE_BINARY WASM_JS WASM_BINARY OUTPUT_DIRECTORY
No UI, service, provider, signature or game-save access.
"""
import hashlib
import json
from pathlib import Path
import platform
import re
import statistics
import subprocess
import sys


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    if len(sys.argv) != 6 or platform.system() != 'Darwin':
        raise SystemExit(__doc__)
    fixture, native, js, wasm, output = map(lambda p: Path(p).resolve(), sys.argv[1:])
    output.mkdir(parents=True, exist_ok=True)
    root = Path(__file__).resolve().parents[1]
    pinned = (fixture, native, js, wasm, Path(__file__).resolve(),
              root/'scripts/racing-performance-wasm.mjs',
              root/'crates/pitgun-racing-simulator/examples/racing_performance_probe.rs')
    fingerprints = {str(p): sha(p) for p in pinned}
    rows = []
    order = ['native-typed', 'native-json', 'wasm-json']
    for iteration in range(3):
        for kind in order[iteration:] + order[:iteration]:
            report = output / f'{kind}-{iteration+1}.json'
            if report.exists():
                raise RuntimeError(f'Refusing to overwrite {report}')
            if kind == 'wasm-json':
                command = ['node', str(root/'scripts/racing-performance-wasm.mjs'),
                           str(fixture), str(js), str(wasm), str(report)]
            else:
                command = [str(native), kind.removeprefix('native-'), str(fixture), str(report)]
            result = subprocess.run(['/usr/bin/time', '-l', *command], capture_output=True, text=True)
            (output/f'{kind}-{iteration+1}.time.txt').write_text(result.stderr)
            if result.returncode:
                raise RuntimeError(f'{kind} failed: {result.stderr}')
            r = json.loads(report.read_text())
            rss = re.search(r'(\d+)\s+maximum resident set size', result.stderr)
            footprint = re.search(r'(\d+)\s+peak memory footprint', result.stderr)
            cpu = re.search(r'([\d.]+) real\s+([\d.]+) user\s+([\d.]+) sys', result.stderr)
            assert rss and footprint and cpu
            r.update(kind=kind, iteration=iteration+1, max_rss_bytes=int(rss[1]),
                     peak_footprint_bytes=int(footprint[1]), process_wall_s=float(cpu[1]),
                     process_user_s=float(cpu[2]), process_system_s=float(cpu[3]))
            r['framework_api_ms'] = r['setup_ms'] + (sum(r['pull_api_ms']) + r['finish_ms']
                if r['mode'] == 'json' else r['compute_ms'])
            rows.append(r)
            report.write_text(json.dumps(r, indent=2)+'\n')
            print(json.dumps({key:r[key] for key in ('kind','iteration','framework_api_ms',
                'host_parse_ms','max_rss_bytes','frames','completion_bytes')}), flush=True)
    # Compare physical outcomes without claiming arbitrary float/byte identity
    # between Rust native and WASM. Timing is finished before this validation.
    first = rows[0]
    for r in rows:
        assert r['frames'] == first['frames']
        assert r['standings'] == first['standings'], 'different standings/times'
        assert r['player_lap_times_ms'] == first['player_lap_times_ms'], 'different lap times'
    for p in pinned:
        assert sha(p) == fingerprints[str(p)], 'input/runtime artifact changed during trials'
    metrics = ('framework_api_ms','setup_ms','compute_ms','finish_ms','host_parse_ms',
               'elapsed_ms','max_rss_bytes','peak_footprint_bytes','process_user_s',
               'process_system_s','stream_bytes','completion_bytes')
    medians = {kind:{key:statistics.median(r[key] for r in rows if r['kind']==kind)
                    for key in metrics} for kind in order}
    summary = {'schema_version':'pitgun.racing-performance-attribution/v1',
               'platform':platform.platform(),'machine':platform.machine(),
               'fingerprints':fingerprints,'trials':rows,'medians':medians,
               'physical_outcomes_equal':True,'frames':first['frames'],
               'notes':['Three fresh processes per mode; rotating order; compilation excluded.',
                        'Same release source/input/catalogue/profile, native vs locally rebuilt WASM.',
                        'Typed API excludes JSON transport/playback trajectories, not Racing orchestration.',
                        'Typed mode avoids full JSON conversion; facade RSS includes real host parsing.',
                        'No extra full-result stringify/canonicalization is added for validation.',
                        'No UI, browser, Authority, compression, persistence or signed finalization.',
                        'Equal standings/lap ms/counts, not full native/WASM floating-point conformance.']}
    (output/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
    print(json.dumps({'medians':medians,'physical_outcomes_equal':True},indent=2),flush=True)


if __name__ == '__main__':
    main()
