import importlib.util
import json
import pathlib
import unittest


MODULE_PATH = pathlib.Path(__file__).parents[1] / "screen_local.py"
SPEC = importlib.util.spec_from_file_location("screen_local", MODULE_PATH)
assert SPEC and SPEC.loader
SCREEN = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(SCREEN)


class LocalScreenTests(unittest.TestCase):
    def setUp(self):
        self.scenario = json.loads(SCREEN.BASE_SCENARIO.read_bytes())
        self.profile = json.loads(SCREEN.BASE_PROFILE.read_bytes())

    def test_development_pairs_keep_the_same_budget(self):
        for axis in (
            "engine_points",
            "cooling_points",
            "aero_points",
            "chassis_points",
        ):
            low, high = SCREEN.development_pair(axis)
            self.assertEqual(sum(low.values()), 40.0)
            self.assertEqual(sum(high.values()), 40.0)
            self.assertLess(low[axis], high[axis])

    def test_plan_is_bounded_and_covers_three_seeds_per_circuit(self):
        plan = SCREEN.build_plan(self.scenario, self.profile)
        self.assertEqual(len(plan), 738)
        self.assertEqual({point["seed"] for point in plan}, {7, 42, 99})
        self.assertEqual(
            {point["circuit_slug"] for point in plan},
            {"monza", "monaco", "suzuka"},
        )

    def test_every_gameplay_variant_uses_the_same_development_budget(self):
        for variant in SCREEN.scenario_variants(self.scenario):
            tuning = variant["scenario"]["request"]["competitors"][0]["tuning"]
            total = sum(
                tuning[name]
                for name in (
                    "engine_points",
                    "cooling_points",
                    "aero_points",
                    "chassis_points",
                )
            )
            self.assertEqual(total, 40.0)

    def test_profile_axes_change_only_the_named_value(self):
        variants = SCREEN.profile_variants(self.profile)
        baseline = variants[0]["profile"]
        brake_low = next(item for item in variants if item["id"] == "physical-brake_force-low")
        expected = json.loads(json.dumps(baseline))
        SCREEN.set_path(expected, "mechanical_overrides.maximum_brake_force_n", 14_000.0)
        self.assertEqual(brake_low["profile"], expected)


if __name__ == "__main__":
    unittest.main()
