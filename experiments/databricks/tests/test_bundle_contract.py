import ast
import pathlib
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[1]


class BundleContractTest(unittest.TestCase):
    def test_bootstrap_notebook_is_valid_python(self):
        source = (ROOT / "src" / "bootstrap_tables.py").read_text()
        ast.parse(source)

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


if __name__ == "__main__":
    unittest.main()
