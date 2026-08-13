import copy
import hashlib
import json
import pathlib
import sys
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[1]
FRAMEWORK = ROOT.parents[1]
sys.path.insert(0, str(ROOT / "adapter"))

from pitgun_databricks_adapter.strategy_effect import (  # noqa: E402
    StrategyEffectManifestError,
    _validate_manifest,
    extract_strategy_effect_evidence,
    summarize_strategy_effect,
)


MANIFEST = ROOT / "campaigns" / "racing-strategy-effect-v1.json"
CHECKSUM = MANIFEST.with_suffix(".sha256")
SOURCE = ROOT / "campaigns" / "racing-opponent-audit-v1.json"
SCENARIOS = FRAMEWORK / "experiments" / "strategy_effect" / "scenarios"


class StrategyEffectCampaignTest(unittest.TestCase):
    def load(self):
        manifest_bytes = MANIFEST.read_bytes()
        manifest = json.loads(manifest_bytes)
        resources = {path.stem: path.read_bytes() for path in SCENARIOS.glob("*.json")}
        source_digest = "sha256:" + hashlib.sha256(SOURCE.read_bytes()).hexdigest()
        return manifest, resources, source_digest, manifest_bytes

    def test_manifest_is_checksumming_45_exact_pairs(self):
        manifest, resources, source_digest, manifest_bytes = self.load()
        expected_digest, expected_name = CHECKSUM.read_text().split()

        self.assertEqual(expected_name, MANIFEST.name)
        self.assertEqual(hashlib.sha256(manifest_bytes).hexdigest(), expected_digest)
        self.assertEqual(len(resources), 90)
        _validate_manifest(manifest, resources, source_digest)

    def test_each_pair_changes_only_player_strategy(self):
        manifest, resources, _, _ = self.load()
        pairs = {}
        for run in manifest["runs"]:
            scenario = json.loads(resources[run["scenario_resource"]])
            player = next(
                row for row in scenario["request"]["competitors"] if row["is_player"]
            )
            strategy = copy.deepcopy(player.pop("stint_strategy"))
            pairs.setdefault(run["pair_key"], []).append(
                (run["strategy_profile"], scenario, strategy)
            )

        self.assertEqual(len(pairs), 45)
        for pair in pairs.values():
            self.assertEqual(pair[0][1], pair[1][1])
            self.assertNotEqual(pair[0][2], pair[1][2])

    def test_changed_resource_fails_closed(self):
        manifest, resources, source_digest, _ = self.load()
        resource = manifest["runs"][0]["scenario_resource"]
        resources[resource] += b" "

        with self.assertRaises(StrategyEffectManifestError):
            _validate_manifest(manifest, resources, source_digest)

    def test_inputs_contain_no_observed_player_data(self):
        _, resources, _, _ = self.load()
        serialized = b"".join(resources.values()).decode()
        for forbidden in ("careerId", "playerName", "leaderboard", "telemetry"):
            self.assertNotIn(forbidden, serialized)

    def test_extract_and_summarize_exact_paired_effect(self):
        manifest, resources, _, _ = self.load()
        selected_runs = manifest["runs"][:2]
        self.assertEqual(selected_runs[0]["pair_key"], selected_runs[1]["pair_key"])
        evidence = []
        for index, run in enumerate(selected_runs):
            standings = [
                {
                    "competitor_id": f"ai_{position}",
                    "position": position,
                    "gap_to_leader_ms": (position - 1) * 1000,
                    "best_lap_ms": 100_000 + position,
                    "total_time_ms": 1_000_000 + (position - 1) * 1000,
                }
                for position in range(1, 10)
            ]
            standings.insert(
                index,
                {
                    "competitor_id": "player",
                    "position": index + 1,
                    "gap_to_leader_ms": index * 500,
                    "best_lap_ms": 99_900 + index * 100,
                    "total_time_ms": 1_000_000 + index * 500,
                },
            )
            result = {
                "configuration_id": "sha256:" + str(index) * 64,
                "run_id": "sha256:" + str(index + 2) * 64,
                "seed": str(run["seed"]),
                "scenario": {
                    "id": "racing.strategy-effect-campaign",
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
            evidence.append(extract_strategy_effect_evidence(run, result, manifest))

        small_manifest = {
            **manifest,
            "runs": selected_runs,
            "planned_pair_count": 1,
            "planned_run_count": 2,
            "matrix": {**manifest["matrix"], "seeds": [selected_runs[0]["seed"]]},
        }
        report = summarize_strategy_effect(small_manifest, evidence, {"test": True})

        self.assertEqual(report["overall"]["pair_count"], 1)
        self.assertEqual(
            report["overall"]["median_total_time_delta_late_minus_balanced_ms"],
            500,
        )
        self.assertTrue(report["causal_interpretation_allowed"])
        self.assertFalse(report["policy_selected"])

    def test_full_manifest_reconciles_all_seed_stability_groups(self):
        manifest, _, _, _ = self.load()
        evidence = []
        for run in manifest["runs"]:
            late = run["strategy_profile"] == "late-one-stop"
            evidence.append(
                {
                    "run_key": run["run_key"],
                    "pair_key": run["pair_key"],
                    "circuit_id": run["circuit_id"],
                    "progression": run["progression"],
                    "seed": run["seed"],
                    "strategy_profile": run["strategy_profile"],
                    "player_position": 3 if late else 4,
                    "player_gap_to_leader_ms": 900 if late else 1000,
                    "player_best_lap_ms": 99_900 if late else 100_000,
                    "player_total_time_ms": 999_000 if late else 1_000_000,
                    "field_spread_ms": 5000,
                }
            )

        report = summarize_strategy_effect(manifest, evidence, {"test": True})

        self.assertEqual(report["sample"], {"successful_run_count": 90, "pair_count": 45})
        self.assertEqual(report["seed_direction_stability"]["group_count"], 15)
        self.assertEqual(report["seed_direction_stability"]["stable_group_count"], 15)
        self.assertEqual(
            report["overall"]["median_total_time_delta_late_minus_balanced_ms"],
            -1000,
        )


if __name__ == "__main__":
    unittest.main()
