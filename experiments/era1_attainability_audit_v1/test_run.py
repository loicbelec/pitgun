from copy import deepcopy
from fractions import Fraction
import importlib.util
from pathlib import Path
import unittest
from unittest.mock import patch
import sys
import tempfile

HERE=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('era1_attainability',HERE/'run.py')
AUDIT=importlib.util.module_from_spec(spec);spec.loader.exec_module(AUDIT)


class AuditTests(unittest.TestCase):
    def setUp(self):self.plan=AUDIT.load(HERE/'plan-v1.json')

    def test_frozen_counts_and_exactly_equal_family_allowances(self):
        matrix,evals=AUDIT.design(self.plan)
        self.assertEqual(len(evals),162);self.assertEqual(len(matrix),153)
        self.assertEqual(sum(not r['fresh']for r in matrix),147)
        for ctx in self.plan['contexts']:
            for budget in self.plan['budgets']:
                for family in self.plan['families']:
                    group=[e for e in evals if e['circuit']==ctx['circuit'] and e['budget']==budget and e['family']==family]
                    self.assertEqual(len(group),6)
                    self.assertEqual(len({AUDIT.encoded(AUDIT.semantic(e['logical_case']))for e in group}),6)
                    for e in group:
                        c=e['logical_case'];self.assertTrue(AUDIT.permitted(c,family))
                        self.assertEqual(sum(s['laps']for s in c['strategy']['stints']),ctx['laps'])

    def test_aliases_differ_only_by_nonphysical_id_and_repeats_are_fixed(self):
        matrix,evals=AUDIT.design(self.plan)
        lookup={r['case']['id']:r['case']for r in matrix if not r['fresh']}
        self.assertEqual(len(evals)-len(lookup),15)
        for e in evals:self.assertEqual(AUDIT.semantic(e['logical_case']),AUDIT.semantic(lookup[e['native_case_id']]))
        repeats=matrix[-6:];self.assertTrue(all(r['fresh']for r in repeats))
        self.assertTrue(all(r['case']['effective_budget']==12 and r['case']['id'].endswith('/baseline')for r in repeats))
        self.assertEqual(len({r['case']['id']for r in repeats}),6)

    def test_actual_floors_and_controlled_midpoint_are_not_confused(self):
        _,evals=AUDIT.design(self.plan)
        for e in evals:
            c=e['logical_case'];budget=e['budget']
            self.assertEqual(c['kind'],'field'if budget==10 else'career')
            self.assertEqual(c['fixture_id'],'craft_engineering:era1-exit'if budget==12 else'craft_engineering:era1-entry')
            self.assertEqual(c['downforce'],.5);self.assertEqual(sum(c['effective_allocation']),budget)
            if budget==12:self.assertGreaterEqual(c['effective_allocation'][1],1)
            if e['family']=='zero_extra':self.assertEqual([c['effective_allocation'][i]for i in [0,3]],[0,0])
            if e['family']=='heavy_80':
                a=c['effective_allocation'];f=AUDIT.offsets(budget)
                self.assertGreaterEqual(Fraction(a[1]-f[1]+a[2]-f[2],budget-sum(f)),Fraction(4,5))
        self.assertTrue(any(e['family']=='free'and e['logical_case']['effective_allocation'][0]>0 for e in evals))

    def test_field_reuse_exact_requests_exist_without_player_reactivity(self):
        sources=AUDIT.field_sources(self.plan)
        self.assertEqual(len(sources),81)
        self.assertTrue(all(meta['reused'] for _,meta in sources))
        self.assertEqual(len({(meta['actor'],meta['circuit'],meta['budget'])for _,meta in sources}),81)
        self.assertTrue(all(c['actor']!='balanced_reference' for c,_ in sources))
        bad=deepcopy(self.plan);bad['retained_files'][bad['field_archive']]='0'*64
        with self.assertRaises(ValueError):AUDIT.field_sources(bad)

    def test_resolved_field_mismatch_blocks_preflight(self):
        class WrongResolution:
            def execute(self,case,resolve=False):
                self.resolve_only=resolve
                return {'data':{'resolved_only':True,'receipt':{'physical_input_digest':'wrong'}}}
        worker=WrongResolution();matrix,_=AUDIT.design(self.plan)
        with tempfile.TemporaryDirectory() as cache:
            with self.assertRaises(ValueError):AUDIT.preflight(worker,self.plan,matrix,Path(cache))
        self.assertTrue(worker.resolve_only)

    def test_partial_data_cannot_produce_an_audit_decision(self):
        with self.assertRaises(ValueError):AUDIT.analyze({'plan':self.plan,'complete':False,'rows':[]})


if __name__=='__main__':unittest.main()
