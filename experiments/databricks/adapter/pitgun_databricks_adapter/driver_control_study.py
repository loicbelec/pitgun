"""Load the immutable human review of the driver-control campaign."""

from __future__ import annotations

import functools
import importlib.resources
import json
from typing import Any


REVIEW_NAME = "racing-v3-driver-control-study-review-v1"
REVIEW_SCHEMA_VERSION = "pitgun.racing-v3-driver-control-study-review/v1"
CAMPAIGN_ID = "racing-v3-driver-control-surface-2026-v2"


class DriverControlStudyReviewError(ValueError):
    """Raised when the packaged review is incomplete or unsupported."""


@functools.lru_cache(maxsize=1)
def load_driver_control_study_review(
    name: str = REVIEW_NAME,
) -> dict[str, Any]:
    if name != REVIEW_NAME:
        raise DriverControlStudyReviewError("review is not packaged or allowlisted")
    root = importlib.resources.files("pitgun_databricks_adapter") / "reviews"
    review = json.loads(root.joinpath(name + ".json").read_bytes())
    if review.get("schema_version") != REVIEW_SCHEMA_VERSION:
        raise DriverControlStudyReviewError("unsupported driver-control study review")
    if review.get("campaign_id") != CAMPAIGN_ID:
        raise DriverControlStudyReviewError("review targets an unsupported campaign")
    lineage = review.get("lineage", {})
    for key in (
        "campaigns_table_version",
        "experimental_runs_table_version",
        "experimental_metrics_table_version",
    ):
        value = lineage.get(key)
        if not isinstance(value, int) or value < 0:
            raise DriverControlStudyReviewError(
                "review requires explicit non-negative Delta versions"
            )
    expected = review.get("expected_evidence", {})
    if expected.get("successful_execution_count") != 1584:
        raise DriverControlStudyReviewError("review execution count changed")
    if expected.get("normalized_metric_count") != 22176:
        raise DriverControlStudyReviewError("review metric count changed")
    conclusion = review.get("reviewed_conclusion", {})
    if conclusion.get("decision") != "STRUCTURAL_REFINEMENT_REQUIRED":
        raise DriverControlStudyReviewError("review conclusion changed")
    if conclusion.get("candidate_selected") is not False:
        raise DriverControlStudyReviewError("review cannot select a candidate")
    if conclusion.get("automatic_catalog_promotion") is not False:
        raise DriverControlStudyReviewError("automatic promotion is forbidden")
    return review
