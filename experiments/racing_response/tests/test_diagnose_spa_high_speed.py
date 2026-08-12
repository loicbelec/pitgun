import importlib.util
import json
import pathlib
import sys
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[3]
EXPERIMENT_ROOT = ROOT / "experiments" / "racing_response"
sys.path.insert(0, str(EXPERIMENT_ROOT))
SPEC = importlib.util.spec_from_file_location(
    "diagnose_spa_high_speed", EXPERIMENT_ROOT / "diagnose_spa_high_speed.py"
)
diagnosis = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(diagnosis)


class SpaHighSpeedDiagnosisTest(unittest.TestCase):
    def test_scenario_grid_changes_only_requested_setup_axes(self):
        base = json.loads(diagnosis.BASE_SCENARIO.read_text())
        scenario = json.loads(
            diagnosis.build_scenario(base, "be-1925", 0.0, 0.9)
        )
        self.assertEqual(scenario["request"]["track_id"], "be-1925")
        tuning = scenario["request"]["competitors"][0]["tuning"]
        self.assertEqual(tuning["downforce_slider"], 0.0)
        self.assertEqual(tuning["gear_ratio_slider"], 0.9)

    def test_band_deltas_separate_high_curvature_from_legacy_corner_metric(self):
        def point(downforce, legacy_corner_speed, band_speeds):
            return {
                "gear_ratio_slider": 0.5,
                "total_time_ms": 100_000 + int(downforce * 100),
                "peak_speed": {"speed_kph": 400.0 - downforce * 10.0},
                "setup_response": {
                    "mean_corner_speed_kph": legacy_corner_speed,
                    "aerodynamic_drag_work_kj": 100.0 + downforce * 10.0,
                    "mean_downforce_n": 1_000.0 + downforce * 100.0,
                },
                "curvature_bands": [
                    {"id": band, "mean_speed_kph": speed}
                    for band, speed in band_speeds.items()
                ],
            }

        low = point(0.0, 200.0, {"medium_curvature": 220.0, "high_curvature": 150.0})
        high = point(1.0, 198.0, {"medium_curvature": 210.0, "high_curvature": 160.0})
        delta = diagnosis.response_delta(high, low)
        self.assertLess(delta["legacy_mean_corner_speed_kph"], 0.0)
        self.assertLess(
            delta["mean_speed_by_curvature_band_kph"]["medium_curvature"], 0.0
        )
        self.assertGreater(
            delta["mean_speed_by_curvature_band_kph"]["high_curvature"], 0.0
        )

    def test_committed_evidence_reproduces_spa_and_preserves_calibration(self):
        report = json.loads(diagnosis.DEFAULT_OUTPUT.read_text())
        self.assertEqual(
            report["diagnosis"]["classification"],
            "GENERAL_SEGMENTATION_WEAKNESS_EXPOSED_BY_SPA",
        )
        self.assertAlmostEqual(
            report["diagnosis"]["spa_speed_reproduced_kph"],
            406.392679991782,
        )
        self.assertEqual(
            report["diagnosis"]["spa_legacy_corner_response_failure_count"], 11
        )
        self.assertFalse(report["guardrail"]["passes"])
        self.assertTrue(
            report["unchanged_calibration_baseline"]["calibration_eligible"]
        )
        self.assertEqual(
            len(report["unchanged_calibration_baseline"]["optima"]), 5
        )


if __name__ == "__main__":
    unittest.main()
