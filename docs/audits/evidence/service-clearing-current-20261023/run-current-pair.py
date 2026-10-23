#!/usr/bin/env python3
"""Internal one-attempt batch; every proof enters through supported cargo xtask."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import time


def output(argv, cwd):
    return subprocess.check_output(argv, cwd=cwd, text=True).strip()


def digest(path):
    value = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            value.update(block)
    return value.hexdigest()


def clean_head(path):
    if output(["git", "status", "--porcelain=v1", "--untracked-files=all"], path):
        raise RuntimeError(f"source is dirty: {path}")
    return output(["git", "rev-parse", "HEAD"], path)


parser = argparse.ArgumentParser(description=__doc__)
for name in ("baseline", "optimized", "output", "cache"):
    parser.add_argument("--" + name, type=Path, required=True)
args = parser.parse_args()
roots = {name: getattr(args, name).resolve() for name in ("baseline", "optimized")}
destination = args.output.resolve()
destination.mkdir(parents=True, exist_ok=False)
state = {"schema": "conduit.6183/current-pair@1", "status": "preflight",
         "attempts_per_command": 1, "commands": [], "proof_acceptance": "pending receipt review",
         "scope": "Current-source protection and ordinary workload comparison; not full #6183 completion.",
         "limits": "Uncalibrated emulator observations; build/host caches and arm order are uncontrolled. "
                   "Keep first versus repeated processes separate; no pure trap/component or physical speed claim."}


def save():
    (destination / "batch.json").write_text(json.dumps(state, indent=2) + "\n")


try:
    heads = {name: clean_head(root) for name, root in roots.items()}
    state["sources"] = heads
    gate = json.loads(output(["gh", "pr", "view", "6268", "--repo", "dancxjo/conduit",
                              "--json", "state,mergeCommit"], roots["baseline"]))
    state["repair_gate"] = gate
    if gate["state"] != "MERGED":
        raise RuntimeError("#6268 repair is not merged; no proof attempt started")
    subprocess.run(["git", "merge-base", "--is-ancestor", gate["mergeCommit"]["oid"],
                    heads["baseline"]], cwd=roots["baseline"], check=True)
    parent = output(["git", "rev-parse", "HEAD^"], roots["optimized"])
    if parent != heads["baseline"]:
        raise RuntimeError("optimized source must be a direct child of the common accepted baseline")
    changed = output(["git", "diff", "--name-only", heads["baseline"], heads["optimized"]],
                     roots["optimized"]).splitlines()
    expected = ["targets/conduitos/src/arch/x86_64/service_memory.rs",
                "targets/conduitos/src/arch/x86_64/service_memory/storage.rs",
                "targets/conduitos/tests/service_slot_lifecycle.rs"]
    if sorted(changed) != sorted(expected):
        raise RuntimeError(f"unexpected comparison delta: {changed}")
    state["changed_paths"] = changed
    state["tool_versions"] = {
        "rustc": output(["rustc", "+stable", "--version", "--verbose"], roots["baseline"]),
        "qemu": output(["qemu-system-x86_64", "--version"], roots["baseline"]),
    }
    steps = [("protection", ["ordinary-domain-proof"]),
             ("latency", ["native-latency-proof", "--trials", "3", "--cache-mode", "retained"]),
             ("canonical", ["graphical-profile-proof"])]
    for arm, root in roots.items():
        for step, suffix in steps:
            if clean_head(root) != heads[arm]:
                raise RuntimeError("source changed during the batch")
            free = shutil.disk_usage(args.cache.resolve().parent).free
            if free < 15 * 1024**3:
                raise RuntimeError("less than 15 GiB cache-filesystem headroom; inspect ownership/cleanup before continuing")
            folder = destination / arm / step
            folder.mkdir(parents=True)
            environment = os.environ.copy()
            controlled = {"CARGO_TARGET_DIR": str(args.cache.resolve()), "CARGO_BUILD_JOBS": "2",
                          "CARGO_INCREMENTAL": "0", "CARGO_PROFILE_DEV_DEBUG": "0",
                          "CARGO_PROFILE_TEST_DEBUG": "0",
                          "CONDUIT_CONDUITOS_TARGET_ROOT": str(folder / "native")}
            environment.update(controlled)
            command = ["cargo", "+stable", "xtask", "make", "conduitos", *suffix, "--locked"]
            record = {"arm": arm, "step": step, "source": heads[arm], "command": command,
                      "environment": controlled, "status": "running", "started_unix": time.time()}
            state["commands"].append(record)
            state["status"] = "running"
            save()
            started = time.monotonic()
            with (folder / "first.log").open("wb") as stream:
                result = subprocess.run(command, cwd=root, env=environment, stdout=stream,
                                        stderr=subprocess.STDOUT)
            record.update(exit_code=result.returncode, seconds=time.monotonic() - started,
                          log_sha256=digest(folder / "first.log"), status="completed")
            if step == "canonical":
                live = root / "target/conduitos/live"
                if live.exists():
                    shutil.copytree(live, folder / "live-media")
            record["retained_files"] = [
                {"path": str(path.relative_to(folder)), "bytes": path.stat().st_size,
                 "sha256": digest(path)} for path in sorted(folder.rglob("*")) if path.is_file()]
            save()
            if clean_head(root) != heads[arm]:
                raise RuntimeError("source changed during a proof command")
            if result.returncode:
                raise RuntimeError(f"{arm}/{step} failed; retained first attempt, no retry")
    state["status"] = "commands_passed_pending_receipt_review"
except Exception as error:
    state["status"] = "stopped"
    state["reason"] = str(error)
    save()
    raise
save()
