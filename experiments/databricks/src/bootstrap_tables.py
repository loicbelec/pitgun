# Databricks notebook source
"""Create the governed Delta tables for Pitgun Calibration V1.

The notebook performs idempotent DDL only. It never reads, writes, migrates, or
deletes objects in ``workspace.default``.
"""

# COMMAND ----------

import json
import platform
import re


dbutils.widgets.text("operation", "bootstrap")
dbutils.widgets.text("campaign_id", "bundle-smoke")
dbutils.widgets.text("catalog_name", "workspace")
dbutils.widgets.text("calibration_schema", "pitgun_calibration")
dbutils.widgets.text("policies_schema", "pitgun_policies")
dbutils.widgets.text("experiment_id", "")

operation = dbutils.widgets.get("operation")
campaign_id = dbutils.widgets.get("campaign_id")
catalog_name = dbutils.widgets.get("catalog_name")
calibration_schema = dbutils.widgets.get("calibration_schema")
policies_schema = dbutils.widgets.get("policies_schema")
experiment_id = dbutils.widgets.get("experiment_id")


def validated_identifier(label: str, value: str) -> str:
    if not re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", value):
        raise ValueError(f"{label} is not a portable SQL identifier: {value!r}")
    return value


if operation not in {"bootstrap", "validate"}:
    raise ValueError(f"unsupported operation: {operation!r}")

catalog_name = validated_identifier("catalog_name", catalog_name)
calibration_schema = validated_identifier("calibration_schema", calibration_schema)
policies_schema = validated_identifier("policies_schema", policies_schema)

for schema_name in (calibration_schema, policies_schema):
    if schema_name.lower() in {"default", "information_schema"}:
        raise ValueError(f"protected schema cannot be a Pitgun target: {schema_name!r}")

calibration = f"`{catalog_name}`.`{calibration_schema}`"
policies = f"`{catalog_name}`.`{policies_schema}`"

required_schemas = {
    row[0]
    for row in spark.sql(f"SHOW SCHEMAS IN `{catalog_name}`").collect()
}
missing_schemas = {
    calibration_schema,
    policies_schema,
} - required_schemas
if missing_schemas:
    raise RuntimeError(
        "bundle-managed schemas are missing: " + ", ".join(sorted(missing_schemas))
    )

# COMMAND ----------

