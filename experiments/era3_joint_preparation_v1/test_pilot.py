import unittest
from copy import deepcopy
import run
from unittest.mock import patch
import tempfile
from pathlib import Path

class PilotTests(unittest.TestCase):
 def test_matrix_is_finite_and_repeat_is_fresh(self):
  m=run.pilot_matrix();self.assertEqual(len(m),22);self.assertTrue(m[-1]['fresh']);self.assertEqual(m[-1]['case'],m[0]['case']);self.assertEqual(len({x['case']['id']for x in m[:-1]}),21)
 def test_actual_component_boundaries_and_full_contexts(self):
  d=run.fixtures();self.assertEqual(len(d['fixtures']),9)
  for f in d['fixtures']:
   self.assertEqual(f['era'],3);self.assertEqual(len(f['regularCircuits']),9);self.assertEqual(set(f['invitationCircuits']),{'MONACO','SPA'});self.assertEqual(len(f['preparedRequests']),11)
   self.assertEqual(f['technology']['components']['engine_id'],'v8_1970'if f['stage']=='era3-entry'else'v6t')
  self.assertEqual([run.cap(run.fixture(s))for s in ['entry','developed','exit']],[24,34,43])
 def test_available_points_remain_distinct_from_saturated_spending(self):
  c=run.make_case('balanced_reference','neutral',True,budget=82);self.assertEqual(c['effective_budget'],82);self.assertEqual(sum(c['effective_allocation']),80);self.assertEqual(c['effective_allocation'],[20]*4)
 def test_all_proposals_respect_floor_distance_and_stop_count(self):
  for row in run.pilot_matrix():
   c=row['case'];f=run.fixture(c['fixture_id'].split('-')[-1]);self.assertLessEqual(sum(c['effective_allocation']),c['effective_budget']);self.assertTrue(all(lo<=v<=20 for lo,v in zip(run.floors(f),c['effective_allocation'])));self.assertIn(len(c['strategy']['pit_laps']),[1,2]);self.assertEqual(sum(s['laps']for s in c['strategy']['stints']),66)


class ExplicitBudgetChecks(unittest.TestCase):
 def test_accepts_valid_unused_points_without_masking_other_failures(self):
  case=run.make_case('balanced_reference','neutral',True,budget=82)
  data={'case':case,'outcome':{'ok':True},'receipt':{'budget':82,'points':[20]*4,'spent_budget':80,'unused_budget':2,'initial_state':{'fuel_mass':100.0}}}
  with patch.object(run.BASE,'checks',return_value=['budget_binding','incomplete_finish']):
   self.assertEqual(run.checks(data),['incomplete_finish'])
   data['receipt']['unused_budget']=0
   self.assertEqual(run.checks(data),['incomplete_finish','available_spent_budget_binding'])
 def test_extended_journal_preserves_resume_and_rejects_row_mutation(self):
  case=run.make_case('balanced_reference','neutral',True,budget=82)
  with tempfile.TemporaryDirectory()as folder,patch.object(run,'checks',return_value=[]):
   root=Path(folder);native=root/'native';native.mkdir();identity={'binary_sha256':'frozen'};matrix=[{'case':case,'fresh':False}]
   row={'case':case,'data':{'case':case},'binary_sha256':'frozen','checks':[]}
   path=native/run.key(case);run.write(path,row)
   journal=run.EraJournal(root/'journal',native,identity,matrix);journal.append(0,row)
   resumed=run.EraJournal(root/'journal',native,identity,matrix);self.assertEqual(resumed.rows(),[row])
   row['checks']=['changed'];run.write(path,row)
   with self.assertRaises(ValueError):run.EraJournal(root/'journal',native,identity,matrix)

class FailedPilotReport(unittest.TestCase):
 def test_partial_fuel_outcomes_do_not_require_a_success_digest(self):
  row={'case':{'id':'incomplete'},'data':{'case':{'id':'incomplete'},'outcome':{'ok':False,'error':'fuel shortfall'},'laps':[],'receipt':{'budget':82,'spent_budget':80}},'wall_s':1.0,'checks':['physical_or_resolution_failure']}
  summary=run.summarize_rows([row,deepcopy(row)])
  self.assertEqual(summary['attemptedGP'],2);self.assertEqual(summary['completedGP'],0);self.assertTrue(summary['exactFullOrPartialRepeat']);self.assertEqual(len(summary['failedCases']),2)

class CeilingDesign(unittest.TestCase):
 def test_four_counterfactuals_change_only_declared_initial_fuel(self):
  import ceiling_compare
  plan=run.load(run.HERE/'ceiling-plan-v1.json');before=run.load(run.HERE/plan['pilot_archive']);m=ceiling_compare.matrix(plan)
  self.assertEqual(len(m),4);self.assertTrue(m[-1]['fresh']);self.assertEqual(m[0]['case'],m[-1]['case'])
  for row,index in zip(m,plan['pilot_row_indices']+[plan['pilot_row_indices'][0]]):
   c=deepcopy(row['case']);self.assertEqual(c.pop('initial_fuel_kg'),110.0);c['id']=c['id'].removesuffix('/explicit-start110');self.assertEqual(c,before['rows'][index]['case'])

if __name__=='__main__':unittest.main()
