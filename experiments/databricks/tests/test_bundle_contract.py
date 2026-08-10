import ast
import pathlib
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[1]


class BundleContractTest(unittest.TestCase):
    def test_python_sources_are_syntactically_valid(self):
        for path in ROOT.rglob("*.py"):
            if path != pathlib.Path(__file__):
                ast.parse(path.read_text(), filename=str(path))

    def test_bootstrap_owns_only_the_five_governed_tables(self):
        source = (ROOT / "src" / "bootstrap_tables.py").read_text()
        expected = {
            "campaigns",
            "runs",
            "metrics",
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
            ROOT
            / "adapter"
            / "pitgun_databricks_adapter"
            / "runner.py"
        ).read_text()
        self.assertIn("runner_spike_job:", job)
        self.assertIn("- name: seed", job)
        for forbidden_parameter in ("runner_url", "runner_path", "command", "scenario"):
            self.assertNotIn(f"- name: {forbidden_parameter}", job)
        self.assertNotIn("shell=True", runner)
        self.assertNotIn("http://", runner)
        self.assertNotIn("https://", runner)


if __name__ == "__main__":
    unittest.main()
