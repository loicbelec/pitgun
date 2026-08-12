import copy
import json
import pathlib
import sys
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "adapter"))

from pitgun_databricks_adapter.candidate_review import (  # noqa: E402
    review_candidate_evidence,
)


class CandidateReviewTest(unittest.TestCase):
    def setUp(self):
        self.manifest = json.loads(
            (
                ROOT / "campaigns" / "racing-aero-candidate-validation-v1.json"
            ).read_text()
        )
        self.policy = json.loads(
            (ROOT / "reviews" / "racing-aero-candidate-review-v1.json").read_text()
        )

    def evidence(self):
        rows = []
        expected_best = {
            circuit: families[0]
            for circuit, families in self.policy["expected_top_families"].items()
        }
        for configuration in self.manifest["configurations"]:
            family = configuration["configuration_family"]
            circuit = configuration["circuit_id"]
            response = configuration["response_id"]
            families = sorted(
                {
                    row["configuration_family"]
                    for row in self.manifest["configurations"]
                    if row["circuit_id"] == circuit
                }
            )
            rank = families.index(family) + 1
            if response == "aero-candidate-v1" and family == expected_best[circuit]:
                rank = 0
            base = 80_000 + rank * 200 + (0 if response == "historical-v1" else -1_000)
            for seed in self.manifest["seeds"]:
                seed_offset = self.manifest["seeds"].index(seed) * 2
                result = {
                    "total_time_ms": base + seed_offset,
                    "observed_maximum_speed_kph": 360.0,
                    "setup_response": {
                        "mean_straight_speed_kph": 300.0,
                        "mean_corner_speed_kph": 200.0,
                        "aerodynamic_drag_work_kj": 20_000.0,
                        "mean_downforce_n": 15_000.0,
                        "maximum_rpm_utilization": 0.8,
                    },
                }
                rows.append(
                    {
                        "experimental_configuration_id": configuration[
                            "expected_experimental_configuration_id"
                        ],
                        "seed": seed,
                        "execution_status": "SUCCESS",
                        "result_json": json.dumps(result),
                    }
                )
        return rows

    def review(self, rows, policy=None):
        return review_candidate_evidence(
            self.manifest,
            "sha256:manifest",
            policy or self.policy,
            "sha256:policy",
            rows,
            {"experimental_runs": 1, "experimental_metrics": 1},
        )

    def test_promotes_complete_coherent_discriminating_evidence(self):
        report = self.review(self.evidence())
        self.assertEqual(report["decision"], "PROMOTE")
        self.assertEqual(report["terminal_counts"], {"SUCCESS": 210})
        self.assertEqual(report["physically_coherent_circuit_count"], 5)
        self.assertFalse(report["automatic_promotion"])

    def test_refines_when_review_threshold_is_not_met(self):
        policy = copy.deepcopy(self.policy)
        policy["gates"]["minimum_setup_discrimination_pct"] = 50.0
        report = self.review(self.evidence(), policy)
        self.assertEqual(report["decision"], "REFINE")
        self.assertTrue(
            any(
                "setup-discrimination-too-low" in reason
                for reason in report["refinement_reasons"]
            )
        )

    def test_rejects_incomplete_terminal_evidence(self):
        rows = self.evidence()
        rows[0]["execution_status"] = "FAILED"
        report = self.review(rows)
        self.assertEqual(report["decision"], "REJECT")
        self.assertIn(
            "incomplete-or-invalid-experimental-evidence", report["hard_failures"]
        )

    def test_rejects_rows_outside_immutable_plan(self):
        rows = self.evidence()
        rows[0]["seed"] = "999"
        with self.assertRaisesRegex(ValueError, "immutable plan"):
            self.review(rows)


if __name__ == "__main__":
    unittest.main()
