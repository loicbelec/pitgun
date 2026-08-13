import importlib.util
import json
import pathlib
import sys
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[3]
EXPERIMENT_ROOT = ROOT / "experiments" / "racing_response"
sys.path.insert(0, str(EXPERIMENT_ROOT))
SPEC = importlib.util.spec_from_file_location(
    "measure_spa_relief_impact",
    EXPERIMENT_ROOT / "measure_spa_relief_impact.py",
)
impact = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(impact)


class SpaReliefImpactTest(unittest.TestCase):
    def test_variants_isolate_vertical_profile(self):
        profiles = impact.build_track_profiles()
        flat = profiles["flat"]
        relief = profiles["spw_relief"]
        self.assertEqual(flat["s"], relief["s"])
        self.assertEqual(flat["x"], relief["x"])
        self.assertEqual(flat["y"], relief["y"])
        self.assertTrue(all(value == 0.0 for value in flat["z"]))
        summary = impact.profile_summary(relief)
        self.assertGreater(summary["elevation_range_m"], 100.0)
        self.assertLess(summary["maximum_absolute_slope_ratio"], 0.15)
        self.assertEqual(summary["closure_error_m"], 0.0)

    def test_scenario_changes_only_setup_and_explicit_track_profile(self):
        base = json.loads(impact.BASE_SCENARIO.read_text())
        profile = impact.build_track_profiles()["spw_relief"]
        scenario = json.loads(impact.build_scenario(base, profile, 0.25, 0.75))
        self.assertEqual(scenario["request"]["track_id"], "be-1925")
        self.assertEqual(scenario["request"]["track_profile"], profile)
        tuning = scenario["request"]["competitors"][0]["tuning"]
        self.assertEqual(tuning["downforce_slider"], 0.25)
        self.assertEqual(tuning["gear_ratio_slider"], 0.75)

    def test_committed_evidence_is_deterministic_and_non_promoting(self):
        report = json.loads(impact.DEFAULT_OUTPUT.read_text())
        self.assertEqual(report["planned_run_count"], 50)
        self.assertEqual(report["status"], "experimental_not_catalog_eligible")
        self.assertTrue(report["determinism"]["midpoint_repeated_output_equal"])
        self.assertFalse(report["automatic_model_change"])
        self.assertFalse(report["response"]["setup_optimum_changed"])
        self.assertGreater(
            report["response"]["matched_grid_relief_minus_flat"][
                "minimum_total_time_ms"
            ],
            0,
        )
        self.assertLess(
            report["response"]["matched_grid_relief_minus_flat"][
                "mean_maximum_speed_kph"
            ],
            0.0,
        )


if __name__ == "__main__":
    unittest.main()
