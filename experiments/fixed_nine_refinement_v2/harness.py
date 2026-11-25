"""Offline native runner with durable row references and one final archive.

Native execution/checking is retained from V1. Checkpoints do not recompress rows.
"""
from copy import deepcopy
import hashlib
import importlib.util
import json
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


def execute_matrix(plan_path, binary, cache, output, matrix, preflight=None):
    plan_path, cache = Path(plan_path), Path(cache)
    p = load(plan_path)
    # Include this new orchestration and protocol without altering the V1 runner.
    p['retained_files'] = dict(p['retained_files'])
    p['retained_files'][str(plan_path.resolve().relative_to(ROOT))] = sha(plan_path)
    for path in HERE.glob('*.py'):
        p['retained_files'][str(path.relative_to(ROOT))] = sha(path)
    worker = BASE.Campaign(Path(binary), cache / 'native', p)
    if worker.identity['binary_sha256'] != p['native_binary_sha256']:
        raise ValueError('not the frozen native executable')
    if preflight is not None:
        preflight(worker, p, matrix, cache)
    journal = Journal(cache / 'journal', worker.cache, worker.identity, matrix)
    timings = []
    for i, item in enumerate(matrix):
        if i < len(journal.records):
            continue
        before = time.monotonic()
        row = worker.execute(item['case'], fresh=item['fresh'])
        native = time.monotonic() - before
        before = time.monotonic()
        journal.append(i, row)
        elapsed = time.monotonic() - before
        timings.append({'index': i, 'native_or_cache_wall_s': native, 'checkpoint_wall_s': elapsed})
        # Tiny progress only; all result bodies stay in the immutable native rows.
        write(cache / 'progress.json', {'completed': i + 1, 'expected': len(matrix),
              'last_case': item['case']['id'], 'last_checks': row['checks']})
        print(f'{i+1}/{len(matrix)} {item["case"]["id"]} {row["checks"] or "OK"}', flush=True)
    worker.verify()
    # Timings are operational observations; keep resume segments separately.
    if timings:
        write(cache / ('timings-' + hashlib.sha256(encoded(timings)).hexdigest() + '.json'), timings)
    journal.export(output, {'schema': 'pitgun.fixed-nine-mexico-pilot-results/v2',
        'plan': load(plan_path), 'provenance': worker.pin,
        'source_matrix_sha256': hashlib.sha256(encoded(matrix)).hexdigest()})
    return load(output)
