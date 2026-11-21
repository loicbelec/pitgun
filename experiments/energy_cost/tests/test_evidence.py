"""Historical native evidence integrity; no cross-platform performance assertion."""
import hashlib
import json
import pathlib
import unittest
ROOT=pathlib.Path(__file__).resolve().parents[3]
HERE=ROOT/'experiments/energy_cost'
class CostEvidenceTests(unittest.TestCase):
    def test_frozen_physics_and_binaries_remain_identified(self):
        path=HERE/'results/local-cost-v1.json';raw=path.read_bytes();r=json.loads(raw)
        self.assertEqual(path.with_suffix('.sha256').read_text().strip(),'sha256:'+hashlib.sha256(raw).hexdigest())
        baseline=json.loads((ROOT/r['plan']['baseline']).read_bytes())
        self.assertEqual(r['identity']['binaries']['before'],baseline['identity']['probe_digest'])
        self.assertEqual(len(r['historical_parity']),10)
        self.assertTrue(all(c['exact_frozen_output_equal'] for c in r['historical_parity']))
        self.assertEqual(len(r['paired']),12)
        self.assertTrue(all(c['exact_full_physical_outputs_equal'] for c in r['paired']))
        expected={c['id']:c['outcomes']['energy']['competitors'] for c in baseline['cases']}
        self.assertEqual(set(expected),{c['id'] for c in r['paired']})
        for case in r['paired']:
            self.assertEqual(case['result_digests'],{car:v['output'].get('result_digest') for car,v in expected[case['id']].items()})
        self.assertEqual(len(r['sensitivities']),11)
        self.assertTrue(all(c['exact_full_physical_outputs_equal'] for c in r['sensitivities']))
        fuel=next(c for c in r['paired'] if c['id']=='barcelona-modern-66-fuel-feasibility')
        self.assertIn('fuel depleted',fuel['failures']['player'])
        self.assertIsNone(fuel['result_digests']['player'])
    def test_observation_cadence_changes_rows_but_preserves_native_result(self):
        r=json.loads((HERE/'results/local-cost-v1.json').read_bytes())
        baseline=json.loads((ROOT/r['plan']['baseline']).read_bytes())
        expected={c['id']:c['outcomes']['energy']['competitors'] for c in baseline['cases']}
        for case in r['cadence']:
            self.assertTrue(case['physics_exact_at_all_rates'])
            for car,ref in expected[case['id']].items():
                outputs=[case['outcomes'][str(hz)]['cars'][car] for hz in [1,2,5]]
                self.assertTrue(all(o['result_digest']==ref['output']['result_digest'] for o in outputs))
                self.assertLess(outputs[0]['observation_rows'],outputs[1]['observation_rows'])
                self.assertLess(outputs[1]['observation_rows'],outputs[2]['observation_rows'])
                self.assertLess(outputs[0]['observation_json_bytes'],outputs[2]['observation_json_bytes'])
                self.assertTrue(all(o['last_observed_time_s']==o['total_time_s'] for o in outputs))
if __name__=='__main__':unittest.main()
