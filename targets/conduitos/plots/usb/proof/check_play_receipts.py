#!/usr/bin/env python3
"""Check bound product Play allocation and acknowledged-stop receipts.

This gate does not establish wire bytes, class outcomes or physical output.
Those require their independent verifiers and retained build/package identities.
"""
import argparse
import json
from pathlib import Path


def verify(text):
    lines = text.splitlines()

    def one(prefix):
        matches = [(index, line) for index, line in enumerate(lines)
                   if line.startswith(prefix + " ")]
        if len(matches) != 1:
            raise ValueError(f"expected one {prefix} receipt")
        index, line = matches[0]
        fields = {}
        for item in line[len(prefix) + 1:].split():
            key, value = item.split("=", 1)
            if key in fields or not value:
                raise ValueError("duplicate or empty receipt field")
            fields[key] = value
        return index, fields

    playing_at, playing = one("CONDUIT_PROTOCOL_PLAY")
    allocations_at, allocations = one("CONDUIT_PROTOCOL_PLAY_ALLOCATIONS")
    stopped_at, stopped = one("CONDUIT_PROTOCOL_PLAY_STOPPED")
    if not playing_at < allocations_at < stopped_at:
        raise ValueError("allocation/stop receipts are out of lifecycle order")
    for key in ("body", "plan", "play"):
        if not playing.get(key) or allocations.get(key) != playing[key] or stopped.get(key) != playing[key]:
            raise ValueError(f"original {key} identity was not retained")
    if allocations.get("alloc") != "0" or allocations.get("realloc") != "0":
        raise ValueError("Play attempted allocation or reallocation")
    if any(line.startswith("CONDUIT_PROTOCOL_OUTPUT ") for line in lines[allocations_at + 1:]):
        raise ValueError("output appeared after execution allocation accounting")
    if any("CONDUIT_PANIC" in line or "protocol-retirement-refused" in line for line in lines):
        raise ValueError("panic or retirement refusal contradicts the stop proof")
    return {"scope": "bound product Play receipts", "body": playing["body"],
            "plan": playing["plan"], "play": playing["play"],
            "allocation_attempts": 0, "reallocation_attempts": 0,
            "acknowledged_stop": True, "wire_or_physical_output_proven": False}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("serial", type=Path)
    args = parser.parse_args()
    print(json.dumps(verify(args.serial.read_text()), sort_keys=True))
