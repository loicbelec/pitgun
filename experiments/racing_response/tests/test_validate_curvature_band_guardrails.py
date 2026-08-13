import importlib.util
import json
import pathlib
import sys
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[3]
EXPERIMENT_ROOT = ROOT / "experiments" / "racing_response"
sys.path.insert(0, str(EXPERIMENT_ROOT))
SPEC = importlib.util.spec_from_file_location(
    "validate_curvature_band_guardrails",
    EXPERIMENT_ROOT / "validate_curvature_band_guardrails.py",
)
guardrails = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(guardrails)


class CurvatureBandGuardrailTest(unittest.TestCase):
    def test_committed_evidence_passes_without_automatic_promotion(self):
        report = json.loads(guardrails.DEFAULT_OUTPUT.read_text())
        self.assertEqual(report["schema_version"], guardrails.SCHEMA_VERSION)
        self.assertEqual(report["planned_run_count"], 154)
        self.assertTrue(report["all_circuit_band_guardrails_pass"])
        self.assertTrue(report["all_circuit_speed_guardrails_pass"])
        self.assertTrue(
            report["decision"][
                "candidate_physically_eligible_for_versioned_promotion"
            ]
        )
        self.assertFalse(
            report["decision"]["automatic_game_or_catalog_promotion"]
        )

    def test_all_seven_circuits_cover_eleven_gearing_levels(self):
        report = json.loads(guardrails.DEFAULT_OUTPUT.read_text())
        self.assertEqual(len(report["circuits"]), 7)
        for circuit in report["circuits"]:
            self.assertEqual(len(circuit["high_minus_low_downforce"]), 11)
            self.assertEqual(circuit["curvature_band_invariant_failures"], [])
            self.assertLess(circuit["maximum_observed_speed_kph"], 400.0)

    def test_suzuka_movement_is_bounded_to_one_grid_step(self):
        review = json.loads(guardrails.DEFAULT_OUTPUT.read_text())[
            "suzuka_optimum_review"
        ]
        self.assertEqual(review["classification"], "one_step_bounded_movement")
        self.assertLessEqual(review["downforce_grid_step"], 0.1)
        self.assertLessEqual(review["gearing_grid_step"], 0.1)


if __name__ == "__main__":
    unittest.main()
