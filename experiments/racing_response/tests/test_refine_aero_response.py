from __future__ import annotations

import pathlib
import sys
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[3]
MODULE_ROOT = ROOT / "experiments" / "racing_response"
sys.path.insert(0, str(MODULE_ROOT))

import refine_aero_response as refinement  # noqa: E402


def point(
    circuit_id: str,
    downforce_index: int,
    gearing_index: int,
    optimum: int,
    maximum_index: int = 2,
) -> dict:
    return {
        "candidate_id": "candidate",
        "circuit_id": circuit_id,
        "downforce_index": downforce_index,
        "downforce_slider": downforce_index / maximum_index,
        "gearing_index": gearing_index,
        "gear_ratio_slider": gearing_index / maximum_index,
        "tuning_response_digest": "sha256:response",
        "experimental_execution_id": "sha256:execution",
        "total_time_ms": 100_000
        + 1_000 * abs(downforce_index - optimum)
        + 100 * gearing_index,
        "observed_maximum_speed_kph": 350.0 - 5 * downforce_index,
        "setup_response": {
            "mean_corner_speed_kph": 180.0 + 3 * downforce_index,
            "aerodynamic_drag_work_kj": 20_000.0 + 100 * downforce_index,
            "mean_downforce_n": 10_000.0 + 200 * downforce_index,
        },
    }


class AeroRefinementTest(unittest.TestCase):
    def test_refinement_keeps_historical_reference_and_bounded_neighborhood(self):
        base = {
            "downforce_slider_gain": 0.55,
            "drag_slider_gain": 0.30,
            "drag_base": 0.85,
        }
        candidates = refinement.refined_candidates(base)

        self.assertEqual(len(candidates), 43)
        self.assertEqual(candidates[0]["id"], refinement.HISTORICAL_ID)
        self.assertEqual(candidates[0]["response"], base)
        self.assertEqual(
            {candidate["downforce_slider_gain"] for candidate in candidates[1:]},
            set(refinement.DOWNFORCE_GAINS),
        )
        self.assertEqual(
            {candidate["drag_slider_gain"] for candidate in candidates[1:]},
            set(refinement.DRAG_GAINS),
        )

    def test_candidate_requires_strict_circuit_order_and_interior_suzuka(self):
        candidate = {
            "id": "candidate",
            "kind": "refined_candidate",
            "downforce_slider_gain": 0.25,
            "drag_slider_gain": 0.75,
        }
        optima = {"it-1922": 0, "jp-1962": 1, "mc-1929": 2}
        points = [
            point(circuit_id, downforce, gearing, optima[circuit_id])
            for circuit_id, _, _ in refinement.screening.CIRCUITS
            for downforce in range(3)
            for gearing in range(3)
        ]

        summary = refinement.summarize_candidate(candidate, points, 3)

        self.assertTrue(summary["shape_assessment"]["eligible"])
        self.assertTrue(
            summary["shape_assessment"]["strict_monza_suzuka_monaco_ordering"]
        )
        self.assertTrue(
            summary["shape_assessment"]["optima_are_materially_separated"]
        )
        self.assertEqual(
            [circuit["downforce_decision_margin_ms"] for circuit in summary["circuits"]],
            [1_000, 1_000, 1_000],
        )

    def test_ordered_but_nearly_identical_optima_are_not_material(self):
        candidate = {
            "id": "candidate",
            "kind": "refined_candidate",
            "downforce_slider_gain": 0.375,
            "drag_slider_gain": 0.85,
        }
        optima = {"it-1922": 0, "jp-1962": 9, "mc-1929": 10}
        points = [
            point(circuit_id, downforce, gearing, optima[circuit_id], 10)
            for circuit_id, _, _ in refinement.screening.CIRCUITS
            for downforce in range(11)
            for gearing in range(11)
        ]

        summary = refinement.summarize_candidate(candidate, points, 11)

        self.assertFalse(summary["shape_assessment"]["eligible"])
        self.assertTrue(
            summary["shape_assessment"]["strict_monza_suzuka_monaco_ordering"]
        )
        self.assertFalse(
            summary["shape_assessment"]["optima_are_materially_separated"]
        )

    def test_compatibility_metric_is_labeled_and_uses_historical_fastest(self):
        def summary(identifier: str, times: tuple[int, int, int]) -> dict:
            return {
                "candidate_id": identifier,
                "circuits": [
                    {
                        "circuit_slug": circuit[2],
                        "fastest": {"total_time_ms": time},
                    }
                    for circuit, time in zip(refinement.screening.CIRCUITS, times)
                ],
            }

        historical = summary(refinement.HISTORICAL_ID, (80_000, 60_000, 90_000))
        candidate = summary("candidate", (81_000, 62_000, 93_000))

        refinement.attach_compatibility(candidate, historical)

        compatibility = candidate["compatibility_pace"]
        self.assertEqual(
            compatibility["gap_to_historical_fastest_ms"],
            {"monza": 1_000, "monaco": 2_000, "suzuka": 3_000},
        )
        self.assertAlmostEqual(
            compatibility["root_mean_square_gap_ms"], 2160.247, places=3
        )
        self.assertIn("not physical calibration", compatibility["meaning"])


if __name__ == "__main__":
    unittest.main()
