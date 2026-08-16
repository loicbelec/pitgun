import hashlib
import json
import pathlib
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[3]
POLICY = ROOT / "catalogs" / "racing" / "v1.3.0" / "simulation" / "policies" / "competitive.json"
CAMPAIGNS = ROOT / "experiments" / "databricks" / "campaigns"


class OpponentPolicyV2Test(unittest.TestCase):
    def load(self):
        return json.loads(POLICY.read_text())

    def test_policy_is_game_compatible_and_fail_closed_for_late_eras(self):
        policy = self.load()
        self.assertEqual(policy["schema_version"], "pitgun.racing-opponent-policy/v2")
        self.assertEqual(policy["scope"]["supported_game_eras"], [1, 2, 3, 4, 5])
        self.assertEqual(policy["scope"]["unsupported_game_eras"], [6, 7])
        self.assertEqual(policy["scope"]["unsupported_era_behavior"], "reject")
        self.assertEqual(policy["composition"]["field_size"], 9)
        self.assertEqual(
            sum(role["count"] for role in policy["composition"]["roles"]), 9
        )

    def test_policy_owns_balance_without_player_strategy_feedback(self):
        policy = self.load()
        self.assertFalse(policy["strategy"]["player_strategy_influence_allowed"])
        self.assertEqual(policy["budget"]["runtime_source"], "event-common-budget")
        self.assertEqual(
            [
                policy["budget"]["calibration_anchors"][stage]["development_points"]
                for stage in ("early", "mid", "late")
            ],
            [4, 27, 37],
        )
        self.assertEqual(len(policy["development_profiles"]), 5)
        self.assertIn("DEFAULT", policy["setup"]["circuit_baselines"])

    def test_governed_campaign_digests_match_checked_in_manifests(self):
        policy = self.load()
        expected = {
            "opponent-audit": "racing-opponent-audit-v1.json",
            "strategy-effect": "racing-strategy-effect-v1.json",
            "budget-effect": "racing-budget-effect-v2.json",
        }
        sources = {source["kind"]: source for source in policy["calibration"]["sources"]}
        for kind, filename in expected.items():
            digest = "sha256:" + hashlib.sha256((CAMPAIGNS / filename).read_bytes()).hexdigest()
            self.assertEqual(sources[kind]["manifest_digest"], digest)

    def test_policy_cannot_promote_itself_or_use_private_data(self):
        policy = self.load()
        review = policy["calibration"]["review"]
        self.assertEqual(review["decision"], "human-reviewed")
        self.assertFalse(review["automatic_promotion"])
        self.assertFalse(review["private_player_data_allowed"])
        serialized = POLICY.read_text()
        for forbidden in ("careerId", "playerName", "leaderboard", "telemetry"):
            self.assertNotIn(forbidden, serialized)


if __name__ == "__main__":
    unittest.main()
