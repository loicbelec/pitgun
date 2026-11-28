import unittest
from copy import deepcopy
import run


class JointMatrixTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.plan = run.legacy.load(run.HERE / 'plan-v1.json')
        cls.matrix, cls.evaluations = run.design(cls.plan)

    def test_equal_actor_inputs_except_identity(self):
        groups = {}
        for e in self.evaluations:
            if e['family'] != 'common':
                continue
            c = deepcopy(self.matrix[e['index']]['case'])
            actor = c.pop('actor'); c.pop('id')
            group = (c['circuit'], c['effective_budget'], e['proposal'])
            prior, actors = groups.setdefault(group, (c, set()))
            self.assertEqual(c, prior)
            actors.add(actor)
        self.assertEqual(len(groups), 72)
        self.assertTrue(all(len(actors) == 10 for _, actors in groups.values()))

    def test_budget_ceiling_is_available_not_forced_spending(self):
        for e in self.evaluations:
            c = self.matrix[e['index']]['case']
            if c['effective_budget'] != 82:
                continue
            self.assertEqual(sum(c['effective_allocation']), 47 if e['family'] == 'shortcut' else 80)
            self.assertTrue(all(p <= 20 for p in c['effective_allocation']))
            self.assertEqual(c['effective_budget'], 82)

    def test_full_era_counts_stints_and_fresh_execution(self):
        self.assertEqual(len(self.matrix), 836)
        self.assertEqual(len(self.evaluations), 816)
        self.assertEqual(sum(m['fresh'] for m in self.matrix), 18)
        by_circuit = {c['circuit']: c['laps'] for c in self.plan['contexts']}
        originals = [m['case'] for m in self.matrix if not m['fresh']]
        for item in self.matrix:
            c = item['case']; s = c['strategy']
            self.assertEqual(sum(t['laps'] for t in s['stints']), by_circuit[c['circuit']])
            self.assertEqual(c['initial_fuel_kg'], 140.)
            self.assertEqual(c['fixture_revision'], 2)
            if item['fresh']:
                self.assertIn(c, originals)
        competitive = [self.matrix[e['index']]['case'] for e in self.evaluations if e['family'] == 'common']
        self.assertTrue(all(len(c['strategy']['pit_laps']) <= 2 for c in competitive))

    def test_fairness_excludes_traits_but_detects_engine_and_fuel_changes(self):
        receipt = {key: 1 for key in run.FAIR_KEYS}
        other = dict(receipt, driver={'different': True})
        self.assertEqual(run.fair_receipt(receipt), run.fair_receipt(other))
        for key in ['engine', 'initial_state', 'points', 'unused_budget']:
            changed = dict(receipt); changed[key] = 2
            self.assertNotEqual(run.fair_receipt(receipt), run.fair_receipt(changed))

    def test_no_partial_campaign_can_qualify(self):
        with self.assertRaises(ValueError):
            run.analyze({'plan': self.plan, 'rows': [], 'complete': False})

    def test_actual_career_anchors_remain_distinct_from_field(self):
        cases = [self.matrix[e['index']]['case'] for e in self.evaluations if e['family'] == 'career']
        self.assertEqual(len(cases), 48)
        for c in cases:
            stage = 'entry' if c['effective_budget'] == 24 else 'exit'
            self.assertEqual(c['fixture_id'], run.fixture(stage)['id'])
            self.assertEqual(c['kind'], 'career')
            self.assertEqual(c['actor'], 'balanced_reference')
            self.assertTrue(all(p >= f for p, f in zip(c['effective_allocation'], run.legacy.floors(run.fixture(stage)))))


if __name__ == '__main__':
    unittest.main()
