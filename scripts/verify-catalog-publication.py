#!/usr/bin/env python3
"""Protect immutable catalog releases and verify their public publication."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import sys
import time
from typing import Any
from urllib.error import HTTPError, URLError
from urllib.parse import urljoin
from urllib.request import Request, urlopen


DIGEST_PREFIX = "sha256:"


def canonical_json(value: Any) -> bytes:
    return json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
        allow_nan=False,
    ).encode("utf-8")


def digest_bytes(value: bytes) -> str:
    return DIGEST_PREFIX + hashlib.sha256(value).hexdigest()


def public_files(root: Path) -> list[Path]:
    return sorted(
        path
        for path in root.rglob("*")
        if path.is_file()
        and not any(
            part.startswith(".") for part in path.relative_to(root).parts
        )
    )


def release_roots(root: Path) -> list[Path]:
    return sorted(path.parent for path in root.glob("*/v*/catalog.json"))


def fetch(url: str, attempts: int = 5) -> tuple[bytes, dict[str, str]]:
    last_error: Exception | None = None
    for attempt in range(attempts):
        try:
            request = Request(url, headers={"Cache-Control": "no-cache"})
            with urlopen(request, timeout=20) as response:
                headers = {key.lower(): value for key, value in response.headers.items()}
                return response.read(), headers
        except (HTTPError, URLError, TimeoutError) as error:
            last_error = error
            if attempt + 1 < attempts:
                time.sleep(2)
    assert last_error is not None
    raise last_error


def assert_public_file(root: Path, base_url: str, path: Path) -> dict[str, str]:
    relative = path.relative_to(root).as_posix()
    actual, headers = fetch(urljoin(base_url, relative))
    expected = path.read_bytes()
    if actual != expected:
        raise ValueError(f"public bytes differ: {relative}")
    return headers


def preflight(root: Path, base_url: str, missing_output: Path) -> None:
    missing: list[str] = []
    for release_root in release_roots(root):
        manifest = release_root / "catalog.json"
        relative_manifest = manifest.relative_to(root).as_posix()
        try:
            fetch(urljoin(base_url, relative_manifest), attempts=1)
        except HTTPError as error:
            if error.code == 404:
                missing.append(release_root.relative_to(root).as_posix())
                continue
            raise

        for path in public_files(release_root):
            assert_public_file(root, base_url, path)
        print(f"OK immutable release unchanged: {release_root.relative_to(root)}")

    missing_output.write_text("".join(f"{path}\n" for path in missing), encoding="utf-8")
    print(f"OK preflight: {len(missing)} release(s) to publish")


def require_header(
    headers: dict[str, str], name: str, expected: str, path: str
) -> None:
    actual = headers.get(name, "")
    if expected.lower() not in actual.lower():
        raise ValueError(
            f"{path}: expected {name} to contain {expected!r}, got {actual!r}"
        )


def verify(root: Path, base_url: str) -> None:
    for path in public_files(root):
        assert_public_file(root, base_url, path)

    for latest_path in sorted(root.glob("*/latest.json")):
        relative = latest_path.relative_to(root).as_posix()
        latest_bytes, headers = fetch(urljoin(base_url, relative))
        require_header(headers, "content-type", "application/json", relative)
        require_header(headers, "access-control-allow-origin", "*", relative)
        require_header(headers, "cache-control", "no-cache", relative)

        pointer = json.loads(latest_bytes)
        domain_url = urljoin(base_url, f"{latest_path.parent.name}/")
        manifest_bytes, _ = fetch(urljoin(domain_url, pointer["manifest"]["path"]))
        manifest = json.loads(manifest_bytes)
        actual_digest = digest_bytes(canonical_json(manifest))
        if actual_digest != pointer["manifest"]["digest"]:
            raise ValueError(f"{relative}: manifest digest mismatch")

        release_bytes, _ = fetch(
            urljoin(domain_url, pointer["release_identity"]["path"])
        )
        release = json.loads(release_bytes)
        if release["manifest_digest"] != actual_digest:
            raise ValueError(f"{relative}: release identity mismatch")

    for release_root in release_roots(root):
        relative = (release_root / "catalog.json").relative_to(root).as_posix()
        _, headers = fetch(urljoin(base_url, relative))
        require_header(headers, "content-type", "application/json", relative)
        require_header(headers, "access-control-allow-origin", "*", relative)
        require_header(headers, "cache-control", "max-age=31536000", relative)
        require_header(headers, "cache-control", "immutable", relative)

    print(f"OK public catalog: {len(public_files(root))} exact file(s)")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--site", type=Path, required=True)
    parser.add_argument("--base-url", required=True)
    parser.add_argument("--mode", choices=("preflight", "verify"), required=True)
    parser.add_argument("--missing-output", type=Path)
    args = parser.parse_args()
    base_url = args.base_url.rstrip("/") + "/"
    try:
        if args.mode == "preflight":
            if args.missing_output is None:
                raise ValueError("--missing-output is required in preflight mode")
            preflight(args.site, base_url, args.missing_output)
        else:
            verify(args.site, base_url)
        return 0
    except (
        OSError,
        ValueError,
        KeyError,
        TypeError,
        json.JSONDecodeError,
        HTTPError,
        URLError,
    ) as error:
        print(f"FAIL catalog publication: {error}")
        return 1


if __name__ == "__main__":
    sys.exit(main())
