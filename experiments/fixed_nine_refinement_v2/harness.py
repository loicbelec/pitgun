"""Offline native runner with durable row references and one final archive.

Native execution/checking is retained from V1. Checkpoints do not recompress rows.
"""
from copy import deepcopy
import hashlib
import importlib.util
import json
import math
import os
from pathlib import Path
import sys
import time

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(ROOT / 'experiments/fixed_nine_thermal_v1'))
spec = importlib.util.spec_from_file_location('fixed_nine_frozen_runner', ROOT / 'experiments/fixed_nine_thermal_v1/run.py')
BASE = importlib.util.module_from_spec(spec)
spec.loader.exec_module(BASE)
encoded, load, sha, write = BASE.encoded, BASE.load, BASE.sha, BASE.write


def key(case, fresh=False, resolve=False):
    return hashlib.sha256(encoded(case)).hexdigest() + ('.fresh' if fresh else '') + ('.resolve' if resolve else '') + '.json'


class Journal:
    """Append a checked reference after the native cache row is atomically saved.

    A trailing interrupted line is preserved separately then truncated. Complete
    malformed lines, reordered cases, changed rows and changed identities fail.
    """
    def __init__(self, folder, native_cache, identity, matrix):
        self.folder = Path(folder)
        self.native_cache = Path(native_cache)
        self.matrix = deepcopy(matrix)
        self.identity = deepcopy(identity)
        self.folder.mkdir(parents=True, exist_ok=True)
        self.path = self.folder / 'rows.jsonl'
        wanted = {'schema': 'pitgun.offline-native-journal/v1', 'identity': identity, 'matrix': matrix}
        manifest = self.folder / 'manifest.json'
        if manifest.exists():
            if load(manifest) != wanted:
                raise ValueError('journal identity or ordered matrix changed')
        else:
            if self.path.exists() and self.path.stat().st_size:
                raise ValueError('journal rows without manifest')
            write(manifest, wanted)
        self.records = self._recover()
        for i, record in enumerate(self.records):
            self._validate(i, record)

    def _recover(self):
        if not self.path.exists():
            return []
        raw = self.path.read_bytes()
        if raw and not raw.endswith(b'\n'):
            boundary = raw.rfind(b'\n') + 1
            tail = raw[boundary:]
            evidence = self.folder / ('interrupted-tail-' + hashlib.sha256(tail).hexdigest() + '.bin')
            evidence.write_bytes(tail)
            with self.path.open('r+b') as stream:
                stream.truncate(boundary)
                stream.flush()
                os.fsync(stream.fileno())
            raw = raw[:boundary]
        return [json.loads(line) for line in raw.splitlines()]

    def _validate(self, index, record):
        if index >= len(self.matrix):
            raise ValueError('too many journal rows')
        item = self.matrix[index]
        filename = key(item['case'], fresh=item['fresh'])
        expected = {'index': index, 'case_sha256': hashlib.sha256(encoded(item['case'])).hexdigest(),
                    'cache_file': filename, 'fresh': item['fresh']}
        if any(record.get(k) != v for k, v in expected.items()):
            raise ValueError('journal sequence or case binding changed')
        path = self.native_cache / filename
        if not path.exists() or sha(path) != record.get('row_sha256'):
            raise ValueError('journal row missing or changed')
        row = load(path)
        if row['case'] != item['case'] or row['data']['case'] != item['case']:
            raise ValueError('journal/native case echo mismatch')
        if row['binary_sha256'] != self.identity['binary_sha256']:
            raise ValueError('journal binary binding changed')
        if row['checks'] != BASE.checks(row['data']):
            raise ValueError('journal native checks changed')
        return row

    def append(self, index, row):
        item = self.matrix[index]
        path = self.native_cache / key(item['case'], fresh=item['fresh'])
        if load(path) != row:
            raise ValueError('cannot checkpoint an unsaved or different row')
        record = {'index': index, 'case_sha256': hashlib.sha256(encoded(item['case'])).hexdigest(),
                  'cache_file': path.name, 'row_sha256': sha(path), 'fresh': item['fresh']}
        if index < len(self.records):
            if self.records[index] != record:
                raise ValueError('already checkpointed row changed')
            self._validate(index, record)
            return
        if index != len(self.records):
            raise ValueError('checkpoint out of order')
        self._validate(index, record)
        with self.path.open('ab') as stream:
            stream.write(encoded(record) + b'\n')
            stream.flush()
            os.fsync(stream.fileno())
        self.records.append(record)

    def rows(self):
        return [self._validate(i, record) for i, record in enumerate(self.records)]

    def export(self, path, metadata):
        if len(self.records) != len(self.matrix):
            raise ValueError('cannot export an incomplete campaign as complete')
        # This is the only archive compression in the checkpoint path.
        write(path, dict(metadata, rows=self.rows(), complete=True,
                         journal_sha256=sha(self.path), journal_entries=len(self.records)))


