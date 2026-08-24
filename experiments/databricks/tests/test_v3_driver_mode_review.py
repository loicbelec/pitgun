import hashlib
import json
import pathlib
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[1]
REVIEW = ROOT / "reviews/racing-v3-driver-mode-surface-review-v1.json"
CHECKSUM = REVIEW.with_suffix(".sha256")
MANIFEST_CHECKSUM = (
    ROOT / "campaigns/racing-v3-driver-mode-surface-v3.sha256"
)


class V3DriverModeReviewTests(unittest.TestCase):
    def setUp(self):
        self.payload = REVIEW.read_bytes()
        self.review = json.loads(self.payload)

    def test_review_is_checksummed_and_pins_completed_evidence(self):
        digest, name = CHECKSUM.read_text().split()
        self.assertEqual(name, REVIEW.name)
        self.assertEqual(hashlib.sha256(self.payload).hexdigest(), digest)
        manifest_digest = MANIFEST_CHECKSUM.read_text().split()[0]
        campaign = self.review["campaign"]
        self.assertEqual(campaign["manifest_digest"], "sha256:" + manifest_digest)
        self.assertEqual(campaign["databricks_job_run_id"], "171983026871914")
        self.assertEqual(campaign["databricks_task_run_id"], "784785835565665")

        quality = self.review["evidence_quality"]
        self.assertTrue(quality["campaign_completed"])
        self.assertEqual(quality["planned_execution_count"], 3168)
        self.assertEqual(quality["successful_execution_count"], 3168)
        self.assertEqual(quality["normalized_metric_row_count"], 44352)
        self.assertEqual(quality["local_parity_failure_count"], 0)
        self.assertEqual(quality["pathological_execution_count"], 0)
        self.assertEqual(quality["physical_ordering_failure_count"], 0)

    def test_review_separates_roster_success_from_mode_failure(self):
        roster = self.review["driver_roster_finding"]
        self.assertEqual(roster["equal_trait_budget"], 2.52)
        self.assertEqual(roster["profile_count"], 33)
        self.assertEqual(roster["contextual_driver_profile_count"], 10)
        self.assertEqual(len(roster["contextual_driver_profile_ids"]), 10)

        modes = self.review["mode_response_finding"]
        self.assertEqual(modes["selection_gate_pass_count"], 0)
        self.assertEqual(modes["universal_attack_profile_count"], 33)
        self.assertEqual(modes["global_context_count_per_profile"], 8)
        self.assertEqual(modes["attack_global_win_count_per_profile"], 8)
        wear = modes["median_race_length_attack_wear_cost_percentage_point_range"]
        self.assertGreater(wear[0], 0.0)
        self.assertLess(wear[1], 1.0)

    def test_decision_is_structural_replayable_and_non_promoting(self):
        decision = self.review["reviewed_decision"]
        self.assertEqual(decision["verdict"], "STRUCTURAL_MODE_DYNAMICS_REQUIRED")
        self.assertEqual(decision["selected_parameter_set_ids"], [])
        self.assertFalse(decision["another_static_coefficient_sweep_recommended"])
        self.assertFalse(decision["independent_702_case_validation_authorized"])
        self.assertFalse(decision["automatic_catalog_promotion_performed"])
        self.assertFalse(decision["automatic_game_publication_performed"])

        boundary = self.review["next_structural_boundary"]
        self.assertTrue(boundary["solver_responsibility"].startswith("Resolve"))
        self.assertTrue(
            boundary["simulator_responsibility"].startswith("Orchestrate")
        )
        self.assertIn("Authority and Verifier", boundary["verification_requirement"])
        self.assertEqual(len(boundary["proposed_progression"]), 3)


if __name__ == "__main__":
    unittest.main()
