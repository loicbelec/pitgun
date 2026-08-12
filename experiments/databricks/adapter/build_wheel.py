#!/usr/bin/env python3
"""Build a deterministic platform wheel using only the Python standard library."""

from __future__ import annotations

import base64
import csv
import hashlib
import io
import pathlib
import subprocess
import zipfile


ROOT = pathlib.Path(__file__).resolve().parent
FRAMEWORK = ROOT.parents[2]
PACKAGE = "pitgun_databricks_adapter"
DISTRIBUTION = "pitgun_databricks_adapter"
BASE_VERSION = "0.2.0a1"
TAG = "py3-none-linux_aarch64"
TIMESTAMP = (2020, 1, 1, 0, 0, 0)


def digest_record(data: bytes) -> str:
    digest = base64.urlsafe_b64encode(hashlib.sha256(data).digest()).rstrip(b"=")
    return "sha256=" + digest.decode("ascii")


def git_revision() -> str:
    return subprocess.check_output(
        ["git", "rev-parse", "--short=12", "HEAD"], cwd=FRAMEWORK, text=True
    ).strip()


def wheel_entries(version: str) -> dict[str, bytes]:
    package_root = ROOT / PACKAGE
    runner = ROOT / "build" / "pitgun"
    scenario_roots = (
        FRAMEWORK / "apps" / "pitgun-cli" / "scenarios" / "racing-batch-v1",
        FRAMEWORK / "apps" / "pitgun-cli" / "scenarios" / "racing-circuit-sweep-v1",
    )
    campaigns = FRAMEWORK / "experiments" / "databricks" / "campaigns"
    if not runner.is_file():
        raise SystemExit(f"missing Linux runner: {runner}")

    dist_info = f"{DISTRIBUTION}-{version}.dist-info"
    entries = {
        f"{PACKAGE}/__init__.py": (package_root / "__init__.py").read_bytes(),
        f"{PACKAGE}/runner.py": (package_root / "runner.py").read_bytes(),
        f"{PACKAGE}/campaign.py": (package_root / "campaign.py").read_bytes(),
        f"{PACKAGE}/opponent_policy.py": (package_root / "opponent_policy.py").read_bytes(),
        f"{PACKAGE}/bin/pitgun": runner.read_bytes(),
        f"{dist_info}/METADATA": (
            "Metadata-Version: 2.1\n"
            "Name: pitgun-databricks-adapter\n"
            f"Version: {version}\n"
            "Summary: Bounded adapter for the packaged Pitgun Rust runner\n"
            "Requires-Python: >=3.10\n"
        ).encode(),
        f"{dist_info}/WHEEL": (
            "Wheel-Version: 1.0\n"
            "Generator: pitgun-build-wheel/v1\n"
            "Root-Is-Purelib: false\n"
            f"Tag: {TAG}\n"
        ).encode(),
    }
    for scenario_root in scenario_roots:
        for scenario in sorted(scenario_root.glob("*.json")):
            if f"{PACKAGE}/scenarios/{scenario.name}" in entries:
                raise SystemExit(f"duplicate packaged scenario name: {scenario.name}")
            entries[f"{PACKAGE}/scenarios/{scenario.name}"] = scenario.read_bytes()
    for campaign in sorted(campaigns.glob("racing-*")):
        if campaign.suffix not in {".json", ".sha256"}:
            continue
        entries[f"{PACKAGE}/campaigns/{campaign.name}"] = campaign.read_bytes()
    return entries


def main() -> None:
    revision = git_revision()
    version = f"{BASE_VERSION}+g{revision}"
    entries = wheel_entries(version)
    dist_info = f"{DISTRIBUTION}-{version}.dist-info"

    record = io.StringIO(newline="")
    writer = csv.writer(record, lineterminator="\n")
    for name, data in sorted(entries.items()):
        writer.writerow((name, digest_record(data), len(data)))
    record_name = f"{dist_info}/RECORD"
    writer.writerow((record_name, "", ""))
    entries[record_name] = record.getvalue().encode()

    destination = ROOT / "dist" / f"{DISTRIBUTION}-{version}-{TAG}.whl"
    destination.parent.mkdir(parents=True, exist_ok=True)
    for existing in destination.parent.glob("*.whl"):
        existing.unlink()

    with zipfile.ZipFile(destination, "w", compression=zipfile.ZIP_DEFLATED) as archive:
        for name, data in sorted(entries.items()):
            info = zipfile.ZipInfo(name, TIMESTAMP)
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = (0o755 if name.endswith("/pitgun") else 0o644) << 16
            archive.writestr(info, data)

    print(destination)


if __name__ == "__main__":
    main()
