import hashlib
import json
import pathlib
import re
import sys
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "adapter"))

from pitgun_databricks_adapter.decision_surface import (  # noqa: E402
    MODEL_ID,
    MODEL_VERSION,
    SCHEMA_VERSION,
    _contains_remote_reference,
    materialize_decision_surface_plan,
)


MANIFEST = ROOT / "campaigns" / "racing-v3-decision-surface-v1.json"
CHECKSUM = MANIFEST.with_suffix(".sha256")
SHA256_PATTERN = re.compile(r"sha256:[0-9a-f]{64}")


class V3DecisionSurfaceCampaignTests(unittest.TestCase):
    def setUp(self):
        self.manifest_bytes = MANIFEST.read_bytes()
        self.manifest = json.loads(self.manifest_bytes)
        self.plan = materialize_decision_surface_plan(self.manifest)

    def test_manifest_is_frozen_complete_and_non_promoting(self):
        expected_digest, expected_name = CHECKSUM.read_text().split()
        self.assertEqual(expected_name, MANIFEST.name)
        self.assertEqual(hashlib.sha256(self.manifest_bytes).hexdigest(), expected_digest)
        self.assertEqual(self.manifest["schema_version"], SCHEMA_VERSION)
        self.assertEqual(self.manifest["model"]["id"], MODEL_ID)
        self.assertEqual(self.manifest["model"]["version"], MODEL_VERSION)
        self.assertEqual(len(self.plan), 4928)
        self.assertEqual(len(self.plan), self.manifest["planned_run_count"])
        self.assertEqual(self.manifest["unique_configuration_count"], 2464)
        self.assertEqual(self.manifest["unique_scenario_count"], 2436)
        self.assertEqual(self.manifest["promotion_policy"], "human-review-required")
        self.assertFalse(self.manifest["automatic_catalog_promotion"])
        self.assertFalse(self.manifest["acceptance_criteria"]["automatic_release"])

    def test_plan_has_exact_content_addressed_inputs_and_natural_keys(self):
        natural_keys = set()
        execution_keys = set()
        for row in self.plan:
            self.assertIsInstance(row["scenario"], dict)
            self.assertIsInstance(row["profile"], dict)
            self.assertIn(row["seed"], {"7", "42"})
            self.assertFalse(_contains_remote_reference(row))
            for key in (
                "configuration_id",
                "expected_experimental_execution_id",
                "expected_probe_result_digest",
                "expected_compact_point_digest",
            ):
                self.assertRegex(row[key], SHA256_PATTERN)
            natural_keys.add((row["configuration_id"], row["seed"]))
            execution_keys.add(row["execution_key"])
        self.assertEqual(len(natural_keys), 4928)
        self.assertEqual(len(execution_keys), 4928)

    def test_plan_covers_reviewed_splits_families_and_dimensions(self):
        self.assertEqual({row["split"] for row in self.plan}, {"calibration", "held_out"})
        self.assertEqual(
            {row["family"] for row in self.plan},
            {"development.marginal", "development.transfer", "setup.grid"},
        )
        self.assertEqual(len({row["circuit_id"] for row in self.plan}), 7)
        self.assertEqual(len({row["vehicle_id"] for row in self.plan}), 4)
        self.assertEqual({row["budget"] for row in self.plan}, {4, 27, 37})

    def test_manifest_contains_no_player_or_private_service_data(self):
        serialized = self.manifest_bytes.decode().lower()
        for forbidden in (
            "career_id",
            "player_name",
            "api_key",
            "authority-signing-secret",
            "telemetry.staging",
        ):
            self.assertNotIn(forbidden, serialized)


if __name__ == "__main__":
    unittest.main()
