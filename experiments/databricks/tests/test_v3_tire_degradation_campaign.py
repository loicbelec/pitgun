import hashlib
import json
import pathlib
import re
import sys
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "adapter"))

from pitgun_databricks_adapter.tire_degradation import (  # noqa: E402
    MODEL_ID,
    MODEL_VERSION,
    SCHEMA_VERSION,
    _canonical_digest,
    _contains_remote_reference,
    materialize_tire_degradation_plan,
)


MANIFEST = ROOT / "campaigns" / "racing-v3-tire-degradation-v1.json"
CHECKSUM = MANIFEST.with_suffix(".sha256")
SHA256_PATTERN = re.compile(r"sha256:[0-9a-f]{64}")


class V3TireDegradationCampaignTests(unittest.TestCase):
    def setUp(self):
        self.manifest_bytes = MANIFEST.read_bytes()
        self.manifest = json.loads(self.manifest_bytes)
        self.plan = materialize_tire_degradation_plan(self.manifest)

    def test_manifest_is_checksummed_bounded_and_non_promoting(self):
        expected_digest, expected_name = CHECKSUM.read_text().split()
        self.assertEqual(expected_name, MANIFEST.name)
        self.assertEqual(hashlib.sha256(self.manifest_bytes).hexdigest(), expected_digest)
        self.assertEqual(self.manifest["schema_version"], SCHEMA_VERSION)
        self.assertEqual(self.manifest["model"]["id"], MODEL_ID)
        self.assertEqual(self.manifest["model"]["version"], MODEL_VERSION)
        self.assertEqual(len(self.plan), self.manifest["planned_run_count"])
        self.assertEqual(len(self.plan), 236)
        self.assertEqual(self.manifest["unique_physical_execution_count"], 232)
        self.assertEqual(self.manifest["promotion_policy"], "human-review-required")
        self.assertFalse(self.manifest["automatic_catalog_promotion"])
        self.assertFalse(self.manifest["acceptance_criteria"]["automatic_release"])

    def test_every_configuration_has_explicit_content_addressed_inputs(self):
        for row in self.plan:
            self.assertIsInstance(row["scenario"], dict)
            self.assertIsInstance(row["profile"], dict)
            self.assertEqual(row["seed"], "42")
            self.assertFalse(_contains_remote_reference(row))
            for key in (
                "expected_scenario_digest",
                "expected_profile_digest",
                "expected_experimental_configuration_id",
                "expected_experimental_execution_id",
            ):
                self.assertRegex(row[key], SHA256_PATTERN)
            self.assertEqual(
                row["expected_experimental_configuration_id"],
                _canonical_digest(
                    {
                        "analysis_role": row["id"],
                        "profile_digest": row["expected_profile_digest"],
                        "scenario_digest": row["expected_scenario_digest"],
                    }
                ),
            )

    def test_plan_covers_the_four_reviewed_experimental_families(self):
        counts = {
            family: sum(row["family"] == family for row in self.plan)
            for family in {row["family"] for row in self.plan}
        }
        self.assertEqual(
            counts,
            {
                "compound.longrun": 96,
                "strategy.window": 80,
                "driver.control": 24,
                "parameter.screen": 36,
            },
        )
        self.assertEqual(len({row["id"] for row in self.plan}), 236)
        self.assertEqual(
            len(
                {
                    row["expected_experimental_configuration_id"]
                    for row in self.plan
                }
            ),
            236,
        )

    def test_campaign_contains_no_observed_player_or_private_service_data(self):
        serialized = self.manifest_bytes.decode()
        for forbidden in (
            "career_id",
            "leaderboard",
            "player_name",
            "api_key",
            "authority-signing-secret",
            "telemetry.staging",
        ):
            self.assertNotIn(forbidden, serialized.lower())


if __name__ == "__main__":
    unittest.main()
