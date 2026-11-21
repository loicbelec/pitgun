#!/usr/bin/env python3
"""Offline baseline of the existing career path. No cloud or external controller calls."""
from __future__ import annotations

import argparse
import concurrent.futures
import hashlib
import json
import math
import os
import pathlib
import platform
import statistics
import subprocess
import tempfile
import time

ROOT = pathlib.Path(__file__).resolve().parents[2]
HERE = pathlib.Path(__file__).resolve().parent


def digest(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


def encode(value: object) -> bytes:
    return (json.dumps(value, sort_keys=True, indent=2, allow_nan=False) + "\n").encode()


def load(path: pathlib.Path):
    def invalid(value):
        raise ValueError(f"non-finite JSON value: {value}")
    return json.loads(path.read_bytes(), parse_constant=invalid)


def physical_projection(result: dict) -> dict:
    """Exclude exported samples/hash only; retain native lap/state/diagnostic values."""
    return {k: v for k, v in result.items() if k not in ("exported_channels", "output_digest")}


def validate_plan(plan: dict) -> None:
    if plan.get("schema") != "pitgun.physics-baseline-plan/v1":
        raise ValueError("unsupported baseline plan")
    ids = [c["id"] for c in plan["cases"]]
    if not ids or len(ids) != len(set(ids)) or len(ids) > 32:
        raise ValueError("require1..32 uniquely named cases")
    for c in plan["cases"]:
        if not c["id"] or any(x not in "abcdefghijklmnopqrstuvwxyz0123456789-" for x in c["id"]):
            raise ValueError("case ID is not a safe filename")
        r = c["request"]
        i = r["input"]
        if not 1 <= i["laps"] <= 100 or not 1 <= len(i["competitors"]) <= 10:
            raise ValueError("case exceeds baseline bounds")
        if r["hz"] != i["hz"] or not math.isfinite(i["hz"]) or not 0 < i["hz"] <= 20:
            raise ValueError("sampling rates must agree and be within(0,20]")
    for a, b in plan.get("same_physics_pairs", []):
        if a not in ids or b not in ids or a == b:
            raise ValueError("unknown/repeated sampling comparison case")


def validate_probe_identity(data: dict, identity: dict) -> None:
    if data["native_binary_digest"] != identity["probe_digest"]:
        raise ValueError("probe executable digest changed during campaign")
    compiled = data["compiled_source_digests"]
    if not compiled or any(identity["source_files"].get(k) != v for k, v in compiled.items()):
        raise ValueError("probe contains stale core sources; rebuild it")


def run_process(command: list[str], timeout: float) -> dict:
    """One fresh Unix child; wait4 gives per-child CPU and lifetime maxRSS."""
    with tempfile.TemporaryFile() as out, tempfile.TemporaryFile() as err:
        start = time.monotonic()
        proc = subprocess.Popen(command, stdout=out, stderr=err, cwd=ROOT)
        timed_out = False
        while True:
            pid, status, usage = os.wait4(proc.pid, os.WNOHANG)
            if pid:
                break
            if time.monotonic() - start > timeout:
                timed_out = True
                try:
                    proc.kill()
                except ProcessLookupError:
                    pass  # It exited after the nonblocking wait; still reap our child.
                _, status, usage = os.wait4(proc.pid, 0)
                break
            time.sleep(0.01)
        proc.returncode = os.waitstatus_to_exitcode(status)
        out.seek(0)
        err.seek(0)
        stdout, stderr = out.read(), err.read()
    rss = usage.ru_maxrss if platform.system() == "Darwin" else usage.ru_maxrss * 1024
    return {
        "returncode": proc.returncode, "timed_out": timed_out,
        "wall_ms": (time.monotonic() - start) * 1000,
        "user_cpu_ms": usage.ru_utime * 1000, "system_cpu_ms": usage.ru_stime * 1000,
        "process_lifetime_max_rss_bytes": rss,
        "stdout": stdout, "stderr": stderr,
    }


def run_case(case, probe, catalog, repetitions, timeout, identity):
    runs, first = [], None
    with tempfile.TemporaryDirectory(prefix="pitgun-physics-baseline-") as tmp:
        request = pathlib.Path(tmp) / "request.json"
        request.write_bytes(encode(case["request"]))
        for repeat in range(repetitions):
            measured = run_process([str(probe), str(request), str(catalog)], timeout)
            raw, error = measured.pop("stdout"), measured.pop("stderr").decode(errors="replace")[:2000]
            if measured["returncode"] == 0:
                data = json.loads(raw)
                if data.get("schema") != "pitgun.physics-baseline-probe/v1":
                    raise ValueError("unexpected probe output")
                validate_probe_identity(data["identity"], identity)
                result = data["result"]
                encode(result)  # Reject non-finite results, including Python's permissive NaN parser.
                if len(result["lap_times_ms"]) != case["request"]["input"]["laps"]:
                    raise ValueError("incomplete player lap history")
                if any(s["status"] != {"type": "finished"} for s in result["standings"]):
                    raise ValueError("unexpected incomplete competitor")
                stable = {"status": "completed", "identity": data["identity"], "result": result}
                measured["probe_timings"] = data["timings"]
            elif not measured["timed_out"] and case.get("purpose") == "diagnostic-known-limitation" and "fuel depleted" in error:
                stable = {"status": "known-fuel-depletion", "error": error.strip()}
            else:
                raise RuntimeError(f"{case['id']} failed: timeout={measured['timed_out']} {error}")
            if first is None:
                first = stable
            elif encode(first) != encode(stable):
                raise ValueError(f"{case['id']}: repeated deterministic outputs differ")
            measured["repeat"] = repeat
            runs.append(measured)
    return {
        "case_id": case["id"], "request": case["request"], "outcome": first,
        "same_binary_repetition_passed": True, "measurements": runs,
        "median_wall_ms": statistics.median(x["wall_ms"] for x in runs),
        "maximum_process_rss_bytes": max(x["process_lifetime_max_rss_bytes"] for x in runs),
    }


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--probe", type=pathlib.Path, required=True)
    p.add_argument("--plan", type=pathlib.Path, default=HERE / "plan-v1.json")
    p.add_argument("--output", type=pathlib.Path, default=HERE / "results/local-baseline-v1.json")
    p.add_argument("--jobs", type=int, choices=range(1, 5), default=1)
    p.add_argument("--repetitions", type=int, choices=range(2, 4), default=2)
    p.add_argument("--timeout", type=float, default=180)
    p.add_argument("--check", action="store_true", help="Compare deterministic outcomes with an existing report; do not overwrite it")
    args = p.parse_args()
    if not math.isfinite(args.timeout) or not 0 < args.timeout <= 600:
        p.error("timeout must be within(0,600] seconds")
    if platform.system() not in ("Darwin", "Linux"):
        p.error("per-child resource measurement currently supports macOS/Linux")
    plan = load(args.plan)
    validate_plan(plan)
    probe = args.probe.resolve(strict=True)
    catalog = (ROOT / plan["catalog"]).resolve(strict=True)
    source_paths = [ROOT / "Cargo.lock"]
    for crate in ("pitgun-racing-solver", "pitgun-racing-simulator", "pitgun-racing-contract", "pitgun-contract"):
        source_paths.extend(sorted((ROOT / "crates" / crate).rglob("*.rs")))
    source_paths.extend(sorted(catalog.rglob("*.json")))
    source_hashes = {str(f.relative_to(ROOT)): digest(f.read_bytes()) for f in source_paths}
    identity = {"plan_digest": digest(args.plan.read_bytes()), "probe_digest": digest(probe.read_bytes()),
                "runner_digest": digest(pathlib.Path(__file__).read_bytes()), "source_files": source_hashes}
    started = time.monotonic()
    with concurrent.futures.ThreadPoolExecutor(max_workers=args.jobs) as pool:
        results = list(pool.map(lambda c: run_case(c, probe, catalog, args.repetitions, args.timeout, identity), plan["cases"]))
    by_id = {r["case_id"]: r for r in results}
    pairs = []
    for a, b in plan.get("same_physics_pairs", []):
        left, right = by_id[a]["outcome"], by_id[b]["outcome"]
        if left["status"] != "completed" or right["status"] != "completed":
            raise ValueError("sampling comparison requires completed cases")
        equal = physical_projection(left["result"]) == physical_projection(right["result"])
        counts = [o["result"]["exported_channels"]["5000"]["samples"] for o in (left, right)]
        pairs.append({"cases": [a, b], "same_native_physical_summary": equal,
                      "requested_hz": [by_id[x]["request"]["hz"] for x in (a, b)],
                      "exported_time_samples": counts,
                      "sampling_request_changes_export_count": counts[0] != counts[1]})
        if not equal:
            raise ValueError("export sampling changed native physical summary")
    report = {"schema": "pitgun.physics-local-baseline/v1", "passed": True, "identity": identity,
              "environment": {"system": platform.system(), "machine": platform.machine(), "python": platform.python_version(), "cpu_count": os.cpu_count(), "jobs": args.jobs},
              "campaign_wall_ms": (time.monotonic() - started) * 1000, "cases": results, "sampling_comparisons": pairs,
              "limitations": ["Native catalog-geometry anchors, not a reproduction of a saved browser career.", "Existing PowerUnitThermalV1/V9 core; no electrical store even for a hybrid-named vehicle.", "No absolute physical validation, portable native/WASM parity, controller comparison or hosted verdict.", "Advance timings include simulator orchestration and progress construction, not pure solver time.", "Per-child maxRSS includes initialization, native retained history and summary hashing; it is not browser RSS.", "Same-physics sampling check covers only declared pairs and deterministic summary fields.", "A known fuel-depletion diagnostic is a recorded failure, not a completed race."]}
    if args.check:
        old = load(args.output)
        if old["identity"] != identity or [r["outcome"] for r in old["cases"]] != [r["outcome"] for r in results]:
            raise ValueError("baseline identity or deterministic outcomes changed")
        print("PASS repeat baseline; measured timings are deliberately excluded from comparison")
    else:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        data = encode(report)
        temporary = args.output.with_suffix(".tmp")
        temporary.write_bytes(data)
        temporary.replace(args.output)
        args.output.with_suffix(".sha256").write_text(digest(data) + "\n")
        print(f"PASS {len(results)} cases x{args.repetitions} fresh native processes -> {args.output}")


if __name__ == "__main__":
    main()
