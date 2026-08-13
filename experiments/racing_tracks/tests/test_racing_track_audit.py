import importlib.util
import copy
import json
import math
import pathlib
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[3]
EXPERIMENT_ROOT = ROOT / "experiments" / "racing_tracks"


def load_module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


audit = load_module("audit_catalog_geometry", EXPERIMENT_ROOT / "audit_catalog_geometry.py")
prototype = load_module(
    "build_spa_elevation_prototype",
    EXPERIMENT_ROOT / "build_spa_elevation_prototype.py",
)


class RacingTrackAuditTest(unittest.TestCase):
    def test_every_current_circuit_is_flat_but_has_verified_source(self):
        report = audit.build_report()
        self.assertEqual(report["circuit_count"], 24)
        self.assertEqual(report["summary"]["flat_vertical_channel_count"], 24)
        self.assertEqual(report["summary"]["verified_exact_source_count"], 24)
        self.assertEqual(report["summary"]["inferred_source_count"], 0)
        self.assertEqual(report["summary"]["invalid_geometry_count"], 0)
        self.assertFalse(report["summary"]["promotion_ready"])

    def test_validation_detects_absent_and_inconsistent_vertical_channels(self):
        data = json.loads(prototype.TRACK.read_text())["data"]

        missing_slope = copy.deepcopy(data)
        del missing_slope["slope_pct"]
        self.assertIn(
            "missing_channel",
            {issue["code"] for issue in audit.validate_geometry(missing_slope)},
        )

        inconsistent = copy.deepcopy(data)
        inconsistent["z_m"][10] = 1.0
        self.assertIn(
            "vertical_channel_mismatch",
            {issue["code"] for issue in audit.validate_geometry(inconsistent)},
        )

    def test_spa_projection_recovers_the_pinned_wgs84_bounds(self):
        points = prototype.requested_points()
        self.assertEqual(len(points), 281)
        self.assertAlmostEqual(points[0]["latitude"], 50.444251, places=6)
        self.assertAlmostEqual(points[0]["longitude"], 5.965020, places=6)
        self.assertEqual(points[-1]["latitude"], points[0]["latitude"])
        self.assertEqual(points[-1]["longitude"], points[0]["longitude"])
        # The canonical centerline was smoothed and resampled at one metre, so
        # compare its recovered bounds to the source with a physical tolerance.
        latitude_error_m = abs(
            min(point["latitude"] for point in points) - 50.427678
        ) * 111_320
        longitude_error_m = abs(
            max(point["longitude"] for point in points) - 5.977560
        ) * 111_320 * math.cos(math.radians(prototype.REFERENCE_LATITUDE_DEG))
        self.assertLess(latitude_error_m, 2.0)
        self.assertLess(longitude_error_m, 2.0)

    def test_stored_spa_profile_is_closed_and_not_promotable(self):
        profile = json.loads(prototype.PROFILE_OUTPUT.read_text())
        self.assertEqual(profile["status"], "experimental_not_catalog_eligible")
        self.assertAlmostEqual(profile["summary"]["closure_error_m"], 0.0)
        self.assertGreater(profile["summary"]["smoothed_elevation_range_m"], 100.0)
        self.assertGreater(profile["summary"]["maximum_absolute_slope"], 0.15)
        self.assertTrue(profile["promotion_blockers"])


if __name__ == "__main__":
    unittest.main()
