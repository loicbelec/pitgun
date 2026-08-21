import json
import pathlib
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[1]
REVIEW = (
    ROOT
    / "reviews"
    / "racing-v3-thermal-refinement-validation-review-v1.json"
)


class ThermalRefinementReviewTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.review = json.loads(REVIEW.read_text())

    def test_review_pins_successful_independent_evidence(self):
        observed = self.review["observed_evidence"]
        self.assertEqual(observed["planned_execution_count"], 12)
        self.assertEqual(observed["successful_execution_count"], 12)
        self.assertEqual(observed["local_parity_failure_count"], 0)
        self.assertEqual(sum(observed["vehicle_family_execution_counts"].values()), 12)
        self.assertEqual(sum(observed["pathological_execution_counts"].values()), 0)
        self.assertEqual(len(self.review["response_summary"]), 12)
        self.assertTrue(
            all(
                row["maximum_temperature_c"] < 180.0
                for row in self.review["response_summary"]
            )
        )

    def test_every_family_passes_without_automatic_promotion(self):
        verdicts = self.review["per_family_verdicts"]
        self.assertEqual(set(verdicts), {"historical_v8", "modern_v6t", "f1_2026"})
        self.assertEqual({row["verdict"] for row in verdicts.values()}, {"PASS"})
        self.assertFalse(self.review["automatic_catalog_promotion"])
        self.assertFalse(self.review["next_gate"]["automatic_catalog_promotion"])

    def test_authored_families_have_an_interior_optimum(self):
        rows = self.review["response_summary"]
        for family in ("modern_v6t", "f1_2026"):
            by_cooling = {
                row["cooling_points"]: row
                for row in rows
                if row["vehicle_family"] == family
            }
            self.assertEqual(set(by_cooling), {0, 10, 20})
            self.assertLess(by_cooling[10]["total_time_ms"], by_cooling[0]["total_time_ms"])
            self.assertLess(by_cooling[10]["total_time_ms"], by_cooling[20]["total_time_ms"])
            self.assertLessEqual(
                by_cooling[10]["derated_fraction"],
                by_cooling[0]["derated_fraction"],
            )
            self.assertLessEqual(
                by_cooling[20]["derated_fraction"],
                by_cooling[10]["derated_fraction"],
            )

    def test_historical_anchors_remain_safe_and_pace_neutral(self):
        historical = [
            row
            for row in self.review["response_summary"]
            if row["vehicle_family"] == "historical_v8"
        ]
        self.assertEqual(len(historical), 6)
        for anchor in {row["anchor_id"] for row in historical}:
            rows = [row for row in historical if row["anchor_id"] == anchor]
            self.assertEqual(len({row["total_time_ms"] for row in rows}), 1)
            self.assertTrue(all(row["derated_fraction"] == 0.0 for row in rows))
            self.assertTrue(all(row["maximum_temperature_c"] < 180.0 for row in rows))

    def test_experimental_fuel_reservoir_is_not_a_game_calibration(self):
        fuel = self.review["fuel_control"]
        self.assertEqual(fuel["experimental_reservoir_kg"], 130.0)
        self.assertFalse(fuel["game_or_real_f1_calibration"])


if __name__ == "__main__":
    unittest.main()
