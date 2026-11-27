from copy import deepcopy
import gzip
import hashlib
import contextlib
import io
import json
from pathlib import Path
import tempfile
import unittest
import sys
from unittest.mock import patch
from harness import BASE, HERE, ROOT, Journal, advance_batch, execute_matrix, encoded, key, load, write
from run_pilot import cases, fairness_checks


class JournalTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        source = load(ROOT / 'experiments/fixed_nine_thermal_v1/results/training-v1.json.gz')
        cls.samples = source['rows'][:2]
        cls.identity = source['provenance']['identity']

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.cache = self.root / 'native'; self.cache.mkdir()
        self.matrix = [{'case': deepcopy(r['case']), 'fresh': False} for r in self.samples]

    def saved(self, i):
        row = deepcopy(self.samples[i])
        write(self.cache / key(row['case']), row)
        return row

    def journal(self, matrix=None, identity=None):
        return Journal(self.root / 'journal', self.cache, identity or self.identity, matrix or self.matrix)

    def test_native_save_before_journal_resume_and_exact_single_export(self):
        first = self.saved(0)  # Process interruption before journal append.
        journal = self.journal()
        with patch.object(BASE.gzip, 'compress', wraps=gzip.compress) as compress:
            journal.append(0, first)
            self.assertEqual(compress.call_count, 0)
            resumed = self.journal()
            resumed.append(0, first)  # Retry is idempotent.
            self.assertEqual(len(resumed.records), 1)
            with self.assertRaises(ValueError):resumed.export(self.root/'partial.gz', {})
            second = self.saved(1); resumed.append(1, second)
            self.assertEqual(compress.call_count, 0)
            resumed.export(self.root/'complete.json.gz', {'schema': 'test'})
            self.assertEqual(compress.call_count, 1)
        result = load(self.root/'complete.json.gz')
        self.assertEqual(result['rows'], [first, second])
        self.assertTrue(result['complete'])
        self.assertEqual(len((self.root/'journal/rows.jsonl').read_text().splitlines()), 2)

    def test_torn_tail_preserved_and_resume_reconstructs_rows(self):
        first = self.saved(0); journal = self.journal(); journal.append(0, first)
        with journal.path.open('ab') as stream:stream.write(b'{"index":1')
        resumed = self.journal()
        self.assertEqual(len(resumed.records), 1)
        tails = list((self.root/'journal').glob('interrupted-tail-*.bin'))
        self.assertEqual(len(tails), 1); self.assertEqual(tails[0].read_bytes(), b'{"index":1')
        second = self.saved(1); resumed.append(1, second)
        self.assertEqual(resumed.rows(), [first, second])

    def test_complete_corruption_is_never_discarded(self):
        first = self.saved(0); journal = self.journal(); journal.append(0, first)
        with journal.path.open('ab') as stream:stream.write(b'not-json\n')
        before = journal.path.read_bytes()
        with self.assertRaises(ValueError):self.journal()
        self.assertEqual(journal.path.read_bytes(), before)

    def test_modified_result_identity_or_order_is_rejected(self):
        first = self.saved(0); journal = self.journal(); journal.append(0, first)
        with self.assertRaises(ValueError):self.journal(matrix=list(reversed(self.matrix)))
        wrong = deepcopy(self.identity); wrong['binary_sha256'] = 'changed'
        with self.assertRaises(ValueError):self.journal(identity=wrong)
        path = self.cache / key(first['case']); data = load(path); data['wall_s'] += 1; write(path, data)
        with self.assertRaises(ValueError):self.journal()

    def test_out_of_order_append_and_unbound_result_are_rejected(self):
        second = self.saved(1); journal = self.journal()
        with self.assertRaises(ValueError):journal.append(1, second)
        first = self.saved(0); changed = deepcopy(first); changed['wall_s'] += 1
        with self.assertRaises(ValueError):journal.append(0, changed)


