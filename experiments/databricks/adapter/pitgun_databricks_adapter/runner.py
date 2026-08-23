"""Invoke the packaged Pitgun binary without duplicating simulation logic."""

from __future__ import annotations

import hashlib
import importlib.metadata
import importlib.resources
import json
import pathlib
import platform
import re
import subprocess
import tempfile
import threading
import time
from typing import Any


RESULT_VERSION = "pitgun.databricks-runner-result/v1"
TUNING_RESPONSE_RESULT_VERSION = "pitgun.databricks-tuning-response-result/v1"
V3_VALIDATION_RESULT_VERSION = "pitgun.databricks-v3-validation-result/v1"
V3_DECISION_SURFACE_RESULT_VERSION = (
    "pitgun.databricks-v3-decision-surface-result/v1"
)
RUNNER_TARGET = "aarch64-unknown-linux-gnu"
PROCESS_TIMEOUT_SECONDS = 120
SCENARIO_FAMILIES = frozenset({"balanced", "high-downforce", "low-downforce"})
SCENARIO_RESOURCE_PATTERN = re.compile(
    r"[a-z0-9]+(?:-[a-z0-9]+)*(?:--[a-z0-9]+(?:[-_][a-z0-9]+)*)?"
)
RESPONSE_RESOURCE_PATTERN = re.compile(r"racing-[a-z0-9]+(?:-[a-z0-9]+)*")
CATALOG_RESOURCE_PATTERN = re.compile(r"racing-v[0-9]+(?:-[0-9]+)*")
V3_CONFIGURATION_PATTERN = re.compile(r"[a-z0-9]+(?:-[a-z0-9]+)*")
V3_DECISION_SURFACE_EXECUTION_PATTERN = re.compile(r"v3ds-[0-9]{4}-[0-9]{6}")
V3_THERMAL_SURFACE_EXECUTION_PATTERN = re.compile(r"v3th-[0-9a-f]{16}")
V3_DRIVER_CONTROL_EXECUTION_PATTERN = re.compile(r"v3dc-[0-9a-f]{16}")
_DECISION_SURFACE_PROBE_ROOT = pathlib.Path(
    tempfile.mkdtemp(prefix="pitgun-v3-decision-surface-probe-")
)
_DECISION_SURFACE_PROBE_LOCK = threading.Lock()
_DECISION_SURFACE_PROBES: dict[str, pathlib.Path] = {}


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
        raise RunnerExecutionError(
            f"packaged runner could not execute: {error}"
        ) from error
    duration_ms = (time.perf_counter_ns() - started) // 1_000_000
    return completed, duration_ms


def _execute(
    runner_bytes: bytes,
    scenario_bytes: bytes,
    seed: int,
    catalog_files: dict[str, bytes] | None = None,
) -> dict[str, Any]:
    if not 0 <= seed <= 2**64 - 1:
        raise ValueError("seed must be an unsigned 64-bit integer")

    with tempfile.TemporaryDirectory(prefix="pitgun-runner-") as temporary:
        root = pathlib.Path(temporary)
        runner = root / "pitgun"
        scenario = root / "scenario.json"
        runner.write_bytes(runner_bytes)
        runner.chmod(0o500)
        scenario.write_bytes(scenario_bytes)

        catalog_root = root / "catalog"
        if catalog_files is not None:
            for relative_path, contents in sorted(catalog_files.items()):
                destination = catalog_root / relative_path
                destination.parent.mkdir(parents=True, exist_ok=True)
                destination.write_bytes(contents)

        version_process, startup_probe_duration_ms = _run([str(runner), "--version"])
        if version_process.returncode != 0:
            raise RunnerExecutionError(
                "packaged runner version probe failed: "
                + version_process.stderr.decode("utf-8", errors="replace").strip()
            )
        runner_version = version_process.stdout.decode("utf-8").strip()

        command = [
            str(runner),
            "run",
            "racing",
            "--scenario",
            str(scenario),
            "--seed",
            str(seed),
        ]
        if catalog_files is not None:
            command.extend(["--catalog-release", str(catalog_root)])
        process, execution_duration_ms = _run(command)
        if process.returncode != 0:
            diagnostic = process.stderr.decode("utf-8", errors="replace").strip()
            raise RunnerExecutionError(
                f"packaged runner failed with exit {process.returncode}: {diagnostic}"
            )
        if process.stderr:
            raise RunnerExecutionError(
                "successful packaged runner wrote unexpected stderr"
            )

        try:
            compact_result = json.loads(process.stdout)
        except (UnicodeDecodeError, json.JSONDecodeError) as error:
            raise RunnerExecutionError(
                "packaged runner returned invalid JSON"
            ) from error
        if compact_result.get("schema_version") != "pitgun.batch-run-result/v1":
            raise RunnerExecutionError(
                "packaged runner returned an unsupported result contract"
            )

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


