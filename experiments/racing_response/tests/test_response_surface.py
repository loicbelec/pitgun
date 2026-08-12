from __future__ import annotations

import pathlib
import sys
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[3]
MODULE_ROOT = ROOT / "experiments" / "racing_response"
sys.path.insert(0, str(MODULE_ROOT))

import response_surface  # noqa: E402


def point(downforce_index: int, gearing_index: int, total_time_ms: int) -> dict:
    return {
        "circuit_index": 0,
        "circuit_id": "it-1922",
        "circuit_archetype": "power",
        "circuit_slug": "monza",
        "downforce_index": downforce_index,
        "downforce_slider": downforce_index / 2,
        "gearing_index": gearing_index,
        "gear_ratio_slider": gearing_index / 2,
        "configuration_id": f"configuration-{downforce_index}-{gearing_index}",
        "scenario_digest": "sha256:scenario",
        "run_id": "sha256:run",
        "total_time_ms": total_time_ms,
        "observed_maximum_speed_kph": 350.0 - downforce_index + gearing_index,
        "setup_response": {
            "circuit": {"length_m": 5786.0},
            "mean_straight_speed_kph": 300.0 - downforce_index,
            "mean_corner_speed_kph": 200.0 + downforce_index,
            "maximum_rpm_utilization": 0.8 + gearing_index / 100,
            "aerodynamic_drag_work_kj": 20_000.0 + downforce_index,
            "mean_downforce_n": 10_000.0 + downforce_index,
        },
    }


class ResponseSurfaceTest(unittest.TestCase):
    def test_slider_levels_are_bounded_and_include_midpoint(self):
        self.assertEqual(
            response_surface.slider_levels(5), [0.0, 0.25, 0.5, 0.75, 1.0]
        )
        for invalid in (1, 2, 4, 22, 23):
            with self.assertRaises(ValueError):
                response_surface.slider_levels(invalid)

    def test_scenario_changes_only_bounded_experiment_inputs(self):
        base = {
            "model": {"id": "pitgun.racing"},
            "request": {
                "track_id": "old",
                "competitors": [
                    {
                        "tuning": {
                            "downforce_slider": 0.5,
                            "gear_ratio_slider": 0.5,
                            "engine_points": 25.0,
                        }
                    }
                ],
            },
        }
        encoded = response_surface.build_scenario(base, "it-1922", 0.25, 0.75)
        decoded = __import__("json").loads(encoded)
        self.assertEqual(decoded["request"]["track_id"], "it-1922")
        self.assertEqual(
            decoded["request"]["competitors"][0]["tuning"]["downforce_slider"],
            0.25,
        )
        self.assertEqual(
            decoded["request"]["competitors"][0]["tuning"]["gear_ratio_slider"],
            0.75,
        )
        self.assertEqual(base["request"]["track_id"], "old")

    def test_summary_reports_optimum_boundary_and_isolated_effects(self):
        points = [
            point(downforce, gearing, 100_000 - 1_000 * downforce + 100 * gearing)
            for downforce in range(3)
            for gearing in range(3)
        ]
        summary = response_surface.summarize_circuit(
            response_surface.CIRCUITS[0], points, 3
        )

        self.assertEqual(summary["fastest"]["downforce_slider"], 1.0)
        self.assertEqual(summary["fastest"]["gear_ratio_slider"], 0.0)
        self.assertTrue(summary["fastest_is_on_boundary"])
        self.assertEqual(summary["total_time_range_ms"], 2_200)
        self.assertEqual(
            summary["isolated_downforce_delta_high_minus_low_at_mid_gearing"][
                "total_time_ms"
            ],
            -2_000,
        )
        self.assertEqual(
            summary["isolated_gearing_delta_long_minus_short_at_mid_downforce"][
                "total_time_ms"
            ],
            200,
        )


if __name__ == "__main__":
    unittest.main()
