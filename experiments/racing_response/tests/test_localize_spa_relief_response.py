import importlib.util
import json
import pathlib
import sys
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[3]
EXPERIMENT_ROOT = ROOT / "experiments" / "racing_response"
sys.path.insert(0, str(EXPERIMENT_ROOT))
SPEC = importlib.util.spec_from_file_location(
    "localize_spa_relief_response",
    EXPERIMENT_ROOT / "localize_spa_relief_response.py",
)
localization = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(localization)


class SpaReliefLocalizationTest(unittest.TestCase):
    def test_smoothing_variants_change_only_elevation(self):
        profiles = localization.build_profiles()
        flat = profiles["flat"]
        self.assertEqual(
            set(profiles), {"flat", "spw_75m", "spw_125m", "spw_175m"}
        )
        for name, profile in profiles.items():
            self.assertEqual(profile["s"], flat["s"], name)
            self.assertEqual(profile["x"], flat["x"], name)
            self.assertEqual(profile["y"], flat["y"], name)
            self.assertEqual(profile["z"][0], profile["z"][-1], name)
        self.assertTrue(all(value == 0.0 for value in flat["z"]))

    def test_committed_evidence_is_bounded_and_non_promoting(self):
        report = json.loads(localization.DEFAULT_OUTPUT.read_text())
        self.assertEqual(report["schema_version"], localization.SCHEMA_VERSION)
        self.assertEqual(report["status"], "experimental_not_catalog_eligible")
        self.assertTrue(report["smoothing_sensitivity"]["bounded"])
        self.assertLessEqual(
            report["smoothing_sensitivity"]["relief_time_spread_ms"], 100
        )
        self.assertTrue(report["vertical_dynamics_guardrail"]["passes"])
        self.assertFalse(report["conclusion"]["catalog_promotion_ready"])
        for delta in report["relief_minus_flat_time_ms"].values():
            self.assertGreater(delta, 0)

    def test_committed_evidence_covers_the_lap_at_25_metres(self):
        report = json.loads(localization.DEFAULT_OUTPUT.read_text())
        points = report["aligned_points"]
        self.assertGreater(len(points), 250)
        self.assertEqual(points[0]["distance_m"], 0.0)
        for previous, current in zip(points, points[1:]):
            self.assertEqual(
                current["distance_m"] - previous["distance_m"],
                localization.DISTANCE_SPACING_M,
            )


if __name__ == "__main__":
    unittest.main()
