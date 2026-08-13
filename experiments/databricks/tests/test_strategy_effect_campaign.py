import copy
import hashlib
import json
import pathlib
import sys
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[1]
FRAMEWORK = ROOT.parents[1]
sys.path.insert(0, str(ROOT / "adapter"))

from pitgun_databricks_adapter.strategy_effect import (  # noqa: E402
    StrategyEffectManifestError,
    _validate_manifest,
)


MANIFEST = ROOT / "campaigns" / "racing-strategy-effect-v1.json"
CHECKSUM = MANIFEST.with_suffix(".sha256")
SOURCE = ROOT / "campaigns" / "racing-opponent-audit-v1.json"
SCENARIOS = FRAMEWORK / "experiments" / "strategy_effect" / "scenarios"


class StrategyEffectCampaignTest(unittest.TestCase):
    def load(self):
        manifest_bytes = MANIFEST.read_bytes()
        manifest = json.loads(manifest_bytes)
        resources = {path.stem: path.read_bytes() for path in SCENARIOS.glob("*.json")}
        source_digest = "sha256:" + hashlib.sha256(SOURCE.read_bytes()).hexdigest()
        return manifest, resources, source_digest, manifest_bytes

    def test_manifest_is_checksumming_45_exact_pairs(self):
        manifest, resources, source_digest, manifest_bytes = self.load()
        expected_digest, expected_name = CHECKSUM.read_text().split()

        self.assertEqual(expected_name, MANIFEST.name)
        self.assertEqual(hashlib.sha256(manifest_bytes).hexdigest(), expected_digest)
        self.assertEqual(len(resources), 90)
        _validate_manifest(manifest, resources, source_digest)

    def test_each_pair_changes_only_player_strategy(self):
        manifest, resources, _, _ = self.load()
        pairs = {}
        for run in manifest["runs"]:
            scenario = json.loads(resources[run["scenario_resource"]])
            player = next(
                row for row in scenario["request"]["competitors"] if row["is_player"]
            )
            strategy = copy.deepcopy(player.pop("stint_strategy"))
            pairs.setdefault(run["pair_key"], []).append(
                (run["strategy_profile"], scenario, strategy)
            )

        self.assertEqual(len(pairs), 45)
        for pair in pairs.values():
            self.assertEqual(pair[0][1], pair[1][1])
            self.assertNotEqual(pair[0][2], pair[1][2])

    def test_changed_resource_fails_closed(self):
        manifest, resources, source_digest, _ = self.load()
        resource = manifest["runs"][0]["scenario_resource"]
        resources[resource] += b" "

        with self.assertRaises(StrategyEffectManifestError):
            _validate_manifest(manifest, resources, source_digest)

    def test_inputs_contain_no_observed_player_data(self):
        _, resources, _, _ = self.load()
        serialized = b"".join(resources.values()).decode()
        for forbidden in ("careerId", "playerName", "leaderboard", "telemetry"):
            self.assertNotIn(forbidden, serialized)


if __name__ == "__main__":
    unittest.main()
