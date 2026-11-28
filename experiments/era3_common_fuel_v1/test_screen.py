import unittest
from copy import deepcopy
import hashlib
import json
from unittest.mock import patch
import screen

class CommonFuelScreen(unittest.TestCase):
 def setUp(self):self.plan=screen.legacy.load(screen.HERE/'plan-v1.json')
 def test_finite_matrix_equal_charge_legal_points_and_official_distances(self):
  items=screen.design(self.plan);self.assertEqual(len(items),100);self.assertEqual(len(screen.context_ids()),12)
  self.assertEqual(len({m['case']['id']for m in items[:96]}),96)
  self.assertEqual({m['case']['initial_fuel_kg']for m in items},{160.})
  self.assertEqual(len([m for m in items if m['fresh']]),4)
  for m in items:
   c=m['case'];self.assertEqual(c['fixture_revision'],2);self.assertLessEqual(sum(c['effective_allocation']),c['effective_budget']);self.assertTrue(all(0<=p<=20 for p in c['effective_allocation']))
   f=next(f for f in screen.corpus()['fixtures']if f['id']==c['fixture_id']);expected=next(r['request']['input']['laps']for r in f['preparedRequests']if r['circuitId']==c['circuit'])
   self.assertEqual(sum(s['laps']for s in c['strategy']['stints']),expected);self.assertIn(len(c['strategy']['pit_laps']),[1,2])
 def test_v2_only_adds_contexts_without_replacing_original_requests(self):
  original=screen.legacy.fixtures();updated=screen.corpus();self.assertEqual(len(updated['fixtures']),9)
  for a,b in zip(original['fixtures'],updated['fixtures']):
   self.assertEqual(a['id'],b['id']);self.assertEqual(b['preparedRequests'][:11],a['preparedRequests']);self.assertEqual(b['preparedRequests'][-1]['circuitId'],'SAOPAULO');self.assertIn('SAOPAULO',b['practiceCircuits']);self.assertEqual(a['technology'],b['technology'])
 def sample(self):
  matrix=screen.design(self.plan);rows=[]
  for m in matrix:
   rows.append({'case':deepcopy(m['case']),'binary_sha256':self.plan['native_binary_sha256'],'checks':[],'wall_s':1.,'data':{'case':deepcopy(m['case']),'outcome':{'ok':True,'time_s':6000.,'final_state':{'fuel_mass':10.}},'laps':[{'minimum_power_fraction':1.,'surface_max_c':100.}]}})
  return {'complete':True,'plan':self.plan,'rows':rows,'source_matrix_sha256':hashlib.sha256(screen.legacy.encoded(matrix)).hexdigest()}
 def test_reserve_and_repeats_are_required_without_hiding_failure(self):
  data=self.sample()
  with patch.object(screen.legacy,'checks',return_value=[]):
   self.assertEqual(screen.review(data)['status'],'PASS')
   data['rows'][0]['data']['outcome']['final_state']['fuel_mass']=4.
   review=screen.review(data);self.assertEqual(review['status'],'REFINE');self.assertTrue(any(x['reason']=='finish_reserve'for x in review['failures']))
 def test_partial_or_changed_matrix_cannot_be_reviewed(self):
  data=self.sample();data['complete']=False
  with self.assertRaises(ValueError):screen.review(data)
  data=self.sample();data['rows'][0]['case']['initial_fuel_kg']=159.
  with self.assertRaises(ValueError):screen.review(data)
 def test_changed_fresh_repeat_fails_even_with_enough_fuel(self):
  data=self.sample();data['rows'][-1]['data']['outcome']['time_s']+=1
  with patch.object(screen.legacy,'checks',return_value=[]):self.assertEqual(screen.review(data)['status'],'REFINE')

if __name__=='__main__':unittest.main()
