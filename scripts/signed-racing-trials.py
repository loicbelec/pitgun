"""macOS offline signed replay trials: INPUT RUNTIME_DIRECTORY OUTPUT_DIRECTORY.

RUNTIME_DIRECTORY contains native-probe and the generated WASM .js/_bg.wasm.
Raw authorizations/results remain private. No server or browser is contacted.
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
    if len(sys.argv) != 4 or platform.system() != 'Darwin':
        raise SystemExit(__doc__)
    source, runtime, output = (Path(p).resolve() for p in sys.argv[1:])
    output.mkdir(mode=0o700, parents=True, exist_ok=True)
    root = Path(__file__).resolve().parents[1]
    native = runtime/'native-probe'
    module = runtime/'pitgun_racing_simulator.js'
    binary = runtime/'pitgun_racing_simulator_bg.wasm'
    driver = root/'scripts/signed-racing-performance.mjs'
    pins = {str(p): sha(p) for p in (source, native, module, binary, driver, Path(__file__))}
    rows = []
    for iteration in range(3):
        for kind in (['native','wasm'] if iteration % 2 == 0 else ['wasm','native']):
            report = output/f'{kind}-{iteration+1}.json'
            if report.exists():
                raise RuntimeError('Refusing to overwrite '+str(report))
            command = ([str(native),'measure',str(source),str(report)] if kind == 'native' else
                       ['node',str(driver),str(source),str(module),str(binary),str(report)])
            result = subprocess.run(['/usr/bin/time','-l',*command], capture_output=True, text=True)
            (output/f'{kind}-{iteration+1}.time.txt').write_text(result.stderr)
            if result.returncode:
                raise RuntimeError(f'{kind} failed: {result.stderr}')
            r = json.loads(report.read_text())
            rss = re.search(r'(\d+)\s+maximum resident set size',result.stderr)
            footprint = re.search(r'(\d+)\s+peak memory footprint',result.stderr)
            assert rss and footprint
            r.update(kind=kind,iteration=iteration+1,max_rss_bytes=int(rss[1]),
                     peak_footprint_bytes=int(footprint[1]))
            r['framework_api_ms'] = r['setup_ms']+sum(r['pull_api_ms'])+r['completion_ms']
            r['terminal_api_ms'] = r['pull_api_ms'][-1]
            rows.append(r)
            report.write_text(json.dumps(r,indent=2)+'\n')
            print(json.dumps({key:r[key] for key in ('kind','iteration','framework_api_ms',
                'terminal_api_ms','max_rss_bytes','frames')}),flush=True)
    for r in rows:
        assert r['frames'] == 37944
        assert r['run_id'] == rows[0]['run_id']
        assert r['standings'] == rows[0]['standings']
        assert r['player_lap_times_ms'] == rows[0]['player_lap_times_ms']
    for path,digest in pins.items():
        assert sha(Path(path)) == digest, 'changed artifact/input'
    keys = ('framework_api_ms','terminal_api_ms','completion_ms','host_parse_ms',
            'max_rss_bytes','peak_footprint_bytes','stream_bytes','completion_bytes')
    medians = {kind:{key:statistics.median(r[key] for r in rows if r['kind'] == kind)
                     for key in keys} for kind in ('native','wasm')}
    summary = {'schema_version':'pitgun.signed-racing-memory-trials/v1','model':'0.14.0',
        'platform':platform.platform(),'pins':pins,'trials':rows,'medians':medians,
        'physical_summaries_equal':True,
        'limits':['Prior real local QA authorization, offline only; no new/verifier/server action.',
                  'Declared historical artifact digest is frozen input, not runtime attestation.',
                  'Current memory snapshots are phase endpoints, not guaranteed phase maxima.',
                  'RSS measured by OS includes real host parsing; no full JSON hashing in timed process.',
                  'No UI, browser, storage, gzip or archive hashes; native/WASM not generally bit equal.',
                  'Whole canonical output/evidence equality is checked separately per runtime.']}
    (output/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
    print(json.dumps(medians,indent=2),flush=True)


if __name__ == '__main__':
    main()