class FakeWorker:
    """Saved native fixtures only: these unit tests launch no solver process."""
    def __init__(self, cache, samples, identity, clock=None):
        self.cache = cache; self.cache.mkdir(parents=True, exist_ok=True)
        self.samples = {r['case']['id']: deepcopy(r) for r in samples}
        self.identity = identity; self.pin = {'identity': identity}
        self.native_calls = 0; self.clock = clock

    def verify(self): pass

    def execute(self, case, fresh=False):
        path = self.cache / key(case, fresh=fresh)
        if path.exists(): return load(path)
        self.native_calls += 1
        if self.clock is not None: self.clock.now += 2.
        row = deepcopy(self.samples[case['id']]); write(path, row)
        return row


class FakeClock:
    def __init__(self): self.now = 0.
    def __call__(self): return self.now


class BatchTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        source = load(ROOT / 'experiments/fixed_nine_thermal_v1/results/training-v1.json.gz')
        cls.samples = source['rows'][:2]
        cls.identity = source['provenance']['identity']

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(); self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.matrix = [{'case': deepcopy(r['case']), 'fresh': False} for r in self.samples]

    def test_partial_batch_resumes_without_recompute_or_partial_archive(self):
        cache = self.root/'batch'; output=self.root/'result.json.gz'
        worker = FakeWorker(cache/'native', self.samples, self.identity)
        with patch.object(BASE, 'Campaign', return_value=worker), contextlib.redirect_stdout(io.StringIO()):
            first = execute_matrix(HERE/'pilot-plan-v2.json', Path('unused'), cache, output,
                self.matrix, max_new_gp=1)
            self.assertFalse(first['complete']); self.assertEqual(first['stop_reason'], 'new_gp_limit')
            self.assertEqual(first['completed'],1);self.assertFalse(output.exists())
            self.assertEqual(worker.native_calls,1)
            second = execute_matrix(HERE/'pilot-plan-v2.json', Path('unused'), cache, output,
                self.matrix, max_new_gp=1)
        self.assertTrue(second['complete']);self.assertEqual(worker.native_calls,2)
        self.assertEqual(second['rows'],self.samples)

    def test_cached_unjournalled_row_does_not_consume_new_gp_allowance(self):
        worker=FakeWorker(self.root/'native',self.samples,self.identity)
        write(worker.cache/key(self.samples[0]['case']),self.samples[0])
        journal=Journal(self.root/'journal',worker.cache,self.identity,self.matrix)
        with contextlib.redirect_stdout(io.StringIO()):
            p=advance_batch(worker,journal,self.matrix,self.root/'progress.json',max_new_gp=1)
        self.assertTrue(p['complete']);self.assertEqual(p['new_gp_this_batch'],1)
        self.assertEqual(worker.native_calls,1);self.assertEqual(journal.rows(),self.samples)

    def test_wall_limit_finishes_current_gp_and_stops_before_next(self):
        clock=FakeClock();worker=FakeWorker(self.root/'native',self.samples,self.identity,clock)
        journal=Journal(self.root/'journal',worker.cache,self.identity,self.matrix)
        with contextlib.redirect_stdout(io.StringIO()):
            p=advance_batch(worker,journal,self.matrix,self.root/'progress.json',
                max_new_gp=20,max_wall_seconds=1.,clock=clock)
        self.assertFalse(p['complete']);self.assertEqual(p['stop_reason'],'wall_time_limit')
        self.assertEqual(len(journal.records),1);self.assertEqual(worker.native_calls,1)
        self.assertEqual(journal.rows(),self.samples[:1])

    def test_caps_are_operational_and_invalid_values_are_rejected(self):
        worker=FakeWorker(self.root/'native',self.samples,self.identity)
        journal=Journal(self.root/'journal',worker.cache,self.identity,self.matrix)
        for gp,seconds in [(0,300),(-1,300),(True,300),(1,0),(1,float('inf'))]:
            with self.assertRaises(ValueError):
                advance_batch(worker,journal,self.matrix,self.root/'progress.json',max_new_gp=gp,max_wall_seconds=seconds)
        self.assertEqual(worker.native_calls,0)

    def test_another_source_archive_cannot_be_overwritten(self):
        cache=self.root/'batch';output=self.root/'historical.json.gz'
        write(output,{'provenance':{'identity':{'historical':True}}})
        before=output.read_bytes();worker=FakeWorker(cache/'native',self.samples,self.identity)
        with patch.object(BASE,'Campaign',return_value=worker):
            with self.assertRaises(ValueError):
                execute_matrix(HERE/'pilot-plan-v2.json',Path('unused'),cache,output,self.matrix)
        self.assertEqual(worker.native_calls,0);self.assertEqual(output.read_bytes(),before)

    def test_partial_cli_does_not_analyze_or_publish_a_decision(self):
        import run_pilot
        args=['run_pilot','--binary','unused','--cache',str(self.root/'cache'),
              '--output',str(self.root/'result.json.gz'),'--max-new-gp','1']
        with patch.object(sys,'argv',args), patch.object(run_pilot,'execute_matrix',return_value={'complete':False}), \
             patch.object(run_pilot,'analyze') as analyze, contextlib.redirect_stdout(io.StringIO()):
            run_pilot.main()
        analyze.assert_not_called();self.assertFalse((self.root/'pilot-decision-v2.json').exists())


