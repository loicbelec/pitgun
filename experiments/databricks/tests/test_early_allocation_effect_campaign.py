import copy
import hashlib
import importlib.util
import json
import pathlib
import tempfile
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[1]
FRAMEWORK = ROOT.parents[1]
BUILDER_PATH = (
    FRAMEWORK / "experiments" / "early_allocation_effect" / "build_campaign.py"
)
SPEC = importlib.util.spec_from_file_location("early_allocation_builder", BUILDER_PATH)
BUILDER = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(BUILDER)

MANIFEST = ROOT / "campaigns" / "racing-early-allocation-effect-v1.json"
CHECKSUM = MANIFEST.with_suffix(".sha256")
SCENARIOS = FRAMEWORK / "experiments" / "early_allocation_effect" / "scenarios"


class EarlyAllocationEffectCampaignTest(unittest.TestCase):
    def load(self):
        manifest_bytes = MANIFEST.read_bytes()
        manifest = json.loads(manifest_bytes)
        resources = {path.stem: path.read_bytes() for path in SCENARIOS.glob("*.json")}
        return manifest, resources, manifest_bytes

    def test_manifest_is_checksummed_and_reproducible(self):
        manifest, resources, manifest_bytes = self.load()
        expected_digest, expected_name = CHECKSUM.read_text().split()

        self.assertEqual(expected_name, MANIFEST.name)
        self.assertEqual(hashlib.sha256(manifest_bytes).hexdigest(), expected_digest)
        self.assertEqual(manifest["schema_version"], BUILDER.SCHEMA_VERSION)
        self.assertEqual(manifest["campaign_id"], BUILDER.CAMPAIGN_ID)
        self.assertEqual(len(resources), 135)

        source, source_digest = BUILDER.load_source()
        with tempfile.TemporaryDirectory() as directory:
            rebuilt = BUILDER.build_manifest(source, source_digest, pathlib.Path(directory))
            self.assertEqual(rebuilt, manifest)
            rebuilt_resources = {
                path.stem: path.read_bytes()
                for path in pathlib.Path(directory).glob("*.json")
            }
        self.assertEqual(rebuilt_resources, resources)
        BUILDER.validate_manifest(manifest, resources, source_digest)

    def test_each_block_contains_exact_single_axis_margins(self):
        manifest, _, _ = self.load()
        blocks = {}
        for run in manifest["runs"]:
            blocks.setdefault(run["block_key"], {})[run["treatment"]] = run

        self.assertEqual(len(blocks), 15)
        self.assertEqual(len(BUILDER.TREATMENTS), 9)
        for treatments in blocks.values():
            self.assertEqual(set(treatments), set(BUILDER.TREATMENTS))
            reference = treatments["reference"]["player_allocation"]
            self.assertEqual(reference, BUILDER.REFERENCE_ALLOCATION)
            for treatment, run in treatments.items():
                if treatment == "reference":
                    continue
                deltas = {
                    key: run["player_allocation"][key] - reference[key]
                    for key in BUILDER.POINT_KEYS
                }
                changed = {key: delta for key, delta in deltas.items() if delta}
                expected_delta = 1 if run["direction"] == "add" else -1
                self.assertEqual(changed, {f"{run['axis']}_points": expected_delta})

    def test_source_lineage_and_scope_exclusions_are_frozen(self):
        manifest, _, _ = self.load()
        self.assertEqual(
            manifest["source"]["manifest_digest"], BUILDER.SOURCE_MANIFEST_DIGEST
        )
        self.assertEqual(
            manifest["source"]["execution_evidence"], BUILDER.SOURCE_EXECUTION
        )
        self.assertTrue(all(value is False for value in manifest["governance"].values()))
        self.assertEqual(manifest["matrix"]["treatment_count"], 9)
        self.assertEqual(len(manifest["matrix"]["circuits"]), 5)
        self.assertEqual(len(manifest["matrix"]["seeds"]), 3)

    def test_changed_treatment_or_resource_is_rejected(self):
        manifest, resources, _ = self.load()
        _, source_digest = BUILDER.load_source()

        changed_manifest = copy.deepcopy(manifest)
        changed_manifest["runs"][0]["player_budget"] += 1
        with self.assertRaises(BUILDER.EarlyAllocationBuildError):
            BUILDER.validate_manifest(changed_manifest, resources, source_digest)

        changed_resources = dict(resources)
        first = manifest["runs"][0]["scenario_resource"]
        changed_resources[first] += b" "
        with self.assertRaises(BUILDER.EarlyAllocationBuildError):
            BUILDER.validate_manifest(manifest, changed_resources, source_digest)

    def test_resources_contain_no_private_game_data(self):
        _, resources, _ = self.load()
        serialized = b"".join(resources.values()).decode()
        for forbidden in ("careerId", "playerName", "leaderboard", "telemetry"):
            self.assertNotIn(forbidden, serialized)


if __name__ == "__main__":
    unittest.main()
