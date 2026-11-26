from copy import deepcopy
from pathlib import Path
import tempfile
import unittest

import run
import candidate as frozen


class ReserveTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.plan = run.load(run.HERE/'plan-v1.json')
        cls.candidate = run.load(run.HERE/'candidate-v1.json')

    def test_candidate_regenerates_from_development_only(self):
        self.assertEqual(frozen.build(), self.candidate)
        self.assertFalse(self.candidate['admitted'])
        self.assertEqual(len(self.candidate['contexts']),10)

    def test_finite_matrix_fresh_roles_and_exact_aliases(self):
        for era, expected in [(1,(105,99,0)),(2,(234,231,3))]:
            matrix, ev = run.design(self.plan,era)
            primaries = {m['case']['id']:m['case'] for m in matrix if not m['fresh']}
            self.assertEqual((len(matrix),len(ev),len(ev)-len(primaries)),expected)
            for e in ev:
                self.assertEqual(run.semantic(e['logical_case']),run.semantic(primaries[e['native_case_id']]))
            self.assertEqual({m['case']['seed']for m in matrix},{2601104729,2601130363,2601155921})
            self.assertEqual(len([m for m in matrix if m['fresh']]),6)
            self.assertEqual({m['case']['actor']for m in matrix if m['fresh']},{'franz_hermann','charles_leclair'})

    def test_integer_budgets_floors_plans_and_no_extrapolation(self):
        before = deepcopy(self.candidate)
        for ctx in self.candidate['contexts']:
            era, circuit = ctx['era'],ctx['circuit']
            lo,hi = (8,12) if era==1 else (12,24)
            for budget in range(lo,hi+1):
                for family in ['free','zero_extra']:
                    p = frozen.reference(self.candidate,era,circuit,budget,family)
                    self.assertEqual(sum(p['effective_allocation']),budget)
                    floors = [0,1,0,0]if era==1 and budget==12 else run.BASE.floors(era)
                    self.assertTrue(all(f<=v<=20 for v,f in zip(p['effective_allocation'],floors)))
                    if family=='zero_extra':
                        self.assertEqual(p['effective_allocation'][0],floors[0])
                        self.assertEqual(p['effective_allocation'][3],floors[3])
                    p['strategy']['pit_laps'].append(1000)
            for budget in [lo-1,hi+1,True,lo+.5]:
                with self.assertRaises(ValueError): frozen.reference(self.candidate,era,circuit,budget,'free')
                with self.assertRaises(ValueError): frozen.field(self.candidate,era,circuit,budget,'franz_hermann')
        self.assertEqual(self.candidate,before)

    def test_intermediate_field_method_remains_the_frozen_one(self):
        i_source=self.candidate['field_sources']['1'];ii_source=self.candidate['field_sources']['2']
        p=frozen.field(self.candidate,1,'MONZA',9,'franz_hermann')
        old=run.BASE.preparation(run.load(run.ROOT/i_source['policy']),1,'MONZA',9,'franz_hermann')
        self.assertEqual(p,{k:old[k]for k in frozen.PREP_KEYS})
        p=frozen.field(self.candidate,2,'MEXICO',20,'charles_leclair')
        old=frozen.joint.preparation(run.load(run.ROOT/ii_source['policy']),'MEXICO',20,'charles_leclair',run.BASE.scale)
        self.assertEqual(p,{k:old[k]for k in frozen.PREP_KEYS})
        with self.assertRaises(ValueError): frozen.field(self.candidate,3,'MONZA',24,'franz_hermann')

    def test_opened_seed_rejected_before_worker_call(self):
        matrix,_=run.design(self.plan,1);matrix[0]['case']['seed']=73013
        with tempfile.TemporaryDirectory()as folder:
            with self.assertRaisesRegex(ValueError,'already opened'):
                run.preflight(object(),self.plan,matrix,Path(folder))

    def test_wrong_native_hardware_rejected(self):
        class Worker:
            def execute(self,case,resolve=False):
                return {'returncode':0,'data':{'resolved_only':True,'receipt':{'components':{'wrong':True}}}}
        plan=dict(self.plan,prior_archives=[]);matrix,_=run.design(plan,1)
        with tempfile.TemporaryDirectory()as folder:
            with self.assertRaisesRegex(ValueError,'hardware'):
                run.preflight(Worker(),plan,matrix,Path(folder))

    def test_partial_evidence_cannot_publish_decision(self):
        data={'plan':self.plan,'complete':False,'rows':[]}
        with self.assertRaisesRegex(ValueError,'incomplete'):
            run.analyze(data,1)

    def test_changed_matrix_count_rejected(self):
        plan=deepcopy(self.plan);plan['expected']['2']['native']+=1
        with self.assertRaisesRegex(ValueError,'counts'):run.design(plan,2)


if __name__=='__main__':unittest.main()