class PilotTests(unittest.TestCase):
    def test_exact_common_matrix_and_predetermined_repeat_cells(self):
        p = load(HERE/'pilot-plan-v2.json'); matrix = cases(p)
        self.assertEqual(len(matrix), 30)
        for budget in p['budgets']:
            for proposal in p['proposals']:
                group = [r['case'] for r in matrix[:24] if r['case']['effective_budget']==budget and r['case']['id'].endswith('/'+proposal['id'])]
                signatures = [{k:v for k,v in c.items()if k not in ['id','actor']} for c in group]
                self.assertEqual(len(signatures),3);self.assertTrue(all(s==signatures[0] for s in signatures))
                self.assertEqual(sum(signatures[0]['effective_allocation']),budget)
        self.assertTrue(all(r['fresh'] for r in matrix[24:]))
        self.assertEqual(p['seed'],271828)  # Opened diagnosis, never a new holdout.

    def test_batching_preserves_the_frozen_pilot_input_matrix(self):
        p=load(HERE/'pilot-plan-v2.json');archive=load(HERE/'results/mexico-pilot-v2.json.gz')
        self.assertEqual(hashlib.sha256(encoded(cases(p))).hexdigest(),archive['source_matrix_sha256'])

    def test_mismatched_resolved_hardware_is_reported(self):
        p=load(HERE/'pilot-plan-v2.json'); matrix=cases(p)
        # Use a retained native receipt as the physically resolved shape.
        source=load(ROOT/'experiments/fixed_nine_thermal_v1/results/validation-v1.json.gz')
        native=next(r for r in source['rows'] if r['case']['id']=='2/MEXICO/player/16/reserved-informed')
        rows=[]
        roster=load(ROOT/'experiments/fixed_opponents_v2/roster-v2.json')['drivers']
        for item in matrix[:24]:
            c=item['case'];row=deepcopy(native);row['case']=c;row['data']['case']=c
            receipt=row['data']['receipt'];receipt['points']=c['effective_allocation'];receipt['budget']=c['effective_budget']
            if c['actor']!='balanced_reference':receipt['driver']['controls']['driver_traits']=roster[c['actor']]['resource']['traits']
            receipt['driver']['controls']['driver_id']=c['actor']
            receipt['driver']['lapse_profile']['noise_stream_id']='career-driver:player'if c['actor']=='balanced_reference'else'fixed-field:'+c['actor']
            rows.append(row)
        self.assertEqual(fairness_checks(rows,p),[])
        rows[4]['data']['receipt']['engine']['n_max']+=100
        self.assertTrue(any(e.startswith('unequal_physical_preparation:')for e in fairness_checks(rows,p)))


if __name__=='__main__':unittest.main()
