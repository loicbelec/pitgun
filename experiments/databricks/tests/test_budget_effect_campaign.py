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


if __name__ == "__main__":
    unittest.main()
