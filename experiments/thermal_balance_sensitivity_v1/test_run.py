import copy
import hashlib
import json
from collections import defaultdict
from pathlib import Path
import unittest

from run import ROOT, cases
from report import compare_policy, metrics

HERE = Path(__file__).parent
PLAN = json.loads((HERE / 'plan-v1.json').read_text())


class ThermalSensitivityTests(unittest.TestCase):
    def test_each_candidate_changes_exactly_one_heat_parameter(self):
        baseline = json.loads((ROOT / PLAN['policies']['reference']['path']).read_text())
        for name, spec in PLAN['policies'].items():
            path = ROOT / spec['path']
            self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(), spec['sha256'])
            policy = json.loads(path.read_text())
            if name != 'reference':
                self.assertTrue(all(c['heat'][spec['changed_field']] == spec['value'] for c in policy['compounds'].values()))
                for tire, c in policy['compounds'].items():
                    c['heat'][spec['changed_field']] = baseline['compounds'][tire]['heat'][spec['changed_field']]
            self.assertEqual(policy, baseline)

    def test_matrix_holds_exposure_and_actual_fixture_budgets(self):
        fixtures = {f['id']: f for f in json.loads((ROOT / PLAN['fixtures']).read_text())['fixtures']}
        development = cases(PLAN)
        validation = cases(PLAN, 'validation', 'carcass-air-45')
        self.assertEqual(len(development), 40)
        self.assertEqual(len(validation), 8)
        self.assertNotIn('MEXICO', {r['circuit'] for r in development})
        self.assertNotIn('ZANDVOORT', {r['circuit'] for r in development})
        groups = defaultdict(list)
        for c in development + validation:
            self.assertEqual(sum(c['discretionary_allocation']), fixtures[c['fixture_id']]['discretionaryPoints'])
            self.assertIn(c['circuit'], fixtures[c['fixture_id']]['calendar'])
            exposure = defaultdict(int)
            for s in c['strategy']['stints']:
                self.assertGreater(s['laps'], 0)
                exposure[s['tire_id']] += s['laps']
            groups[(c['fixture_id'], c['circuit'])].append(dict(exposure))
        for values in groups.values():
            self.assertTrue(all(v == values[0] for v in values))

    def test_reserved_phase_rejects_unplanned_and_reference_selection(self):
        for name in [None, 'reference', 'new-after-seeing-mexico']:
            with self.assertRaises(ValueError): cases(PLAN, 'validation', name)
        self.assertEqual(PLAN['expected_fresh_repeats'], 6)

    def row(self, hot=.6, grip=.95):
        lap = {'time_s': 100., 'hot_fraction': hot, 'cold_fraction': 0., 'peak_window_fraction': 1.-hot,
            'temperature_grip_mean': grip, 'wear_grip_mean': .98, 'surface_mean_c': 115., 'carcass_mean_c': 120.,
            'surface_max_c': 130., 'carcass_max_c': 140., 'wear_end': .4}
        return {'checks': [], 'case': {'strategy': {'stints': [{'laps': 4}]}},
            'data': {'laps': [copy.deepcopy(lap) for _ in range(4)],
                'outcome': {'time_s': 422., 'drive_time_s': 400., 'service_time_s': 22.}}}

    def test_warmup_exclusion_does_not_hide_race_time(self):
        row = self.row(hot=1.)
        for lap in row['data']['laps'][:3]: lap['hot_fraction'] = 0.
        self.assertEqual(metrics(row)['hot_fraction'], 1.)
        self.assertEqual(metrics(row, warmup=0)['hot_fraction'], .25)
        self.assertEqual(metrics(row)['race_time_s'], 422.)

    def test_thermal_selection_rejects_entry_grip_regression(self):
        plan = copy.deepcopy(PLAN)
        plan['development_contexts'] = [plan['development_contexts'][0], plan['development_contexts'][2]]
        for c in plan['development_contexts']: c['stops'] = [1]
        lookup = {}
        for c in plan['development_contexts']:
            suffix = f"{c['fixture_id']}/{c['circuit']}/1"
            lookup['reference/' + suffix] = self.row(hot=.8, grip=.96)
            lookup['trial/' + suffix] = self.row(hot=.5, grip=.94)
        self.assertTrue(compare_policy(plan, lookup, 'trial', 'development')['eligible'])
        entry = 'trial/craft_engineering:era1-entry/MONZA/1'
        lookup[entry] = self.row(hot=0., grip=.90)
        result = compare_policy(plan, lookup, 'trial', 'development')
        self.assertFalse(result['eligible'])
        self.assertTrue(any('entry_grip_loss' in r for r in result['rejection_reasons']))


if __name__ == '__main__': unittest.main()
