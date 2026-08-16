import copy
import hashlib
import importlib.util
import json
import pathlib
import tempfile
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[1]
FRAMEWORK = ROOT.parents[1]
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


if __name__ == "__main__":
    unittest.main()
