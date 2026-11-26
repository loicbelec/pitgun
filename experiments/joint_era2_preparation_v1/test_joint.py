import unittest
from copy import deepcopy

import run
from selection import choose, preparation


class JointCampaignTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.plan = run.load(run.HERE / 'plan-v1.json')
        cls.matrix, cls.evaluations = run.design(cls.plan)

    def test_finite_equal_pool_and_exact_native_aliases(self):
        self.assertEqual((len(self.matrix), len(self.evaluations)), (858, 840))
        self.assertEqual(sum(e['reused'] for e in self.evaluations), 279)
        old = run.source_rows(self.plan)
        groups = {}
        for item, e in zip(self.matrix, self.evaluations):
            c = item['case']
            self.assertEqual(run.semantic(c), run.semantic(e['logical_case']))
            if e['reused']:
                self.assertEqual(c, old[run.encoded(run.semantic(c))]['case'])
            common = {k: c[k] for k in ['effective_allocation', 'gear', 'downforce', 'strategy', 'seed', 'effective_budget']}
            g = (e['circuit'], e['budget'], e['proposal'])
            self.assertEqual(groups.setdefault(g, common), common)
            self.assertEqual(sum(c['effective_allocation']), e['budget'])
            self.assertTrue(all(f <= p <= 20 for p, f in zip(c['effective_allocation'], [2, 1, 4, 1])))
        self.assertEqual(len(groups), 84)

    def test_fixed_repeat_groups_are_not_selected_from_outcomes(self):
        repeats = self.matrix[840:]
        self.assertEqual(len(repeats), 18)
        self.assertTrue(all(m['fresh'] for m in repeats))
        self.assertEqual({(m['case']['circuit'], m['case']['effective_budget']) for m in repeats}, {('MEXICO', 12), ('MONZA', 24)})
        self.assertNotIn('balanced_reference', {m['case']['actor'] for m in repeats})
        self.assertEqual(len({m['case']['actor'] for m in repeats}), 9)

    def test_fixture_scope_does_not_impersonate_actual_entry(self):
        for e in self.evaluations:
            c = e['logical_case']
            self.assertEqual(c['fixture_id'], 'craft_engineering:era2-exit')
            self.assertEqual(c['kind'], 'career' if e['actor'] == 'balanced_reference' and e['budget'] == 24 else 'field')

    def test_style_selection_is_bounded_and_order_independent(self):
        base = deepcopy(self.matrix[0]['case'])
        def row(t, df):
            c = dict(base, downforce=df)
            return {'case': c, 'checks': [], 'data': {'outcome': {'time_s': t}}}
        options = [(0, row(100., .4)), (1, row(100.2, .5)), (2, row(100.4, .5))]
        self.assertEqual(choose(options, 'balanced', run.BASE)[0], 1)
        self.assertEqual(choose(list(reversed(options)), 'balanced', run.BASE)[0], 1)
        broken = deepcopy(options); broken[0][1]['checks'] = ['bad']
        with self.assertRaises(ValueError): choose(broken, 'balanced', run.BASE)

    def test_complete_anchor_budget_projection_and_independent_copies(self):
        anchors = {str(b): {'preparation': {'effective_allocation': run.BASE.scale([2,7,10,5], b, [2,1,4,1]),
                   'effective_budget': b, 'gear': .5, 'downforce': .4,
                   'strategy': {'stints': [{'tire_id': 'hard', 'laps': 35}, {'tire_id': 'medium', 'laps': 36}], 'pit_laps': [35]}, 'mode': 'balanced'}} for b in [12,18,24]}
        policy = {'contexts': [{'circuit': 'MEXICO', 'floors': [2,1,4,1], 'actors': {'franz_hermann': {'anchors': anchors}}}]}
        before = deepcopy(policy)
        for b in range(12,25):
            p = preparation(policy, 'MEXICO', b, 'franz_hermann', run.BASE.scale)
            self.assertEqual(sum(p['effective_allocation']), b)
            self.assertTrue(all(f <= v <= 20 for v,f in zip(p['effective_allocation'], [2,1,4,1])))
            if str(b) in anchors:
                self.assertEqual(p['effective_allocation'], anchors[str(b)]['preparation']['effective_allocation'])
            p['strategy']['pit_laps'].append(70)
        self.assertEqual(policy, before)
        self.assertEqual(preparation(policy,'MEXICO',15,'franz_hermann',run.BASE.scale)['source_anchor_budget'],12)
        for b in [11,25,True,18.5]:
            with self.assertRaises(ValueError): preparation(policy,'MEXICO',b,'franz_hermann',run.BASE.scale)

    def test_protocol_change_rejected_before_any_compute(self):
        changed = deepcopy(self.plan); changed['total_rows'] -= 1
        with self.assertRaises(ValueError): run.design(changed)
        changed = deepcopy(self.plan); changed['contexts'][0]['proposals'][1] = deepcopy(changed['contexts'][0]['proposals'][0])
        with self.assertRaises(ValueError): run.design(changed)


if __name__ == '__main__': unittest.main()
