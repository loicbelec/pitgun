"""Invoke the packaged Pitgun binary without duplicating simulation logic."""

from __future__ import annotations

import hashlib
import importlib.metadata
import importlib.resources
import json
import pathlib
import platform
import subprocess
import tempfile
import time
from typing import Any


RESULT_VERSION = "pitgun.databricks-runner-result/v1"
RUNNER_TARGET = "aarch64-unknown-linux-gnu"
PROCESS_TIMEOUT_SECONDS = 120


class RunnerExecutionError(RuntimeError):
    """Raised when the packaged, bounded runner cannot produce a valid result."""


def _sha256(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


def _run(command: list[str]) -> tuple[subprocess.CompletedProcess[bytes], int]:
    started = time.perf_counter_ns()
    try:
        completed = subprocess.run(
            command,
            check=False,
            capture_output=True,
            timeout=PROCESS_TIMEOUT_SECONDS,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise RunnerExecutionError(f"packaged runner could not execute: {error}") from error
    duration_ms = (time.perf_counter_ns() - started) // 1_000_000
    return completed, duration_ms


def _execute(runner_bytes: bytes, scenario_bytes: bytes, seed: int) -> dict[str, Any]:
    if not 0 <= seed <= 2**64 - 1:
        raise ValueError("seed must be an unsigned 64-bit integer")

    with tempfile.TemporaryDirectory(prefix="pitgun-runner-") as temporary:
        root = pathlib.Path(temporary)
        runner = root / "pitgun"
        scenario = root / "scenario.json"
        runner.write_bytes(runner_bytes)
        runner.chmod(0o500)
        scenario.write_bytes(scenario_bytes)

        version_process, startup_probe_duration_ms = _run([str(runner), "--version"])
        if version_process.returncode != 0:
            raise RunnerExecutionError(
                "packaged runner version probe failed: "
                + version_process.stderr.decode("utf-8", errors="replace").strip()
            )
        runner_version = version_process.stdout.decode("utf-8").strip()

        process, execution_duration_ms = _run(
            [
                str(runner),
                "run",
                "racing",
                "--scenario",
                str(scenario),
                "--seed",
                str(seed),
            ]
        )
        if process.returncode != 0:
            diagnostic = process.stderr.decode("utf-8", errors="replace").strip()
            raise RunnerExecutionError(
                f"packaged runner failed with exit {process.returncode}: {diagnostic}"
            )
        if process.stderr:
            raise RunnerExecutionError("successful packaged runner wrote unexpected stderr")

        try:
            compact_result = json.loads(process.stdout)
        except (UnicodeDecodeError, json.JSONDecodeError) as error:
            raise RunnerExecutionError("packaged runner returned invalid JSON") from error
        if compact_result.get("schema_version") != "pitgun.batch-run-result/v1":
            raise RunnerExecutionError("packaged runner returned an unsupported result contract")

        return {
            "schema_version": RESULT_VERSION,
            "adapter": {
                "version": importlib.metadata.version("pitgun-databricks-adapter"),
            },
            "runner_artifact": {
                "version": runner_version,
                "target": RUNNER_TARGET,
                "digest": _sha256(runner_bytes),
            },
            "host": {
                "machine": platform.machine(),
                "system": platform.system(),
            },
            "measurements": {
                "startup_probe_duration_ms": startup_probe_duration_ms,
                "execution_duration_ms": execution_duration_ms,
            },
            "canonical_result_digest": _sha256(process.stdout),
            "result": compact_result,
        }


def execute_packaged_racing(seed: int = 42) -> dict[str, Any]:
    """Execute only the scenario and native binary embedded in this wheel."""

    package = importlib.resources.files("pitgun_databricks_adapter")
    runner_bytes = package.joinpath("bin", "pitgun").read_bytes()
    scenario_bytes = package.joinpath("scenarios", "balanced.json").read_bytes()
    return _execute(runner_bytes, scenario_bytes, seed)
