import importlib.util
import json
import pathlib
import sys
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[3]
EXPERIMENT_ROOT = ROOT / "experiments" / "racing_response"
sys.path.insert(0, str(EXPERIMENT_ROOT))
SPEC = importlib.util.spec_from_file_location(
    "diagnose_mixed_circuit", EXPERIMENT_ROOT / "diagnose_mixed_circuit.py"
)
diagnosis = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(diagnosis)


class MixedCircuitDiagnosisTest(unittest.TestCase):
    def base_response(self):
        return json.loads(diagnosis.CURRENT_RESPONSE.read_text())

    def test_candidate_space_is_bounded_and_starts_with_current_response(self):
        candidates = diagnosis.candidate_responses(self.base_response())
        self.assertEqual(len(candidates), 35)
        current = candidates[0]
        self.assertEqual(current["kind"], "current_reference")
        self.assertEqual(current["coefficient_distance_from_current"], 0.0)
        self.assertEqual(current["downforce_slider_gain"], 0.375)
        self.assertEqual(current["corner_aero_scale"], 1.05)
        self.assertTrue(
            all(
                candidate["response"]["drag_slider_gain"] == 0.95
                for candidate in candidates
            )
        )

    def test_fine_optimum_explains_coarse_suzuka_label(self):
        points = []
        levels = diagnosis.screening.slider_levels(11)
        for downforce_index, downforce in enumerate(levels):
            for gearing_index, gearing in enumerate(levels):
                points.append(
                    {
                        "circuit_id": "jp-1962",
                        "downforce_index": downforce_index,
                        "downforce_slider": downforce,
                        "gearing_index": gearing_index,
                        "gear_ratio_slider": gearing,
                        "total_time_ms": int(
                            88_000 + abs(downforce - 0.3) * 1000 + gearing * 10
                        ),
                    }
                )
        result = diagnosis.diagnose_review_grid(points, 11)
        self.assertEqual(result["classification"], "review-grid-aliasing")
        self.assertEqual(result["continuous_optimum"]["downforce_slider"], 0.3)
        self.assertEqual(
            result["reviewed_configuration_ranking"][0]["configuration_family"],
            "low-downforce",
        )
        self.assertLess(
            result["distance_to_continuous_downforce_optimum"]["low-downforce"],
            result["distance_to_continuous_downforce_optimum"]["balanced"],
        )

    def test_expected_bounds_preserve_distinct_circuit_archetypes(self):
        bounds = diagnosis.EXPECTED_DOWNFORCE_BOUNDS
        self.assertLess(bounds["it-1922"][1], bounds["jp-1962"][0])
        self.assertLess(bounds["jp-1962"][1], bounds["mc-1929"][0])
        self.assertEqual(
            set(bounds), {row[0] for row in diagnosis.CALIBRATION_CIRCUITS}
        )

    def test_holdout_failure_does_not_retroactively_change_calibration_rank(self):
        circuits = []
        for circuit_id, (lower, upper) in diagnosis.EXPECTED_DOWNFORCE_BOUNDS.items():
            circuits.append(
                {
                    "circuit_id": circuit_id,
                    "fastest": {"downforce_slider": (lower + upper) / 2},
                    "physical_invariant_failure_count": 0,
                    "maximum_observed_speed_kph": 380.0,
                }
            )
        circuits.extend(
            [
                {
                    "circuit_id": "gb-1948",
                    "fastest": {"downforce_slider": 0.7},
                    "physical_invariant_failure_count": 0,
                    "maximum_observed_speed_kph": 385.0,
                },
                {
                    "circuit_id": "be-1925",
                    "fastest": {"downforce_slider": 0.0},
                    "physical_invariant_failure_count": 11,
                    "maximum_observed_speed_kph": 406.0,
                },
            ]
        )
        assessment = diagnosis.assess_circuits(circuits)
        self.assertTrue(assessment["calibration_eligible"])
        self.assertFalse(assessment["global_guardrails_pass"])
        self.assertFalse(assessment["eligible"])


if __name__ == "__main__":
    unittest.main()
