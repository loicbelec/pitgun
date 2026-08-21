import copy
import importlib.util
import json
import pathlib
import unittest


EXPERIMENT = pathlib.Path(__file__).parents[1]
SPEC = importlib.util.spec_from_file_location("thermal_refinement", EXPERIMENT / "screen_local.py")
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(MODULE)


class ThermalRefinementTests(unittest.TestCase):
    def setUp(self):
        self.contract, _ = MODULE.load_contract()
        self.base_scenario = json.loads(MODULE.BASE_SCENARIO.read_text())
        self.base_profile = json.loads(MODULE.BASE_PROFILE.read_text())

    def test_contract_changes_one_interpretable_axis_only(self):
        policy = self.contract["family_policy"]["modern_v6t"]
        self.assertEqual(policy["changed_axis"], "soft_limit_offset_c")
        self.assertEqual(policy["unit"], "degC")
        variants = MODULE.parameter_sets(self.contract, self.base_profile)
        anchor = self.contract["anchor_parameters"]
        for item in variants:
            actual = item["profile"]["engine_thermal_resolution"]
            for key, value in anchor.items():
                if key != "soft_limit_offset_c":
                    self.assertEqual(actual[key], value)

    def test_plan_is_bounded_and_excludes_reserved_validation(self):
        plan = MODULE.build_plan(self.contract, self.base_scenario, self.base_profile)
        expected = 7 * 3 * 2 * 3
        self.assertEqual(len(plan), expected)
        self.assertEqual({item["vehicle_id"] for item in plan}, {"modern_v6t"})
        self.assertNotIn("gb-1948", {item["circuit_id"] for item in plan})
        self.assertNotIn(20260901, {item["seed"] for item in plan})

    def test_zero_cooling_severity_is_not_a_pathology_guard(self):
        rule = self.contract["selection_rule"]
        self.assertIsNone(rule["zero_cooling_derated_fraction_cap"])
        self.assertIn("recoverability", rule["zero_cooling_interpretation"].lower())

    def test_source_campaign_and_review_are_content_addressed(self):
        source = self.contract["source_evidence"]
        review = MODULE.ROOT / source["review_artifact"]
        self.assertEqual(
            MODULE.sha256(MODULE.SOURCE_CAMPAIGN.read_bytes()),
            source["campaign_digest"],
        )
        self.assertEqual(
            MODULE.sha256(review.read_bytes()),
            source["review_digest"],
        )

    def test_candidate_evaluation_requires_an_interior_optimum(self):
        def point(circuit, seed, cooling, elapsed, temperature, derated):
            return {
                "parameter_set_id": "candidate",
                "soft_limit_offset_c": 0.0,
                "distance_from_anchor_c": 1.0,
                "circuit_id": circuit,
                "seed": seed,
                "cooling_points": cooling,
                "total_time_ms": elapsed,
                "mechanical_diagnostics": {
                    "maximum_engine_temperature_c": temperature,
                    "engine_derated_time_s": derated,
                    "generated_engine_heat_kj": 100.0,
                    "removed_engine_heat_kj": 90.0,
                    "fixed_drag_area_m2": 1.0,
                },
            }

        passing = [
            point("track", 42, 0, 102_000, 170.0, 55.0),
            point("track", 42, 10, 100_000, 140.0, 10.0),
            point("track", 42, 20, 101_000, 120.0, 0.0),
        ]
        self.assertTrue(MODULE.evaluate_candidate(copy.deepcopy(passing))["passed"])
        passing[2]["total_time_ms"] = 99_000
        self.assertFalse(MODULE.evaluate_candidate(passing)["passed"])


if __name__ == "__main__":
    unittest.main()
