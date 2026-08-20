import importlib.util
import json
import pathlib
import unittest


MODULE_PATH = pathlib.Path(__file__).parents[1] / "validate_local.py"
SPEC = importlib.util.spec_from_file_location("tire_degradation_validation", MODULE_PATH)
assert SPEC and SPEC.loader
VALIDATION = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(VALIDATION)


class TireDegradationValidationTests(unittest.TestCase):
    def setUp(self):
        self.scenario = json.loads(VALIDATION.shared.BASE_SCENARIO.read_bytes())
        self.profile = json.loads(VALIDATION.PROFILE.read_bytes())
        self.plan = VALIDATION.build_plan(self.scenario, self.profile)

    def test_plan_has_exact_bounded_coverage(self):
        self.assertEqual(len(self.plan), 236)
        self.assertEqual(
            {point["family"] for point in self.plan},
            {
                "compound.longrun",
                "strategy.window",
                "driver.control",
                "parameter.screen",
            },
        )
        self.assertEqual(
            sum(point["family"] == "compound.longrun" for point in self.plan), 96
        )
        self.assertEqual(
            sum(point["family"] == "strategy.window" for point in self.plan), 80
        )
        self.assertEqual(
            sum(point["family"] == "driver.control" for point in self.plan), 24
        )
        self.assertEqual(
            sum(point["family"] == "parameter.screen" for point in self.plan), 36
        )

    def test_compounds_change_catalog_inputs_not_rust_code(self):
        rows = [
            point
            for point in self.plan
            if point["family"] == "compound.longrun"
            and point["circuit_slug"] == "spa"
            and point["vehicle_id"] == "f1_2026"
            and point["fuel_load_kg"] == 100.0
        ]
        self.assertEqual({point["compound"] for point in rows}, set(VALIDATION.COMPOUNDS))
        for point in rows:
            strategy = point["scenario"]["request"]["competitors"][0][
                "stint_strategy"
            ]
            self.assertEqual(strategy["stints"][0]["tire_id"], point["compound"])
            self.assertEqual(point["profile"], self.profile)

    def test_parameter_screen_only_changes_runtime_profile_coefficients(self):
        rows = [
            point
            for point in self.plan
            if point["family"] == "parameter.screen"
            and point["circuit_slug"] == "spa"
        ]
        self.assertEqual(len(rows), 9)
        self.assertEqual(
            {
                point["profile"]["tire_degradation"][
                    "thermal_deviation_wear_gain"
                ]
                for point in rows
            },
            set(VALIDATION.THERMAL_GAINS),
        )
        self.assertEqual(
            {
                point["profile"]["tire_contact"][
                    "workload_energy_to_full_wear_j"
                ]
                for point in rows
            },
            set(VALIDATION.WORKLOAD_ENERGIES_J),
        )

    def test_crossover_ignores_launch_lap(self):
        self.assertEqual(VALIDATION.first_crossover_lap([200, 100, 105], [150, 101, 104]), 3)
        self.assertIsNone(VALIDATION.first_crossover_lap([200, 100], [150, 101]))


if __name__ == "__main__":
    unittest.main()
