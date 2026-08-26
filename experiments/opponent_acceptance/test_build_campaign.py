from __future__ import annotations

import importlib.util
import json
import pathlib
import unittest


MODULE_PATH = pathlib.Path(__file__).with_name("build_campaign.py")
SPEC = importlib.util.spec_from_file_location("opponent_acceptance_build", MODULE_PATH)
assert SPEC and SPEC.loader
build = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(build)


class OpponentAcceptanceBuildTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.source, cls.source_digest = build.load_source(build.DEFAULT_SOURCE)
        cls.manifest, cls.resources = build.build_manifest(
            cls.source, cls.source_digest
        )

    def test_complete_paired_matrix(self) -> None:
        build.validate_manifest(self.manifest, self.resources)
        self.assertEqual(self.manifest["planned_run_count"], 135)
        self.assertEqual(len(self.resources), 135)
        groups: dict[str, list[dict]] = {}
        for run in self.manifest["runs"]:
            groups.setdefault(run["source_field_id"], []).append(run)
        self.assertEqual(len(groups), 45)
        for rows in groups.values():
            self.assertEqual(
                {row["player_reference"] for row in rows},
                {"naive", "balanced", "circuit-informed"},
            )
            self.assertEqual(
                len({row["source_opponent_contract_digest"] for row in rows}), 1
            )
            self.assertEqual(
                len({row["source_opponent_component_digest"] for row in rows}), 1
            )

    def test_rust_wire_removes_only_browser_metadata(self) -> None:
        selected = self.source["fields"][0]["scenarios"][0]
        original = selected["request"]
        resolved = build.rust_wire_request(original)
        self.assertNotEqual(resolved, original)
        for competitor in resolved["competitors"]:
            self.assertNotIn("style", competitor)
            self.assertNotIn("points", competitor)
        restored = json.loads(json.dumps(resolved))
        for index, competitor in enumerate(restored["competitors"]):
            source = original["competitors"][index]
            if "style" in source:
                competitor["style"] = source["style"]
            if "points" in source:
                competitor["points"] = source["points"]
        self.assertEqual(restored, original)

    def test_source_contains_no_private_game_state(self) -> None:
        build.reject_forbidden_keys(self.source)

    def test_source_uses_the_governed_fuel_contract_without_an_override(self) -> None:
        self.assertEqual(self.source["catalog"], build.EXPECTED_CATALOG)
        for field in self.source["fields"]:
            for scenario in field["scenarios"]:
                self.assertNotIn("initial_fuel_mass_kg", scenario["request"])

    def test_manifest_cannot_promote_or_mutate_policy(self) -> None:
        self.assertTrue(
            all(value is False for value in self.manifest["governance"].values())
        )


if __name__ == "__main__":
    unittest.main()
