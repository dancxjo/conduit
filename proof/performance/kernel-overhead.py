#!/usr/bin/env python3
"""Retain independent native runs of the actual instrumented fixed kernel."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import statistics
import subprocess
import time
import tomllib

ROOT = Path(__file__).resolve().parents[2]
TEST = "scheduler::scan_characterization::characterize_loaded_and_sparse_fixed_scheduler_scans"


def git(*args):
    return subprocess.check_output(["git", *args], cwd=ROOT, text=True).strip()


def write(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n")


def custody(source):
    if git("rev-parse", "HEAD") != source or git("status", "--porcelain"):
        raise RuntimeError("source changed; capture is not accepted")


def capture(output, runs, source):
    env = os.environ.copy()
    env.update(CARGO_BUILD_JOBS="2", CARGO_INCREMENTAL="0",
               CARGO_PROFILE_RELEASE_OPT_LEVEL="3", CARGO_PROFILE_RELEASE_DEBUG="0",
               CARGO_PROFILE_RELEASE_OVERFLOW_CHECKS="true",
               CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS="false",
               RUSTUP_TOOLCHAIN=tomllib.loads((ROOT / "rust-toolchain.toml").read_text())["toolchain"]["channel"])
    command = ["cargo", "test", "--locked", "--release", "-p", "conduit-kernel",
               "--features", "alloc,step-test-support", "--lib", "--no-run", "--message-format=json"]
    with (output / "build.jsonl").open("w") as stdout, (output / "build.stderr.log").open("w") as stderr:
        subprocess.run(command, cwd=ROOT, env=env, stdout=stdout, stderr=stderr, check=True)
    executables = [json.loads(line)["executable"] for line in (output / "build.jsonl").read_text().splitlines()
                   if line.startswith("{") and json.loads(line).get("executable")]
    if len(executables) != 1:
        raise RuntimeError("expected exactly one kernel test executable")
    executable = Path(executables[0])
    executable_digest = hashlib.sha256(executable.read_bytes()).hexdigest()
    custody(source)
    cpu = next((line.split(":", 1)[1].strip() for line in Path("/proc/cpuinfo").read_text().splitlines()
                if line.startswith("model name")), "unavailable")
    metadata = {"source": source, "build_command": command,
                "rustc": subprocess.check_output(["rustc", "-Vv"], env=env, text=True),
                "executable": str(executable), "executable_sha256": executable_digest,
                "system": platform.system(), "release": platform.release(), "machine": platform.machine(),
                "cpu_model": cpu, "affinity": sorted(os.sched_getaffinity(0)),
                "profile": {key: value for key, value in env.items() if key.startswith("CARGO_PROFILE_RELEASE_")},
                "scope": "test-only observations of production FixedScheduler/FixedValueStore; native Linux",
                "limits": ["instrumented Step wall time includes observation/harness work",
                           "copy bytes count successful fixed-value-store writes only",
                           "compiler moves, Sign copies and Host transport copies are unmeasured",
                           "test-only allocator/scan observations are absent from product builds",
                           "no browser/WASM attribution: native test allocator and observation seams are not installed in that product",
                           "no timing threshold, causal speedup, independent confidence interval or parent5395 completion"]}
    write(output / "metadata.json", metadata)
    combined = []
    processes = []
    for run in range(runs):
        custody(source)
        if hashlib.sha256(executable.read_bytes()).hexdigest() != executable_digest:
            raise RuntimeError("measurement executable changed")
        arguments = ["/usr/bin/time", "-v", "-o", str(output / f"process-{run}.resource.log"),
                     str(executable), TEST, "--exact", "--ignored", "--nocapture", "--test-threads=1"]
        started = time.perf_counter_ns()
        with (output / f"process-{run}.stdout.log").open("w") as stdout, (output / f"process-{run}.stderr.log").open("w") as stderr:
            subprocess.run(arguments, cwd=ROOT, env=env, stdout=stdout, stderr=stderr, check=True)
        processes.append({"run": run, "command": arguments,
                          "whole_process_wall_ns": time.perf_counter_ns() - started,
                          "boundary": "process startup, observed fixture, assertions and stdout; not Step CPU time"})
        rows = {prefix: [] for prefix in ["SCHEDULER_SCAN_ROW", "SCHEDULER_OVERHEAD_ROW", "SCHEDULER_REPLAY_ROW"]}
        for line in (output / f"process-{run}.stdout.log").read_text().splitlines():
            for prefix in rows:
                position = line.find(prefix + " ")
                if position >= 0:
                    rows[prefix].append(json.loads(line[position + len(prefix) + 1:]))
        if any(len(values) != 48 for values in rows.values()):
            raise RuntimeError("missing raw fixture observations")
        for scan, overhead in zip(rows["SCHEDULER_SCAN_ROW"], rows["SCHEDULER_OVERHEAD_ROW"]):
            keys = ("active_nodes", "sparse", "trial")
            if any(scan[key] != overhead[key] for key in keys):
                raise RuntimeError("foreign observation join")
            ready = 1 if scan["sparse"] else scan["active_nodes"]
            if (any(overhead[key] != 0 for key in ["ingress_allocations", "ingress_reallocations", "step_allocations", "step_reallocations", "step_value_store_copy_bytes"])
                    or overhead["ingress_value_store_copy_bytes"] != ready
                    or overhead["ingress_store_calls"] != ready or scan["steps"] != 2 * ready):
                raise RuntimeError("fixture allocation/copy/Step custody changed")
            combined.append({"process": run, "scan": scan, "overhead": overhead})
        write(output / f"process-{run}.replays.json", rows["SCHEDULER_REPLAY_ROW"])
    custody(source)
    summaries = []
    for active in [1, 8, 32]:
        for sparse in [False, True]:
            selected = [row for row in combined if row["scan"]["active_nodes"] == active and row["scan"]["sparse"] == sparse]
            means = [statistics.mean(row["scan"]["step_ns"] for row in selected if row["process"] == run) for run in range(runs)]
            summaries.append({"active_nodes": active, "sparse": sparse, "process_mean_step_ns": means,
                              "mean_of_process_means_ns": statistics.mean(means),
                              "sample_sd_of_process_means_ns": statistics.stdev(means),
                              "scope": "descriptive independent-process variation on one uncontrolled host"})
    write(output / "raw.json", combined)
    write(output / "processes.json", processes)
    write(output / "summary.json", summaries)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--runs", type=int, default=3)
    args = parser.parse_args()
    if not 3 <= args.runs <= 9:
        parser.error("runs must be between 3 and 9")
    source = git("rev-parse", "HEAD")
    custody(source)
    output = args.output.resolve()
    if output.is_relative_to(ROOT):
        relative = str(output.relative_to(ROOT))
        if subprocess.run(["git", "check-ignore", "-q", "--", relative], cwd=ROOT).returncode:
            parser.error("output inside source must be git-ignored")
    output.mkdir(parents=True, exist_ok=False)
    status = {"source": source, "success": False}
    try:
        capture(output, args.runs, source)
        status["success"] = True
    except Exception as error:
        status["failure"] = str(error)
        raise
    finally:
        status["artifacts_sha256"] = {str(path.relative_to(output)): hashlib.sha256(path.read_bytes()).hexdigest()
                                      for path in sorted(output.rglob("*")) if path.is_file()}
        write(output / "status.json", status)


if __name__ == "__main__":
    main()
