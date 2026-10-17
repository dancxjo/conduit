#!/usr/bin/env python3
"""Source-pinned observations of the real native SignSink capacity-reservation path."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import statistics
import subprocess
import tomllib

ROOT = Path(__file__).resolve().parents[2]
TEST = "observe_production_sign_capacity_cost"


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def custody(source):
    if subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip() != source:
        raise RuntimeError("source changed")
    if subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT, text=True).strip():
        raise RuntimeError("capture requires a committed clean source")


def write(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    output = args.output.resolve()
    source = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    custody(source)
    output.mkdir(parents=True, exist_ok=False)
    env = os.environ.copy()
    env.update(CARGO_INCREMENTAL="0", CARGO_BUILD_JOBS="2", CARGO_PROFILE_DEV_DEBUG="0",
               CARGO_PROFILE_TEST_OPT_LEVEL="1", CARGO_PROFILE_TEST_DEBUG="0",
               CARGO_PROFILE_TEST_OVERFLOW_CHECKS="true",
               CARGO_PROFILE_TEST_DEBUG_ASSERTIONS="true",
               RUSTUP_TOOLCHAIN=tomllib.loads((ROOT / "rust-toolchain.toml").read_text())["toolchain"]["channel"])
    build_command = ["cargo", "test", "--locked", "-p", "conduit-kernel",
               "--features", "alloc", "--test", "sign_capacity_cost", "--no-run", "--message-format=json"]
    with (output / "build.jsonl").open("w") as stdout, (output / "build.stderr.log").open("w") as stderr:
        subprocess.run(build_command, cwd=ROOT, env=env, stdout=stdout, stderr=stderr, check=True)
    executables = [row["executable"] for line in (output / "build.jsonl").read_text().splitlines()
                   if line.startswith("{") and (row := json.loads(line)).get("executable")]
    if len(executables) != 1:
        raise RuntimeError("expected one integration-test executable")
    executable = Path(executables[0])
    executable_hash = digest(executable)
    records = []
    processes = []
    for process in range(3):
        custody(source)
        if digest(executable) != executable_hash:
            raise RuntimeError("executable changed")
        command = ["/usr/bin/time", "-v", "-o", str(output / f"process-{process}.resource.log"),
                   str(executable), TEST, "--exact", "--ignored", "--nocapture", "--test-threads=1"]
        load_before = os.getloadavg()
        with (output / f"process-{process}.stdout.log").open("w") as stdout, (output / f"process-{process}.stderr.log").open("w") as stderr:
            subprocess.run(command, cwd=ROOT, env=env, stdout=stdout, stderr=stderr, check=True)
        processes.append({"process": process, "command": command,
                          "load_average_before": load_before, "load_average_after": os.getloadavg()})
        rows = []
        for line in (output / f"process-{process}.stdout.log").read_text().splitlines():
            if "SIGN_CAPACITY_COST_ROW " in line:
                rows.append(json.loads(line.split("SIGN_CAPACITY_COST_ROW ", 1)[1]))
        groups = [(sink, capacity, state) for sink, capacities in
                  [("fixed", [1, 8, 128, 4096]), ("hosted", [1, 8, 128, 4096, 60000])]
                  for capacity in capacities for state in
                  ["Empty", "Sparse", "HalfTransient", "FullTransient", "FullSticky"]]
        expected = {(sink, capacity, state, trial) for sink, capacity, state in groups for trial in range(8)}
        if len(rows) != 360 or {(r["sink"], r["capacity"], r["state"], r["trial"]) for r in rows} != expected:
            raise RuntimeError("missing, duplicate or foreign capacity observations")
        for row in rows:
            occupied = {"Empty": 0, "Sparse": 1, "HalfTransient": row["capacity"] // 2,
                        "FullTransient": row["capacity"], "FullSticky": row["capacity"]}[row["state"]]
            accepted = row["state"] != "FullSticky" and not (row["state"] == "Sparse" and row["capacity"] == 1)
            if row["calls"] != 256 or row["occupied"] != occupied or row["accepted"] != accepted:
                raise RuntimeError("capacity acceptance or occupancy changed")
            records.append({"process": process, **row})
    custody(source)
    if digest(executable) != executable_hash:
        raise RuntimeError("executable changed after capture")
    summaries = []
    for sink, capacity, state in groups:
        means = [statistics.mean(r["elapsed_ns"] for r in records
                                 if (r["sink"], r["capacity"], r["state"], r["process"]) ==
                                 (sink, capacity, state, process)) for process in range(3)]
        summaries.append({"sink": sink, "capacity": capacity, "state": state,
                          "calls_per_batch": 256, "process_mean_batch_ns": means,
                          "mean_batch_ns": statistics.mean(means),
                          "process_mean_sample_sd_ns": statistics.stdev(means)})
    write(output / "observations.json", {"rows": records, "summaries": summaries})
    write(output / "metadata.json", {"source": source, "executable": str(executable),
          "executable_sha256": executable_hash, "system": platform.platform(),
          "build_command": build_command, "processes": processes,
          "cpuinfo": Path("/proc/cpuinfo").read_text(), "affinity": sorted(os.sched_getaffinity(0)),
          "rustc": subprocess.check_output(["rustc", "-Vv"], env=env, text=True),
          "profile": {k: v for k, v in env.items() if k.startswith("CARGO_PROFILE_")},
          "scope": "Actual fixed/hosted SignSink ensure_capacity batches across occupancy/retention classes; preparation and snapshots excluded",
          "limits": ["loop and black-box barriers included; no timer subtraction", "batch results include loop, black-box and result-store work",
                     "not causal scheduler CPU share, compiler-copy counting, browser or whole-Host cost",
                     "storage extent is retained event bytes, not total memory traffic",
                     "shared development host; concurrent work may affect wall times; load averages retained", "no automatic timing threshold or speedup claim"]})
    write(output / "sha256.json", {p.name: digest(p) for p in sorted(output.iterdir()) if p.is_file()})
    print(output)


if __name__ == "__main__":
    main()
