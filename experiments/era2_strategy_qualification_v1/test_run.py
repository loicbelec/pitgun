import copy
import hashlib
import json
from pathlib import Path
import unittest

from run import cases, numerical_checks, ROOT

HERE = Path(__file__).parent
PLAN = json.loads((HERE / 'plan-v1.json').read_text())
FIXTURES = json.loads((ROOT / PLAN['fixtures']).read_text())['fixtures']


class QualificationTests(unittest.TestCase):
    def test_both_real_calendars_and_exact_discretionary_budgets_are_covered(self):
        matrix = cases(PLAN)
        self.assertEqual(len(matrix), 195)
        expected_circuits = set()
        for profile in PLAN['stages'].values():
            fixture = next(f for f in FIXTURES if f['id'] == profile['fixture_id'])
            expected_circuits.update(fixture['calendar'])
            self.assertEqual(profile['discretionary_points'], fixture['discretionaryPoints'])
            self.assertEqual(profile['offsets'], [fixture['offsets'][a] for a in ['aero', 'chassis', 'engine', 'cooling']])
            for row in [r for r in matrix if r['fixture_id'] == fixture['id']]:
                self.assertEqual(sum(row['discretionary_allocation']), fixture['discretionaryPoints'])
                self.assertTrue(all(s['laps'] > 0 for s in row['strategy']['stints']))
                self.assertNotIn('recompute_invariant', row)
                if profile['downforce_locked']:
                    self.assertEqual(row['downforce'], .5)
        self.assertEqual({c['circuit'] for c in PLAN['contexts']}, expected_circuits)
        self.assertEqual(hashlib.sha256((ROOT / PLAN['policy']).read_bytes()).hexdigest(), PLAN['policy_sha256'])

    def test_exit_zero_discretionary_cooling_retains_the_hq_bonus(self):
        profile = PLAN['stages']['exit']
        free = profile['setups']['no_discretionary_cooling']
        self.assertEqual(free[3], 0)
        self.assertEqual(free[3] + profile['offsets'][3], 1)
        self.assertEqual(sum(free) + sum(profile['offsets']), 24)
        self.assertEqual(PLAN['stages']['entry']['engine_id'], 'v8_1960')
        self.assertEqual(profile['engine_id'], 'v8_1970')

    def finish(self):
        case = cases(PLAN)[0]
        fixture = next(f for f in FIXTURES if f['id'] == case['fixture_id'])
        offsets = [fixture['offsets'][axis] for axis in ['aero', 'chassis', 'engine', 'cooling']]
        return {'case': case, 'outcome': {'ok': True, 'service_time_s': 22., 'time_s': 122., 'elapsed_accounting_residual_s': 0.},
                'receipt': {'laps': 2, 'stops': [1], 'pit_loss_s': 22., 'budget': 12, 'bonus_offsets': offsets,
                            'points': [p+b for p,b in zip(case['discretionary_allocation'], offsets)], 'components': None},
                'laps': [{'time_s': 50.}, {'time_s': 50.}], 'maximum_carry_jumps_speed_engine_surface_carcass_wear': [0.]*5,
                'pit_resets_exact': True, 'thermal_ledger': {'maximum_tire_balance_residual_j': 1e-9, 'tire_generated_heat_j': 7.,
                 'tire_direct_environment_heat_j': 3., 'tire_rolling_dissipation_j': 6., 'tire_deformation_dissipation_j': 4.}}

    def test_pit_time_and_hq_bonuses_cannot_disappear(self):
        data = self.finish()
        self.assertEqual(numerical_checks(data), [])
        bad = copy.deepcopy(data); bad['outcome']['time_s'] = 100.
        self.assertIn('independent_elapsed_accounting', numerical_checks(bad))
        bad = copy.deepcopy(data); bad['receipt']['points'][1] -= 1
        self.assertIn('budget_or_bonus_binding', numerical_checks(bad))
        bad = copy.deepcopy(data); bad['receipt']['components'] = {'engine_id': 'v8_1970'}
        self.assertIn('installed_components_binding', numerical_checks(bad))

    def test_every_repeat_has_a_main_case_and_uses_supported_execution(self):
        ids = {c['id'] for c in cases(PLAN)}
        repeats = [f'{circuit}/{stage}/{setup}/{recipe}' for circuit in PLAN['repeat_contexts']
                   for stage, setup, recipe in PLAN['repeat_selection']]
        self.assertEqual(len(repeats), 6)
        self.assertTrue(all(i in ids for i in repeats))


if __name__ == '__main__':
    unittest.main()
