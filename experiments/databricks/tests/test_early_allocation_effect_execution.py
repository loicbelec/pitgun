import copy
import hashlib
import json
import pathlib
import sys
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[1]
FRAMEWORK = ROOT.parents[1]
sys.path.insert(0, str(ROOT / "adapter"))

from pitgun_databricks_adapter.early_allocation_effect import (  # noqa: E402
    EarlyAllocationEffectManifestError,
    _validate_manifest,
    extract_early_allocation_effect_evidence,
    summarize_early_allocation_effect,
)


MANIFEST = ROOT / "campaigns" / "racing-early-allocation-effect-v1.json"
SCENARIOS = FRAMEWORK / "experiments" / "early_allocation_effect" / "scenarios"
SOURCE = ROOT / "campaigns" / "racing-budget-effect-v2.json"


class EarlyAllocationEffectExecutionTest(unittest.TestCase):
    def load(self):
        manifest = json.loads(MANIFEST.read_text())
        resources = {path.stem: path.read_bytes() for path in SCENARIOS.glob("*.json")}
        source_digest = "sha256:" + hashlib.sha256(SOURCE.read_bytes()).hexdigest()
        return manifest, resources, source_digest

    def result(self, manifest, run, total_time_ms):
        standings = [
            {
                "competitor_id": f"ai_{position}",
                "position": position,
                "gap_to_leader_ms": (position - 1) * 1000,
                "best_lap_ms": 100_000 + position,
                "total_time_ms": 1_000_000 + position * 1000,
            }
            for position in range(1, 10)
        ]
        standings.insert(
            4,
            {
                "competitor_id": "player",
                "position": 5,
                "gap_to_leader_ms": 4000,
                "best_lap_ms": total_time_ms // 10,
                "total_time_ms": total_time_ms,
            },
        )
        digest = hashlib.sha256(run["run_key"].encode()).hexdigest()
        return {
            "configuration_id": "sha256:" + digest,
            "run_id": "sha256:" + digest,
            "seed": str(run["seed"]),
            "scenario": {
                "id": "racing.early-allocation-effect-campaign",
                "version": "1.0.0",
            },
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
            "summary": {"standings": standings},
        }

    def test_full_campaign_is_accepted_and_changed_input_fails_closed(self):
        manifest, resources, source_digest = self.load()
        _validate_manifest(manifest, resources, source_digest)

        changed = dict(resources)
        first = manifest["runs"][0]["scenario_resource"]
        changed[first] += b" "
        with self.assertRaises(EarlyAllocationEffectManifestError):
            _validate_manifest(manifest, changed, source_digest)

        changed_manifest = copy.deepcopy(manifest)
        changed_manifest["governance"]["automatic_game_or_catalog_promotion"] = True
        with self.assertRaises(EarlyAllocationEffectManifestError):
            _validate_manifest(changed_manifest, resources, source_digest)

    def test_all_axes_are_summarized_without_selecting_a_profile(self):
        manifest, _, _ = self.load()
        benefit = {"aero": 300, "chassis": 200, "cooling": 100, "engine": 400}
        evidence = []
        for run in manifest["runs"]:
            baseline = 1_000_000 + int(run["seed"]) % 100
            if run["direction"] == "add":
                total_time = baseline - benefit[run["axis"]]
            elif run["direction"] == "remove":
                total_time = baseline + benefit[run["axis"]]
            else:
                total_time = baseline
            evidence.append(
                extract_early_allocation_effect_evidence(
                    run, self.result(manifest, run, total_time), manifest
                )
            )

        report = summarize_early_allocation_effect(
            manifest, evidence, {"mlflow_run_id": "test"}
        )
        self.assertEqual(
            report["schema_version"], "pitgun.early-allocation-effect-report/v1"
        )
        self.assertEqual(
            report["sample"],
            {
                "successful_run_count": 135,
                "block_count": 15,
                "axis_comparison_count": 60,
            },
        )
        self.assertEqual(
            report["evidence_ranking_by_median_marginal_benefit"],
            ["engine", "aero", "chassis", "cooling"],
        )
        self.assertEqual(report["seed_direction_stability"]["stable_group_rate"], 1.0)
        self.assertFalse(report["allocation_profile_selected"])
        self.assertFalse(report["automatic_game_or_catalog_promotion"])


if __name__ == "__main__":
    unittest.main()
