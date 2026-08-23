import copy
import hashlib
import json
import pathlib
import re
import sys
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[1]
FRAMEWORK = ROOT.parents[1]
sys.path.insert(0, str(ROOT))
sys.path.insert(0, str(ROOT / "adapter"))

from build_v3_driver_mode_manifest import (  # noqa: E402
    MODEL,
    build_manifest,
    canonical_pretty,
)
from pitgun_databricks_adapter.driver_control import (  # noqa: E402
    MODE_CAMPAIGN_NAME,
    MODE_SCHEMA_VERSION,
    materialize_driver_control_plan,
)


MANIFEST = ROOT / "campaigns/racing-v3-driver-mode-surface-v3.json"
CHECKSUM = MANIFEST.with_suffix(".sha256")
BASE_PROFILE = (
    FRAMEWORK
    / "experiments/racing_v3_driver_control/shortlist/profile-v11.halton-19.json"
)
ROSTER = (
    FRAMEWORK
    / "experiments/racing_v3_driver_control/driver-archetypes-equal-budget-v1.json"
)
SHA256_PATTERN = re.compile(r"sha256:[0-9a-f]{64}")
MUTABLE_KEYS = {
    "mode_commitments",
    "commitment_error_gain",
    "commitment_error_exponent",
    "correction_workload_gain",
}


class V3DriverModeCampaignTests(unittest.TestCase):
    def setUp(self):
        self.payload = MANIFEST.read_bytes()
        self.manifest = json.loads(self.payload)
        self.plan = materialize_driver_control_plan(self.manifest)

    def test_manifest_is_reproducible_checksummed_and_non_promoting(self):
        digest, name = CHECKSUM.read_text().split()
        self.assertEqual(name, MANIFEST.name)
        self.assertEqual(hashlib.sha256(self.payload).hexdigest(), digest)
        self.assertEqual(canonical_pretty(build_manifest()), self.payload)
        self.assertEqual(self.manifest["schema_version"], MODE_SCHEMA_VERSION)
        self.assertEqual(self.manifest["model"], MODEL)
        self.assertEqual(self.manifest["planned_run_count"], 3168)
        self.assertEqual(self.manifest["local_replay_run_count"], 96)
        self.assertEqual(self.manifest["new_evidence_run_count"], 3072)
        self.assertEqual(self.manifest["unique_profile_count"], 33)
        self.assertEqual(len(self.plan), 3168)
        self.assertEqual(self.manifest["promotion_policy"], "human-review-required")
        self.assertFalse(self.manifest["automatic_catalog_promotion"])

    def test_equal_budget_roster_and_model_are_frozen(self):
        roster = json.loads(ROSTER.read_bytes())
        expected_budget = roster["design"]["trait_budget"]
        self.assertEqual(
            self.manifest["source_evidence"]["equal_budget_roster_digest"],
            "sha256:" + hashlib.sha256(canonical_pretty(roster)).hexdigest(),
        )
        self.assertEqual(
            self.manifest["source_evidence"]["local_review_verdict"],
            "MODE_RESPONSE_REFINEMENT_REQUIRED",
        )
        for driver in roster["drivers"]:
            self.assertAlmostEqual(sum(driver["traits"].values()), expected_budget)
        self.assertTrue(self.manifest["governance"]["driver_traits_are_fixed"])
        self.assertTrue(
            self.manifest["governance"]["model_and_non_driver_physics_are_fixed"]
        )

    def test_only_declared_mode_response_coefficients_vary(self):
        base = json.loads(BASE_PROFILE.read_bytes())
        base_non_driver = copy.deepcopy(base)
        base_control = base_non_driver.pop("driver_control_profile")
        fixed_control = {
            key: value for key, value in base_control.items() if key not in MUTABLE_KEYS
        }
        self.assertEqual(len(self.manifest["parameter_sets"]), 33)
        self.assertEqual(
            self.manifest["parameter_sets"][0]["parameter_set_id"],
            "anchor-halton-19",
        )
        for profile in self.manifest["profiles"].values():
            actual_non_driver = copy.deepcopy(profile)
            actual_control = actual_non_driver.pop("driver_control_profile")
            self.assertEqual(actual_non_driver, base_non_driver)
            self.assertEqual(
                {
                    key: value
                    for key, value in actual_control.items()
                    if key not in MUTABLE_KEYS
                },
                fixed_control,
            )
            modes = actual_control["mode_commitments"]
            self.assertLess(modes["manage"], modes["balanced"])
            self.assertLess(modes["balanced"], modes["attack"])
            self.assertLessEqual(
                actual_control["base_control_error"]
                + actual_control["commitment_error_gain"],
                0.25,
            )

    def test_plan_is_paired_content_addressed_and_holds_out_validation_axes(self):
        self.assertEqual(len({row["execution_key"] for row in self.plan}), len(self.plan))
        self.assertEqual(
            len({row["configuration_id"] for row in self.plan}), len(self.plan)
        )
        paired = {}
        global_contexts = {}
        for row in self.plan:
            self.assertRegex(row["configuration_id"], SHA256_PATTERN)
            paired_key = (
                row["parameter_set_id"],
                row["circuit_id"],
                row["horizon"],
                row["driver_id"],
                row["seed"],
            )
            paired.setdefault(paired_key, set()).add(row["mode"])
            global_key = (
                row["parameter_set_id"],
                row["circuit_id"],
                row["horizon"],
                row["seed"],
            )
            global_contexts.setdefault(global_key, set()).add(
                (row["driver_id"], row["mode"])
            )
        self.assertEqual(len(paired), 1056)
        self.assertTrue(
            all(modes == {"manage", "balanced", "attack"} for modes in paired.values())
        )
        self.assertEqual(len(global_contexts), 264)
        self.assertTrue(all(len(candidates) == 12 for candidates in global_contexts.values()))
        self.assertNotIn("jp-1962", {row["circuit_id"] for row in self.plan})
        self.assertNotIn("99", {row["seed"] for row in self.plan})
        self.assertEqual({row["tire_id"] for row in self.plan}, {"medium"})

    def test_anchor_replay_and_databricks_job_are_explicit(self):
        replay = [row for row in self.plan if row["expected_local_evidence"]]
        self.assertEqual(len(replay), 96)
        self.assertEqual(
            {row["parameter_set_id"] for row in replay}, {"anchor-halton-19"}
        )
        jobs = (ROOT / "resources/jobs.yml").read_text()
        notebook = (ROOT / "src/execute_v3_driver_control_surface.py").read_text()
        loader = (
            ROOT / "adapter/pitgun_databricks_adapter/driver_control.py"
        ).read_text()
        self.assertIn("v3_driver_mode_surface_job:", jobs)
        self.assertIn(MODE_CAMPAIGN_NAME, jobs)
        self.assertIn(MODE_CAMPAIGN_NAME, loader)
        self.assertIn("global_winning_driver_count", notebook)
        self.assertIn("global_short_attack_win_count", notebook)
        self.assertIn('"candidate_selected": False', notebook)
        self.assertIn('"automatic_catalog_promotion": False', notebook)
        self.assertNotIn("policy_releases", notebook)

    def test_manifest_contains_no_private_or_remote_inputs(self):
        serialized = self.payload.decode().lower()
        for forbidden in (
            "career_id",
            "player_name",
            "api_key",
            "http://",
            "https://",
            "dbfs:/",
        ):
            self.assertNotIn(forbidden, serialized)


if __name__ == "__main__":
    unittest.main()
