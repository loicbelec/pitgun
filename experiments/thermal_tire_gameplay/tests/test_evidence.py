"""Protect frozen audit identity, failures and interpretation; no physical-fit claims."""
import hashlib,importlib.util,json,pathlib,unittest
HERE=pathlib.Path(__file__).resolve().parents[1]
class EvidenceTests(unittest.TestCase):
 def load(self,name):
  p=HERE/'results'/f'{name}.json';raw=p.read_bytes();self.assertEqual(p.with_suffix('.sha256').read_text().strip(),'sha256:'+hashlib.sha256(raw).hexdigest());return json.loads(raw)
 def test_primary_archive_preserves_preflight_failures_and_service_boundaries(self):
  r=self.load('local-gameplay-v1');self.assertEqual(len(r['preflight']),22);self.assertTrue(all(x['frozen_result_equal'] for x in r['preflight']));self.assertEqual(len(r['results']),164)
  self.assertEqual(len(r['analysis']['failures']),4);self.assertTrue(all('fuel depleted' in x['error'] for x in r['analysis']['failures']))
  stops=[lap for row in r['results'] for lap in row['data']['laps'] if lap['pit_stop_completed']]
  self.assertTrue(stops);self.assertTrue(all(x['after_service']['tire_wear']==0 and x['wear_final']>0 for x in stops))
  self.assertEqual(r['status'],'AUDIT_COMPLETE_NOT_A_GAMEPLAY_ACCEPTANCE')
 def test_grid_uses_equal_budgets_and_reports_minima_of_actual_completed_runs(self):
  r=self.load('budget-grid-v1');self.assertEqual(r['allocations'],187);self.assertEqual(len(r['rows']),374);self.assertFalse(r['failures'])
  budgets={g['id']:g['budget'] for g in r['plan']['groups']}
  for row in r['rows']:
   values=[v for k,v in row['tuning'].items() if k.endswith('_points')];self.assertEqual(sum(values),budgets[row['group']]);self.assertTrue(all(0<=x<=20 for x in values))
  for winner in r['winners']:
   eligible=[row for row in r['rows'] if row['group']==winner['group'] and row['model']==winner['model'] and (row['tuning']['cooling_points']==0)==(winner['category']=='zero') and row['outcome']['ok']]
   self.assertEqual(winner['full_data']['outcome']['total_time_s'],min(x['outcome']['total_time_s'] for x in eligible));self.assertTrue(winner['exact_winner_repeated'])
 def test_fine_grid_preserves_budgets_and_repeats_both_selected_minima(self):
  r=self.load('grid-refinement-v1');self.assertEqual(len(r['rows']),72);self.assertFalse(r['failures'])
  for row in r['rows']:
   car=row['request']['input']['competitors'][0];t=car['tuning'];self.assertEqual(sum(v for k,v in t.items() if k.endswith('_points')),car['budget_cap']);self.assertIn(t['cooling_points'],range(1,7))
  self.assertEqual(len(r['comparisons']),8);self.assertTrue(all(c['zero_and_selected_positive_repeated_exactly'] for c in r['comparisons']))
 def test_driver_followup_does_not_claim_policy_labels_are_physical_driver_controls(self):
  r=self.load('driver-followup-v1');self.assertEqual(len(r['results']),4)
  controls=[row['data']['resolved']['driver'] for row in r['results'] if row['model']=='energy']
  self.assertNotEqual(controls[0],controls[1]);self.assertIn('not a one-factor',r['reason'])
if __name__=='__main__':unittest.main()
