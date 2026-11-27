import copy
import gzip
import json
from collections import defaultdict
from pathlib import Path
import unittest

from run import ROOT, cases, matched, repeat_ids
from report import case_key

PLAN = json.loads((Path(__file__).parent / 'plan-v1.json').read_text())


class QualificationTests(unittest.TestCase):
    def test_matrix_counts_and_actual_fixture_means(self):
        fixtures = {f['id']: f for f in json.loads((ROOT / PLAN['fixtures']).read_text())['fixtures']}
        expected = {'era1': 120, 'era2': 237}
        for era in expected:
            matrix = cases(PLAN, era)
            self.assertEqual(len(matrix), expected[era])
            for c in matrix:
                f = fixtures[c['fixture_id']]
                self.assertEqual(sum(c['discretionary_allocation']), f['discretionaryPoints'])
                self.assertNotIn('recompute_invariant', c)
                self.assertEqual(c['policy_id'], PLAN['policy_id'])
                self.assertTrue(all(s['laps'] > 0 for s in c['strategy']['stints']))
                low, high = f['sliders']['downforce']
                self.assertTrue(low <= c['downforce'] <= high)
            self.assertTrue(set(repeat_ids(PLAN, era)) <= {c['id'] for c in matrix})
        self.assertEqual(sum(expected.values()) + sum(PLAN['expected_repeats'].values()), 369)

    def test_era2_calendar_union_is_covered_at_each_stage(self):
        matrix = cases(PLAN, 'era2')
        actual = {'MONTREAL', 'SPIELBERG', 'MEXICO', 'SAOPAULO', 'MONZA', 'SILVERSTONE', 'ZANDVOORT'}
        for fixture in ['craft_engineering:era2-entry', 'craft_engineering:era2-exit']:
            self.assertEqual({c['circuit'] for c in matrix if c['fixture_id'] == fixture}, actual)

    def test_matched_stop_controls_keep_exact_integer_compound_exposure(self):
        for laps in [52, 53, 70, 71, 72]:
            for stops in [2, 3]:
                strategy = matched(laps, stops)
                exposure = defaultdict(int)
                for s in strategy['stints']: exposure[s['tire_id']] += s['laps']
                self.assertEqual(dict(exposure), {'medium': laps // 2, 'hard': laps - laps // 2})
                cumulative, pits = 0, []
                for s in strategy['stints'][:-1]: cumulative += s['laps']; pits.append(cumulative)
                self.assertEqual(pits, strategy['pit_laps'])

    def test_era1_slider_controls_are_legal_and_not_paired_with_downforce(self):
        matrix = cases(PLAN, 'era1')
        controls = [c for c in matrix if '/gear-' in c['id']]
        self.assertEqual(len(controls), 18)
        for c in controls:
            self.assertEqual(c['downforce'], .5)
            self.assertIn(c['gear'], [.35, .85])
            self.assertEqual([s['tire_id'] for s in c['strategy']['stints']], ['soft', 'medium'])

    def test_all_fourteen_screen_cases_have_exact_semantic_matches(self):
        keys = {case_key(c) for era in ['era1', 'era2'] for c in cases(PLAN, era)}
        screen_cases = []
        for p in PLAN['retained_files']:
            if p.endswith('.json.gz'):
                screen_cases += [r['case'] for r in json.load(gzip.open(ROOT / p))['rows'] if r['case']['policy_id'] == PLAN['policy_id']]
        self.assertEqual(len(screen_cases), 14)
        for c in screen_cases: self.assertIn(case_key(c), keys)


if __name__ == '__main__': unittest.main()
