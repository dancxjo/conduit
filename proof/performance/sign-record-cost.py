#!/usr/bin/env python3
"""Source-pinned observations of the real native FixedSignLog record path."""
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
TEST = "observe_production_sign_record_cost"


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
    env.update(CARGO_INCREMENTAL="0", CARGO_BUILD_JOBS="2",
               CARGO_PROFILE_RELEASE_OPT_LEVEL="3", CARGO_PROFILE_RELEASE_DEBUG="0",
               CARGO_PROFILE_RELEASE_OVERFLOW_CHECKS="true",
               CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS="false",
               RUSTUP_TOOLCHAIN=tomllib.loads((ROOT / "rust-toolchain.toml").read_text())["toolchain"]["channel"])
    build_command = ["cargo", "test", "--locked", "--release", "-p", "conduit-kernel",
               "--test", "sign_record_cost", "--no-run", "--message-format=json"]
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
            if "SIGN_RECORD_COST_ROW " in line:
                rows.append(json.loads(line.split("SIGN_RECORD_COST_ROW ", 1)[1]))
        expected = {(capacity, trial) for capacity in [1, 8, 32, 128, 1024] for trial in range(32)}
        if len(rows) != 160 or {(r["capacity"], r["trial"]) for r in rows} != expected:
            raise RuntimeError("missing, duplicate or foreign observations")
        for row in rows:
            if (row["records_per_phase"] != row["capacity"]
                    or row["retained_event_bytes"] != row["capacity"] * row["event_size"]):
                raise RuntimeError("record/storage extent changed")
            records.append({"process": process, **row})
    custody(source)
    if digest(executable) != executable_hash:
        raise RuntimeError("executable changed after capture")
    summaries = []
    for capacity in [1, 8, 32, 128, 1024]:
        for phase in ["prepare_ns", "append_ns", "full_retention_ns"]:
            means = [statistics.mean(r[phase] for r in records
                                     if r["capacity"] == capacity and r["process"] == process)
                     for process in range(3)]
            summaries.append({"capacity": capacity, "phase": phase, "process_means_ns": means,
                              "mean_ns": statistics.mean(means), "process_mean_sample_sd_ns": statistics.stdev(means)})
    write(output / "observations.json", {"rows": records, "summaries": summaries})
    write(output / "metadata.json", {"source": source, "executable": str(executable),
          "executable_sha256": executable_hash, "system": platform.platform(),
          "build_command": build_command, "processes": processes,
          "cpuinfo": Path("/proc/cpuinfo").read_text(), "affinity": sorted(os.sched_getaffinity(0)),
          "rustc": subprocess.check_output(["rustc", "-Vv"], env=env, text=True),
          "profile": {k: v for k, v in env.items() if k.startswith("CARGO_PROFILE_RELEASE_")},
          "scope": "Production native FixedSignLog record-call batches; ordinary append and full transient retention measured separately",
          "limits": ["loop and black-box barriers included; no timer subtraction", "one-record batches are timer-sensitive",
                     "not causal scheduler CPU share, compiler-copy counting, browser or whole-Host cost",
                     "storage extent is retained event bytes, not total memory traffic",
                     "shared development host; concurrent work may affect wall times; load averages retained", "no automatic timing threshold or speedup claim"]})
    write(output / "sha256.json", {p.name: digest(p) for p in sorted(output.iterdir()) if p.is_file()})
    print(output)


if __name__ == "__main__":
    main()