def _execute_tuning_response_probe(
    probe_bytes: bytes, scenario_bytes: bytes, response_bytes: bytes, seed: int
) -> dict[str, Any]:
    if not 0 <= seed <= 2**64 - 1:
        raise ValueError("seed must be an unsigned 64-bit integer")

    with tempfile.TemporaryDirectory(prefix="pitgun-tuning-response-") as temporary:
        root = pathlib.Path(temporary)
        probe = root / "tuning_response_probe"
        scenario = root / "scenario.json"
        response = root / "response.json"
        probe.write_bytes(probe_bytes)
        probe.chmod(0o500)
        scenario.write_bytes(scenario_bytes)
        response.write_bytes(response_bytes)

        process, execution_duration_ms = _run(
            [str(probe), str(scenario), str(response), str(seed)]
        )
        if process.returncode != 0:
            diagnostic = process.stderr.decode("utf-8", errors="replace").strip()
            raise RunnerExecutionError(
                f"packaged tuning-response probe failed with exit {process.returncode}: {diagnostic}"
            )
        if process.stderr:
            raise RunnerExecutionError(
                "successful tuning-response probe wrote unexpected stderr"
            )
        try:
            result = json.loads(process.stdout)
        except (UnicodeDecodeError, json.JSONDecodeError) as error:
            raise RunnerExecutionError(
                "tuning-response probe returned invalid JSON"
            ) from error
        if result.get("schema_version") != "pitgun.racing-tuning-response-probe/v1":
            raise RunnerExecutionError(
                "tuning-response probe returned an unsupported contract"
            )

        return {
            "schema_version": TUNING_RESPONSE_RESULT_VERSION,
            "adapter": {
                "version": importlib.metadata.version("pitgun-databricks-adapter")
            },
            "runner_artifact": {
                "kind": "tuning_response_probe",
                "target": RUNNER_TARGET,
                "digest": _sha256(probe_bytes),
            },
            "host": {"machine": platform.machine(), "system": platform.system()},
            "measurements": {"execution_duration_ms": execution_duration_ms},
            "canonical_result_digest": _sha256(process.stdout),
            "result": result,
        }


