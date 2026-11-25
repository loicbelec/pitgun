from copy import deepcopy
import gzip
import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
from harness import BASE, HERE, ROOT, Journal, encoded, key, load, write
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
