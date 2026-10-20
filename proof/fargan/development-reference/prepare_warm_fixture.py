#!/usr/bin/env python3
"""Explicit private reference run; no downloads/checkpoints/parameters copied."""
import argparse
import hashlib
import json
import math
import pathlib
import struct
import subprocess

PIN = "503d81b138d76621aae4b12786e90de48aa8db3a"
MODEL = "d35f0510da476716183a33caafe45cc095e48d496f06de2db7655e780a385e47"
PERIODS = [64, 80, 96, 128, 160]

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("oracle", type=pathlib.Path)
    parser.add_argument("output", type=pathlib.Path)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    context = [(i % 17 - 8) / 256 for i in range(320)]
    features = []
    for i, period in enumerate(PERIODS):
        value = [0.] * 20
        value[0], value[1] = -8 + i / 2, i / 8
        value[18] = math.log2(256 / period) - 1.5
        value[19] = 0.2 + i / 10
        features.extend(value)
    raw = struct.pack("<420f", *(context + features))
    input_file, output_file = args.output / "warm-input.bin", args.output / "warm-state.bin"
    input_file.write_bytes(raw)
    subprocess.run([str(args.oracle.resolve()), str(input_file.resolve()), str(output_file.resolve())], check=True)
    state = output_file.read_bytes()
    if len(state) != 3864 or not all(map(math.isfinite, struct.unpack("<965f", state[:3860]))):
        raise ValueError("exact finite warm state geometry")
    if struct.unpack("<i", state[3860:])[0] != PERIODS[3]:
        raise ValueError("delayed period must be p3")
    sha = lambda data: hashlib.sha256(data).hexdigest()
    manifest = {"upstream_sha": PIN, "model_f32_sha256": MODEL,
        "profile": "fargan-supplied-context-warm-oracle@1", "periods": PERIODS,
        "input_sha256": sha(raw), "state_sha256": sha(state),
        "oracle_binary_sha256": sha(args.oracle.read_bytes()),
        "oracle_source_sha256": sha(pathlib.Path(__file__).with_name("warm_start_oracle.c").read_bytes())}
    (args.output / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print(json.dumps(manifest, indent=2))

if __name__ == "__main__":
    main()
