import hashlib
import json
import pathlib
import re
import sys
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[1]
FRAMEWORK = ROOT.parents[1]
sys.path.insert(0, str(ROOT / "adapter"))

from pitgun_databricks_adapter.thermal_surface import (  # noqa: E402
    MODEL_ID,
    MODEL_VERSION,
    SCHEMA_VERSION,
    materialize_thermal_surface_plan,
)


MANIFEST = ROOT / "campaigns" / "racing-v3-thermal-adequacy-v1.json"
CHECKSUM = MANIFEST.with_suffix(".sha256")
LOCAL_REPORT = (
    FRAMEWORK
    / "experiments"
    / "racing_v3_thermal"
    / "results"
    / "local-thermal-screen-v1.json"
)
SHA256_PATTERN = re.compile(r"sha256:[0-9a-f]{64}")


class V3ThermalSurfaceCampaignTests(unittest.TestCase):
    def setUp(self):
        self.payload = MANIFEST.read_bytes()
        self.manifest = json.loads(self.payload)
        self.plan = materialize_thermal_surface_plan(self.manifest)

    def test_manifest_is_checksummed_bounded_and_non_promoting(self):
        expected_digest, expected_name = CHECKSUM.read_text().split()
        self.assertEqual(expected_name, MANIFEST.name)
        self.assertEqual(hashlib.sha256(self.payload).hexdigest(), expected_digest)
        self.assertEqual(self.manifest["schema_version"], SCHEMA_VERSION)
        self.assertEqual(self.manifest["model"]["id"], MODEL_ID)
        self.assertEqual(self.manifest["model"]["version"], MODEL_VERSION)
        self.assertEqual(len(self.plan), 1560)
        self.assertEqual(self.manifest["planned_run_count"], len(self.plan))
        self.assertEqual(self.manifest["local_replay_run_count"], 720)
        self.assertEqual(self.manifest["new_evidence_run_count"], 840)
        self.assertEqual(self.manifest["promotion_policy"], "human-review-required")
        self.assertFalse(self.manifest["automatic_catalog_promotion"])

    def test_plan_replays_fifteen_sets_and_densifies_eight_boundaries(self):
        parameter_sets = self.manifest["parameter_sets"]
        retained = [
            item
            for item in parameter_sets
            if item["origin"] == "retained_local_engaged_healthy"
        ]
        transition = [
            item
            for item in parameter_sets
            if item["origin"] == "deterministic_healthy_hot_interpolation"
        ]
        self.assertEqual(len(retained), 15)
        self.assertEqual(len(transition), 8)
        self.assertEqual(len(self.manifest["profiles"]), 23)
        self.assertTrue(
            all(len(item["source_parameter_set_ids"]) == 2 for item in transition)
        )

    def test_local_replay_keeps_exact_rust_identity_and_metric_evidence(self):
        replay = [row for row in self.plan if row["expected_local_evidence"]]
        self.assertEqual(len(replay), 720)
        self.assertEqual(
            {row["split"] for row in replay},
            {"local_replay_calibration", "local_replay_held_out"},
        )
        for row in replay:
            expected = row["expected_local_evidence"]
            # The refs address the pretty JSON payload packaged in the wheel;
            # these two identities come from Rust's canonical resolved input.
            self.assertRegex(row["scenario_ref"], SHA256_PATTERN)
            self.assertRegex(row["profile_ref"], SHA256_PATTERN)
            self.assertRegex(expected["scenario_digest"], SHA256_PATTERN)
            self.assertRegex(expected["profile_digest"], SHA256_PATTERN)
            self.assertRegex(expected["experimental_execution_id"], SHA256_PATTERN)
            self.assertEqual(len(expected["metrics"]), 7)

    def test_validation_inputs_were_absent_from_local_selection(self):
        local = json.loads(LOCAL_REPORT.read_text())
        local_circuits = {point["circuit_id"] for point in local["points"]}
        local_seeds = {point["seed"] for point in local["points"]}
        validation = [row for row in self.plan if row["split"] == "final_validation"]
        self.assertEqual({row["circuit_id"] for row in validation}, {"es-1991"})
        self.assertEqual({row["seed"] for row in validation}, {"20260821"})
        self.assertNotIn("es-1991", local_circuits)
        self.assertNotIn(20260821, local_seeds)
        self.assertTrue(all(row["expected_local_evidence"] is None for row in validation))

    def test_adequacy_contract_is_explicitly_era_aware(self):
        contract = self.manifest["adequacy_contract"]
        self.assertTrue(contract["classification_only_not_real_f1_calibration"])
        self.assertFalse(contract["historical_v8"]["thermal_engagement_required"])
        self.assertIn("long_run_thermal_engagement", contract["modern_v6t"]["required"])
        self.assertIn("long_run_thermal_engagement", contract["f1_2026"]["required"])
        self.assertFalse(
            contract["f1_2026"]["energy_controller_pace_feedback_in_scope"]
        )
        self.assertEqual(
            contract["verdicts"],
            ["PASS", "REFINE", "STRUCTURAL_CHANGE_REQUIRED"],
        )

    def test_inputs_are_content_addressed_private_free_and_offline(self):
        keys = {row["execution_key"] for row in self.plan}
        identities = {row["configuration_id"] for row in self.plan}
        self.assertEqual(len(keys), len(self.plan))
        self.assertEqual(len(identities), len(self.plan))
        serialized = self.payload.decode().lower()
        for forbidden in (
            "career_id",
            "player_name",
            "api_key",
            "authority-signing-secret",
            "http://",
            "https://",
            "dbfs:/",
        ):
            self.assertNotIn(forbidden, serialized)

    def test_executor_is_bounded_resumable_and_review_only(self):
        runner = (
            ROOT / "adapter" / "pitgun_databricks_adapter" / "runner.py"
        ).read_text()
        notebook = (ROOT / "src" / "execute_v3_thermal_surface.py").read_text()
        jobs = (ROOT / "resources" / "jobs.yml").read_text()
        self.assertIn("execute_packaged_v3_thermal_surface", runner)
        self.assertIn("V3_THERMAL_SURFACE_EXECUTION_PATTERN.fullmatch", runner)
        self.assertIn("v3_thermal_surface_job:", jobs)
        self.assertIn("racing-v3-thermal-adequacy-2026-v1", jobs)
        self.assertIn("execute_v3_thermal_surface.py", jobs)
        self.assertIn("target.execution_status <> 'SUCCESS'", notebook)
        self.assertIn("local_thermal_evidence_mismatch", notebook)
        self.assertIn('"per_family_verdicts_selected": False', notebook)
        self.assertIn('"automatic_catalog_promotion": False', notebook)
        for forbidden in ("policy_releases", "catalogs/racing", "requests.get"):
            self.assertNotIn(forbidden, notebook)


if __name__ == "__main__":
    unittest.main()