def _execute_v3_validation_probe(
    probe_bytes: bytes,
    scenario_bytes: bytes,
    profile_bytes: bytes,
    seed: int,
) -> dict[str, Any]:
    if not 0 <= seed <= 2**64 - 1:
        raise ValueError("seed must be an unsigned 64-bit integer")

    with tempfile.TemporaryDirectory(prefix="pitgun-v3-validation-") as temporary:
        root = pathlib.Path(temporary)
        probe = root / "v3_validation_probe"
        scenario = root / "scenario.json"
        profile = root / "profile.json"
        probe.write_bytes(probe_bytes)
        probe.chmod(0o500)
        scenario.write_bytes(scenario_bytes)
        profile.write_bytes(profile_bytes)
        process, execution_duration_ms = _run(
            [str(probe), str(scenario), str(profile), str(seed)]
        )
        if process.returncode != 0:
            diagnostic = process.stderr.decode("utf-8", errors="replace").strip()
            raise RunnerExecutionError(
                f"packaged V3 validation probe failed with exit {process.returncode}: {diagnostic}"
            )
        if process.stderr:
            raise RunnerExecutionError(
                "successful V3 validation probe wrote unexpected stderr"
            )
        try:
            result = json.loads(process.stdout)
        except (UnicodeDecodeError, json.JSONDecodeError) as error:
            raise RunnerExecutionError("V3 validation probe returned invalid JSON") from error
        if result.get("schema_version") != "pitgun.racing-v3-validation-probe/v1":
            raise RunnerExecutionError(
                "V3 validation probe returned an unsupported contract"
            )

        return {
            "schema_version": V3_VALIDATION_RESULT_VERSION,
            "adapter": {
                "version": importlib.metadata.version("pitgun-databricks-adapter")
            },
            "runner_artifact": {
                "kind": "v3_validation_probe",
                "target": RUNNER_TARGET,
                "digest": _sha256(probe_bytes),
            },
            "host": {"machine": platform.machine(), "system": platform.system()},
            "measurements": {"execution_duration_ms": execution_duration_ms},
            "canonical_result_digest": _sha256(process.stdout),
            "result": result,
        }


def _execute_v3_decision_surface_probe(
    probe_bytes: bytes,
    scenario_bytes: bytes,
    profile_bytes: bytes,
    seed: int,
) -> dict[str, Any]:
    if not 0 <= seed <= 2**64 - 1:
        raise ValueError("seed must be an unsigned 64-bit integer")

    probe_digest = _sha256(probe_bytes)
    with _DECISION_SURFACE_PROBE_LOCK:
        probe = _DECISION_SURFACE_PROBES.get(probe_digest)
        if probe is None:
            probe = _DECISION_SURFACE_PROBE_ROOT / probe_digest.removeprefix("sha256:")
            probe.write_bytes(probe_bytes)
            probe.chmod(0o500)
            _DECISION_SURFACE_PROBES[probe_digest] = probe

    with tempfile.TemporaryDirectory(prefix="pitgun-v3-decision-surface-") as temporary:
        root = pathlib.Path(temporary)
        scenario = root / "scenario.json"
        profile = root / "profile.json"
        scenario.write_bytes(scenario_bytes)
        profile.write_bytes(profile_bytes)
        process, execution_duration_ms = _run(
            [str(probe), str(scenario), str(profile), str(seed)]
        )
        if process.returncode != 0:
            diagnostic = process.stderr.decode("utf-8", errors="replace").strip()
            raise RunnerExecutionError(
                "packaged V3 decision-surface probe failed with exit "
                f"{process.returncode}: {diagnostic}"
            )
        if process.stderr:
            raise RunnerExecutionError(
                "successful V3 decision-surface probe wrote unexpected stderr"
            )
        try:
            result = json.loads(process.stdout)
        except (UnicodeDecodeError, json.JSONDecodeError) as error:
            raise RunnerExecutionError(
                "V3 decision-surface probe returned invalid JSON"
            ) from error
        if (
            result.get("schema_version")
            != "pitgun.racing-v3-decision-surface-probe/v1"
        ):
            raise RunnerExecutionError(
                "V3 decision-surface probe returned an unsupported contract"
            )
        return {
            "schema_version": V3_DECISION_SURFACE_RESULT_VERSION,
            "adapter": {
                "version": importlib.metadata.version("pitgun-databricks-adapter")
            },
            "runner_artifact": {
                "kind": "v3_decision_surface_probe",
                "target": RUNNER_TARGET,
                "digest": _sha256(probe_bytes),
            },
            "host": {"machine": platform.machine(), "system": platform.system()},
            "measurements": {"execution_duration_ms": execution_duration_ms},
            "canonical_result_digest": _sha256(process.stdout),
            "result": result,
        }


