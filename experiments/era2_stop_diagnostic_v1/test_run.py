import copy
import gzip
import json
from collections import defaultdict
from pathlib import Path
import unittest

from run import ROOT, cases, numerical_checks

PLAN = json.loads((Path(__file__).parent / 'plan-v1.json').read_text())


class DiagnosticTests(unittest.TestCase):
    def test_bounded_matrix_preserves_exact_compound_exposure(self):
        matrix = cases(PLAN)
        self.assertEqual(len(matrix), 44)
        self.assertEqual(sum(c['diagnostic'] == 'nominal' for c in matrix), 32)
        groups = defaultdict(list)
        for c in matrix:
            stints = c['strategy']['stints']
            self.assertTrue(all(s['laps'] > 0 for s in stints))
            self.assertEqual(sum(s['laps'] for s in stints), {'MONZA': 53, 'SAOPAULO': 71}[c['circuit']])
            totals = defaultdict(int)
            for stint in stints:
                totals[stint['tire_id']] += stint['laps']
            groups[tuple(c['id'].split('/')[:2])].append(dict(totals))
            cumulative = 0
            pits = []
            for stint in stints[:-1]:
                cumulative += stint['laps']; pits.append(cumulative)
            self.assertEqual(pits, c['strategy']['pit_laps'])
        self.assertEqual(len(groups), 8)
        for exposures in groups.values():
            self.assertTrue(all(e == exposures[0] for e in exposures))

    def test_ablations_hold_every_preparation_field_constant(self):
        matrix = cases(PLAN)
        lookup = {c['id']: c for c in matrix}
        for c in matrix:
            if c['diagnostic'] == 'nominal': continue
            original = lookup['/'.join(c['id'].split('/')[:-1]) + '/nominal']
            changed = copy.deepcopy(c)
            changed['id'] = original['id']; changed['diagnostic'] = 'nominal'
            self.assertEqual(changed, original)

    def test_service_fuel_and_identity_checks_detect_corruption(self):
        old = json.load(gzip.open(ROOT / PLAN['baseline_archive']))
        data = copy.deepcopy(next(r['data'] for r in old['rows'] if r['case']['id'] == 'MONZA/exit/balanced/MH-50'))
        data['case']['diagnostic'] = 'nominal'
        receipt = data['receipt']
        receipt['heat_parameters_preserved'] = True
        receipt['nominal_physical_input_digest'] = receipt['physical_input_digest']
        self.assertEqual(numerical_checks(data), [])
        for field, value, error in [('fuel', 110., 'fuel_scope'), ('heat', False, 'changed_heat_laws'), ('identity', 'bad', 'diagnostic_identity')]:
            bad = copy.deepcopy(data)
            if field == 'fuel': bad['receipt']['initial_state']['fuel_mass'] = value
            if field == 'heat': bad['receipt']['heat_parameters_preserved'] = value
            if field == 'identity': bad['receipt']['nominal_physical_input_digest'] = value
            self.assertIn(error, numerical_checks(bad))
        bad = copy.deepcopy(data); bad['outcome']['service_time_s'] = 0.
        self.assertIn('service_cost', numerical_checks(bad))


if __name__ == '__main__':
    unittest.main()
