import importlib.util
import pathlib
import unittest


MODULE_PATH = pathlib.Path(__file__).parents[1] / "review_v11_shortlist.py"
SPEC = importlib.util.spec_from_file_location("review_v11_shortlist", MODULE_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(MODULE)


class ReviewV11ShortlistTests(unittest.TestCase):
    def test_review_is_reproducible_and_fail_closed(self) -> None:
        report = MODULE.review()

        self.assertEqual(report["verdict"], "STRUCTURAL_REFINEMENT_REQUIRED")
        self.assertEqual(report["selected_profile_ids"], [])
        self.assertFalse(report["automatic_publication_performed"])
        self.assertEqual(len(report["profiles"]), 3)

    def test_all_profiles_are_finite_but_globally_dominated(self) -> None:
        report = MODULE.review()

        for profile in report["profiles"]:
            self.assertEqual(profile["pathological_execution_count"], 0)
            self.assertFalse(profile["holdout_gate_passed"])
            self.assertEqual(
                profile["winner_analysis"]["global_driver_mode_winner_counts"],
                {"smooth_operator:attack": 54},
            )


if __name__ == "__main__":
    unittest.main()