def _execute_v3_driver_control_probe(
    probe_bytes: bytes,
    scenario_bytes: bytes,
    profile_bytes: bytes,
    driver_experiment_bytes: bytes,
    seed: int,
) -> dict[str, Any]:
    if not 0 <= seed <= 2**64 - 1:
        raise ValueError("seed must be an unsigned 64-bit integer")
    with tempfile.TemporaryDirectory(prefix="pitgun-v3-driver-control-") as temporary:
        root = pathlib.Path(temporary)
        probe = root / "v3_driver_control_probe"
        scenario = root / "scenario.json"
        profile = root / "profile.json"
        driver_experiment = root / "driver-experiment.json"
        probe.write_bytes(probe_bytes)
        probe.chmod(0o500)
        scenario.write_bytes(scenario_bytes)
        profile.write_bytes(profile_bytes)
        driver_experiment.write_bytes(driver_experiment_bytes)
        process, execution_duration_ms = _run(
            [
                str(probe), str(scenario), str(profile),
                str(driver_experiment), str(seed),
            ]
        )
        if process.returncode != 0:
            diagnostic = process.stderr.decode("utf-8", errors="replace").strip()
            raise RunnerExecutionError(
                "packaged V3 driver-control probe failed with exit "
                f"{process.returncode}: {diagnostic}"
            )
        if process.stderr:
            raise RunnerExecutionError(
                "successful V3 driver-control probe wrote unexpected stderr"
            )
        try:
            result = json.loads(process.stdout)
        except (UnicodeDecodeError, json.JSONDecodeError) as error:
            raise RunnerExecutionError(
                "V3 driver-control probe returned invalid JSON"
            ) from error
        if result.get("schema_version") != "pitgun.racing-v3-driver-control-probe/v1":
            raise RunnerExecutionError(
                "V3 driver-control probe returned an unsupported contract"
            )
        return {
            "schema_version": "pitgun.databricks-v3-driver-control-result/v1",
            "adapter": {
                "version": importlib.metadata.version("pitgun-databricks-adapter")
            },
            "runner_artifact": {
                "kind": "v3_driver_control_probe",
                "target": RUNNER_TARGET,
                "digest": _sha256(probe_bytes),
            },
            "host": {"machine": platform.machine(), "system": platform.system()},
            "measurements": {"execution_duration_ms": execution_duration_ms},
            "canonical_result_digest": _sha256(process.stdout),
            "result": result,
        }


def execute_packaged_racing(
    seed: int = 42, configuration_family: str = "balanced"
) -> dict[str, Any]:
    """Execute one allowlisted scenario and the native binary embedded in this wheel."""

    if configuration_family not in SCENARIO_FAMILIES:
        allowed = ", ".join(sorted(SCENARIO_FAMILIES))
        raise ValueError(
            f"unsupported configuration family; expected one of: {allowed}"
        )

    package = importlib.resources.files("pitgun_databricks_adapter")
    runner_bytes = package.joinpath("bin", "pitgun").read_bytes()
    scenario_bytes = package.joinpath(
        "scenarios", f"{configuration_family}.json"
    ).read_bytes()
    return _execute(runner_bytes, scenario_bytes, seed)


def execute_packaged_racing_scenario(
    seed: int, scenario_resource: str
) -> dict[str, Any]:
    """Execute one exact scenario resource embedded in the reviewed wheel."""

    if not SCENARIO_RESOURCE_PATTERN.fullmatch(scenario_resource):
        raise ValueError("scenario resource must be one canonical packaged identifier")

    package = importlib.resources.files("pitgun_databricks_adapter")
    scenario = package.joinpath("scenarios", f"{scenario_resource}.json")
    if not scenario.is_file():
        raise ValueError("scenario resource is not packaged or allowlisted")
    runner_bytes = package.joinpath("bin", "pitgun").read_bytes()
    return _execute(runner_bytes, scenario.read_bytes(), seed)


