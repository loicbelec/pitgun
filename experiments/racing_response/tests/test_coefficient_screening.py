from __future__ import annotations

import pathlib
import sys
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[3]
MODULE_ROOT = ROOT / "experiments" / "racing_response"
sys.path.insert(0, str(MODULE_ROOT))

import coefficient_screening  # noqa: E402


def point(
    circuit_index: int,
    circuit_id: str,
    downforce_index: int,
    gearing_index: int,
    total_time_ms: int,
) -> dict:
    return {
        "candidate_id": "df-0.25-drag-0.60",
        "circuit_index": circuit_index,
        "circuit_id": circuit_id,
        "downforce_index": downforce_index,
        "downforce_slider": downforce_index / 2,
        "gearing_index": gearing_index,
        "gear_ratio_slider": gearing_index / 2,
        "total_time_ms": total_time_ms,
        "observed_maximum_speed_kph": 350.0 - 5 * downforce_index,
        "setup_response": {
            "mean_corner_speed_kph": 180.0 + 3 * downforce_index,
            "aerodynamic_drag_work_kj": 20_000.0 + 100 * downforce_index,
            "mean_downforce_n": 10_000.0 + 200 * downforce_index,
        },
    }


class CoefficientScreeningTest(unittest.TestCase):
    def test_candidates_are_bounded_and_include_historical_aero_response(self):
        base = {
            "schema_version": "pitgun.racing-tuning-response/v1",
            "downforce_slider_gain": 0.55,
            "drag_slider_gain": 0.30,
            "gear_ratio_slider_reduction": 0.20,
        }
        candidates = coefficient_screening.candidate_responses(base)

        self.assertEqual(len(candidates), 25)
        historical = next(
            candidate
            for candidate in candidates
            if candidate["id"] == "df-0.55-drag-0.30"
        )
        self.assertEqual(historical["response"], base)
        self.assertTrue(
            all(
                candidate["response"]["gear_ratio_slider_reduction"] == 0.20
                for candidate in candidates
            )
        )

    def test_eligibility_requires_interior_differentiated_physical_optima(self):
        candidate = {
            "id": "df-0.25-drag-0.60",
            "downforce_slider_gain": 0.25,
            "drag_slider_gain": 0.60,
        }
        optimum_by_circuit = (0, 1, 2)
        points = []
        for circuit_index, (circuit_id, _, _) in enumerate(
            coefficient_screening.CIRCUITS
        ):
            optimum = optimum_by_circuit[circuit_index]
            for downforce_index in range(3):
                for gearing_index in range(3):
                    time = (
                        100_000
                        + 1_000 * abs(downforce_index - optimum)
                        + 100 * gearing_index
                    )
                    points.append(
                        point(
                            circuit_index,
                            circuit_id,
                            downforce_index,
                            gearing_index,
                            time,
                        )
                    )

        summary = coefficient_screening.summarize_candidate(candidate, points, 3)

        self.assertTrue(summary["assessment"]["eligible_for_deeper_calibration"])
        self.assertEqual(
            summary["assessment"]["downforce_boundary_optimum_count"], 2
        )
        self.assertEqual(
            summary["assessment"]["downforce_interior_optimum_count"], 1
        )
        self.assertEqual(
            summary["assessment"]["distinct_downforce_optimum_count"], 3
        )
        self.assertEqual(summary["assessment"]["downforce_optimum_range"], 1.0)
        self.assertEqual(
            summary["assessment"]["physical_invariant_failure_count"], 0
        )

    def test_broken_aerodynamic_invariant_rejects_candidate(self):
        candidate = {
            "id": "df-0.25-drag-0.60",
            "downforce_slider_gain": 0.25,
            "drag_slider_gain": 0.60,
        }
        points = []
        for circuit_index, (circuit_id, _, _) in enumerate(
            coefficient_screening.CIRCUITS
        ):
            for downforce_index in range(3):
                for gearing_index in range(3):
                    value = point(
                        circuit_index,
                        circuit_id,
                        downforce_index,
                        gearing_index,
                        100_000 + 1_000 * abs(downforce_index - 1),
                    )
                    value["observed_maximum_speed_kph"] = 350.0 + downforce_index
                    points.append(value)

        summary = coefficient_screening.summarize_candidate(candidate, points, 3)

        self.assertFalse(summary["assessment"]["eligible_for_deeper_calibration"])
        self.assertEqual(
            summary["assessment"]["physical_invariant_failure_count"], 3
        )


if __name__ == "__main__":
    unittest.main()
