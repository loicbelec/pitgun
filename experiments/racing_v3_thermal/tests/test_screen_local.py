import copy
import importlib.util
import pathlib
import unittest


MODULE_PATH = pathlib.Path(__file__).parents[1] / "screen_local.py"
SPEC = importlib.util.spec_from_file_location("thermal_screen_local", MODULE_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(MODULE)


def base_profile():
    return {
        "engine_thermal_resolution": {
            "thermal_capacity_multiplier": 1.0,
            "heat_generation_multiplier": 1.0,
            "static_cooling_multiplier": 1.0,
            "speed_cooling_multiplier": 1.0,
            "soft_limit_offset_c": 0.0,
            "derate_slope_multiplier": 1.0,
            "minimum_power_fraction": 0.2,
            "derating_shape": "linear-threshold",
            "smooth_knee_width_c": 0.0,
            "cooling_drag_area_m2_at_cap": 0.0,
        }
    }


class ThermalScreenTests(unittest.TestCase):
    def test_activation_boundary_covers_every_numeric_axis_and_two_knees(self):
        variants = MODULE.activation_profiles(base_profile())
        self.assertEqual(len(variants), 1 + 2 * len(MODULE.NUMERIC_AXES) + 2)
        self.assertEqual(
            {item["axis"] for item in variants},
            {item[0] for item in MODULE.NUMERIC_AXES}
            | {"baseline", "derating_shape"},
        )

    def test_halton_sampling_is_deterministic_bounded_and_does_not_mutate_base(self):
        profile = base_profile()
        original = copy.deepcopy(profile)
        axes = [item[0] for item in MODULE.NUMERIC_AXES] + ["derating_shape"]
        first = MODULE.adaptive_profiles(profile, axes, 16)
        second = MODULE.adaptive_profiles(profile, axes, 16)
        self.assertEqual(first, second)
        self.assertEqual(profile, original)
        for item in first:
            thermal = item["profile"]["engine_thermal_resolution"]
            for axis, low, high in MODULE.NUMERIC_AXES:
                self.assertGreaterEqual(thermal[axis], low)
                self.assertLessEqual(thermal[axis], high)
            if thermal["derating_shape"] == "linear-threshold":
                self.assertEqual(thermal["smooth_knee_width_c"], 0.0)
            else:
                self.assertGreaterEqual(thermal["smooth_knee_width_c"], 2.0)
                self.assertLessEqual(thermal["smooth_knee_width_c"], 20.0)

    def test_contexts_cover_partitions_eras_and_every_physical_vehicle(self):
        contexts = MODULE.context_metadata()
        self.assertEqual(
            len(contexts), len(MODULE.VEHICLE_ANCHORS) * len(MODULE.CIRCUITS)
        )
        self.assertEqual({item["partition"] for item in contexts}, {"calibration", "held-out"})
        self.assertEqual({item["era"] for item in contexts}, {1, 2, 3, 4, 5})
        self.assertEqual(
            {item["vehicle_id"] for item in contexts},
            {"classic_v8_1960", "classic_v8_1970", "modern_v6t", "f1_2026"},
        )

    def test_pareto_frontier_rejects_dominated_and_pathological_points(self):
        def item(identifier, pace, temperature, derating, cooling, pathological=0):
            return {
                "parameter_set_id": identifier,
                "median_total_time_ms": pace,
                "median_maximum_engine_temperature_c": temperature,
                "maximum_engine_temperature_c": temperature,
                "maximum_engine_derated_fraction": derating,
                "median_cooling_time_effect_ms": cooling,
                "pathological_execution_count": pathological,
            }

        report = MODULE.pareto_summary(
            [
                item("balanced", 100.0, 120.0, 0.01, 10.0),
                item("cool", 105.0, 110.0, 0.00, -10.0),
                item("cold", 90.0, 95.0, 0.00, -20.0),
                item("dominated", 110.0, 130.0, 0.02, 20.0),
                item("invalid", 80.0, 200.0, 0.70, -30.0, 1),
            ]
        )
        self.assertEqual(
            report["frontier_parameter_set_ids"], ["balanced", "cool"]
        )
        self.assertEqual(report["pathological_parameter_set_count"], 1)

    def test_refinement_is_deterministic_and_binds_both_sides_of_transition(self):
        axes = [item[0] for item in MODULE.NUMERIC_AXES] + ["derating_shape"]
        broad = MODULE.adaptive_profiles(base_profile(), axes, 8)
        aggregates = []
        for index, item in enumerate(broad):
            aggregates.append(
                {
                    "parameter_set_id": item["parameter_set_id"],
                    "pathological_execution_count": 0 if index < 4 else 1,
                    "maximum_engine_temperature_c": 105.0 + index * 10.0,
                }
            )
        first, anchors = MODULE.refinement_profiles(
            base_profile(), axes, broad, aggregates, 16
        )
        second, repeated_anchors = MODULE.refinement_profiles(
            base_profile(), axes, broad, aggregates, 16
        )
        self.assertEqual(first, second)
        self.assertEqual(anchors, repeated_anchors)
        self.assertTrue(
            any(
                aggregate["pathological_execution_count"] == 0
                for aggregate in aggregates
                if aggregate["parameter_set_id"] in anchors
            )
        )
        self.assertTrue(
            any(
                aggregate["pathological_execution_count"] > 0
                for aggregate in aggregates
                if aggregate["parameter_set_id"] in anchors
            )
        )

    def test_derating_controls_join_dependency_closure_after_heat_activation(self):
        baseline = {
            "context_id": "context",
            "workload": "long",
            "partition": "calibration",
            "axis": "baseline",
            "level": "baseline",
            "total_time_ms": 100_000,
            "mechanical_diagnostics": {
                "maximum_engine_temperature_c": 100.0,
                "engine_derated_time_s": 0.0,
                "generated_engine_heat_kj": 100.0,
                "removed_engine_heat_kj": 90.0,
                "fixed_drag_area_m2": 1.0,
            },
        }
        heat = copy.deepcopy(baseline)
        heat.update(
            {
                "axis": "heat_generation_multiplier",
                "level": "high",
                "total_time_ms": 100_100,
            }
        )
        heat["mechanical_diagnostics"]["maximum_engine_temperature_c"] = 101.0
        report = MODULE.activation_summary([baseline, heat])
        self.assertIn("heat_generation_multiplier", report["measured_active_axes"])
        self.assertEqual(
            report["conditionally_selected_axes"],
            [
                "derate_slope_multiplier",
                "minimum_power_fraction",
                "derating_shape",
            ],
        )


if __name__ == "__main__":
    unittest.main()