def _read_packaged_tree(root: Any) -> dict[str, bytes]:
    if not root.is_dir():
        raise ValueError("catalog resource is not packaged or allowlisted")
    files: dict[str, bytes] = {}

    def visit(directory: Any, prefix: str = "") -> None:
        for entry in sorted(directory.iterdir(), key=lambda item: item.name):
            relative = f"{prefix}/{entry.name}" if prefix else entry.name
            if entry.is_dir():
                visit(entry, relative)
            elif entry.is_file():
                files[relative] = entry.read_bytes()

    visit(root)
    if not files:
        raise ValueError("catalog resource is empty")
    return files


def execute_packaged_racing_catalog_scenario(
    seed: int,
    scenario_resource: str,
    catalog_resource: str = "racing-v1-2-0",
) -> dict[str, Any]:
    """Execute one reviewed scenario against one packaged immutable catalog."""

    if not SCENARIO_RESOURCE_PATTERN.fullmatch(scenario_resource):
        raise ValueError("scenario resource must be one canonical packaged identifier")
    if not CATALOG_RESOURCE_PATTERN.fullmatch(catalog_resource):
        raise ValueError("catalog resource must be one canonical packaged identifier")
    package = importlib.resources.files("pitgun_databricks_adapter")
    scenario = package.joinpath("scenarios", f"{scenario_resource}.json")
    if not scenario.is_file():
        raise ValueError("scenario resource is not packaged or allowlisted")
    catalog_files = _read_packaged_tree(package.joinpath("catalogs", catalog_resource))
    runner_bytes = package.joinpath("bin", "pitgun").read_bytes()
    return _execute(runner_bytes, scenario.read_bytes(), seed, catalog_files)


def inspect_packaged_runner() -> dict[str, str | int]:
    """Return the exact embedded runner identity without executing a scenario."""

    package = importlib.resources.files("pitgun_databricks_adapter")
    runner_bytes = package.joinpath("bin", "pitgun").read_bytes()
    with tempfile.TemporaryDirectory(prefix="pitgun-runner-inspect-") as temporary:
        runner = pathlib.Path(temporary) / "pitgun"
        runner.write_bytes(runner_bytes)
        runner.chmod(0o500)
        process, duration_ms = _run([str(runner), "--version"])
        if process.returncode != 0:
            raise RunnerExecutionError("packaged runner identity probe failed")
        return {
            "version": process.stdout.decode("utf-8").strip(),
            "target": RUNNER_TARGET,
            "digest": _sha256(runner_bytes),
            "startup_probe_duration_ms": duration_ms,
        }


def execute_packaged_tuning_response(
    seed: int, scenario_resource: str, response_resource: str
) -> dict[str, Any]:
    """Run one reviewed scenario with one reviewed experimental response."""

    if not SCENARIO_RESOURCE_PATTERN.fullmatch(scenario_resource):
        raise ValueError("scenario resource must be one canonical packaged identifier")
    if not RESPONSE_RESOURCE_PATTERN.fullmatch(response_resource):
        raise ValueError("response resource must be one canonical packaged identifier")
    package = importlib.resources.files("pitgun_databricks_adapter")
    scenario = package.joinpath("scenarios", f"{scenario_resource}.json")
    response = package.joinpath("responses", f"{response_resource}.json")
    if not scenario.is_file() or not response.is_file():
        raise ValueError("scenario or tuning response is not packaged or allowlisted")
    probe_bytes = package.joinpath("bin", "tuning_response_probe").read_bytes()
    return _execute_tuning_response_probe(
        probe_bytes, scenario.read_bytes(), response.read_bytes(), seed
    )


def inspect_packaged_tuning_response_probe() -> dict[str, str]:
    """Return the exact experimental probe identity without executing it."""

    package = importlib.resources.files("pitgun_databricks_adapter")
    probe_bytes = package.joinpath("bin", "tuning_response_probe").read_bytes()
    return {"target": RUNNER_TARGET, "digest": _sha256(probe_bytes)}


