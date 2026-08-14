import hashlib
import json
import pathlib
import sys
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[1]
FRAMEWORK = ROOT.parents[1]
sys.path.insert(0, str(ROOT / "adapter"))

from pitgun_databricks_adapter.budget_effect import (  # noqa: E402
    BudgetEffectManifestError,
    POINT_KEYS,
    _validate_manifest,
    extract_budget_effect_evidence,
    summarize_budget_effect,
)


MANIFEST = ROOT / "campaigns" / "racing-budget-effect-v1.json"
CHECKSUM = MANIFEST.with_suffix(".sha256")
SOURCE = ROOT / "campaigns" / "racing-strategy-effect-v1.json"
SCENARIOS = FRAMEWORK / "experiments" / "budget_effect" / "scenarios"


class BudgetEffectCampaignTest(unittest.TestCase):
    def load(self):
        manifest_bytes = MANIFEST.read_bytes()
        manifest = json.loads(manifest_bytes)
        resources = {path.stem: path.read_bytes() for path in SCENARIOS.glob("*.json")}
        source_digest = "sha256:" + hashlib.sha256(SOURCE.read_bytes()).hexdigest()
        return manifest, resources, source_digest, manifest_bytes

    def test_manifest_is_checksumming_45_exact_triplets(self):
        manifest, resources, source_digest, manifest_bytes = self.load()
        expected_digest, expected_name = CHECKSUM.read_text().split()

        self.assertEqual(expected_name, MANIFEST.name)
        self.assertEqual(hashlib.sha256(manifest_bytes).hexdigest(), expected_digest)
        self.assertEqual(len(resources), 135)
        _validate_manifest(manifest, resources, source_digest)

    def test_each_triplet_changes_only_player_budget_and_allocation(self):
        manifest, resources, _, _ = self.load()
        triplets = {}
        for run in manifest["runs"]:
            scenario = json.loads(resources[run["scenario_resource"]])
            player = next(
                row for row in scenario["request"]["competitors"] if row["is_player"]
            )
            treatment = {
                "budget_cap": player.pop("budget_cap"),
                "allocation": {
                    key: player["tuning"].pop(key) for key in POINT_KEYS
                },
            }
            triplets.setdefault(run["triplet_key"], []).append(
                (scenario, treatment)
            )

        self.assertEqual(len(triplets), 45)
        for triplet in triplets.values():
            baseline = triplet[0][0]
            self.assertTrue(all(row[0] == baseline for row in triplet[1:]))
            self.assertEqual(len({row[1]["budget_cap"] for row in triplet}), 3)
            self.assertEqual(len({json.dumps(row[1], sort_keys=True) for row in triplet}), 3)

    def test_budget_allocation_is_balanced_and_exhaustive(self):
        manifest, resources, _, _ = self.load()
        for run in manifest["runs"]:
            scenario = json.loads(resources[run["scenario_resource"]])
            player = next(
                row for row in scenario["request"]["competitors"] if row["is_player"]
            )
            points = [player["tuning"][key] for key in POINT_KEYS]
            self.assertEqual(sum(points), player["budget_cap"])
            self.assertLessEqual(max(points) - min(points), 1)

    def test_changed_resource_fails_closed(self):
        manifest, resources, source_digest, _ = self.load()
        resource = manifest["runs"][0]["scenario_resource"]
        resources[resource] += b" "

        with self.assertRaises(BudgetEffectManifestError):
            _validate_manifest(manifest, resources, source_digest)

    def test_inputs_contain_no_observed_player_data(self):
        _, resources, _, _ = self.load()
        serialized = b"".join(resources.values()).decode()
        for forbidden in ("careerId", "playerName", "leaderboard", "telemetry"):
            self.assertNotIn(forbidden, serialized)

    def result(self, manifest, run, player_position, player_total_time):
        standings = [
            {
                "competitor_id": f"ai_{position}",
                "position": position,
                "gap_to_leader_ms": (position - 1) * 1000,
                "best_lap_ms": 100_000 + position,
                "total_time_ms": 1_000_000 + (position - 1) * 1000,
            }
            for position in range(1, 10)
        ]
        standings.insert(
            player_position - 1,
            {
                "competitor_id": "player",
                "position": player_position,
                "gap_to_leader_ms": (player_position - 1) * 500,
                "best_lap_ms": 99_000 + player_total_time // 10_000,
                "total_time_ms": player_total_time,
            },
        )
        return {
            "configuration_id": "sha256:" + str(player_position) * 64,
            "run_id": "sha256:" + str(run["treatment_percentage"] // 10) * 64,
            "seed": str(run["seed"]),
            "scenario": {"id": "racing.budget-effect-campaign", "version": "1.0.0"},
            "model": {
                "id": manifest["catalog"]["model_id"],
                "version": manifest["catalog"]["model_version"],
                "digest": manifest["catalog"]["model_digest"],
            },
            "data_pack": {
                "id": "pitgun.racing.simulation",
                "version": manifest["catalog"]["version"],
                "digest": manifest["catalog"]["simulation_pack_digest"],
            },
            "summary": {"standings": standings},
        }

    def test_extract_and_summarize_exact_dose_response(self):
        manifest, _, _, _ = self.load()
        selected = manifest["runs"][:3]
        self.assertEqual(len({row["triplet_key"] for row in selected}), 1)
        evidence = []
        outcomes = {
            "field-090": (4, 1_010_000),
            "field-100": (3, 1_000_000),
            "field-110": (2, 990_000),
        }
        for run in selected:
            position, total_time = outcomes[run["treatment"]]
            evidence.append(
                extract_budget_effect_evidence(
                    run, self.result(manifest, run, position, total_time), manifest
                )
            )
        small_manifest = {
            **manifest,
            "runs": selected,
            "planned_triplet_count": 1,
            "planned_run_count": 3,
            "matrix": {**manifest["matrix"], "seeds": [selected[0]["seed"]]},
        }
        report = summarize_budget_effect(small_manifest, evidence, {"test": True})

        self.assertEqual(report["sample"], {"successful_run_count": 3, "triplet_count": 1})
        self.assertEqual(
            report["overall"]["median_total_time_delta_090_minus_100_ms"], 10_000
        )
        self.assertEqual(
            report["overall"]["median_total_time_delta_110_minus_100_ms"], -10_000
        )
        self.assertEqual(report["overall"]["monotonic_total_time_rate"], 1.0)
        self.assertFalse(report["budget_target_selected"])

    def test_full_manifest_reconciles_seed_stability(self):
        manifest, _, _, _ = self.load()
        evidence = []
        for run in manifest["runs"]:
            offset = {"field-090": 10_000, "field-100": 0, "field-110": -10_000}[
                run["treatment"]
            ]
            evidence.append(
                {
                    "run_key": run["run_key"],
                    "triplet_key": run["triplet_key"],
                    "circuit_id": run["circuit_id"],
                    "progression": run["progression"],
                    "seed": run["seed"],
                    "treatment": run["treatment"],
                    "player_budget": run["player_budget"],
                    "player_position": {"field-090": 4, "field-100": 3, "field-110": 2}[
                        run["treatment"]
                    ],
                    "player_gap_to_leader_ms": 1000 + offset,
                    "player_best_lap_ms": 100_000 + offset,
                    "player_total_time_ms": 1_000_000 + offset,
                    "field_spread_ms": 5000,
                }
            )

        report = summarize_budget_effect(manifest, evidence, {"test": True})

        self.assertEqual(report["sample"], {"successful_run_count": 135, "triplet_count": 45})
        self.assertEqual(report["seed_direction_stability"]["group_count"], 15)
        self.assertEqual(report["seed_direction_stability"]["stable_group_count"], 15)


if __name__ == "__main__":
    unittest.main()
