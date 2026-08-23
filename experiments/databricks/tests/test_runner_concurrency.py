import concurrent.futures
import pathlib
import stat
import sys
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "adapter"))

from pitgun_databricks_adapter import runner  # noqa: E402


class RunnerConcurrencyTest(unittest.TestCase):
    def test_driver_control_probe_is_materialized_once_and_reused(self):
        probe_bytes = b"unique-test-driver-control-probe"
        digest = runner._sha256(probe_bytes)
        with runner._DRIVER_CONTROL_PROBE_LOCK:
            runner._DRIVER_CONTROL_PROBES.pop(digest, None)

        with concurrent.futures.ThreadPoolExecutor(max_workers=16) as executor:
            paths = list(
                executor.map(
                    lambda _: runner._materialize_driver_control_probe(probe_bytes),
                    range(64),
                )
            )

        self.assertEqual(len(set(paths)), 1)
        self.assertEqual(paths[0].read_bytes(), probe_bytes)
        self.assertEqual(stat.S_IMODE(paths[0].stat().st_mode), 0o500)
        self.assertIs(runner._DRIVER_CONTROL_PROBES[digest], paths[0])


if __name__ == "__main__":
    unittest.main()