def validate_limits(max_new_gp, max_wall_seconds):
    if isinstance(max_new_gp, bool) or not isinstance(max_new_gp, int) or max_new_gp < 1:
        raise ValueError('max_new_gp must be a positive integer')
    if not math.isfinite(max_wall_seconds) or max_wall_seconds <= 0:
        raise ValueError('max_wall_seconds must be finite and positive')


def advance_batch(worker, journal, matrix, progress_path, max_new_gp=20,
                  max_wall_seconds=300., clock=time.monotonic, started=None):
    """Stop between cases. A cache hit never consumes a new-GP allowance."""
    validate_limits(max_new_gp, max_wall_seconds)
    started = clock() if started is None else started
    timings, new_gp, stop_reason = [], 0, None
    for i, item in enumerate(matrix):
        if i < len(journal.records):
            continue
        if clock() - started >= max_wall_seconds:
            stop_reason = 'wall_time_limit'
            break
        cached = (worker.cache / key(item['case'], fresh=item['fresh'])).is_file()
        if not cached and new_gp >= max_new_gp:
            stop_reason = 'new_gp_limit'
            break
        before = clock()
        row = worker.execute(item['case'], fresh=item['fresh'])
        native = clock() - before
        if not cached:
            new_gp += 1
        before = clock()
        journal.append(i, row)
        elapsed = clock() - before
        timings.append({'index': i, 'native_or_cache_wall_s': native, 'checkpoint_wall_s': elapsed})
        write(progress_path, {'completed': i + 1, 'expected': len(matrix),
              'last_case': item['case']['id'], 'last_checks': row['checks']})
        print(f'{i+1}/{len(matrix)} {item["case"]["id"]} {row["checks"] or "OK"}', flush=True)
    complete = len(journal.records) == len(matrix)
    return {'schema': 'pitgun.offline-native-batch-progress/v1',
            'complete': complete, 'completed': len(journal.records), 'expected': len(matrix),
            'remaining': len(matrix) - len(journal.records), 'new_gp_this_batch': new_gp,
            'stop_reason': None if complete else stop_reason,
            'limits': {'max_new_gp': max_new_gp, 'max_wall_seconds': max_wall_seconds},
            'timings': timings}


def execute_matrix(plan_path, binary, cache, output, matrix, preflight=None,
                   max_new_gp=20, max_wall_seconds=300.):
    validate_limits(max_new_gp, max_wall_seconds)
    started = time.monotonic()
    plan_path, cache, output = Path(plan_path), Path(cache), Path(output)
    p = load(plan_path)
    # Include this new orchestration and protocol without altering the V1 runner.
    p['retained_files'] = dict(p['retained_files'])
    p['retained_files'][str(plan_path.resolve().relative_to(ROOT))] = sha(plan_path)
    for folder in {HERE, plan_path.resolve().parent}:
        for path in folder.glob('*.py'):
            p['retained_files'][str(path.relative_to(ROOT))] = sha(path)
    worker = BASE.Campaign(Path(binary), cache / 'native', p)
    if worker.identity['binary_sha256'] != p['native_binary_sha256']:
        raise ValueError('not the frozen native executable')
    matrix_digest = hashlib.sha256(encoded(matrix)).hexdigest()
    if output.exists():
        previous = load(output)
        if (previous.get('provenance', {}).get('identity') != worker.identity
            or previous.get('source_matrix_sha256') != matrix_digest):
            raise ValueError('archive belongs to another source/matrix; use a new results namespace')
    preflight_metadata = preflight(worker, p, matrix, cache) if preflight is not None else None
    journal = Journal(cache / 'journal', worker.cache, worker.identity, matrix)
    progress = advance_batch(worker, journal, matrix, cache / 'progress.json',
        max_new_gp=max_new_gp, max_wall_seconds=max_wall_seconds, started=started)
    worker.verify()
    # Timings are operational observations; keep resume segments separately.
    if progress['timings']:
        write(cache / ('timings-' + hashlib.sha256(encoded(progress['timings'])).hexdigest() + '.json'), progress['timings'])
    summary = {k: v for k, v in progress.items() if k != 'timings'}
    write(cache / 'progress.json', summary)
    if not progress['complete']:
        # A bounded partial batch never publishes a complete archive or decision.
        return summary
    metadata = {'schema': p.get('results_schema', 'pitgun.fixed-nine-mexico-pilot-results/v2'),
        'plan': load(plan_path), 'provenance': worker.pin, 'source_matrix_sha256': matrix_digest}
    if preflight_metadata is not None:
        metadata['preflight'] = preflight_metadata
    journal.export(output, metadata)
    return load(output)
