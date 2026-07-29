#!/usr/bin/env python3
"""Run the fixed Conduit RP2040 USB-CDC HIL exchange."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import pathlib
import select
import struct
import subprocess
import sys
import termios
import time


REQUEST = struct.Struct(">4sH16s32sI")
HEADER = struct.Struct(">4sH16s32s32s32s16sQBIH")
EVENT = struct.Struct(">4sH16s32s16sQIIBHBH16s")
PROTOCOL_VERSION = 1
EXPECTED_PRODUCT_PATH = pathlib.Path("/dev/serial/by-id")
ROOT = pathlib.Path(__file__).resolve().parents[1]
FIRMWARE_ROOT = ROOT / "firmware/conduit-rp2040-hil"
FIRMWARE_INPUTS = (
    "../../Cargo.lock",
    "../../Cargo.toml",
    "../../crates/conduit-core/Cargo.toml",
    "../../crates/conduit-core/src",
    "../../crates/conduit-embedded/Cargo.toml",
    "../../crates/conduit-embedded/src/lib.rs",
    "Cargo.toml",
    "build.rs",
    "memory.x",
    "src/lib.rs",
    "src/main.rs",
)

EVENT_KINDS = {
    1: "allocation-prepared",
    2: "node-prepared",
    3: "run-started",
    4: "decision",
    5: "value-accepted",
    6: "value-consumed",
    7: "pressure-entered",
    8: "pressure-cleared",
    9: "node-completed",
    10: "cancellation-requested",
    11: "run-succeeded",
    12: "run-cancelled",
    13: "run-failed",
}


def parser() -> argparse.ArgumentParser:
    value = argparse.ArgumentParser()
    value.add_argument("--port", default=os.environ.get("CONDUIT_RP2040_HIL_PORT"))
    value.add_argument("--expected-plan-hash")
    value.add_argument("--expected-firmware-identity")
    value.add_argument("--maximum-decisions", type=int, default=64)
    value.add_argument("--timeout-seconds", type=float, default=10.0)
    value.add_argument("--probe", action="store_true")
    value.add_argument("--require-hardware", action="store_true")
    return value


def discover_port() -> str | None:
    if not EXPECTED_PRODUCT_PATH.is_dir():
        return None
    matches = sorted(
        entry
        for entry in EXPECTED_PRODUCT_PATH.iterdir()
        if "Conduit_RP2040_HIL" in entry.name
        or "conduit-rp2040-hil" in entry.name.lower()
    )
    return str(matches[0]) if len(matches) == 1 else None


def configure_raw(fd: int) -> None:
    attributes = termios.tcgetattr(fd)
    attributes[0] = 0
    attributes[1] = 0
    attributes[2] = termios.CS8 | termios.CREAD | termios.CLOCAL
    attributes[3] = 0
    attributes[4] = termios.B115200
    attributes[5] = termios.B115200
    attributes[6][termios.VMIN] = 0
    attributes[6][termios.VTIME] = 0
    termios.tcsetattr(fd, termios.TCSANOW, attributes)
    termios.tcflush(fd, termios.TCIOFLUSH)


def read_exact(fd: int, length: int, deadline: float) -> bytes:
    output = bytearray()
    while len(output) < length:
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise TimeoutError(f"timed out after {len(output)} of {length} bytes")
        readable, _, _ = select.select([fd], [], [], remaining)
        if readable:
            chunk = os.read(fd, length - len(output))
            if chunk:
                output.extend(chunk)
    return bytes(output)


def hash_bytes(value: str) -> bytes:
    normalized = value.removeprefix("sha256:")
    decoded = bytes.fromhex(normalized)
    if len(decoded) != 32:
        raise ValueError("expected plan hash must be 32 bytes")
    return decoded


def current_firmware_identity(
    target: str = "thumbv6m-none-eabi", profile: str = "release"
) -> bytes:
    digest = hashlib.sha256()
    for relative in FIRMWARE_INPUTS:
        source = (FIRMWARE_ROOT / relative).resolve()
        if source.is_dir():
            inputs = [
                (f"{relative}/{path.relative_to(source).as_posix()}", path)
                for path in sorted(source.rglob("*"))
                if path.is_file()
            ]
        else:
            inputs = [(relative, source)]
        for label, path in inputs:
            content = path.read_bytes()
            digest.update(label.encode("utf-8"))
            digest.update(b"\0")
            digest.update(len(content).to_bytes(8, "big"))
            digest.update(content)
    for label, content in (
        ("cargo-target", target.encode("utf-8")),
        ("cargo-profile", profile.encode("utf-8")),
        ("rustc-version", subprocess.check_output(["rustc", "-vV"])),
    ):
        digest.update(label.encode("utf-8"))
        digest.update(b"\0")
        digest.update(len(content).to_bytes(8, "big"))
        digest.update(content)
    return digest.digest()


def main() -> int:
    arguments = parser().parse_args()
    port = arguments.port or discover_port()
    if arguments.probe:
        report = {
            "schema": "conduit.rp2040-hil-probe/v1",
            "detected": port is not None,
            "port": port,
            "expected_firmware_identity": (
                f"sha256:{current_firmware_identity().hex()}"
            ),
        }
        print(json.dumps(report, sort_keys=True))
        return 0 if port is not None or not arguments.require_hardware else 2
    if not port:
        message = "no unique Conduit RP2040 HIL USB-CDC device detected"
        if arguments.require_hardware:
            print(message, file=sys.stderr)
            return 2
        print(json.dumps({"executed": False, "reason": message}, sort_keys=True))
        return 0
    if not arguments.expected_plan_hash:
        print("--expected-plan-hash is required for an HIL run", file=sys.stderr)
        return 2
    expected_plan = hash_bytes(arguments.expected_plan_hash)
    expected_firmware = (
        hash_bytes(arguments.expected_firmware_identity)
        if arguments.expected_firmware_identity
        else current_firmware_identity()
    )
    nonce = os.urandom(16)
    request = REQUEST.pack(
        b"CNH1",
        PROTOCOL_VERSION,
        nonce,
        expected_plan,
        arguments.maximum_decisions,
    )
    fd = os.open(port, os.O_RDWR | os.O_NOCTTY | os.O_NONBLOCK)
    try:
        configure_raw(fd)
        os.write(fd, request)
        deadline = time.monotonic() + arguments.timeout_seconds
        header = HEADER.unpack(read_exact(fd, HEADER.size, deadline))
        (
            magic,
            version,
            response_nonce,
            plan,
            firmware_identity,
            capability_report_hash,
            boot_id,
            run_sequence,
            status,
            decisions,
            count,
        ) = header
        if (
            magic != b"CNR1"
            or version != PROTOCOL_VERSION
            or response_nonce != nonce
            or plan != expected_plan
            or firmware_identity != expected_firmware
            or capability_report_hash == bytes(32)
            or status != 1
        ):
            raise RuntimeError("HIL response header failed identity or status validation")
        events = []
        for expected_sequence in range(count):
            fields = EVENT.unpack(read_exact(fd, EVENT.size, deadline))
            (
                event_magic,
                event_version,
                event_nonce,
                event_plan,
                event_boot,
                event_run,
                sequence,
                tick,
                subject_kind,
                subject_index,
                kind,
                value_length,
                value,
            ) = fields
            if (
                event_magic != b"CNE1"
                or event_version != PROTOCOL_VERSION
                or event_nonce != nonce
                or event_plan != expected_plan
                or event_boot != boot_id
                or event_run != run_sequence
                or sequence != expected_sequence
                or kind not in EVENT_KINDS
                or value_length > len(value)
            ):
                raise RuntimeError("HIL event attribution or sequence validation failed")
            events.append(
                {
                    "sequence": sequence,
                    "tick": tick,
                    "subject_kind": subject_kind,
                    "subject_index": subject_index,
                    "kind": EVENT_KINDS[kind],
                    "value": value[:value_length].hex(),
                }
            )
        kinds = {event["kind"] for event in events}
        required = {
            "allocation-prepared",
            "node-prepared",
            "run-started",
            "value-accepted",
            "value-consumed",
            "pressure-entered",
            "pressure-cleared",
            "node-completed",
            "run-succeeded",
        }
        if not required.issubset(kinds):
            raise RuntimeError(f"HIL evidence omitted {sorted(required - kinds)}")
        accepted = [
            bytes.fromhex(event["value"])
            for event in events
            if event["kind"] == "value-accepted"
        ]
        if accepted != [(42).to_bytes(4, "big"), b"\x01"]:
            raise RuntimeError("HIL semantic values differ from the representative oracle")
        report = {
            "schema": "conduit.rp2040-hil-report/v1",
            "executed": True,
            "port": port,
            "plan_hash": f"sha256:{plan.hex()}",
            "firmware_identity": f"sha256:{firmware_identity.hex()}",
            "capability_report_hash": f"sha256:{capability_report_hash.hex()}",
            "boot_id": boot_id.hex(),
            "run_sequence": run_sequence,
            "decisions": decisions,
            "evidence_records": count,
            "normalized": {
                "values": [value.hex() for value in accepted],
                "pressure_entered": "pressure-entered" in kinds,
                "pressure_cleared": "pressure-cleared" in kinds,
                "terminal": "run-succeeded",
            },
        }
        print(json.dumps(report, indent=2, sort_keys=True))
        return 0
    finally:
        os.close(fd)


if __name__ == "__main__":
    raise SystemExit(main())
