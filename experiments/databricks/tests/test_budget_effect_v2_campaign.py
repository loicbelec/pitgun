import copy
import hashlib
import importlib.util
import json
import pathlib
import sys
import tempfile
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[1]
FRAMEWORK = ROOT.parents[1]
sys.path.insert(0, str(ROOT / "adapter"))

from pitgun_databricks_adapter.budget_effect_v2 import (  # noqa: E402
    BudgetEffectV2ManifestError,
    _validate_manifest,
    extract_budget_effect_v2_evidence,
    summarize_budget_effect_v2,
)

BUILDER_PATH = FRAMEWORK / "experiments" / "budget_effect_v2" / "build_campaign.py"
SPEC = importlib.util.spec_from_file_location("budget_effect_v2_builder", BUILDER_PATH)
BUILDER = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(BUILDER)

MANIFEST = ROOT / "campaigns" / "racing-budget-effect-v2.json"
CHECKSUM = MANIFEST.with_suffix(".sha256")
SCENARIOS = FRAMEWORK / "experiments" / "budget_effect_v2" / "scenarios"


class BudgetEffectV2CampaignTest(unittest.TestCase):
    def load(self):
        manifest_bytes = MANIFEST.read_bytes()
        manifest = json.loads(manifest_bytes)
        resources = {path.stem: path.read_bytes() for path in SCENARIOS.glob("*.json")}
        return manifest, resources, manifest_bytes

    def test_manifest_is_checksummed_and_reproducible(self):
        manifest, resources, manifest_bytes = self.load()
        expected_digest, expected_name = CHECKSUM.read_text().split()

        self.assertEqual(expected_name, MANIFEST.name)
        self.assertEqual(hashlib.sha256(manifest_bytes).hexdigest(), expected_digest)
        self.assertEqual(manifest["schema_version"], BUILDER.SCHEMA_VERSION)
        self.assertEqual(manifest["campaign_id"], BUILDER.CAMPAIGN_ID)
        self.assertEqual(len(resources), 135)

        source, source_digest = BUILDER.load_source()
        with tempfile.TemporaryDirectory() as directory:
            rebuilt = BUILDER.build_manifest(source, source_digest, pathlib.Path(directory))
            self.assertEqual(rebuilt, manifest)
            rebuilt_resources = {
                path.stem: path.read_bytes() for path in pathlib.Path(directory).glob("*.json")
            }
        self.assertEqual(rebuilt_resources, resources)
        _validate_manifest(manifest, resources, source_digest)

    def test_gameplay_progression_provenance_is_frozen(self):
        manifest, _, _ = self.load()
        self.assertEqual(
            manifest["source"]["gameplay_progression"], BUILDER.PROGRESSION_ARTIFACT
        )
        self.assertEqual(
            manifest["controlled_input"]["treatments_by_progression"],
            BUILDER.TREATMENTS,
        )
        self.assertEqual(
            [row["referenceBudget"] for row in manifest["matrix"]["progression"]],
            [4, 27, 37],
        )
        self.assertNotIn("playerBudget", json.dumps(manifest["matrix"]))

    def test_each_triplet_changes_only_player_budget_and_allocation(self):
        manifest, resources, _ = self.load()
        triplets = {}
        for run in manifest["runs"]:
            scenario = json.loads(resources[run["scenario_resource"]])
            player = BUILDER.controlled_player(scenario)
            treatment = {
                "budget_cap": player.pop("budget_cap"),
                "allocation": {
                    key: player["tuning"].pop(key) for key in BUILDER.POINT_KEYS
                },
            }
            triplets.setdefault(run["triplet_key"], []).append((scenario, treatment))

        self.assertEqual(len(triplets), 45)
        for triplet in triplets.values():
            baseline = triplet[0][0]
            self.assertTrue(all(row[0] == baseline for row in triplet[1:]))
            self.assertEqual(len({row[1]["budget_cap"] for row in triplet}), 3)

    def test_player_and_opponent_budgets_stay_unsaturated(self):
        manifest, resources, _ = self.load()
        for run in manifest["runs"]:
            scenario = json.loads(resources[run["scenario_resource"]])
            expected = BUILDER.TREATMENTS[run["progression"]][run["treatment"]]
            player = BUILDER.controlled_player(scenario)
            player_points = [player["tuning"][key] for key in BUILDER.POINT_KEYS]
            self.assertEqual(player["budget_cap"], expected)
            self.assertEqual(sum(player_points), expected)
            self.assertLessEqual(max(player_points), 20)

            opponents = [
                row for row in scenario["request"]["competitors"] if not row["is_player"]
            ]
            self.assertEqual(len(opponents), 9)
            for opponent in opponents:
                points = [opponent["tuning"][key] for key in BUILDER.POINT_KEYS]
                self.assertEqual(opponent["budget_cap"], run["reference_budget"])
                self.assertEqual(sum(points), run["reference_budget"])
                self.assertLessEqual(max(points), 20)

    def test_changed_resource_or_private_data_is_rejected_by_contract(self):
        manifest, resources, _ = self.load()
        serialized = b"".join(resources.values()).decode()
        for forbidden in ("careerId", "playerName", "leaderboard", "telemetry"):
            self.assertNotIn(forbidden, serialized)

        _, source_digest = BUILDER.load_source()
        changed_manifest = copy.deepcopy(manifest)
        changed_manifest["runs"][0]["player_budget"] += 1
        with self.assertRaises(BUILDER.BudgetEffectV2BuildError):
            BUILDER.validate_manifest(changed_manifest, resources, source_digest)

        changed_resources = dict(resources)
        first = manifest["runs"][0]["scenario_resource"]
        changed_resources[first] += b" "
        with self.assertRaises(BUILDER.BudgetEffectV2BuildError):
            BUILDER.validate_manifest(manifest, changed_resources, source_digest)
        with self.assertRaises(BudgetEffectV2ManifestError):
            _validate_manifest(manifest, changed_resources, source_digest)

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
            "run_id": "sha256:" + str(run["player_budget"] % 10) * 64,
            "seed": str(run["seed"]),
            "scenario": {"id": "racing.budget-effect-campaign", "version": "2.0.0"},
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

    def test_extract_and_summarize_economy_backed_dose_response(self):
        manifest, _, _ = self.load()
        selected = manifest["runs"][:3]
        self.assertEqual(len({row["triplet_key"] for row in selected}), 1)
        outcomes = {
            "below": (4, 1_010_000),
            "reference": (3, 1_000_000),
            "above": (2, 990_000),
        }
        evidence = []
        for run in selected:
            position, total_time = outcomes[run["treatment"]]
            evidence.append(
                extract_budget_effect_v2_evidence(
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
        report = summarize_budget_effect_v2(
            small_manifest, evidence, {"test": True}
        )

        self.assertEqual(report["schema_version"], "pitgun.budget-effect-report/v2")
        self.assertEqual(report["sample"], {"successful_run_count": 3, "triplet_count": 1})
        self.assertEqual(
            report["overall"]["median_total_time_delta_below_minus_reference_ms"],
            10_000,
        )
        self.assertEqual(
            report["overall"]["median_total_time_delta_above_minus_reference_ms"],
            -10_000,
        )
        self.assertEqual(report["overall"]["monotonic_total_time_rate"], 1.0)
        self.assertFalse(report["budget_target_selected"])

    def test_full_manifest_reconciles_all_seed_stability_groups(self):
        manifest, _, _ = self.load()
        evidence = []
        for run in manifest["runs"]:
            offset = {"below": 10_000, "reference": 0, "above": -10_000}[
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
                    "reference_budget": run["reference_budget"],
                    "opponent_budget": run["opponent_budget"],
                    "player_budget": run["player_budget"],
                    "player_position": {
                        "below": 4,
                        "reference": 3,
                        "above": 2,
                    }[run["treatment"]],
                    "player_gap_to_leader_ms": 1000 + offset,
                    "player_best_lap_ms": 100_000 + offset,
                    "player_total_time_ms": 1_000_000 + offset,
                    "field_spread_ms": 5000,
                }
            )

        report = summarize_budget_effect_v2(manifest, evidence, {"test": True})

        self.assertEqual(
            report["sample"], {"successful_run_count": 135, "triplet_count": 45}
        )
        self.assertEqual(report["seed_direction_stability"]["group_count"], 15)
        self.assertEqual(
            report["seed_direction_stability"]["stable_group_count"], 15
        )


if __name__ == "__main__":
    unittest.main()
