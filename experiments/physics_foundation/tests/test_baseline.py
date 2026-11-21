"""Protect scientific identity, resource isolation and failure reporting in the runner."""
import copy
import importlib.util
import pathlib
import sys
import unittest

HERE = pathlib.Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("physics_baseline", HERE / "baseline_local.py")
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)


class BaselineTests(unittest.TestCase):
    def test_archived_evidence_matches_its_checksum(self):
        report = HERE / "results/local-baseline-v1.json"
        self.assertEqual(report.with_suffix(".sha256").read_text().strip(), runner.digest(report.read_bytes()))

    def test_duplicate_or_unsafe_case_ids_cannot_overwrite_evidence(self):
        plan = runner.load(HERE / "plan-v1.json")
        duplicate = copy.deepcopy(plan)
        duplicate["cases"].append(duplicate["cases"][0])
        with self.assertRaises(ValueError):
            runner.validate_plan(duplicate)
        plan["cases"][0]["id"] = "../result"
        with self.assertRaises(ValueError):
            runner.validate_plan(plan)

    def test_disagreeing_sampling_inputs_are_rejected(self):
        plan = runner.load(HERE / "plan-v1.json")
        plan["cases"][0]["request"]["hz"] = 1
        with self.assertRaises(ValueError):
            runner.validate_plan(plan)

    def test_failure_is_not_a_successful_simulation(self):
        result = runner.run_process([sys.executable, "-c", "import sys;print('specific failure',file=sys.stderr);sys.exit(7)"], 2)
        self.assertEqual(result["returncode"], 7)
        self.assertIn(b"specific failure", result["stderr"])
        self.assertFalse(result["timed_out"])

    def test_timeout_kills_and_reaps_only_owned_child(self):
        result = runner.run_process([sys.executable, "-c", "import time;time.sleep(30)"], .05)
        self.assertTrue(result["timed_out"])
        self.assertLess(result["wall_ms"], 2000)
        self.assertLess(result["returncode"], 0)

    def test_rss_is_measured_per_child_not_previous_high_water_mark(self):
        large = runner.run_process([sys.executable, "-c", "a=bytearray(64*1024*1024);a[::4096]=b'1'*(len(a)//4096)"], 2)
        small = runner.run_process([sys.executable, "-c", "print(1)"], 2)
        self.assertGreater(large["process_lifetime_max_rss_bytes"], small["process_lifetime_max_rss_bytes"] + 32*1024*1024)

    def test_stale_probe_cannot_be_attributed_to_edited_equations(self):
        with self.assertRaisesRegex(ValueError, "stale core"):
            runner.validate_probe_identity(
                {"native_binary_digest": "binary", "compiled_source_digests": {"solver.rs": "old"}},
                {"probe_digest": "binary", "source_files": {"solver.rs": "new"}},
            )

    def test_sampling_comparison_keeps_physical_state_and_excludes_export_only(self):
        a = {"fuel": {"final_fuel_mass_kg": 90}, "output_digest": "a", "exported_channels": {"5000": 10}}
        b = {**a, "output_digest": "b", "exported_channels": {"5000": 50}}
        self.assertEqual(runner.physical_projection(a), runner.physical_projection(b))
        b["fuel"] = {"final_fuel_mass_kg": 89}
        self.assertNotEqual(runner.physical_projection(a), runner.physical_projection(b))


if __name__ == "__main__":
    unittest.main()
