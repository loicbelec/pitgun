import hashlib
import json
import pathlib
import re
import sys
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[1]
FRAMEWORK = ROOT.parents[1]
sys.path.insert(0, str(ROOT / "adapter"))

from pitgun_databricks_adapter.driver_control import (  # noqa: E402
    MODEL_ID,
    MODEL_VERSION,
    SCHEMA_VERSION,
    materialize_driver_control_plan,
)


MANIFEST = ROOT / "campaigns/racing-v3-driver-control-surface-v2.json"
CHECKSUM = MANIFEST.with_suffix(".sha256")
FAILED_MANIFEST = ROOT / "campaigns/racing-v3-driver-control-surface-v1.json"
LOCAL_REPORT = (
    FRAMEWORK
    / "experiments/racing_v3_driver_control/results/local-driver-control-screen-v1.json"
)
SHA256_PATTERN = re.compile(r"sha256:[0-9a-f]{64}")


class V3DriverControlCampaignTests(unittest.TestCase):
    def setUp(self):
        self.payload = MANIFEST.read_bytes()
        self.manifest = json.loads(self.payload)
        self.plan = materialize_driver_control_plan(self.manifest)

    def test_manifest_is_checksummed_bounded_and_non_promoting(self):
        digest, name = CHECKSUM.read_text().split()
        self.assertEqual(name, MANIFEST.name)
        self.assertEqual(hashlib.sha256(self.payload).hexdigest(), digest)
        self.assertEqual(self.manifest["schema_version"], SCHEMA_VERSION)
        self.assertEqual(self.manifest["model"]["id"], MODEL_ID)
        self.assertEqual(self.manifest["model"]["version"], MODEL_VERSION)
        self.assertEqual(len(self.plan), 1584)
        self.assertEqual(self.manifest["planned_run_count"], len(self.plan))
        self.assertEqual(self.manifest["local_replay_run_count"], 48)
        self.assertEqual(self.manifest["new_evidence_run_count"], 1536)
        self.assertEqual(self.manifest["promotion_policy"], "human-review-required")
        self.assertFalse(self.manifest["automatic_catalog_promotion"])

    def test_parameter_space_has_baseline_and_32_bounded_points(self):
        parameter_sets = self.manifest["parameter_sets"]
        self.assertEqual(len(parameter_sets), 33)
        self.assertEqual(parameter_sets[0]["parameter_set_id"], "baseline-v10")
        self.assertEqual(len(self.manifest["profiles"]), 33)
        for row in parameter_sets:
            profile = row["parameters"]
            commitments = profile["mode_commitments"]
            self.assertLess(commitments["manage"], commitments["balanced"])
            self.assertLess(commitments["balanced"], commitments["attack"])
            self.assertLessEqual(commitments["attack"], 1.0)
            self.assertLessEqual(profile["base_control_error"] + profile["commitment_error_gain"], 0.25)
            self.assertLessEqual(profile["correction_workload_gain"], 10.0)
        explored = parameter_sets[1:]
        self.assertTrue(
            all(
                0.01
                <= row["parameters"]["mode_commitments"]["attack"]
                - row["parameters"]["mode_commitments"]["balanced"]
                <= 0.08
                for row in explored
            )
        )

    def test_failed_v1_is_preserved_and_v2_has_a_new_campaign_identity(self):
        failed = json.loads(FAILED_MANIFEST.read_text())
        self.assertEqual(
            failed["campaign_id"], "racing-v3-driver-control-surface-2026-v1"
        )
        self.assertEqual(
            self.manifest["campaign_id"], "racing-v3-driver-control-surface-2026-v2"
        )
        self.assertEqual(
            self.manifest["supersedes"]["campaign_id"], failed["campaign_id"]
        )
        self.assertEqual(self.manifest["supersedes"]["failed_run_id"], "904736248097501")
        invalid_v1 = [
            row for row in failed["parameter_sets"]
            if row["parameters"]["correction_workload_gain"] > 10.0
        ]
        self.assertEqual(len(invalid_v1), 4)

    def test_baseline_replay_preserves_local_rust_evidence(self):
        replay = [row for row in self.plan if row["expected_local_evidence"]]
        self.assertEqual(len(replay), 48)
        self.assertEqual({row["parameter_set_id"] for row in replay}, {"baseline-v10"})
        for row in replay:
            expected = row["expected_local_evidence"]
            for key in (
                "experimental_execution_id", "scenario_digest", "profile_digest",
                "driver_experiment_digest",
            ):
                self.assertRegex(expected[key], SHA256_PATTERN)
            self.assertEqual(len(expected["metrics"]), 11)

    def test_complete_local_matrix_is_reserved_for_independent_validation(self):
        local = json.loads(LOCAL_REPORT.read_text())
        self.assertEqual(len(local["runs"]), 702)
        governance = self.manifest["governance"]
        self.assertTrue(governance["complete_702_case_matrix_reserved_for_final_validation"])
        campaign_circuits = {row["circuit_id"] for row in self.plan}
        campaign_drivers = {row["driver_id"] for row in self.plan}
        campaign_compounds = {row["tire_id"] for row in self.plan}
        campaign_seeds = {row["seed"] for row in self.plan}
        self.assertNotIn(governance["held_out_circuit"], campaign_circuits)
        self.assertTrue(set(governance["held_out_driver_ids"]).isdisjoint(campaign_drivers))
        self.assertTrue(set(governance["held_out_compounds"]).isdisjoint(campaign_compounds))
        self.assertNotIn(governance["held_out_seed"], campaign_seeds)

    def test_execution_plan_is_paired_content_addressed_and_private_free(self):
        self.assertEqual(len({row["execution_key"] for row in self.plan}), len(self.plan))
        self.assertEqual(len({row["configuration_id"] for row in self.plan}), len(self.plan))
        groups = {}
        for row in self.plan:
            self.assertRegex(row["configuration_id"], SHA256_PATTERN)
            key = (
                row["parameter_set_id"], row["circuit_id"], row["horizon"],
                row["driver_id"], row["seed"],
            )
            groups.setdefault(key, set()).add(row["mode"])
        self.assertEqual(len(groups), 528)
        self.assertTrue(all(modes == {"manage", "balanced", "attack"} for modes in groups.values()))
        serialized = self.payload.decode().lower()
        for forbidden in (
            "career_id", "player_name", "api_key", "http://", "https://", "dbfs:/",
        ):
            self.assertNotIn(forbidden, serialized)

    def test_executor_is_resumable_and_never_selects_or_promotes(self):
        runner = (ROOT / "adapter/pitgun_databricks_adapter/runner.py").read_text()
        notebook = (ROOT / "src/execute_v3_driver_control_surface.py").read_text()
        jobs = (ROOT / "resources/jobs.yml").read_text()
        self.assertIn("execute_packaged_v3_driver_control", runner)
        self.assertIn("V3_DRIVER_CONTROL_EXECUTION_PATTERN.fullmatch", runner)
        self.assertIn("v3_driver_control_surface_job:", jobs)
        self.assertIn("racing-v3-driver-control-surface-v2", jobs)
        self.assertIn("target.execution_status <> 'SUCCESS'", notebook)
        self.assertIn("local_driver_control_evidence_mismatch", notebook)
        self.assertIn("validate_packaged_v3_driver_control_profiles", notebook)
        self.assertIn("target.campaign_id = source.campaign_id", notebook)
        self.assertIn("missing_metric_keys", notebook)
        self.assertIn("normalized_metric_execution_count", notebook)
        self.assertIn("driver-control normalized metrics are incomplete", notebook)
        self.assertIn('"candidate_selected": False', notebook)
        self.assertIn('"automatic_catalog_promotion": False', notebook)
        self.assertNotIn("policy_releases", notebook)


if __name__ == "__main__":
    unittest.main()
