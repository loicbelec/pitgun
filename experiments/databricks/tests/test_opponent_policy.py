import copy
import pathlib
import sys
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / "adapter"))

from pitgun_databricks_adapter.opponent_policy import (
    OpponentPolicySelectionError,
    canonical_json,
    digest_json,
    select_reference_policy,
)


SUMMARIES = [
    {
        "configuration_family": "balanced",
        "configuration_id": "sha256:" + "1" * 64,
        "setup": {"downforce_slider": 0.5, "gear_ratio_slider": 0.5},
        "strategy": {"id": "single-lap-no-stop"},
        "successful_seed_count": 3,
        "mean_lap_time_ms": 85281.33333333333,
        "lap_time_stddev_ms": 30.554141381415967,
        "mean_maximum_speed_kmh": 355.5712096373124,
    },
    {
        "configuration_family": "high-downforce",
        "configuration_id": "sha256:" + "2" * 64,
        "setup": {"downforce_slider": 0.8, "gear_ratio_slider": 0.7},
        "strategy": {"id": "single-lap-no-stop"},
        "successful_seed_count": 3,
        "mean_lap_time_ms": 83869.66666666667,
        "lap_time_stddev_ms": 30.706495874470743,
        "mean_maximum_speed_kmh": 351.91935275940386,
    },
    {
        "configuration_family": "low-downforce",
        "configuration_id": "sha256:" + "3" * 64,
        "setup": {"downforce_slider": 0.2, "gear_ratio_slider": 0.3},
        "strategy": {"id": "single-lap-no-stop"},
        "successful_seed_count": 3,
        "mean_lap_time_ms": 86807.0,
        "lap_time_stddev_ms": 31.016124838541646,
        "mean_maximum_speed_kmh": 359.04302172499933,
    },
]

LINEAGE = {
    "campaign_id": "racing-reference-it-1922-2026-v1",
    "campaign_manifest_digest": "sha256:" + "a" * 64,
    "runs_table_name": "workspace.pitgun_calibration.runs",
    "runs_table_version": 2,
    "metrics_table_name": "workspace.pitgun_calibration.metrics",
    "metrics_table_version": 1,
    "mlflow_run_id": "mlflow-run",
    "source_git_revision": "revision",
    "model_digest": "sha256:" + "b" * 64,
    "data_pack_digest": "sha256:" + "c" * 64,
    "circuit_model_id": "it-1922",
    "era": 2026,
}


class OpponentPolicyTest(unittest.TestCase):
    def test_selection_is_order_independent_and_role_diverse(self):
        first = select_reference_policy(summaries=SUMMARIES, lineage=LINEAGE)
        second = select_reference_policy(
            summaries=list(reversed(SUMMARIES)), lineage=LINEAGE
        )

        self.assertEqual(canonical_json(first), canonical_json(second))
        self.assertEqual(digest_json(first), digest_json(second))
        self.assertEqual(
            [profile["role"] for profile in first["profiles"]],
            ["front-runner", "midfield", "challenger"],
        )
        self.assertEqual(
            [profile["id"] for profile in first["profiles"]],
            ["high-downforce", "balanced", "low-downforce"],
        )
        self.assertEqual(first["calibration"]["source"]["delta_tables"]["runs"]["version"], 2)
        self.assertFalse(
            first["calibration"]["selection"]["constraints"]["hidden_player_data_allowed"]
        )

    def test_fastest_profile_cannot_fill_the_field(self):
        policy = select_reference_policy(summaries=SUMMARIES, lineage=LINEAGE)
        roles = policy["composition"]["roles"]
        self.assertEqual(len({role["eligible_profile_ids"][0] for role in roles}), 3)
        self.assertEqual(sum(role["count"] for role in roles), 3)

    def test_invalid_or_under_evidenced_candidates_fail_closed(self):
        invalid = copy.deepcopy(SUMMARIES)
        invalid[0]["successful_seed_count"] = 2
        with self.assertRaises(OpponentPolicySelectionError):
            select_reference_policy(summaries=invalid, lineage=LINEAGE)

        duplicated = copy.deepcopy(SUMMARIES)
        duplicated[0]["setup"] = duplicated[1]["setup"]
        with self.assertRaises(OpponentPolicySelectionError):
            select_reference_policy(summaries=duplicated, lineage=LINEAGE)


if __name__ == "__main__":
    unittest.main()
