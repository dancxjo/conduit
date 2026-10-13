#!/usr/bin/env python3
"""Verify exact wire bytes for the explicitly authored nine-repeat PCM request."""
import json
from pathlib import Path
import struct
import sys
entries = [json.loads(line) for line in Path(sys.argv[1]).read_text().splitlines() if line]
expected = Path(sys.argv[2]).read_bytes()
assert len(expected) == 1536 and any(expected)
packets, controls = [], []
for entry in entries:
    raw = bytes.fromhex(entry['wire_hex'])
    header = 12 if entry['type'] == 0 else 16
    assert len(raw) >= header
    kind, length = struct.unpack_from('<II', raw)
    assert kind == entry['type'] and length == len(raw) - header
    if entry['direction'] != 'qemu-to-device':
        continue
    body = raw[header:]
    if kind == 102:
        assert len(body) == 196
        assert body[0] == 1 and int.from_bytes(body[2:4], 'little') == 192
        packets.append(body[4:])
    if kind == 100:
        assert len(body) >= 10
        endpoint, request, request_type, status, value, index, count = struct.unpack_from('<BBBBHHH', body)
        controls.append((request_type, request, value, index, count, body[10:]))
assert len(packets) == 72 and b''.join(packets) == expected * 9
assert (0x22, 1, 0x100, 1, 3, bytes.fromhex('80bb00')) in controls
assert (0xa2, 0x81, 0x100, 1, 3, b'') in controls
print(json.dumps({'scope': 'finite nine-repeat request', 'packets': 72, 'exact_pcm_bytes': 13824, 'native_completion_and_retirement_required': True, 'audible_output_claimed': False}))
