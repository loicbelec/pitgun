import copy
import json
from pathlib import Path
import unittest

from run import cases, numerical_checks, strategy


class QualificationTests(unittest.TestCase):
    def test_registered_full_distance_budget_and_stop_coverage(self):
        plan = json.loads((Path(__file__).parent / "plan-v1.json").read_text())
        matrix = cases(plan)
        self.assertEqual(len(matrix), 114)
        for context in plan["contexts"]:
            rows = [row for row in matrix if row["circuit"] == context["circuit"]]
            self.assertEqual(len(rows), 38)
            self.assertEqual({len(row["strategy"]["pit_laps"]) for row in rows}, {0, 1, 2, 3})
            for row in rows:
                self.assertEqual(sum(row["allocation"]), 8)
                self.assertEqual(sum(s["laps"] for s in row["strategy"]["stints"]), context["laps"])
                self.assertTrue(all(s["laps"] > 0 for s in row["strategy"]["stints"]))

    def valid_finish(self):
        return {"outcome": {"ok": True, "service_time_s": 20., "time_s": 120., "elapsed_accounting_residual_s": 0.},
                "receipt": {"laps": 2, "stops": [1], "pit_loss_s": 20.},
                "laps": [{"time_s": 50.}, {"time_s": 50.}],
                "maximum_carry_jumps_speed_engine_surface_carcass_wear": [0.] * 5,
                "pit_resets_exact": True,
                "thermal_ledger": {"maximum_tire_balance_residual_j": 1e-9, "tire_generated_heat_j": 7.,
                                   "tire_direct_environment_heat_j": 3., "tire_rolling_dissipation_j": 6.,
                                   "tire_deformation_dissipation_j": 4.}}

    def test_pit_cost_cannot_disappear_from_successful_result(self):
        data = self.valid_finish()
        self.assertEqual(numerical_checks(data), [])
        missing_cost = copy.deepcopy(data)
        missing_cost["outcome"]["time_s"] = 100.
        self.assertIn("independent_elapsed_accounting", numerical_checks(missing_cost))
        wrong_stop = copy.deepcopy(data)
        wrong_stop["receipt"]["stops"].append(2)
        self.assertIn("service_cost", numerical_checks(wrong_stop))

    def test_false_success_and_physical_failure_remain_distinct(self):
        data = self.valid_finish()
        data["maximum_carry_jumps_speed_engine_surface_carcass_wear"][2] = 1.
        data["thermal_ledger"]["tire_generated_heat_j"] = 9.
        self.assertIn("boundary_jump", numerical_checks(data))
        self.assertIn("dissipation_accounting", numerical_checks(data))
        self.assertEqual(numerical_checks({"outcome": {"ok": False}}), ["physical_or_resolution_failure"])

    def test_rounded_pit_positions_cannot_create_empty_stints(self):
        with self.assertRaises(ValueError):
            strategy(3, ["soft", "medium", "hard"], [[1, 10], [2, 10]])


if __name__ == "__main__":
    unittest.main()