def execute_packaged_v3_tire_degradation(
    configuration_id: str,
    campaign_name: str = "racing-v3-tire-degradation-v1",
) -> dict[str, Any]:
    """Execute one exact V3 scenario/profile pair from the reviewed campaign."""

    from .tire_degradation import (
        load_tire_degradation_campaign,
        materialize_tire_degradation_plan,
    )

    if not V3_CONFIGURATION_PATTERN.fullmatch(configuration_id):
        raise ValueError("configuration id must be one canonical packaged identifier")
    manifest, _ = load_tire_degradation_campaign(campaign_name)
    configurations = {
        row["id"]: row for row in materialize_tire_degradation_plan(manifest)
    }
    try:
        configuration = configurations[configuration_id]
    except KeyError as error:
        raise ValueError("configuration is not packaged or allowlisted") from error

    package = importlib.resources.files("pitgun_databricks_adapter")
    probe_bytes = package.joinpath("bin", "v3_validation_probe").read_bytes()
    scenario_bytes = json.dumps(
        configuration["scenario"], indent=2, ensure_ascii=False
    ).encode() + b"\n"
    profile_bytes = json.dumps(
        configuration["profile"], indent=2, ensure_ascii=False
    ).encode() + b"\n"
    return _execute_v3_validation_probe(
        probe_bytes, scenario_bytes, profile_bytes, int(configuration["seed"])
    )


def inspect_packaged_v3_validation_probe() -> dict[str, str]:
    """Return the exact V3 validation probe identity without executing it."""

    package = importlib.resources.files("pitgun_databricks_adapter")
    probe_bytes = package.joinpath("bin", "v3_validation_probe").read_bytes()
    return {"target": RUNNER_TARGET, "digest": _sha256(probe_bytes)}


def execute_packaged_v3_decision_surface(
    execution_key: str,
    campaign_name: str = "racing-v3-decision-surface-v2",
) -> dict[str, Any]:
    """Execute one exact scenario/profile/seed from the reviewed campaign."""

    from .decision_surface import (
        load_decision_surface_execution,
    )

    if not V3_DECISION_SURFACE_EXECUTION_PATTERN.fullmatch(execution_key):
        raise ValueError("execution key must be one canonical packaged identifier")
    configuration = load_decision_surface_execution(execution_key, campaign_name)

    package = importlib.resources.files("pitgun_databricks_adapter")
    probe_bytes = package.joinpath("bin", "v3_decision_surface_probe").read_bytes()
    scenario_bytes = json.dumps(
        configuration["scenario"], indent=2, ensure_ascii=False
    ).encode() + b"\n"
    profile_bytes = json.dumps(
        configuration["profile"], indent=2, ensure_ascii=False
    ).encode() + b"\n"
    return _execute_v3_decision_surface_probe(
        probe_bytes,
        scenario_bytes,
        profile_bytes,
        int(configuration["seed"]),
    )


def execute_packaged_v3_thermal_surface(
    execution_key: str,
    campaign_name: str = "racing-v3-thermal-adequacy-v1",
) -> dict[str, Any]:
    """Execute one exact thermal scenario/profile/seed from the reviewed campaign."""

    from .thermal_surface import load_thermal_surface_execution

    if not V3_THERMAL_SURFACE_EXECUTION_PATTERN.fullmatch(execution_key):
        raise ValueError("thermal execution key must be one canonical identifier")
    configuration = load_thermal_surface_execution(execution_key, campaign_name)
    package = importlib.resources.files("pitgun_databricks_adapter")
    probe_bytes = package.joinpath("bin", "v3_decision_surface_probe").read_bytes()
    scenario_bytes = json.dumps(
        configuration["scenario"], indent=2, ensure_ascii=False
    ).encode() + b"\n"
    profile_bytes = json.dumps(
        configuration["profile"], indent=2, ensure_ascii=False
    ).encode() + b"\n"
    return _execute_v3_decision_surface_probe(
        probe_bytes,
        scenario_bytes,
        profile_bytes,
        int(configuration["seed"]),
    )


