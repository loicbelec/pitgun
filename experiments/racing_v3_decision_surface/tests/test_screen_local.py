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
        self.assertEqual(len(plan), 882)
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

    def test_v5_profile_covers_fuel_and_degradation_controls(self):
        families = {item["family"] for item in SCREEN.profile_variants(self.profile)}
        self.assertIn("physical.fuel_brake_specific_consumption", families)
        self.assertIn("physical.fuel_idle_flow", families)
        self.assertIn("physical.degradation_reference_load_coefficient", families)
        self.assertIn("physical.degradation_thermal_gain", families)
        self.assertIn("physical.degradation_thermal_cap", families)

    def test_long_run_evidence_is_checksumming_and_binds_v3_09(self):
        evidence = SCREEN.load_long_run_evidence(SCREEN.LONG_RUN_REPORT)
        self.assertEqual(evidence["model"]["version"], "0.9.0")
        self.assertEqual(evidence["execution_count"], 236)
        self.assertEqual(evidence["ordered_wear_group_count"], 32)
        self.assertGreater(evidence["maximum_observed_thermal_wear_multiplier"], 1.5)
        self.assertLess(evidence["maximum_observed_thermal_wear_multiplier"], 3.0)
        self.assertEqual(evidence["fastest_stop_laps"], [22])

    def test_stored_v5_report_is_complete_and_checksumed(self):
        report_bytes = SCREEN.DEFAULT_OUTPUT.read_bytes()
        digest = SCREEN.DEFAULT_OUTPUT.with_suffix(".sha256").read_text().strip()
        report = json.loads(report_bytes)
        self.assertEqual(digest, SCREEN.sha256(report_bytes))
        self.assertEqual(report["schema_version"], "pitgun.racing-v3-local-screen/v5")
        self.assertEqual(report["campaign"]["model"]["version"], "0.9.0")
        self.assertEqual(report["campaign"]["execution_count"], 882)
        self.assertEqual(len(report["points"]), 882)
        self.assertIsInstance(report["verdicts"], list)
        self.assertGreater(len(report["verdicts"]), 0)
        self.assertEqual(
            report["long_run_evidence"]["model"], report["campaign"]["model"]
        )


if __name__ == "__main__":
    unittest.main()
