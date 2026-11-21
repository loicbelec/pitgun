"""Cheap evidence integrity and refusal checks, not cross-platform replay claims."""
import copy
import hashlib
import importlib.util
import json
import pathlib
import unittest

HERE = pathlib.Path(__file__).resolve().parents[1]
ROOT = HERE.parents[1]
spec = importlib.util.spec_from_file_location('energy_runner', HERE/'run_local.py')
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)


class EnergyEvidenceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.path = HERE/'results/local-energy-v1.json'
        cls.report = json.loads(cls.path.read_bytes())

    def test_archive_identifies_sources_and_retains_frozen_legacy_digests(self):
        r = self.report
        digest = lambda raw: 'sha256:'+hashlib.sha256(raw).hexdigest()
        self.assertEqual(self.path.with_suffix('.sha256').read_text().strip(), digest(self.path.read_bytes()))
        self.assertEqual(r['identity']['plan_digest'], digest((HERE/'plan-v1.json').read_bytes()))
        # Historical evidence must not freeze every later model's source bytes.
        # Live campaigns check the actual tree and compiled source identities.
        for name, path in runner.SOURCES.items():
            self.assertEqual(r['identity']['compiled_sources'][name], r['identity']['source_files'][path])
        for value in r['identity']['source_files'].values():
            self.assertRegex(value, r'^sha256:[0-9a-f]{64}$')
        previous = json.loads((ROOT/'experiments/physics_foundation/results/local-baseline-v1.json').read_bytes())
        old = {c['case_id']:c['outcome']['result']['output_digest'] for c in previous['cases']}
        self.assertEqual(old, {c['id']:c['output_digest'] for c in r['historical_canonical_parity']})
        self.assertTrue(all(c['exact_frozen_output_equal'] for c in r['historical_canonical_parity']))

    def test_required_cases_complete_but_feasibility_failure_remains_explicit(self):
        r = self.report
        required = [c for c in r['cases'] if c['required_completion']]
        self.assertEqual(len(required), 11)
        for c in required:
            self.assertTrue(c['same_binary_repetition_equal'])
            for mode in ('legacy', 'energy'):
                for v in c['outcomes'][mode]['competitors'].values():
                    self.assertTrue(v['output']['ok'])
                    if mode == 'energy':
                        runner.validate_energy(v['output'], r['criteria'])
        failures = [c for c in r['cases'] if not c['required_completion']]
        self.assertEqual([c['id'] for c in failures], ['barcelona-modern-66-fuel-feasibility'])
        for mode in ('legacy', 'energy'):
            v = failures[0]['outcomes'][mode]['competitors']['player']['output']
            self.assertFalse(v['ok'])
            self.assertIn('fuel depleted', v['error'])
            self.assertNotIn('total_time_s', v)

    def test_accounting_guard_refuses_corrupted_work_clock_fuel_and_raw_grip(self):
        good = self.report['cases'][0]['outcomes']['energy']['competitors']['player']['output']
        for field in ('vehicle_work_residual_j', 'mass_service_boundary_residual_j',
                      'shaft_work_residual_j', 'thermal_storage_residual_j',
                      'clock_residual_s', 'fuel_mass_residual_kg'):
            bad = copy.deepcopy(good)
            bad['checks'][field] += 1
            with self.subTest(field=field), self.assertRaises(ValueError):
                runner.validate_energy(bad, self.report['criteria'])
        bad = copy.deepcopy(good)
        bad['energy']['maximum_raw_contact_utilization'] = 1.001
        with self.assertRaisesRegex(ValueError, 'raw contact'):
            runner.validate_energy(bad, self.report['criteria'])

    def test_timing_exclusion_does_not_erase_physical_differences(self):
        a = {'solve_ms':1, 'final_state':{'engine_temp':100}, 'result_digest':'one'}
        b = {**a, 'solve_ms':2}
        self.assertEqual(runner.stable(a), runner.stable(b))
        options = {'options':('1', '0.5', '55', 'smooth-driver')}
        self.assertEqual(runner.stable(options), runner.stable(json.loads(json.dumps(options))))
        b['final_state'] = {'engine_temp':101}
        self.assertNotEqual(runner.stable(a), runner.stable(b))


if __name__ == '__main__':
    unittest.main()
