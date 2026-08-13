import unittest

from pitgun_databricks_adapter.opponent_audit_analysis import (
    OpponentAuditResultError,
    diagnose_opponent_audit,
    extract_opponent_audit_evidence,
    summarize_opponent_audit,
)


MANIFEST = {
    "catalog": {
        "model_id": "pitgun.racing",
        "model_version": "2.0.0",
        "model_digest": "sha256:model",
        "version": "1.2.0",
        "simulation_pack_digest": "sha256:pack",
    }
}
ENTRY = {
    "run_key": "monza-early-42-balanced-one-stop--neutral",
    "seed": 42,
    "circuit_id": "MONZA",
    "progression": "early",
    "strategy_profile": "balanced-one-stop",
    "player_reference": "neutral",
    "distinct_opponent_tunings": 9,
    "distinct_opponent_strategies": 8,
    "opponent_budget_min": 40,
    "opponent_budget_max": 45,
}


def result(position=2, gap=1200):
    standings = [
        {
            "competitor_id": f"ai_{index}",
            "position": index,
            "gap_to_leader_ms": (index - 1) * 1000,
            "best_lap_ms": 100_000 + index,
            "total_time_ms": 1_000_000 + (index - 1) * 1000,
        }
        for index in range(1, 10)
    ]
    standings.insert(
        position - 1,
        {
            "competitor_id": "player",
            "position": position,
            "gap_to_leader_ms": gap,
            "best_lap_ms": 99_900,
            "total_time_ms": 1_000_000 + gap,
        },
    )
    return {
        "configuration_id": "sha256:configuration",
        "run_id": "sha256:run",
        "seed": "42",
        "scenario": {
            "id": "racing.opponent-audit-campaign",
            "version": "1.0.0",
        },
        "model": {
            "id": "pitgun.racing",
            "version": "2.0.0",
            "digest": "sha256:model",
        },
        "data_pack": {
            "id": "pitgun.racing.simulation",
            "version": "1.2.0",
            "digest": "sha256:pack",
        },
        "summary": {"standings": standings},
    }


class OpponentAuditAnalysisTest(unittest.TestCase):
    def test_extracts_competitiveness_and_diversity_metrics(self):
        evidence = extract_opponent_audit_evidence(ENTRY, result(), MANIFEST)

        self.assertEqual(evidence["player_position"], 2)
        self.assertEqual(evidence["player_gap_to_leader_ms"], 1200)
        self.assertEqual(
            evidence["metrics"]["racing.opponent-audit.player-podium"],
            (1.0, "boolean"),
        )
        self.assertEqual(
            evidence["metrics"]["racing.opponent-audit.distinct-opponent-tunings"],
            (9.0, "count"),
        )

    def test_rejects_identity_mismatch(self):
        changed = result()
        changed["model"]["version"] = "1.0.0"

        with self.assertRaises(OpponentAuditResultError):
            extract_opponent_audit_evidence(ENTRY, changed, MANIFEST)

    def test_summarizes_references_without_policy_promotion(self):
        neutral = extract_opponent_audit_evidence(ENTRY, result(), MANIFEST)
        informed_entry = {**ENTRY, "player_reference": "circuit-informed"}
        informed = extract_opponent_audit_evidence(
            informed_entry, result(position=1, gap=0), MANIFEST
        )

        report = summarize_opponent_audit([neutral, informed])

        self.assertEqual(report["overall"]["neutral"]["win_rate"], 0.0)
        self.assertEqual(report["overall"]["circuit-informed"]["win_rate"], 1.0)
        self.assertFalse(report["automatic_game_or_catalog_promotion"])

    def test_diagnosis_separates_exact_pairs_from_confounded_axes(self):
        runs = []
        evidence = []
        for seed in (1, 2, 3):
            for strategy in ("balanced-one-stop", "late-one-stop"):
                source_id = f"monza-early-{seed}-{strategy}"
                for reference, position, gap in (
                    ("neutral", 2, 1200),
                    ("circuit-informed", 1, 0),
                ):
                    run_key = f"{source_id}--{reference}"
                    runs.append(
                        {
                            "run_key": run_key,
                            "source_scenario_id": source_id,
                            "source_contract_digest": "sha256:" + strategy[0] * 64,
                            "seed": seed,
                            "circuit_id": "MONZA",
                            "progression": "early",
                            "strategy_profile": strategy,
                            "player_reference": reference,
                            "distinct_opponent_tunings": 9,
                            "distinct_opponent_strategies": 7 + seed % 2,
                            "opponent_budget_min": 40,
                            "opponent_budget_max": 45,
                        }
                    )
                    evidence.append(
                        {
                            "run_key": run_key,
                            "circuit_id": "MONZA",
                            "progression": "early",
                            "strategy_profile": strategy,
                            "player_reference": reference,
                            "player_position": position,
                            "player_gap_to_leader_ms": gap,
                            "field_spread_ms": 5000 + seed,
                        }
                    )
        manifest = {
            "campaign_id": "audit",
            "matrix": {
                "seeds": [1, 2, 3],
                "progression": [{"id": "early", "playerBudget": 40}],
            },
            "runs": runs,
        }

        report = diagnose_opponent_audit(
            manifest,
            evidence,
            {"runs": {"version": 4}, "metrics": {"version": 3}},
        )

        self.assertEqual(report["sample"]["setup_pair_count"], 6)
        self.assertEqual(
            report["setup_alignment"]["by_circuit"]["MONZA"][
                "mean_position_delta_informed_minus_neutral"
            ],
            -1.0,
        )
        self.assertEqual(
            report["setup_alignment"]["seed_direction_stability"][
                "stable_group_rate"
            ],
            1.0,
        )
        self.assertEqual(
            report["strategy_response"]["same_source_contract_pair_count"], 0
        )
        self.assertFalse(report["strategy_response"]["causal_interpretation_allowed"])
        self.assertFalse(report["policy_selected"])

    def test_diagnosis_rejects_an_incomplete_snapshot(self):
        with self.assertRaises(OpponentAuditResultError):
            diagnose_opponent_audit(
                {"campaign_id": "audit", "runs": [{"run_key": "expected"}]},
                [],
                {},
            )


if __name__ == "__main__":
    unittest.main()
