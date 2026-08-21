import hashlib
import json
import pathlib
import sys
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[1]
FRAMEWORK = ROOT.parents[1]
sys.path.insert(0, str(ROOT / "adapter"))

from pitgun_databricks_adapter.thermal_surface import (  # noqa: E402
    REFINEMENT_VALIDATION_CAMPAIGN_NAME,
    materialize_thermal_surface_plan,
)


MANIFEST = ROOT / "campaigns" / f"{REFINEMENT_VALIDATION_CAMPAIGN_NAME}.json"


class V3ThermalRefinementValidationTests(unittest.TestCase):
    def setUp(self):
        self.payload = MANIFEST.read_bytes()
        self.manifest = json.loads(self.payload)
        self.digest = "sha256:" + hashlib.sha256(self.payload).hexdigest()
        self.plan = materialize_thermal_surface_plan(self.manifest)

    def test_manifest_is_immutable_bounded_and_new_evidence_only(self):
        expected_digest, expected_name = MANIFEST.with_suffix(".sha256").read_text().split()
        self.assertEqual(expected_name, MANIFEST.name)
        self.assertEqual(expected_digest, hashlib.sha256(self.payload).hexdigest())
        self.assertEqual(self.digest, "sha256:" + expected_digest)
        self.assertEqual(len(self.plan), 12)
        self.assertEqual(self.manifest["planned_run_count"], 12)
        self.assertEqual(self.manifest["local_replay_run_count"], 0)
        self.assertEqual(self.manifest["new_evidence_run_count"], 12)
        self.assertFalse(self.manifest["automatic_catalog_promotion"])
        self.assertEqual(
            self.manifest["source_evidence"]["experimental_fuel_reservoir_kg"],
            130.0,
        )
        self.assertEqual(
            self.manifest["source_evidence"]["supersedes_campaign_id"],
            "racing-v3-thermal-refinement-validation-2026-v1",
        )

    def test_plan_uses_only_the_reserved_validation_boundary(self):
        self.assertEqual({row["circuit_id"] for row in self.plan}, {"gb-1948"})
        self.assertEqual({row["seed"] for row in self.plan}, {"20260901"})
        self.assertEqual({row["laps"] for row in self.plan}, {52})
        self.assertEqual({row["workload"] for row in self.plan}, {"full-race"})
        self.assertEqual({row["cooling_points"] for row in self.plan}, {0, 10, 20})
        self.assertTrue(all(row["expected_local_evidence"] is None for row in self.plan))

    def test_profiles_are_explicitly_family_specific(self):
        profiles = self.manifest["dimensions"]["profiles_by_family"]
        self.assertEqual(profiles["historical_v8"], "historical-default")
        self.assertEqual(profiles["modern_v6t"], "modern-v6t-soft-limit--3.0c")
        self.assertEqual(profiles["f1_2026"], "f1-2026-adaptive-038")
        self.assertEqual(
            {row["vehicle_id"] for row in self.plan},
            {"classic_v8_1960", "classic_v8_1970", "modern_v6t", "f1_2026"},
        )

    def test_source_selection_is_content_addressed(self):
        source = self.manifest["source_evidence"]
        result = (
            FRAMEWORK
            / "experiments"
            / "racing_v3_thermal_refinement"
            / "results"
            / "local-refinement-v1.json"
        ).read_bytes()
        self.assertEqual(
            source["artifact_digest"],
            "sha256:" + hashlib.sha256(result).hexdigest(),
        )
        self.assertEqual(source["selected_parameter_set_id"], "soft-limit--3.0c")

    def test_job_uses_the_bounded_shared_executor(self):
        jobs = (ROOT / "resources" / "jobs.yml").read_text()
        self.assertIn("v3_thermal_refinement_validation_job:", jobs)
        self.assertIn("campaign_name: racing-v3-thermal-refinement-validation-v2", jobs)
        self.assertIn("pitgun.promotion: human-review-required", jobs)


if __name__ == "__main__":
    unittest.main()