def inspect_packaged_v3_decision_surface_probe() -> dict[str, str]:
    """Return the exact packaged decision-surface probe identity."""

    package = importlib.resources.files("pitgun_databricks_adapter")
    probe_bytes = package.joinpath("bin", "v3_decision_surface_probe").read_bytes()
    return {"target": RUNNER_TARGET, "digest": _sha256(probe_bytes)}


def execute_packaged_v3_driver_control(
    execution_key: str,
    campaign_name: str = "racing-v3-driver-control-surface-v2",
) -> dict[str, Any]:
    """Execute one exact governed driver-control surface point."""

    from .driver_control import load_driver_control_execution

    if not V3_DRIVER_CONTROL_EXECUTION_PATTERN.fullmatch(execution_key):
        raise ValueError("driver-control execution key must be canonical")
    configuration = load_driver_control_execution(execution_key, campaign_name)
    package = importlib.resources.files("pitgun_databricks_adapter")
    probe_bytes = package.joinpath("bin", "v3_driver_control_probe").read_bytes()
    encode = lambda value: json.dumps(  # noqa: E731
        value, indent=2, ensure_ascii=False
    ).encode() + b"\n"
    return _execute_v3_driver_control_probe(
        probe_bytes,
        encode(configuration["scenario"]),
        encode(configuration["profile"]),
        encode(configuration["driver_experiment"]),
        int(configuration["seed"]),
    )


def inspect_packaged_v3_driver_control_probe() -> dict[str, str]:
    """Return the exact packaged driver-control probe identity."""

    package = importlib.resources.files("pitgun_databricks_adapter")
    probe_bytes = package.joinpath("bin", "v3_driver_control_probe").read_bytes()
    return {"target": RUNNER_TARGET, "digest": _sha256(probe_bytes)}


def validate_packaged_v3_driver_control_profiles(
    campaign_name: str = "racing-v3-driver-control-surface-v2",
) -> dict[str, Any]:
    """Validate every packaged unique profile with the authoritative Rust contract."""

    from .driver_control import load_driver_control_campaign

    manifest, manifest_digest = load_driver_control_campaign(campaign_name)
    package = importlib.resources.files("pitgun_databricks_adapter")
    probe_bytes = package.joinpath("bin", "v3_driver_control_probe").read_bytes()
    validated = []
    with tempfile.TemporaryDirectory(prefix="pitgun-v3-driver-preflight-") as temporary:
        root = pathlib.Path(temporary)
        probe = root / "v3_driver_control_probe"
        probe.write_bytes(probe_bytes)
        probe.chmod(0o500)
        for index, (profile_ref, profile) in enumerate(sorted(manifest["profiles"].items())):
            profile_path = root / f"profile-{index:02d}.json"
            profile_path.write_text(json.dumps(profile, indent=2, ensure_ascii=False) + "\n")
            process, _ = _run([str(probe), "--validate-profile", str(profile_path)])
            if process.returncode != 0:
                diagnostic = process.stderr.decode("utf-8", errors="replace").strip()
                raise RunnerExecutionError(
                    f"packaged driver-control profile {profile_ref} violates the Rust contract: {diagnostic}"
                )
            if process.stderr:
                raise RunnerExecutionError(
                    f"successful driver-control profile validation wrote stderr: {profile_ref}"
                )
            try:
                result = json.loads(process.stdout)
            except (UnicodeDecodeError, json.JSONDecodeError) as error:
                raise RunnerExecutionError("profile preflight returned invalid JSON") from error
            if (
                result.get("schema_version")
                != "pitgun.racing-v3-driver-control-profile-validation/v1"
                or result.get("model") != manifest["model"]
            ):
                raise RunnerExecutionError("profile preflight returned an invalid identity")
            validated.append(result["profile_digest"])
    return {
        "campaign_name": campaign_name,
        "manifest_digest": manifest_digest,
        "validated_profile_count": len(validated),
        "validated_profile_digests": validated,
        "probe_artifact_digest": _sha256(probe_bytes),
    }
