import json
import pathlib
import sys
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "adapter"))

REVIEW = ROOT / "reviews/racing-v3-driver-control-study-review-v1.json"
NOTEBOOK = ROOT / "src/review_v3_driver_control_study.py"


class V3DriverControlStudyTests(unittest.TestCase):
    def test_review_is_pinned_non_promoting_and_rejects_coefficient_selection(self):
        review = json.loads(REVIEW.read_text())
        self.assertEqual(review["expected_evidence"]["successful_execution_count"], 1584)
        self.assertEqual(review["expected_evidence"]["normalized_metric_count"], 22176)
        self.assertEqual(review["expected_evidence"]["parameter_set_count"], 33)
        self.assertEqual(review["expected_evidence"]["selection_gate_pass_count"], 0)
        self.assertEqual(
            review["reviewed_conclusion"]["decision"],
            "STRUCTURAL_REFINEMENT_REQUIRED",
        )
        self.assertFalse(review["reviewed_conclusion"]["candidate_selected"])
        self.assertFalse(
            review["reviewed_conclusion"]["automatic_catalog_promotion"]
        )

    def test_review_pins_exact_delta_versions_and_metric_backfill(self):
        review = json.loads(REVIEW.read_text())
        lineage = review["lineage"]
        versions = (
            lineage["campaigns_table_version"],
            lineage["experimental_runs_table_version"],
            lineage["experimental_metrics_table_version"],
        )
        self.assertEqual(versions, (61, 95, 94))
        self.assertEqual(
            review["expected_evidence"]["backfilled_metric_count"], 672
        )
        self.assertEqual(
            lineage["metric_backfill_job_run_id"], "421996403445493"
        )

    def test_notebook_is_read_only_fail_closed_and_explanatory(self):
        notebook = NOTEBOOK.read_text()
        self.assertIn('option("versionAsOf", version)', notebook)
        self.assertIn("normalized metric count changed", notebook)
        self.assertIn("paired group count changed", notebook)
        self.assertIn('"circuit_slug",', notebook)
        self.assertIn("race-length winner conclusion changed", notebook)
        self.assertIn("ATTACK wins every paired", notebook)
        self.assertIn("improve the physics before calibrating again", notebook)
        for forbidden in (
            "DeltaTable",
            ".merge(",
            "INSERT INTO",
            "UPDATE ",
            "DELETE FROM",
            "policy_releases",
            "catalog promotion",
        ):
            self.assertNotIn(forbidden, notebook)


if __name__ == "__main__":
    unittest.main()
