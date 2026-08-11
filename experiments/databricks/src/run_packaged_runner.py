# Databricks notebook source
"""Execute the bounded Pitgun Rust runner installed from the adapter wheel."""

# COMMAND ----------

import json

from pitgun_databricks_adapter import execute_packaged_racing


dbutils.widgets.text("seed", "42")
dbutils.widgets.text("configuration_family", "balanced")
seed_text = dbutils.widgets.get("seed")
configuration_family = dbutils.widgets.get("configuration_family")
try:
    seed = int(seed_text)
except ValueError as error:
    raise ValueError("seed must be an unsigned decimal integer") from error

result = execute_packaged_racing(seed, configuration_family)
dbutils.notebook.exit(json.dumps(result, sort_keys=True, separators=(",", ":")))
