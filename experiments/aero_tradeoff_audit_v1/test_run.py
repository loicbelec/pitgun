import gzip
import json
from pathlib import Path
import unittest

from run import ROOT, cases, repeat_ids

PLAN = json.loads((Path(__file__).parent / 'plan-v1.json').read_text())


class AeroAuditTests(unittest.TestCase):
    def test_bounded_controls_hold_every_other_case_field(self):
        matrix = cases(PLAN)
        self.assertEqual(len(matrix), 42)
        self.assertEqual(len(repeat_ids(PLAN)), 6)
        self.assertTrue(set(repeat_ids(PLAN)) <= {c['id'] for c in matrix})
        for context in PLAN['contexts']:
            rows = [c for c in matrix if c['circuit'] == context['circuit']]
            self.assertEqual(len(rows), 14)
            for c in rows:
                self.assertEqual(c['gear'], .5)
                self.assertEqual(c['discretionary_allocation'], [4,4,4,4])
                self.assertEqual(sum(s['laps'] for s in c['strategy']['stints']), context['laps'])
                self.assertEqual(c['strategy']['pit_laps'], [context['laps']//2])
            self.assertEqual({c['downforce'] for c in rows if c['aero_mode']=='coupled'}, {0.,.2,.5,.8,1.})

    def test_all_nine_coupled_bridges_exist_before_solving(self):
        previous = json.load(gzip.open(ROOT / PLAN['baseline_archive']))
        old = {r['case']['id']: r['case'] for r in previous['rows']}
        count = 0
        for c in cases(PLAN):
            if c['aero_mode'] != 'coupled' or c['downforce'] not in [.2,.5,.8]: continue
            key = f"era2/{c['circuit']}/exit/balanced/SM-50"
            if c['downforce'] != .5: key += f"/downforce-{c['downforce']}"
            new = {k:v for k,v in c.items() if k not in ['id','aero_mode']}
            before = {k:v for k,v in old[key].items() if k!='id'}
            self.assertEqual(new, before); count += 1
        self.assertEqual(count, 9)


if __name__ == '__main__': unittest.main()
