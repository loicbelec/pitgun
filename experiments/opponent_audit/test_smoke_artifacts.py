from __future__ import annotations

import hashlib
import json
import pathlib
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[2]
RESULT = (
    ROOT / "experiments" / "opponent_audit" / "results" / "racing-opponent-smoke-v1.json"
)
SCENARIOS = ROOT / "experiments" / "opponent_audit" / "scenarios"


def canonical_pretty(value: object) -> bytes:
    return (json.dumps(value, indent=2, ensure_ascii=False) + "\n").encode()


class OpponentSmokeArtifactTest(unittest.TestCase):
    def test_result_is_content_addressed_and_complete(self) -> None:
        result = json.loads(RESULT.read_text())
        claimed = result.pop("artifact_digest")
        calculated = "sha256:" + hashlib.sha256(canonical_pretty(result)).hexdigest()

        self.assertEqual(claimed, calculated)
        self.assertEqual(result["schema_version"], "pitgun.racing-opponent-smoke/v1")
        self.assertEqual(result["summary"]["completed_runs"], 15)
        self.assertTrue(result["summary"]["all_retries_byte_identical"])
        self.assertEqual(len(result["runs"]), 15)
        self.assertEqual(len({run["run_id"] for run in result["runs"]}), 15)

    def test_resolved_scenarios_pin_v2_and_contain_one_controlled_player(self) -> None:
        paths = sorted(SCENARIOS.glob("*.json"))
        self.assertEqual(len(paths), 15)
        for path in paths:
            scenario = json.loads(path.read_text())
            self.assertEqual(scenario["model"]["version"], "2.0.0")
            self.assertEqual(scenario["data_pack"]["version"], "1.2.0")
            competitors = scenario["request"]["competitors"]
            self.assertEqual(len(competitors), 10)
            self.assertEqual(
                [competitor["id"] for competitor in competitors if competitor["is_player"]],
                ["player"],
            )

    def test_artifacts_contain_no_observed_player_identity(self) -> None:
        forbidden = {"careerId", "playerName", "leaderboard", "telemetry"}
        keys: set[str] = set()

        def visit(value: object) -> None:
            if isinstance(value, dict):
                for key, nested in value.items():
                    keys.add(key)
                    visit(nested)
            elif isinstance(value, list):
                for nested in value:
                    visit(nested)

        visit(json.loads(RESULT.read_text()))
        for path in SCENARIOS.glob("*.json"):
            visit(json.loads(path.read_text()))
        self.assertTrue(forbidden.isdisjoint(keys))


if __name__ == "__main__":
    unittest.main()
