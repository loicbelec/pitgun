import hashlib
import json
import pathlib
import sys
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[1]
FRAMEWORK = ROOT.parents[1]
sys.path.insert(0, str(ROOT / "adapter"))

from pitgun_databricks_adapter.opponent_acceptance import (  # noqa: E402
    EXPECTED_CATALOG,
    OpponentAcceptanceError,
    _validate_manifest,
    extract_opponent_acceptance_evidence,
    summarize_opponent_acceptance,
)


MANIFEST = ROOT / "campaigns" / "racing-opponent-acceptance-v1.json"
CHECKSUM = ROOT / "campaigns" / "racing-opponent-acceptance-v1.sha256"
SCENARIOS = FRAMEWORK / "experiments" / "opponent_acceptance" / "scenarios"


class OpponentAcceptanceCampaignTest(unittest.TestCase):
    def load(self):
        manifest_bytes = MANIFEST.read_bytes()
        manifest = json.loads(manifest_bytes)
        resources = {
            path.stem: "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()
            for path in SCENARIOS.glob("*.json")
        }
        return manifest, resources, manifest_bytes

    def test_manifest_is_exact_complete_and_resource_backed(self):
        manifest, resources, manifest_bytes = self.load()
        expected_digest, expected_name = CHECKSUM.read_text().split()

        self.assertEqual(expected_name, MANIFEST.name)
        self.assertEqual(hashlib.sha256(manifest_bytes).hexdigest(), expected_digest)
        self.assertEqual(len(resources), 135)
        self.assertEqual(manifest["catalog"], EXPECTED_CATALOG)
        _validate_manifest(manifest, resources)

    def test_changed_resource_or_catalog_fails_closed(self):
        manifest, resources, _ = self.load()
        changed_resources = dict(resources)
        changed_resources.pop(next(iter(changed_resources)))
        with self.assertRaises(OpponentAcceptanceError):
            _validate_manifest(manifest, changed_resources)

        changed_manifest = json.loads(json.dumps(manifest))
        changed_manifest["catalog"]["model_version"] = "unreviewed"
        with self.assertRaises(OpponentAcceptanceError):
            _validate_manifest(changed_manifest, resources)

    def test_native_result_is_normalized_with_budget_parity(self):
        manifest, _, _ = self.load()
        entry = manifest["runs"][0]
        standings = [
            {
                "competitor_id": "player" if position == 3 else f"ai_{position}",
                "position": position,
                "gap_to_leader_ms": 0 if position == 1 else position * 1000,
                "total_time_ms": 100000 + position * 1000,
                "best_lap_ms": 90000 + position,
            }
            for position in range(1, 11)
        ]
        result = {
            "model": {
                "id": manifest["catalog"]["model_id"],
                "version": manifest["catalog"]["model_version"],
                "digest": manifest["catalog"]["model_digest"],
            },
            "data_pack": {
                "id": "pitgun.racing.simulation",
                "version": manifest["catalog"]["version"],
                "digest": manifest["catalog"]["simulation_pack_digest"],
            },
            "seed": str(entry["seed"]),
            "scenario": {
                "id": "racing.opponent-acceptance-matrix",
                "version": "1.0.0",
            },
            "configuration_id": "sha256:configuration",
            "run_id": "sha256:run",
            "summary": {"standings": standings},
        }
        evidence = extract_opponent_acceptance_evidence(entry, result, manifest)
        self.assertEqual(evidence["player_position"], 3)
        self.assertIn(
            "racing.opponent-acceptance.player-budget-delta-to-opponent-median",
            evidence["metrics"],
        )

    def test_summary_requires_complete_evidence_and_human_review(self):
        manifest, _, _ = self.load()
        evidence = []
        for entry in manifest["runs"]:
            reference = entry["player_reference"]
            position = {"naive": 5, "balanced": 4, "circuit-informed": 3}[
                reference
            ]
            evidence.append(
                {
                    "run_key": entry["run_key"],
                    "source_field_id": entry["source_field_id"],
                    "circuit_id": entry["circuit_id"],
                    "circuit_class": entry["circuit_class"],
                    "progression": entry["progression"],
                    "player_reference": reference,
                    "player_position": position,
                    "player_gap_to_leader_ms": position * 1000,
                    "field_spread_ms": 20000,
                    "player_budget": entry["player_budget"],
                    "opponent_budget_median": entry["opponent_budget_median"],
                }
            )
        report = summarize_opponent_acceptance(
            evidence,
            {"sentinel_count": 3, "all_identical": True},
        )
        self.assertEqual(report["decision"], "REVIEW_REQUIRED")
        self.assertTrue(all(report["observed_gates"].values()))
        self.assertFalse(report["automatic_game_or_catalog_promotion"])
        self.assertFalse(report["automatic_policy_mutation"])
        self.assertEqual(len(report["paired_effects"]), 45)


if __name__ == "__main__":
    unittest.main()
