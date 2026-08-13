import importlib.util
import json
import pathlib
import sys
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[3]
EXPERIMENT_ROOT = ROOT / "experiments" / "racing_response"
sys.path.insert(0, str(EXPERIMENT_ROOT))
SPEC = importlib.util.spec_from_file_location(
    "evaluate_continuous_curvature_response",
    EXPERIMENT_ROOT / "evaluate_continuous_curvature_response.py",
)
evaluation = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(evaluation)


class ContinuousCurvatureResponseEvaluationTest(unittest.TestCase):
    def test_baseline_is_the_governed_current_reference(self):
        baseline = evaluation.baseline_reference()
        self.assertEqual(baseline["kind"], "current_reference")
        self.assertEqual(len(baseline["circuits"]), 7)

    def test_committed_candidate_passes_speed_but_is_not_promoted(self):
        report = json.loads(evaluation.DEFAULT_OUTPUT.read_text())
        self.assertEqual(report["schema_version"], evaluation.SCHEMA_VERSION)
        self.assertEqual(report["status"], "experimental_not_promoted")
        self.assertEqual(report["planned_run_count"], 847)
        self.assertTrue(report["maximum_speed_guardrail"]["passes"])
        self.assertLess(report["maximum_speed_guardrail"]["observed_kph"], 400.0)
        self.assertFalse(report["decision"]["candidate_passes_governed_grid"])
        self.assertFalse(
            report["decision"]["automatic_game_or_catalog_promotion"]
        )

    def test_comparison_quantifies_all_seven_circuits(self):
        report = json.loads(evaluation.DEFAULT_OUTPUT.read_text())
        self.assertEqual(len(report["comparison"]), 7)
        self.assertEqual(
            {row["circuit_slug"] for row in report["comparison"]},
            {
                "monza",
                "monaco",
                "budapest",
                "suzuka",
                "singapore",
                "silverstone",
                "spa",
            },
        )
        self.assertEqual(
            report["optimum_change_count"],
            sum(row["optimum_changed"] for row in report["comparison"]),
        )


if __name__ == "__main__":
    unittest.main()
