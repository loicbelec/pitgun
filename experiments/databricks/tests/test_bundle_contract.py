import ast
import hashlib
import json
import pathlib
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[1]


class BundleContractTest(unittest.TestCase):
    def test_python_sources_are_syntactically_valid(self):
        for path in ROOT.rglob("*.py"):
            if path != pathlib.Path(__file__):
                ast.parse(path.read_text(), filename=str(path))

    def test_bootstrap_owns_only_the_seven_governed_tables(self):
        source = (ROOT / "src" / "bootstrap_tables.py").read_text()
        expected = {
            "campaigns",
            "runs",
            "metrics",
            "experimental_runs",
            "experimental_metrics",
            "candidates",
            "policy_releases",
        }
        for logical_name in expected:
            self.assertIn(f'"{logical_name}": f"""', source)
        self.assertEqual(source.count("CREATE TABLE IF NOT EXISTS"), len(expected))
        self.assertIn('{"default", "information_schema"}', source)

    def test_bundle_configuration_contains_no_bound_identity_or_secret(self):
        configuration = "\n".join(
            path.read_text()
            for path in sorted(ROOT.rglob("*"))
            if path.is_file()
            and path.suffix in {".yml", ".py"}
            and path != pathlib.Path(__file__)
        )
        for forbidden in (
            "dbc-",
            "databricks@pitgun.com",
            "DATABRICKS_TOKEN",
            "host:",
        ):
            self.assertNotIn(forbidden, configuration)

    def test_runner_job_exposes_no_arbitrary_code_boundary(self):
        job = (ROOT / "resources" / "jobs.yml").read_text()
        runner = (
            ROOT / "adapter" / "pitgun_databricks_adapter" / "runner.py"
        ).read_text()
        self.assertIn("runner_spike_job:", job)
        self.assertIn("- name: seed", job)
        for forbidden_parameter in ("runner_url", "runner_path", "command", "scenario"):
            self.assertNotIn(f"- name: {forbidden_parameter}", job)
        self.assertIn("- name: configuration_family", job)
        self.assertIn("SCENARIO_FAMILIES = frozenset(", runner)
        self.assertNotIn("shell=True", runner)
        self.assertNotIn("http://", runner)
        self.assertNotIn("https://", runner)

    def test_budget_job_is_bounded_and_non_promoting(self):
        job = (ROOT / "resources" / "jobs.yml").read_text()
        notebook = (ROOT / "src" / "execute_budget_effect.py").read_text()

        self.assertIn("budget_effect_job:", job)
        self.assertIn("budget_effect_v2_job:", job)
        self.assertIn("execute_budget_effect.py", job)
        self.assertIn("campaign_name: racing-budget-effect-v1", job)
        self.assertIn("campaign_name: racing-budget-effect-v2", job)
        self.assertIn("campaign_id: racing-budget-effect-2026-v2", job)
        self.assertIn("pitgun.promotion: forbidden", job)
        self.assertIn('campaign_name == "racing-budget-effect-v1"', notebook)
        self.assertIn('campaign_name == "racing-budget-effect-v2"', notebook)
        self.assertIn("unsupported budget campaign", notebook)
        self.assertIn("load_budget_effect_v2_campaign", notebook)
        self.assertIn("summarize_budget_effect_v2", notebook)
        self.assertIn('"budget_target_selected": False', notebook)
        self.assertIn('"automatic_game_or_catalog_promotion": False', notebook)
        self.assertNotIn("candidate", notebook.lower())
        self.assertNotIn("policy_release", notebook.lower())

    def test_early_allocation_job_is_bounded_and_non_promoting(self):
        job = (ROOT / "resources" / "jobs.yml").read_text()
        notebook = (ROOT / "src" / "execute_budget_effect.py").read_text()
        builder = (ROOT / "adapter" / "build_wheel.py").read_text()

        self.assertIn("early_allocation_effect_job:", job)
        self.assertIn("campaign_id: racing-early-allocation-effect-2026-v1", job)
        self.assertIn("campaign_name: racing-early-allocation-effect-v1", job)
        self.assertIn("pitgun.promotion: forbidden", job)
        self.assertIn('campaign_name == "racing-early-allocation-effect-v1"', notebook)
        self.assertIn("load_early_allocation_effect_campaign", notebook)
        self.assertIn('"allocation_profile_selected": False', notebook)
        self.assertIn('"early_allocation_effect" / "scenarios"', builder)
        self.assertIn('f"{PACKAGE}/early_allocation_effect.py"', builder)

    def test_racing_v2_catalog_boundary_is_packaged_and_bounded(self):
        builder = (ROOT / "adapter" / "build_wheel.py").read_text()
        runner = (
            ROOT / "adapter" / "pitgun_databricks_adapter" / "runner.py"
        ).read_text()
        framework = ROOT.parents[1]
        scenarios = sorted(
            (framework / "experiments" / "opponent_audit" / "scenarios").glob("*.json")
        )
        catalog_files = sorted(
            path
            for path in (framework / "catalogs" / "racing" / "v1.2.0").rglob("*")
            if path.is_file()
        )

        self.assertEqual(len(scenarios), 15)
        self.assertGreater(len(catalog_files), 1)
        self.assertIn('"racing-v1-2-0"', builder)
        self.assertIn('"opponent_audit" / "scenarios"', builder)
        self.assertIn('"opponent_audit" / "campaign_scenarios"', builder)
        self.assertIn('f"{PACKAGE}/opponent_audit.py"', builder)
        self.assertIn('"strategy_effect" / "scenarios"', builder)
        self.assertIn('f"{PACKAGE}/strategy_effect.py"', builder)
        self.assertIn('"budget_effect" / "scenarios"', builder)
        self.assertIn('f"{PACKAGE}/budget_effect.py"', builder)
        self.assertIn('"budget_effect_v2" / "scenarios"', builder)
        self.assertIn('f"{PACKAGE}/budget_effect_v2.py"', builder)
        self.assertIn("execute_packaged_racing_catalog_scenario", runner)
        self.assertIn("CATALOG_RESOURCE_PATTERN.fullmatch", runner)
        self.assertIn(
            'command.extend(["--catalog-release", str(catalog_root)])', runner
        )

    def test_reference_campaign_manifest_is_frozen_and_reconciled(self):
        campaign_root = ROOT / "campaigns"
        manifest_path = campaign_root / "racing-reference-v1.json"
        checksum_path = campaign_root / "racing-reference-v1.sha256"
        expected_digest, expected_name = checksum_path.read_text().split()
        self.assertEqual(expected_name, manifest_path.name)
        self.assertEqual(
            hashlib.sha256(manifest_path.read_bytes()).hexdigest(), expected_digest
        )

        manifest = json.loads(manifest_path.read_text())
        families = manifest["configuration_families"]
        seeds = manifest["seeds"]
        self.assertEqual(manifest["planned_run_count"], len(families) * len(seeds))
        self.assertGreaterEqual(len(families), 3)
        self.assertGreaterEqual(len(seeds), 2)
        self.assertEqual(len({family["id"] for family in families}), len(families))
        self.assertEqual(
            len({family["expected_configuration_id"] for family in families}),
            len(families),
        )

    def test_circuit_sweep_manifest_is_frozen_and_reconciled(self):
        campaign_root = ROOT / "campaigns"
        manifest_path = campaign_root / "racing-circuit-sweep-v1.json"
        checksum_path = campaign_root / "racing-circuit-sweep-v1.sha256"
        expected_digest, expected_name = checksum_path.read_text().split()
        self.assertEqual(expected_name, manifest_path.name)
        self.assertEqual(
            hashlib.sha256(manifest_path.read_bytes()).hexdigest(), expected_digest
        )

        manifest = json.loads(manifest_path.read_text())
        configurations = manifest["configurations"]
        seeds = manifest["seeds"]
        self.assertEqual(manifest["schema_version"], "pitgun.calibration-campaign/v2")
        self.assertEqual(len(manifest["circuits"]), 5)
        self.assertEqual(len(configurations), 35)
        self.assertEqual(
            len({row["configuration_family"] for row in configurations}), 7
        )
        self.assertEqual(manifest["planned_run_count"], 105)
        self.assertEqual(
            manifest["planned_run_count"], len(configurations) * len(seeds)
        )
        self.assertEqual(
            len({row["expected_configuration_id"] for row in configurations}),
            len(configurations),
        )

    def test_circuit_sweep_job_uses_the_generic_governed_executor(self):
        job = (ROOT / "resources" / "jobs.yml").read_text()
        notebook = (ROOT / "src" / "execute_reference_campaign.py").read_text()

        self.assertIn("circuit_sweep_job:", job)
        self.assertIn("campaign_name: racing-circuit-sweep-v1", job)
        self.assertIn("campaign_name: racing-reference-v1", job)
        self.assertIn("load_calibration_campaign", notebook)
        self.assertIn("execute_packaged_racing_scenario", notebook)
        self.assertIn('entry["circuit_id"]', notebook)
        self.assertIn('"circuits":', notebook)

    def test_opponent_audit_job_is_resumable_governed_and_non_promoting(self):
        job = (ROOT / "resources" / "jobs.yml").read_text()
        notebook = (ROOT / "src" / "execute_opponent_audit.py").read_text()
        bootstrap = (ROOT / "src" / "bootstrap_tables.py").read_text()

        self.assertIn("opponent_audit_job:", job)
        self.assertIn("campaign_id: racing-opponent-audit-2026-v1", job)
        self.assertIn("timeout_seconds: 7200", job)
        self.assertNotIn('scenario_resource: "{{job.parameters', job)
        self.assertIn("load_opponent_audit_campaign", notebook)
        self.assertIn("execute_packaged_racing_catalog_scenario", notebook)
        self.assertIn('row["execution_status"] == "SUCCESS"', notebook)
        self.assertIn("unexpected_keys", notebook)
        self.assertIn('"automatic_game_or_catalog_promotion": False', notebook)
        for column in (
            "execution_key",
            "progression",
            "strategy_profile",
            "player_reference",
            "source_contract_digest",
        ):
            self.assertIn(f'"{column}":', bootstrap)

    def test_opponent_diagnosis_is_pinned_read_only_and_non_promoting(self):
        job = (ROOT / "resources" / "jobs.yml").read_text()
        notebook = (ROOT / "src" / "diagnose_opponent_audit.py").read_text()

        self.assertIn("opponent_diagnosis_job:", job)
        self.assertIn('campaigns_table_version\n          default: "19"', job)
        self.assertIn('runs_table_version\n          default: "5"', job)
        self.assertIn('metrics_table_version\n          default: "3"', job)
        self.assertIn('.option("versionAsOf", campaigns_version)', notebook)
        self.assertIn("runs_table_version", job)
        self.assertIn("metrics_table_version", job)
        self.assertIn('.option("versionAsOf", runs_version)', notebook)
        self.assertIn('.option("versionAsOf", metrics_version)', notebook)
        self.assertIn("diagnose_opponent_audit", notebook)
        self.assertIn('"pitgun.opponent_policy_selected": "false"', notebook)
        for forbidden in ("DeltaTable", "MERGE INTO", "UPDATE ", "DELETE "):
            self.assertNotIn(forbidden, notebook)

    def test_strategy_effect_job_is_resumable_causal_and_non_promoting(self):
        job = (ROOT / "resources" / "jobs.yml").read_text()
        notebook = (ROOT / "src" / "execute_strategy_effect.py").read_text()

        self.assertIn("strategy_effect_job:", job)
        self.assertIn("racing-strategy-effect-2026-v1", job)
        self.assertIn("load_strategy_effect_campaign", notebook)
        self.assertIn("extract_strategy_effect_evidence", notebook)
        self.assertIn("summarize_strategy_effect", notebook)
        self.assertIn('row["execution_status"] == "SUCCESS"', notebook)
        self.assertIn("whenNotMatchedInsertAll", notebook)
        self.assertIn("pair_invariant_digest", notebook)
        self.assertIn('"automatic_game_or_catalog_promotion": False', notebook)
        for forbidden in ("policy_releases", '"release_state": "PUBLISHED"'):
            self.assertNotIn(forbidden, notebook)

    def test_candidate_validation_is_immutable_experimental_and_reviewed(self):
        campaign_root = ROOT / "campaigns"
        manifest_path = campaign_root / "racing-aero-candidate-validation-v1.json"
        checksum_path = campaign_root / "racing-aero-candidate-validation-v1.sha256"
        expected_digest, expected_name = checksum_path.read_text().split()
        self.assertEqual(expected_name, manifest_path.name)
        self.assertEqual(
            hashlib.sha256(manifest_path.read_bytes()).hexdigest(), expected_digest
        )

        manifest = json.loads(manifest_path.read_text())
        self.assertEqual(manifest["schema_version"], "pitgun.calibration-campaign/v3")
        self.assertEqual(manifest["execution_class"], "experimental-tuning-response")
        self.assertEqual(manifest["promotion_policy"], "human-review-required")
        self.assertFalse(manifest["acceptance_criteria"]["automatic_release"])
        self.assertEqual(len(manifest["responses"]), 2)
        self.assertEqual(len(manifest["configurations"]), 70)
        self.assertEqual(manifest["planned_run_count"], 210)
        self.assertEqual(
            len(
                {
                    row["expected_experimental_configuration_id"]
                    for row in manifest["configurations"]
                }
            ),
            70,
        )

        job = (ROOT / "resources" / "jobs.yml").read_text()
        notebook = (ROOT / "src" / "execute_candidate_validation.py").read_text()
        runner = (
            ROOT / "adapter" / "pitgun_databricks_adapter" / "runner.py"
        ).read_text()
        self.assertIn("candidate_validation_job:", job)
        self.assertIn("campaign_name: racing-aero-candidate-validation-v1", job)
        self.assertIn("experimental_runs", notebook)
        self.assertIn("experimental_execution_id", notebook)
        self.assertNotIn('"run_id"', notebook)
        self.assertIn('"REVIEW_REQUIRED"', notebook)
        self.assertIn("execute_packaged_tuning_response", runner)
        self.assertNotIn("shell=True", runner)

    def test_v3_tire_job_reuses_exact_probe_contract_and_cannot_promote(self):
        job = (ROOT / "resources" / "jobs.yml").read_text()
        notebook = (ROOT / "src" / "execute_v3_tire_degradation.py").read_text()
        runner = (
            ROOT / "adapter" / "pitgun_databricks_adapter" / "runner.py"
        ).read_text()
        builder = (ROOT / "adapter" / "build_wheel.py").read_text()

        self.assertIn("v3_tire_degradation_job:", job)
        self.assertIn("campaign_name: racing-v3-tire-degradation-v1", job)
        self.assertIn("execute_packaged_v3_tire_degradation", notebook)
        self.assertIn("expected_experimental_execution_id", notebook)
        self.assertIn('"automatic_catalog_promotion": False', notebook)
        self.assertIn("V3_VALIDATION_RESULT_VERSION", runner)
        self.assertIn('f"{PACKAGE}/bin/v3_validation_probe"', builder)
        self.assertNotIn("shell=True", runner)
        for forbidden in ("policy_releases", '"release_state": "PUBLISHED"'):
            self.assertNotIn(forbidden, notebook)

    def test_candidate_review_pins_delta_and_cannot_promote_automatically(self):
        job = (ROOT / "resources" / "jobs.yml").read_text()
        notebook = (ROOT / "src" / "review_candidate_evidence.py").read_text()
        policy = json.loads(
            (ROOT / "reviews" / "racing-aero-candidate-review-v1.json").read_text()
        )

        self.assertIn("candidate_review_job:", job)
        self.assertIn('experimental_runs_table_version: "1"', job)
        self.assertIn('experimental_metrics_table_version: "1"', job)
        self.assertIn('.option("versionAsOf", runs_version)', notebook)
        self.assertIn('.option("versionAsOf", metrics_version)', notebook)
        self.assertIn("review_candidate_evidence", notebook)
        self.assertFalse(policy["automatic_promotion"])
        self.assertEqual(policy["decision_states"], ["PROMOTE", "REFINE", "REJECT"])
        for forbidden in ("UPDATE ", "MERGE INTO", "PUBLISHED", "catalog release"):
            self.assertNotIn(forbidden, notebook)

    def test_reference_campaign_job_is_idempotent_and_governed(self):
        job = (ROOT / "resources" / "jobs.yml").read_text()
        notebook = (ROOT / "src" / "execute_reference_campaign.py").read_text()
        bootstrap = (ROOT / "src" / "bootstrap_tables.py").read_text()

        self.assertIn("reference_campaign_job:", job)
        self.assertIn("depends_on:", job)
        self.assertIn("execute_reference_campaign.py", job)
        self.assertIn("DeltaTable.forName", notebook)
        self.assertIn("whenNotMatchedInsertAll", notebook)
        self.assertIn("target.execution_status <> 'SUCCESS'", notebook)
        self.assertIn("mlflow.start_run", notebook)
        self.assertIn("if is_new_tracking_run:", notebook)
        self.assertIn('"plots/configuration-pace.svg"', notebook)
        self.assertIn("successful_count + invalid_count + failed_count", notebook)
        self.assertNotIn(
            '"source_git_revision": "source.source_git_revision"', notebook
        )
        for governed_column in (
            "manifest_digest",
            "mlflow_run_id",
            "configuration_family",
            "adapter_version",
            "runner_artifact_digest",
            "canonical_result_digest",
        ):
            self.assertIn(f'"{governed_column}"', bootstrap)

    def test_reference_policy_selection_pins_delta_and_requires_review(self):
        job = (ROOT / "resources" / "jobs.yml").read_text()
        notebook = (ROOT / "src" / "select_reference_opponent_policy.py").read_text()

        self.assertIn("reference_policy_job:", job)
        for table, version in (("campaigns", "3"), ("runs", "2"), ("metrics", "1")):
            self.assertIn(f'{table}_table_version: "{version}"', job)
            self.assertIn(f'.option("versionAsOf", {table}_table_version)', notebook)
        self.assertIn('"release_state": "PROPOSED"', notebook)
        self.assertIn("target.release_state = 'PROPOSED'", notebook)
        self.assertNotIn('"release_state": "PUBLISHED"', notebook)
        self.assertIn("select_reference_policy", notebook)


if __name__ == "__main__":
    unittest.main()
