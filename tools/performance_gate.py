#!/usr/bin/env python3
"""Repeatable report plus deterministic artifact-size regression gate."""

from __future__ import annotations

import argparse
import glob
import json
import math
import platform
import subprocess
import sys
import time
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
BASELINE = ROOT / "benchmarks" / "baseline-v1.json"


def run(command: list[str], *, capture: bool = False) -> str:
    result = subprocess.run(
        command,
        cwd=ROOT,
        check=True,
        text=True,
        stdout=subprocess.PIPE if capture else subprocess.DEVNULL,
        stderr=subprocess.PIPE if capture else subprocess.DEVNULL,
    )
    return result.stdout.strip() if capture else ""


def one_artifact(pattern: str) -> Path:
    matches = [Path(value) for value in glob.glob(str(ROOT / pattern))]
    if not matches:
        raise RuntimeError(f"missing built artifact: {pattern}")
    return max(matches, key=lambda path: path.stat().st_mtime_ns)


def build_artifacts() -> dict[str, Path]:
    run(["cargo", "build", "--release", "-p", "conduct", "-p", "conduit-core"])
    run(
        [
            "cargo",
            "build",
            "--release",
            "-p",
            "conduit-core",
            "--no-default-features",
            "--target",
            "thumbv6m-none-eabi",
        ]
    )
    return {
        "conduct-release": ROOT / "target/release/conduct",
        "conduit-core-release": one_artifact(
            "target/release/deps/libconduit_core-*.rlib"
        ),
        "conduit-core-thumbv6m-release": one_artifact(
            "target/thumbv6m-none-eabi/release/deps/libconduit_core-*.rlib"
        ),
    }


def host_metadata(baseline: dict) -> dict:
    rustc = run(["rustc", "-Vv"], capture=True)
    release = next(
        line.removeprefix("release: ")
        for line in rustc.splitlines()
        if line.startswith("release: ")
    )
    host = next(
        line.removeprefix("host: ")
        for line in rustc.splitlines()
        if line.startswith("host: ")
    )
    cpu = platform.processor()
    if not cpu and Path("/proc/cpuinfo").exists():
        for line in Path("/proc/cpuinfo").read_text(encoding="utf-8").splitlines():
            if line.lower().startswith("model name"):
                cpu = line.split(":", 1)[1].strip()
                break
    return {
        "commit": run(["git", "rev-parse", "HEAD"], capture=True),
        "rustc": release,
        "host_target": host,
        "os": platform.platform(),
        "machine": platform.machine(),
        "cpu": cpu or "unknown",
        "fixture_revision": baseline["fixture_revision"],
    }


def workload_report(workloads: list[dict]) -> list[dict]:
    # Exclude compilation from workload timing while retaining the exact
    # release-profile test binaries that each reviewed command executes.
    run(["cargo", "test", "--release", "--workspace", "--no-run"])
    report = []
    for workload in workloads:
        started = time.perf_counter_ns()
        run(workload["command"])
        report.append(
            {
                "id": workload["id"],
                "elapsed_ns": time.perf_counter_ns() - started,
                "gate": workload["timing"],
            }
        )
    return report


def check_sizes(baseline: dict, paths: dict[str, Path]) -> tuple[dict, list[str]]:
    measured = {}
    failures = []
    for artifact_id, policy in baseline["artifacts"].items():
        size = paths[artifact_id].stat().st_size
        baseline_size = policy["baseline_bytes"]
        percent_allowance = math.ceil(
            baseline_size * policy["maximum_growth_percent"] / 100
        )
        allowance = max(percent_allowance, policy["maximum_growth_bytes"])
        limit = baseline_size + allowance
        measured[artifact_id] = {
            "kind": policy["kind"],
            "path": str(paths[artifact_id].relative_to(ROOT)),
            "bytes": size,
            "baseline_bytes": baseline_size,
            "limit_bytes": limit,
        }
        if size > limit:
            failures.append(
                f"{artifact_id}: {size} bytes exceeds reviewed limit {limit}"
            )
    return measured, failures


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--update",
        action="store_true",
        help="replace artifact byte baselines after explicit review",
    )
    args = parser.parse_args()
    baseline = json.loads(BASELINE.read_text(encoding="utf-8"))
    paths = build_artifacts()
    measured, failures = check_sizes(baseline, paths)
    if args.update:
        for artifact_id, result in measured.items():
            baseline["artifacts"][artifact_id]["baseline_bytes"] = result["bytes"]
        baseline["reviewed_commit"] = run(
            ["git", "rev-parse", "HEAD"], capture=True
        )
        baseline["rustc"] = host_metadata(baseline)["rustc"]
        BASELINE.write_text(
            json.dumps(baseline, indent=2, sort_keys=False) + "\n",
            encoding="utf-8",
        )
        return 0
    report = {
        "schema": "conduit.performance-report/v1",
        "metadata": host_metadata(baseline),
        "artifacts": measured,
        "workloads": workload_report(baseline["workloads"]),
    }
    print(json.dumps(report, indent=2, sort_keys=True))
    for failure in failures:
        print(f"performance gate: {failure}", file=sys.stderr)
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
