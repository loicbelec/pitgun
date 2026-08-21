import hashlib
import importlib.util
import json
import pathlib
import tempfile
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[3]
BUILDER_PATH = (
    ROOT / "experiments" / "racing_v3_thermal_refinement" / "build_candidate.py"
)
SPEC = importlib.util.spec_from_file_location("thermal_candidate_builder", BUILDER_PATH)
BUILDER = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(BUILDER)
CANDIDATE = (
    ROOT
    / "experiments"
    / "racing_v3_thermal_refinement"
    / "candidates"
    / "thermal-family-profile-v1.json"
)
CHECKSUM = CANDIDATE.with_suffix(".sha256")


class ThermalFamilyProfileCandidateTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.payload = CANDIDATE.read_bytes()
        cls.candidate = json.loads(cls.payload)

    def test_generated_candidate_is_current(self):
        self.assertEqual(self.candidate, BUILDER.build_candidate())
        expected = hashlib.sha256(self.payload).hexdigest()
        self.assertEqual(
            CHECKSUM.read_text(),
            f"{expected}  {CANDIDATE.name}\n",
        )

    def test_candidate_is_family_specific_and_fail_closed(self):
        self.assertEqual(
            set(self.candidate["profiles"]),
            {"historical_v8", "modern_v6t", "f1_2026"},
        )
        contract = self.candidate["resolution_contract"]
        self.assertEqual(contract["selection_key"], "vehicle_id")
        self.assertEqual(contract["unknown_vehicle_behavior"], "reject")
        self.assertTrue(contract["era_only_selection_forbidden"])

    def test_reviewed_thermal_differences_are_preserved(self):
        profiles = self.candidate["profiles"]
        self.assertEqual(
            profiles["modern_v6t"]["engine_thermal_resolution"][
                "soft_limit_offset_c"
            ],
            -3.0,
        )
        self.assertEqual(
            profiles["f1_2026"]["engine_thermal_resolution"][
                "soft_limit_offset_c"
            ],
            0.9183673469387728,
        )
        self.assertEqual(
            profiles["historical_v8"]["engine_thermal_resolution"][
                "cooling_drag_area_m2_at_cap"
            ],
            0.0,
        )

    def test_experimental_fuel_is_explicitly_excluded(self):
        excluded = self.candidate["excluded_from_candidate"]
        self.assertEqual(excluded["experimental_fuel_reservoir_kg"], 130.0)
        self.assertEqual(excluded["owner_issue"], 246)

    def test_candidate_does_not_authorize_deployment(self):
        promotion = self.candidate["promotion"]
        self.assertTrue(promotion["candidate_creation_authorized"])
        self.assertTrue(promotion["rust_wasm_integration_authorized"])
        for boundary in (
            "catalog_publication_authorized",
            "authority_verifier_promotion_authorized",
            "game_staging_promotion_authorized",
            "production_promotion_authorized",
            "automatic_promotion",
        ):
            self.assertFalse(promotion[boundary])

    def test_non_passing_review_cannot_build_candidate(self):
        review = json.loads(BUILDER.REVIEW.read_text())
        review["per_family_verdicts"]["modern_v6t"]["verdict"] = "REFINE"
        with tempfile.TemporaryDirectory() as directory:
            path = pathlib.Path(directory) / "review.json"
            path.write_text(json.dumps(review))
            with self.assertRaisesRegex(
                BUILDER.CandidateBuildError,
                "only all-PASS evidence",
            ):
                BUILDER.build_candidate(review_path=path)


if __name__ == "__main__":
    unittest.main()
