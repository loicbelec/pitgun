import importlib.util
import json
import pathlib
import unittest


MODULE_PATH = pathlib.Path(__file__).parents[1] / "validate_local.py"
SPEC = importlib.util.spec_from_file_location("validate_local", MODULE_PATH)
assert SPEC and SPEC.loader
VALIDATION = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(VALIDATION)


class LocalValidationTests(unittest.TestCase):
    def setUp(self):
        self.scenario = json.loads(VALIDATION.BASE_SCENARIO.read_bytes())

    def test_plan_has_exact_bounded_coverage(self):
        plan = VALIDATION.build_plan(self.scenario)
        self.assertEqual(len(plan), 280)
        self.assertEqual(
            {point["circuit_slug"] for point in plan},
            {"spa", "barcelona", "singapore", "mexico"},
        )
        self.assertEqual(
            {point["vehicle_id"] for point in plan},
            {"classic_v8_1960", "classic_v8_1970", "modern_v6t", "f1_2026"},
        )
        self.assertEqual(
            sum(point["family"] == "heldout.setup" for point in plan), 160
        )
        self.assertEqual(
            sum(point["family"] == "longrun.tire" for point in plan), 96
        )
        self.assertEqual(
            sum(point["family"] == "progression.anchor" for point in plan), 24
        )

    def test_progression_explicitly_covers_eras_three_and_four(self):
        plan = VALIDATION.build_plan(self.scenario)
        anchors = [point for point in plan if point["family"] == "progression.anchor"]
        self.assertEqual({point["era"] for point in anchors}, {1, 2, 3, 4, 5})

    def test_every_case_keeps_a_constant_development_budget(self):
        for point in VALIDATION.build_plan(self.scenario):
            tuning = point["scenario"]["request"]["competitors"][0]["tuning"]
            self.assertEqual(
                sum(
                    tuning[name]
                    for name in (
                        "engine_points",
                        "cooling_points",
                        "aero_points",
                        "chassis_points",
                    )
                ),
                40.0,
            )

    def test_explicit_stints_derive_their_pit_boundaries(self):
        self.assertEqual(
            VALIDATION.explicit_stint_strategy((("medium", 8), ("soft", 16))),
            {
                "stints": [
                    {"tire_id": "medium", "laps": 8},
                    {"tire_id": "soft", "laps": 16},
                ],
                "pit_laps": [8],
            },
        )

    def test_stint_pace_excludes_the_launch_lap(self):
        summary = VALIDATION.stint_pace_summary(
            [200_000, 100_000, 101_000, 102_000, 130_000, 110_000, 111_000, 112_000],
            [4],
        )
        self.assertEqual(summary[0]["clean_lap_count"], 3)
        self.assertEqual(summary[0]["pace_drift_ms"], 0)
        self.assertEqual(summary[1]["clean_lap_count"], 4)

    def test_campaign_verdict_records_unsupported_active_vehicle(self):
        verdicts = VALIDATION.campaign_verdicts(
            {"distinct_fastest_setup_count": 2},
            {
                "minimum_no_stop_compound_time_range_ms": 100,
                "minimum_no_stop_compound_final_wear_range_pct": 1.0,
                "distinct_fastest_one_stop_count": 2,
            },
            {"era_3_and_4_explicitly_covered": True},
            ["classic_v8_1960"],
        )
        active_vehicle = next(
            item for item in verdicts if item["capability"] == "active_vehicle_compatibility"
        )
        self.assertEqual(active_vehicle["verdict"], "REFINE")
        self.assertIn("classic_v8_1960", active_vehicle["reason"])


if __name__ == "__main__":
    unittest.main()
