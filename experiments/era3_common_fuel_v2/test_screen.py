import unittest
from copy import deepcopy
from unittest.mock import patch
import screen

class PairedChargeTests(unittest.TestCase):
 def setUp(self):self.plan=screen.legacy.load(screen.HERE/'plan-v1.json');self.old=screen.legacy.load(screen.ROOT/self.plan['paired_archive'])
 def test_exact100paired_cases_only_change_mass_and_record_id(self):
  matrix=screen.design(self.plan);self.assertEqual(len(matrix),100);self.assertEqual(self.plan['minimum_finish_reserve_kg'],5.)
  for item,old in zip(matrix,self.old['rows']):
   expected=deepcopy(old['case']);expected['id']=expected['id'].replace('/common160/','/common140/');expected['initial_fuel_kg']=140.
   self.assertEqual(item['case'],expected)
   receipt=deepcopy(old['data']['receipt']);receipt['initial_state']['fuel_mass']=140.
   screen.validate_pair(item['case'],receipt,old)
 def test_other_preparation_or_physical_change_rejects(self):
  case=screen.design(self.plan)[0]['case'];old=self.old['rows'][0];r=deepcopy(old['data']['receipt']);r['initial_state']['fuel_mass']=140.;r['pit_loss_s']+=1
  with self.assertRaises(ValueError):screen.validate_pair(case,r,old)
  r=deepcopy(old['data']['receipt']);r['initial_state']['fuel_mass']=140.;case['gear']+=.01
  with self.assertRaises(ValueError):screen.validate_pair(case,r,old)
 def test_review_cannot_qualify_an_incomplete_comparison(self):
  with self.assertRaises(ValueError):screen.review({'complete':False})

if __name__=='__main__':unittest.main()
