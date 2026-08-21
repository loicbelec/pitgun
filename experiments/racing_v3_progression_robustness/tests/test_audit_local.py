import importlib.util
import json
import pathlib
import unittest


MODULE_PATH = pathlib.Path(__file__).parents[1] / "audit_local.py"
SPEC = importlib.util.spec_from_file_location("audit_local", MODULE_PATH)
assert SPEC and SPEC.loader
AUDIT = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(AUDIT)


class ProgressionRobustnessAuditTests(unittest.TestCase):
    def setUp(self):
        self.scenario = json.loads(AUDIT.shared.BASE_SCENARIO.read_bytes())

    def test_balanced_points_preserve_the_economy_budgets(self):
        for _, budget, _ in AUDIT.PROGRESSION:
            points = AUDIT.balanced_points(budget)
            self.assertEqual(sum(points.values()), budget)
            self.assertLessEqual(max(points.values()) - min(points.values()), 1)

    def test_every_directed_transfer_preserves_budget(self):
        for _, budget, delta in AUDIT.PROGRESSION:
            for donor in AUDIT.AXES:
                for target in AUDIT.AXES:
                    if donor == target:
                        continue
                    points = AUDIT.transfer_points(budget, delta, donor, target)
                    self.assertEqual(sum(points.values()), budget)
                    self.assertEqual(
                        points[AUDIT.POINT_KEYS[target]],
                        AUDIT.balanced_points(budget)[AUDIT.POINT_KEYS[target]] + delta,
                    )

    def test_plan_is_bounded_and_separates_held_out_tracks(self):
        plan = AUDIT.build_plan(self.scenario)
        self.assertEqual(len(plan), 4928)
        self.assertEqual({point["seed"] for point in plan}, {7, 42})
        self.assertEqual(
            {point["circuit_id"] for point in plan if point["split"] == "calibration"},
            {row[0] for row in AUDIT.CALIBRATION_CIRCUITS},
        )
        self.assertEqual(
            {point["circuit_id"] for point in plan if point["split"] == "held_out"},
            {row[0] for row in AUDIT.HELD_OUT_CIRCUITS},
        )

    def test_plan_covers_all_vehicles_and_economy_anchors(self):
        plan = AUDIT.build_plan(self.scenario)
        self.assertEqual(
            {point["vehicle_id"] for point in plan},
            {row[0] for row in AUDIT.VEHICLES},
        )
        self.assertEqual(
            {(point["progression"], point["budget"]) for point in plan},
            {("early", 4), ("mid", 27), ("late", 37)},
        )

    def test_strategy_evidence_is_checksumed_and_binds_v3_09(self):
        evidence = AUDIT.load_strategy_evidence(AUDIT.STRATEGY_REPORT)
        self.assertEqual(evidence["model"]["version"], "0.9.0")
        self.assertEqual(evidence["group_count"], 16)
        self.assertEqual(evidence["fastest_stop_laps"], [22])
        self.assertGreater(evidence["median_generic_strategy_regret_ms"], 0)

    def test_stored_report_is_complete_and_checksumed(self):
        report_bytes = AUDIT.OUTPUT.read_bytes()
        report = json.loads(report_bytes)
        self.assertEqual(
            AUDIT.OUTPUT.with_suffix(".sha256").read_text().strip(),
            AUDIT.sha256(report_bytes),
        )
        self.assertEqual(report["schema_version"], AUDIT.SCHEMA_VERSION)
        self.assertEqual(report["campaign"]["model"]["version"], "0.9.0")
        self.assertEqual(report["campaign"]["execution_count"], 4928)
        self.assertEqual(len(report["points"]), 4928)
        self.assertEqual(len(report["vehicle_progression_verdicts"]), 12)
        self.assertEqual(report["development_summary"]["group_count"], 84)
        self.assertEqual(report["marginal_summary"]["group_count"], 84)
        self.assertEqual(report["setup_summary"]["group_count"], 28)

    def test_development_summary_exposes_transfer_progression_shape(self):
        points = []
        for vehicle_id, _, vehicle_anchor in AUDIT.VEHICLES:
            for progression, budget, _ in AUDIT.PROGRESSION:
                for circuit_id, circuit_slug, _, split in AUDIT.CIRCUITS:
                    baseline = float(budget * 1_000)
                    for donor in AUDIT.AXES:
                        for target in AUDIT.AXES:
                            if donor == target:
                                continue
                            points.append(
                                {
                                    "family": "development.transfer",
                                    "case_id": f"transfer-{donor}-to-{target}",
                                    "split": split,
                                    "circuit_id": circuit_id,
                                    "circuit_slug": circuit_slug,
                                    "vehicle_id": vehicle_id,
                                    "vehicle_anchor": vehicle_anchor,
                                    "progression": progression,
                                    "budget": budget,
                                    "transfer_delta": 1,
                                    "donor_axis": donor,
                                    "target_axis": target,
                                    "allocation": {},
                                    "total_time_ms": baseline - 10,
                                }
                            )
                    points.append(
                        {
                            "family": "development.transfer",
                            "case_id": "balanced",
                            "split": split,
                            "circuit_id": circuit_id,
                            "circuit_slug": circuit_slug,
                            "vehicle_id": vehicle_id,
                            "vehicle_anchor": vehicle_anchor,
                            "progression": progression,
                            "budget": budget,
                            "total_time_ms": baseline,
                        }
                    )
        summary = AUDIT.development_summary(points)
        effects = [
            row
            for row in summary["progression_effects"]
            if row["vehicle_id"] == "classic_v8_1960"
        ]
        self.assertEqual(len(effects), 8)
        self.assertEqual({row["shape"] for row in effects}, {"stable"})


if __name__ == "__main__":
    unittest.main()
