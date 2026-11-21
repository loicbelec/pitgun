"""Cheap archive consistency checks; do not claim cross-platform replay parity."""
import hashlib
import json
import pathlib
import unittest

ROOT=pathlib.Path(__file__).resolve().parents[3]


class PowerEvidenceTests(unittest.TestCase):
    def test_archive_keeps_frozen_legacy_results_and_baseline_identity(self):
        path=ROOT/'experiments/power_telemetry/results/local-power-v1.json'
        report=json.loads(path.read_bytes())
        self.assertEqual(path.with_suffix('.sha256').read_text().strip(),'sha256:'+hashlib.sha256(path.read_bytes()).hexdigest())
        old_path=ROOT/'experiments/physics_foundation/results/local-baseline-v1.json'
        self.assertEqual(report['identity']['baseline_digest'],'sha256:'+hashlib.sha256(old_path.read_bytes()).hexdigest())
        old={x['case_id']:x for x in json.loads(old_path.read_bytes())['cases']}
        self.assertEqual(set(old),{x['case_id'] for x in report['cases']})
        for case in report['cases']:
            self.assertTrue(case['legacy_output_equal_with_projection_off_and_on'])
            self.assertEqual(case['legacy_output_digest'],old[case['case_id']]['outcome']['result']['output_digest'])
            self.assertTrue(case['new_projection_repeated_equal'])

    def test_archive_distinguishes_engine_load_from_unobserved_pedals(self):
        report=json.loads((ROOT/'experiments/power_telemetry/results/local-power-v1.json').read_bytes())
        for case in report['cases']:
            self.assertTrue(case['native_ledger_checks_passed'])
            self.assertLessEqual(case['maximum_equivalent_engine_load_pct'],100)
            for lap in case['laps'].values():
                self.assertIsNone(lap['pedal_command_pct'])
                self.assertIsNone(lap['brake_command_pct'])
                self.assertLess(lap['export_1hz_samples'],lap['export_5hz_samples'])


if __name__=='__main__':unittest.main()