table_definitions = {
    "campaigns": f"""
        CREATE TABLE IF NOT EXISTS {calibration}.campaigns (
          campaign_id STRING NOT NULL COMMENT 'Stable campaign identifier; table grain key.',
          question STRING NOT NULL COMMENT 'Bounded calibration question answered by the campaign.',
          parameter_space_version STRING NOT NULL COMMENT 'Version of the materialized configuration space.',
          scenario_id STRING NOT NULL,
          scenario_version STRING NOT NULL,
          scenario_digest STRING,
          model_id STRING NOT NULL,
          model_version STRING NOT NULL,
          model_digest STRING NOT NULL,
          data_pack_id STRING NOT NULL,
          data_pack_version STRING NOT NULL,
          data_pack_digest STRING NOT NULL,
          runner_version STRING NOT NULL,
          source_git_revision STRING NOT NULL,
          status STRING NOT NULL,
          planned_run_count BIGINT,
          created_at TIMESTAMP NOT NULL,
          updated_at TIMESTAMP NOT NULL
        )
        USING DELTA
        COMMENT 'Grain: one row per Pitgun calibration campaign.'
        TBLPROPERTIES (
          'pitgun.grain' = 'campaign_id',
          'pitgun.owner_domain' = 'calibration',
          'pitgun.contract_version' = 'v1'
        )
    """,
    "runs": f"""
        CREATE TABLE IF NOT EXISTS {calibration}.runs (
          campaign_id STRING NOT NULL COMMENT 'Parent campaign identifier.',
          configuration_id STRING NOT NULL COMMENT 'Content-derived resolved model-input identity.',
          seed STRING NOT NULL COMMENT 'Unsigned deterministic seed encoded losslessly as decimal text.',
          run_id STRING COMMENT 'Deterministic run identity when execution succeeds.',
          scenario_id STRING NOT NULL,
          scenario_version STRING NOT NULL,
          scenario_digest STRING NOT NULL,
          model_id STRING NOT NULL,
          model_version STRING NOT NULL,
          model_digest STRING NOT NULL,
          data_pack_id STRING NOT NULL,
          data_pack_version STRING NOT NULL,
          data_pack_digest STRING NOT NULL,
          runner_version STRING NOT NULL,
          source_git_revision STRING NOT NULL,
          circuit_id STRING,
          era INT,
          setup_json STRING COMMENT 'Canonical materialized setup parameters.',
          strategy_json STRING COMMENT 'Canonical materialized strategy parameters.',
          execution_status STRING NOT NULL,
          failure_phase STRING,
          failure_code STRING,
          failure_message STRING,
          duration_ms BIGINT,
          result_json STRING COMMENT 'Canonical pitgun.batch-run-result/v1 document.',
          started_at TIMESTAMP,
          completed_at TIMESTAMP,
          ingested_at TIMESTAMP NOT NULL
        )
        USING DELTA
        COMMENT 'Grain: one row per campaign, resolved configuration, and seed execution.'
        TBLPROPERTIES (
          'pitgun.grain' = 'campaign_id,configuration_id,seed',
          'pitgun.owner_domain' = 'calibration',
          'pitgun.contract_version' = 'v1'
        )
    """,
    "metrics": f"""
        CREATE TABLE IF NOT EXISTS {calibration}.metrics (
          campaign_id STRING NOT NULL,
          run_id STRING NOT NULL,
          configuration_id STRING NOT NULL,
          seed STRING NOT NULL,
          metric_id STRING NOT NULL,
          metric_value DOUBLE NOT NULL,
          metric_unit STRING NOT NULL,
          sample_count BIGINT,
          statistic STRING,
          recorded_at TIMESTAMP NOT NULL
        )
        USING DELTA
        COMMENT 'Grain: one row per successful deterministic run and metric identifier.'
        TBLPROPERTIES (
          'pitgun.grain' = 'run_id,metric_id',
          'pitgun.owner_domain' = 'calibration',
          'pitgun.contract_version' = 'v1'
        )
    """,
    "candidates": f"""
        CREATE TABLE IF NOT EXISTS {calibration}.candidates (
          campaign_id STRING NOT NULL,
          candidate_id STRING NOT NULL,
          configuration_id STRING NOT NULL,
          circuit_id STRING NOT NULL,
          era INT NOT NULL,
          difficulty_band STRING NOT NULL,
          selection_score DOUBLE NOT NULL,
          constraint_results_json STRING NOT NULL,
          candidate_rank INT,
          decision_state STRING NOT NULL,
          decision_reason STRING,
          reviewed_at TIMESTAMP
        )
        USING DELTA
        COMMENT 'Grain: one reviewed opponent candidate per campaign, circuit, era, and difficulty band.'
        TBLPROPERTIES (
          'pitgun.grain' = 'campaign_id,candidate_id,difficulty_band',
          'pitgun.owner_domain' = 'calibration',
          'pitgun.contract_version' = 'v1'
        )
    """,
    "policy_releases": f"""
        CREATE TABLE IF NOT EXISTS {policies}.releases (
          policy_id STRING NOT NULL,
          policy_version STRING NOT NULL,
          artifact_digest STRING NOT NULL,
          artifact_uri STRING NOT NULL,
          source_campaign_id STRING NOT NULL,
          source_candidate_set_digest STRING NOT NULL,
          target_catalog_id STRING NOT NULL,
          target_catalog_version STRING NOT NULL,
          release_state STRING NOT NULL,
          approved_by STRING,
          approved_at TIMESTAMP,
          published_at TIMESTAMP
        )
        USING DELTA
        COMMENT 'Grain: one immutable reviewed Pitgun policy release version.'
        TBLPROPERTIES (
          'pitgun.grain' = 'policy_id,policy_version',
          'pitgun.owner_domain' = 'policies',
          'pitgun.contract_version' = 'v1'
        )
    """,
}

if operation == "bootstrap":
    for ddl in table_definitions.values():
        spark.sql(ddl)

# COMMAND ----------

expected_tables = {
    calibration: {"campaigns", "runs", "metrics", "candidates"},
    policies: {"releases"},
}
observed = {}
for namespace, expected in expected_tables.items():
    # SHOW TABLES returns namespace, tableName, isTemporary across supported
    # Spark runtimes; positional access avoids runtime-specific field casing.
    actual = {row[1] for row in spark.sql(f"SHOW TABLES IN {namespace}").collect()}
    missing = expected - actual
    if missing:
        raise RuntimeError(
            f"governed tables missing from {namespace}: {', '.join(sorted(missing))}"
        )
    observed[namespace.replace("`", "")] = sorted(expected)

result = {
    "schema_version": "pitgun.databricks-bootstrap-result/v1",
    "operation": operation,
    "campaign_id": campaign_id,
    "experiment_id": experiment_id,
    "host": {"machine": platform.machine(), "system": platform.system()},
    "tables": observed,
}
dbutils.notebook.exit(json.dumps(result, sort_keys=True, separators=(",", ":")))
