import hashlib
import json
import pathlib
import sys
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[1]
FRAMEWORK = ROOT.parents[1]
sys.path.insert(0, str(ROOT / "adapter"))

from pitgun_databricks_adapter.opponent_audit import (  # noqa: E402
    OpponentAuditManifestError,
    _validate_manifest,
)

sys.path.insert(0, str(FRAMEWORK / "experiments" / "opponent_audit"))
from run_smoke import OpponentSmokeError, validate_source  # noqa: E402


MANIFEST = ROOT / "campaigns" / "racing-opponent-audit-v1.json"
CHECKSUM = ROOT / "campaigns" / "racing-opponent-audit-v1.sha256"
SCENARIOS = FRAMEWORK / "experiments" / "opponent_audit" / "campaign_scenarios"
CIRCUIT_BASELINES = {
    "BUDAPEST": (0.72, 0.40),
    "MONACO": (0.86, 0.28),
    "MONZA": (0.22, 0.78),
    "SINGAPORE": (0.82, 0.34),
    "SUZUKA": (0.62, 0.50),
}


class OpponentAuditCampaignTest(unittest.TestCase):
    def load(self):
        manifest_bytes = MANIFEST.read_bytes()
        manifest = json.loads(manifest_bytes)
        resources = {
            path.stem: "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()
            for path in SCENARIOS.glob("*.json")
        }
        return manifest, resources, manifest_bytes

    def test_manifest_is_frozen_complete_and_resource_backed(self):
        manifest, resources, manifest_bytes = self.load()
        expected_digest, expected_name = CHECKSUM.read_text().split()

        self.assertEqual(expected_name, MANIFEST.name)
        self.assertEqual(hashlib.sha256(manifest_bytes).hexdigest(), expected_digest)
        self.assertEqual(len(resources), 180)
        _validate_manifest(manifest, resources)

    def test_missing_or_changed_resource_fails_closed(self):
        manifest, resources, _ = self.load()
        resources.pop(manifest["runs"][0]["scenario_resource"])

        with self.assertRaises(OpponentAuditManifestError):
            _validate_manifest(manifest, resources)

    def test_changed_game_source_identity_fails_closed(self):
        with self.assertRaises(OpponentSmokeError):
            validate_source(
                {
                    "schemaVersion": "pitgun.opponent-contract-audit/v1",
                    "artifactDigest": "sha256:" + "0" * 64,
                }
            )

    def test_scenarios_pin_v2_and_contain_no_observed_player(self):
        for path in SCENARIOS.glob("*.json"):
            scenario = json.loads(path.read_text())
            self.assertEqual(scenario["model"]["version"], "2.0.0")
            self.assertEqual(scenario["data_pack"]["version"], "1.2.0")
            players = [
                competitor
                for competitor in scenario["request"]["competitors"]
                if competitor["is_player"]
            ]
            self.assertEqual(len(players), 1)
            self.assertEqual(players[0]["id"], "player")
            tuning = players[0]["tuning"]
            if path.stem.endswith("--neutral"):
                expected = (0.5, 0.5)
            else:
                expected = CIRCUIT_BASELINES[scenario["request"]["track_id"]]
            self.assertEqual(
                (tuning["downforce_slider"], tuning["gear_ratio_slider"]),
                expected,
            )
            serialized = json.dumps(scenario)
            for forbidden in ("careerId", "playerName", "leaderboard", "telemetry"):
                self.assertNotIn(forbidden, serialized)

    def test_manifest_records_opponent_contract_dimensions(self):
        manifest, _, _ = self.load()
        for run in manifest["runs"]:
            self.assertLessEqual(run["opponent_budget_min"], run["opponent_budget_max"])
            self.assertGreaterEqual(run["distinct_opponent_tunings"], 1)
            self.assertLessEqual(run["distinct_opponent_tunings"], 9)
            self.assertGreaterEqual(run["distinct_opponent_strategies"], 1)
            self.assertLessEqual(run["distinct_opponent_strategies"], 9)


if __name__ == "__main__":
    unittest.main()
