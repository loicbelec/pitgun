from __future__ import annotations

import pathlib
import sys
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[3]
MODULE_ROOT = ROOT / "experiments" / "racing_response"
sys.path.insert(0, str(MODULE_ROOT))

import calibrate_aero_bases as calibration  # noqa: E402
import refine_aero_bases as refinement  # noqa: E402


class AeroBaseCalibrationTest(unittest.TestCase):
    def test_neutral_preservation_is_derived_from_historical_blends(self):
        historical = {
            "drag_base": 0.85,
            "drag_slider_gain": 0.30,
            "downforce_base": 0.75,
            "downforce_slider_gain": 0.55,
        }

        result = calibration.neutral_preserving_bases(
            historical,
            calibration.SELECTED_DOWNFORCE_GAIN,
            calibration.SELECTED_DRAG_GAIN,
        )

        self.assertAlmostEqual(result["drag_base"], 0.525)
        self.assertAlmostEqual(result["downforce_base"], 0.8375)

    def test_search_is_bounded_around_neutral_preservation(self):
        base = {
            "drag_base": 0.85,
            "drag_slider_gain": 0.30,
            "downforce_base": 0.75,
            "downforce_slider_gain": 0.55,
            "schema_version": "pitgun.racing-tuning-response/v1",
        }

        candidates = calibration.calibration_candidates(base)

        self.assertEqual(len(candidates), 27)
        self.assertEqual(candidates[0]["id"], calibration.refinement.HISTORICAL_ID)
        self.assertEqual(candidates[1]["id"], calibration.ANCHOR_ID)
        experimental = candidates[2:]
        self.assertEqual(
            {candidate["drag_base"] for candidate in experimental},
            set(calibration.DRAG_BASES),
        )
        self.assertEqual(
            {candidate["downforce_base"] for candidate in experimental},
            set(calibration.DOWNFORCE_BASES),
        )
        self.assertTrue(
            all(
                candidate["drag_slider_gain"] == calibration.SELECTED_DRAG_GAIN
                and candidate["downforce_slider_gain"]
                == calibration.SELECTED_DOWNFORCE_GAIN
                for candidate in experimental
            )
        )

    def test_refinement_space_contains_the_coarse_extension_anchor(self):
        self.assertEqual(len(refinement.DRAG_BASES), 5)
        self.assertEqual(len(refinement.DOWNFORCE_BASES), 5)
        self.assertIn(0.625, refinement.DRAG_BASES)
        self.assertIn(1.05625, refinement.DOWNFORCE_BASES)
        self.assertGreater(min(refinement.DOWNFORCE_BASES), 0.925)

    def test_refinement_grid_has_a_two_dimensional_interior(self):
        self.assertGreater(len(refinement.DRAG_BASES[1:-1]), 0)
        self.assertGreater(len(refinement.DOWNFORCE_BASES[1:-1]), 0)
        self.assertIn(0.650, refinement.DRAG_BASES[1:-1])
        self.assertIn(1.05625, refinement.DOWNFORCE_BASES[1:-1])


if __name__ == "__main__":
    unittest.main()
