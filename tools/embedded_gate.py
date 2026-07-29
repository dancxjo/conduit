#!/usr/bin/env python3
"""Cross-link and inspect the fixed RP2040 reference firmware."""

from __future__ import annotations

import json
import pathlib
import re
import subprocess
import sys

from rp2040_hil import current_firmware_identity


ROOT = pathlib.Path(__file__).resolve().parents[1]
BUDGET_PATH = ROOT / "conformance/c5/rp2040-budgets-v1.json"


def run(*command: str) -> str:
    result = subprocess.run(
        command,
        cwd=ROOT,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    return result.stdout


def main() -> int:
    budget = json.loads(BUDGET_PATH.read_text(encoding="utf-8"))
    run(
        "cargo",
        "build",
        "-p",
        "conduit-rp2040-hil",
        "--target",
        budget["target"],
        "--release",
    )
    artifact = ROOT / budget["artifact"]
    size_lines = run("size", str(artifact)).strip().splitlines()
    if len(size_lines) != 2:
        raise RuntimeError("unexpected size output")
    text, data, bss, _decimal, _hexadecimal, _name = size_lines[1].split()
    text_bytes = int(text)
    data_bytes = int(data)
    bss_bytes = int(bss)
    flash_bytes = text_bytes + data_bytes
    static_ram_bytes = data_bytes + bss_bytes
    if flash_bytes > budget["maximum_flash_bytes"]:
        raise RuntimeError(
            f"RP2040 flash budget exceeded: {flash_bytes} > "
            f"{budget['maximum_flash_bytes']}"
        )
    if static_ram_bytes > budget["maximum_static_ram_bytes"]:
        raise RuntimeError(
            f"RP2040 static RAM budget exceeded: {static_ram_bytes} > "
            f"{budget['maximum_static_ram_bytes']}"
        )
    undefined = run("nm", "-u", str(artifact)).splitlines()
    forbidden = [
        symbol
        for symbol in budget["allocator_symbols_forbidden"]
        if any(symbol in line for line in undefined)
    ]
    if forbidden:
        raise RuntimeError(f"allocator linkage detected: {', '.join(forbidden)}")
    header = run("readelf", "-h", str(artifact))
    if "Machine:                           ARM" not in header:
        raise RuntimeError("firmware artifact is not an ARM ELF")
    sections = run("readelf", "-W", "-S", str(artifact))
    boot2 = re.search(
        r"\[\s*\d+\]\s+\.boot2\s+\S+\s+([0-9a-fA-F]+)\s+"
        r"[0-9a-fA-F]+\s+([0-9a-fA-F]+)",
        sections,
    )
    if boot2 is None:
        raise RuntimeError("firmware artifact omits the RP2040 .boot2 section")
    boot2_address = int(boot2.group(1), 16)
    boot2_size = int(boot2.group(2), 16)
    if boot2_address != 0x10000000 or boot2_size != 0x100:
        raise RuntimeError(
            "RP2040 .boot2 must occupy exactly "
            f"0x10000000..0x10000100, got address={boot2_address:#x} "
            f"size={boot2_size:#x}"
        )
    report = {
        "schema": "conduit.rp2040-budget-report/v1",
        "target": budget["target"],
        "artifact": budget["artifact"],
        "firmware_identity": (
            f"sha256:{current_firmware_identity(target=budget['target'], profile='release').hex()}"
        ),
        "flash": {
            "bytes": flash_bytes,
            "maximum_bytes": budget["maximum_flash_bytes"],
            "kind": "linked-elf-load-image",
        },
        "static_ram": {
            "bytes": static_ram_bytes,
            "maximum_bytes": budget["maximum_static_ram_bytes"],
            "kind": "linked-elf-data-plus-bss",
        },
        "stack": {
            "bytes": budget["declared_stack_budget_bytes"],
            "kind": "profile-declared-reviewed-ceiling-not-elf-measurement",
        },
        "boot2": {
            "address": f"0x{boot2_address:08x}",
            "bytes": boot2_size,
            "kind": "linked-rp2040-second-stage-bootloader",
        },
        "allocator_undefined_symbols": [],
    }
    json.dump(report, sys.stdout, indent=2, sort_keys=True)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
