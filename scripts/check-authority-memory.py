#!/usr/bin/env python3
"""Exercise a disposable Authority under its production memory limit, never a live URL.

Only Docker's ephemeral localhost port is used. The signing secret is a public test
fixture. --expect-oom is for reproducing the old image, not a passing release gate.
"""
import argparse
from collections import Counter
from concurrent.futures import ThreadPoolExecutor
import hashlib
import hmac
import json
from pathlib import Path
import subprocess
import time
import urllib.error
import urllib.request
import uuid

SECRET = "authority-memory-regression-only"
ROOT = Path(__file__).resolve().parents[1]


def docker(*args):
    return subprocess.check_output(["docker", *args], text=True).strip()


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--image", required=True)
    parser.add_argument("--platform")
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--baseline-report", type=Path)
    parser.add_argument("--expect-oom", action="store_true")
    args = parser.parse_args()
    name = "authority-memory-" + uuid.uuid4().hex[:12]
    command = ["run", "-d", "--name", name, "--memory", "256m", "--memory-swap", "256m",
               "--cpus", "0.5", "--pids-limit", "128", "--read-only", "--cap-drop", "ALL",
               "--security-opt", "no-new-privileges", "-p", "127.0.0.1::8080",
               "-e", "PITGUN_SIGNING_SECRET=" + SECRET, "-e", "PITGUN_SIGNING_KEY_ID=memory-test",
               "-e", "PITGUN_RACING_MODEL_VERSION=0.14.0",
               "-e", "PITGUN_RACING_CATALOG_RELEASE_DIR=/opt/pitgun/catalogs/racing/v1.8.0"]
    if args.platform:
        command += ["--platform", args.platform]
    docker(*command, args.image)
    report = {"image": args.image, "memoryLimitBytes": 256 * 1024 * 1024,
              "cpus": 0.5, "concurrency": 8}
    try:
        for _ in range(120):
            if "listening" in docker("logs", name):
                break
            if not json.loads(docker("inspect", name))[0]["State"]["Running"]:
                raise AssertionError("Authority stopped before listening")
            time.sleep(0.5)
        else:
            raise AssertionError("Authority did not start within 60 seconds")
        port = json.loads(docker("inspect", name))[0]["NetworkSettings"]["Ports"]["8080/tcp"][0]["HostPort"]
        base = "http://127.0.0.1:" + port

        def memory():
            return {key: int(docker("exec", name, "cat", "/sys/fs/cgroup/memory." + key))
                    for key in ["current", "peak"]}

        def request(path, body=None):
            data = None if body is None else json.dumps(body).encode()
            req = urllib.request.Request(base + path, data=data, headers={"Content-Type": "application/json"})
            try:
                with urllib.request.urlopen(req, timeout=15) as response:
                    raw = response.read()
                    return response.status, json.loads(raw) if raw else None
            except urllib.error.HTTPError as error:
                return error.code, json.loads(error.read())
            except (OSError, TimeoutError) as error:
                return type(error).__name__, None

        report["startupMemory"] = memory()
        release = json.loads((ROOT / "catalogs/racing/v1.8.0/release.json").read_text())
        payload = {"subject": "memory-test", "seed": "42", "catalog_release": release,
                   "input": {"track_id": "MONZA", "laps": 53, "competitors": [{"id": "player", "driver_id": "default", "name": "Player",
                                              "team_id": "test", "is_player": True,
                                              "tuning": {"engine_points": 2, "cooling_points": 2,
                                                         "aero_points": 2, "chassis_points": 2,
                                                         "downforce_slider": 0.5, "gear_ratio_slider": 0.5},
                                              "budget_cap": 8}],
                             "vehicle_id": "f1_2026", "era": 2, "hz": 20}}
        attempt = {**payload, "execution_id": "018f3b78-7e9a-7d20-a5e1-4ed92f02a591"}
        endpoints = [("/v1/authorizations/racing", payload),
                     ("/v1/authorizations/racing/attempts", attempt)]

        def verify(body):
            signed = body["signed"]
            expected = hmac.new(SECRET.encode(), canonical(signed["authorization"]), hashlib.sha256).hexdigest()
            assert hmac.compare_digest(expected, signed["signature"]), "invalid authorization signature"
            # Issuance time and random nonce intentionally differ between requests.
            stable = json.loads(json.dumps(body))
            del stable["signed"]["signature"]
            del stable["signed"]["authorization"]["nonce"]
            del stable["signed"]["authorization"]["validity"]
            return stable

        reference = {}
        for path, body in endpoints:
            status, response = request(path, body)
            assert status == 200, (path, status, response)
            reference[path] = verify(response)
        report["authorizations"] = reference
        if args.baseline_report:
            previous = json.loads(args.baseline_report.read_text())
            assert previous["authorizations"] == reference, "canonical authorization changed"
            report["matchesBaselineAuthorizations"] = True

        with ThreadPoolExecutor(max_workers=8) as pool:
            statuses = list(pool.map(lambda n: request("/healthz" if n % 2 else "/readyz")[0], range(128)))
        report["healthResponses"] = dict(Counter(str(status) for status in statuses))
        state = json.loads(docker("inspect", name))[0]["State"]
        if not args.expect_oom:
            assert all(status == 200 for status in statuses), report["healthResponses"]

            def authorize(n):
                path, body = endpoints[n % 2]
                status, response = request(path, body)
                assert status == 200, (path, status, response)
                assert verify(response) == reference[path], "authorization changed under concurrency"
                return status

            with ThreadPoolExecutor(max_workers=8) as pool:
                assert list(pool.map(authorize, range(64))) == [200] * 64
            report["verifiedSignedResponses"] = 66
            invalid = {**attempt, "catalog_release": {**release, "manifest_digest": "sha256:" + "0" * 64}}
            report["invalidCatalogStatus"] = request(endpoints[1][0], invalid)[0]
            assert report["invalidCatalogStatus"] == 400
            report["finalMemory"] = memory()
            events = docker("exec", name, "cat", "/sys/fs/cgroup/memory.events")
            report["memoryEvents"] = dict(line.split() for line in events.splitlines())
            assert int(report["memoryEvents"]["oom_kill"]) == 0
            state = json.loads(docker("inspect", name))[0]["State"]
            assert state["Running"] and not state["OOMKilled"], state
        else:
            assert state["OOMKilled"] and state["ExitCode"] == 137, state
        report["outcome"] = "expected-oom" if args.expect_oom else "passed"
        report["containerState"] = {key: state[key] for key in ["Running", "OOMKilled", "ExitCode"]}
    finally:
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps(report, indent=2) + "\n")
        docker("rm", "-f", name)
    print(json.dumps({key: value for key, value in report.items() if key != "authorizations"}, indent=2))


if __name__ == "__main__":
    main()
