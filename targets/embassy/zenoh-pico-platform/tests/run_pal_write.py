#!/usr/bin/env python3
"""CPU-test actual platform.c against effect fixtures; no download/network/device.
Usage: python3 tests/run_pal_write.py --upstream PATH --configuration CONFIG_H --output PRIVATE_DIR
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess

p = argparse.ArgumentParser(description=__doc__)
p.add_argument("--upstream", type=Path, required=True)
p.add_argument("--configuration", type=Path, required=True)
p.add_argument("--output", type=Path, required=True)
p.add_argument("--cc", default="cc")
a = p.parse_args()
root = Path(__file__).resolve().parents[1]
out = a.output.resolve()
out.mkdir(parents=True, exist_ok=True)
config = a.configuration.read_text()
lines = config.splitlines()
original = next(line for line in lines if line.startswith("#define ZP_SYSTEM_PLATFORM_HEADER "))
config = config.replace(original, '#define ZP_SYSTEM_PLATFORM_HEADER "' + str(root / "include/conduit_zenoh_pico_types.h") + '"')
header = out / "include/zenoh-pico/config.h"
header.parent.mkdir(parents=True, exist_ok=True)
header.write_text(config)
binary = out / "pal-write-test"
command = [a.cc, "-std=c11", "-O1", "-Wall", "-Wextra", "-Werror", "-ffunction-sections", "-fdata-sections", "-DZENOH_COMPILER_GCC", "-DZENOH_C_STANDARD=11", "-DZENOH_CONDUIT_EMBASSY", "-DZ_BUILD_LOG=0", "-DCONDUIT_Z_ALLOC_ALIGNMENT=16", "-I" + str(out / "include"), "-I" + str(a.upstream.resolve() / "include"), "-I" + str(root / "include"), str(root / "c/platform.c"), str(root / "tests/pal_write.c"), "-Wl,--gc-sections", "-o", str(binary)]
sha = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
proof = {"scope": "CPU actual target PAL with explicit effect fixtures, not actual networking/MCU/durableACK", "command": command, "platform_sha256": sha(root / "c/platform.c"), "fixture_sha256": sha(root / "tests/pal_write.c"), "original_config_sha256": sha(a.configuration), "fixture_config_sha256": sha(header), "config_delta": "platform header location only"}
r = subprocess.run(command, capture_output=True, text=True)
(out / "build.log").write_text(r.stdout + r.stderr)
proof["build_exit"] = r.returncode
if r.returncode == 0:
    r = subprocess.run([str(binary)], capture_output=True, text=True, timeout=5)
    (out / "run.log").write_text(r.stdout + r.stderr)
    proof["run_exit"] = r.returncode
    proof["binary_sha256"] = sha(binary)
(out / "proof.json").write_text(json.dumps(proof, indent=2) + "\n")
print(json.dumps(proof, indent=2))
raise SystemExit(r.returncode if r.returncode >= 0 else 1)
