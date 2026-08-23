import json
import pathlib
import subprocess
import sys
import unittest


EXPERIMENT = pathlib.Path(__file__).parents[1]


class ReviewEqualBudgetRosterTests(unittest.TestCase):
    def test_review_is_reproducible_and_fail_closed(self) -> None:
        completed = subprocess.run(
            [
                sys.executable,
                str(EXPERIMENT / "review_equal_budget_roster.py"),
                "--check",
            ],
            check=False,
            capture_output=True,
            text=True,
        )

        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertIn("reproduced", completed.stdout)

        report = json.loads(
            (
                EXPERIMENT
                / "results"
                / "equal-budget-driver-control-review-v1.json"
            ).read_bytes()
        )
        self.assertEqual(report["verdict"], "MODE_RESPONSE_REFINEMENT_REQUIRED")
        self.assertEqual(
            report["contextual_driver_profile_ids"], ["halton-19", "halton-27"]
        )
        self.assertEqual(report["selected_profile_ids"], [])
        self.assertFalse(report["automatic_publication_performed"])


if __name__ == "__main__":
    unittest.main()
