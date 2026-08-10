# Databricks notebook source
"""Execute the bounded Pitgun Rust runner installed from the adapter wheel."""

# COMMAND ----------

import json

from pitgun_databricks_adapter import execute_packaged_racing


dbutils.widgets.text("seed", "42")
seed_text = dbutils.widgets.get("seed")
try:
    seed = int(seed_text)
except ValueError as error:
    raise ValueError("seed must be an unsigned decimal integer") from error

result = execute_packaged_racing(seed)
dbutils.notebook.exit(json.dumps(result, sort_keys=True, separators=(",", ":")))

