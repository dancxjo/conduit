#!/usr/bin/env python3
"""Independent standard-library reader for Conduit canonical form v1 vectors."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import struct
from pathlib import Path
from typing import Any

MAGIC = b"CND\x01"
HASH_DOMAIN = b"conduit.semantic-hash/v1\0"
MAX_DEPTH = 64
ID = re.compile(r"^[a-z](?:[a-z0-9_-]|[.](?=[a-z])|/(?!.*[/])(?=[a-z]))*$")


def identifier(value: str) -> bytes:
    if not ID.fullmatch(value) or value.endswith((".", "/")):
        raise ValueError(f"invalid identifier: {value!r}")
    encoded = value.encode("ascii")
    return b"\x22" + struct.pack(">Q", len(encoded)) + encoded


def encode(value: Any, depth: int = 0) -> bytes:
    if value is None:
        return b"\x00"
    if not isinstance(value, dict) or len(value) != 1:
        raise ValueError(f"value must be a single-tag object or null: {value!r}")

    tag, payload = next(iter(value.items()))
    if tag == "boolean":
        return b"\x02" if payload else b"\x01"
    if tag == "integer":
        return b"\x10" + int(payload).to_bytes(16, "big", signed=True)
    if tag == "bytes":
        raw = bytes.fromhex(payload)
        return b"\x20" + struct.pack(">Q", len(raw)) + raw
    if tag == "text":
        raw = payload.encode("utf-8")
        return b"\x21" + struct.pack(">Q", len(raw)) + raw
    if tag == "identifier":
        return identifier(payload)

    depth += 1
    if depth > MAX_DEPTH:
        raise ValueError("canonical value nesting exceeds 64")
    if tag == "list":
        members = [encode(member, depth) for member in payload]
        return b"\x30" + struct.pack(">Q", len(members)) + b"".join(members)
    if tag == "set":
        members = sorted(encode(member, depth) for member in payload)
        if len(set(members)) != len(members):
            raise ValueError("duplicate canonical set value")
        return b"\x32" + struct.pack(">Q", len(members)) + b"".join(members)
    if tag == "map":
        names: set[str] = set()
        members: list[tuple[bytes, bytes]] = []
        for field in payload:
            name = field["name"]
            if name in names:
                raise ValueError(f"duplicate canonical map key: {name}")
            names.add(name)
            key = identifier(name)
            disposition = field.get("disposition", "semantic")
            if disposition == "annotation":
                continue
            encoded_value = encode(field["value"], depth)
            if disposition == "defaulted":
                if encoded_value == encode(field["default"], depth):
                    continue
            elif disposition != "semantic":
                raise ValueError(f"unknown disposition: {disposition}")
            members.append((key, encoded_value))
        members.sort()
        return (
            b"\x31"
            + struct.pack(">Q", len(members))
            + b"".join(key + value for key, value in members)
        )
    raise ValueError(f"unknown canonical value tag: {tag}")


def descriptor(kind: str, schema_version: int, body: Any) -> bytes:
    return MAGIC + identifier(kind) + struct.pack(">I", schema_version) + encode(body)


class Reader:
    """Strict canonical-byte reader used to prove the vectors are consumable."""

    def __init__(self, data: bytes) -> None:
        self.data = data
        self.offset = 0

    def take(self, length: int) -> bytes:
        end = self.offset + length
        if end > len(self.data):
            raise ValueError("truncated canonical value")
        value = self.data[self.offset : end]
        self.offset = end
        return value

    def length(self) -> int:
        return struct.unpack(">Q", self.take(8))[0]

    def value(self, depth: int = 0) -> Any:
        tag = self.take(1)[0]
        if tag == 0x00:
            return None
        if tag == 0x01:
            return {"boolean": False}
        if tag == 0x02:
            return {"boolean": True}
        if tag == 0x10:
            return {"integer": int.from_bytes(self.take(16), "big", signed=True)}
        if tag == 0x20:
            return {"bytes": self.take(self.length()).hex()}
        if tag == 0x21:
            return {"text": self.take(self.length()).decode("utf-8")}
        if tag == 0x22:
            value = self.take(self.length()).decode("ascii")
            identifier(value)
            return {"identifier": value}

        depth += 1
        if depth > MAX_DEPTH:
            raise ValueError("canonical value nesting exceeds 64")
        if tag == 0x30:
            return {"list": [self.value(depth) for _ in range(self.length())]}
        if tag == 0x31:
            fields = []
            previous = None
            for _ in range(self.length()):
                key_start = self.offset
                key = self.value(depth)
                if not isinstance(key, dict) or "identifier" not in key:
                    raise ValueError("canonical map key is not an identifier")
                key_bytes = self.data[key_start : self.offset]
                if previous is not None and key_bytes <= previous:
                    raise ValueError("canonical map keys are not strictly ordered")
                previous = key_bytes
                fields.append({"name": key["identifier"], "value": self.value(depth)})
            return {"map": fields}
        if tag == 0x32:
            members = []
            previous = None
            for _ in range(self.length()):
                member_start = self.offset
                member = self.value(depth)
                member_bytes = self.data[member_start : self.offset]
                if previous is not None and member_bytes <= previous:
                    raise ValueError("canonical set members are not strictly ordered")
                previous = member_bytes
                members.append(member)
            return {"set": members}
        raise ValueError(f"unknown canonical tag: {tag:#04x}")

    def descriptor(self) -> tuple[str, int, Any]:
        if self.take(len(MAGIC)) != MAGIC:
            raise ValueError("not a canonical descriptor v1")
        kind = self.value()
        if not isinstance(kind, dict) or "identifier" not in kind:
            raise ValueError("descriptor kind is not an identifier")
        schema_version = struct.unpack(">I", self.take(4))[0]
        body = self.value()
        if self.offset != len(self.data):
            raise ValueError("trailing canonical descriptor bytes")
        return kind["identifier"], schema_version, body


def verify(path: Path, show: bool) -> None:
    suite = json.loads(path.read_text(encoding="utf-8"))
    if suite["canonical_form_version"] != 1:
        raise ValueError("reader only supports canonical form version 1")
    for vector in suite["vectors"]:
        canonical = descriptor(
            vector["kind"], vector["schema_version"], vector["body"]
        )
        digest = "sha256:" + hashlib.sha256(HASH_DOMAIN + canonical).hexdigest()
        decoded_kind, decoded_version, decoded_body = Reader(canonical).descriptor()
        if descriptor(decoded_kind, decoded_version, decoded_body) != canonical:
            raise AssertionError(f"{vector['name']}: decoded value did not round-trip")
        for equivalent in vector.get("equivalent_bodies", []):
            alternative = descriptor(
                vector["kind"], vector["schema_version"], equivalent
            )
            if alternative != canonical:
                raise AssertionError(
                    f"{vector['name']}: equivalent input changed canonical bytes"
                )
        for different in vector.get("different_bodies", []):
            alternative = descriptor(
                vector["kind"], vector["schema_version"], different
            )
            if alternative == canonical:
                raise AssertionError(
                    f"{vector['name']}: semantic change retained canonical bytes"
                )
            if hashlib.sha256(HASH_DOMAIN + alternative).digest() == hashlib.sha256(
                HASH_DOMAIN + canonical
            ).digest():
                raise AssertionError(
                    f"{vector['name']}: semantic change retained semantic hash"
                )
        if show:
            print(vector["name"])
            print(f"canonical_hex={canonical.hex()}")
            print(f"semantic_hash={digest}")
            continue
        if canonical.hex() != vector["canonical_hex"]:
            raise AssertionError(f"{vector['name']}: canonical bytes differ")
        if digest != vector["semantic_hash"]:
            raise AssertionError(f"{vector['name']}: semantic hash differs")
        print(f"ok {vector['name']} {digest}")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "vectors",
        nargs="?",
        type=Path,
        default=Path(__file__).with_name("canonical-v1.json"),
    )
    parser.add_argument(
        "--show",
        action="store_true",
        help="print computed values while preparing reviewed frozen vectors",
    )
    args = parser.parse_args()
    verify(args.vectors, args.show)


if __name__ == "__main__":
    main()
