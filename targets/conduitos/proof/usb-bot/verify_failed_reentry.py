"""Verify failed recovery and acknowledged guest retirement, without a block result."""
import argparse
import json
import struct
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("--serial", required=True)
parser.add_argument("--wire", required=True)
parser.add_argument("--phase-error", action="store_true")
args = parser.parse_args()
lines = Path(args.serial).read_text().splitlines()
assert not any(line.startswith("CONDUIT_PROTOCOL_OUTPUT ") for line in lines)
assert not any("REFUSAL" in line for line in lines)
assert lines[-1] == "CONDUIT_PROTOCOL_RETIRED complete"
receipts = [next(line for line in lines if line.startswith(prefix)) for prefix in (
    "CONDUIT_PROTOCOL_PLAY ", "CONDUIT_PROTOCOL_PLAY_ALLOCATIONS ",
    "CONDUIT_PROTOCOL_PLAY_STOPPED ")]
assert "alloc=0 realloc=0" in receipts[1]
for field in ("body", "plan", "play"):
    values = [next(word for word in line.split() if word.startswith(field + "="))
              for line in receipts]
    assert len(set(values)) == 1
assert [lines.index(line) for line in receipts] == sorted(lines.index(line) for line in receipts)

packets = [json.loads(line) for line in Path(args.wire).read_text().splitlines()]
commands, stalls, controls, data, phase_errors = [], [], [], [], []
for index, packet in enumerate(packets):
    body = bytes.fromhex(packet["wire_hex"])[16:]
    if packet["type"] == 101:
        endpoint, status, length, _, _ = struct.unpack_from("<BBHIH", body)
        payload = body[10:]
        if packet["direction"] == "qemu-to-device" and endpoint == 2:
            assert len(payload) == 31 and payload[:4] == b"USBC"
            if payload[12:16] == bytes([128, 0, 10, 0x28]):
                assert struct.unpack_from("<II", payload, 4) == (1, 512)
                assert payload[17:21] == bytes(4) and payload[22:24] == bytes([0, 1])
                commands.append(index)
        if commands and packet["direction"] == "device-to-qemu" and endpoint == 129:
            if args.phase_error:
                assert status == 0
                if length == 512:
                    assert payload == bytes((i * 19 + 5) % 256 for i in range(512))
                    data.append(index)
                else:
                    assert length == 13 and payload == b"USBS" + struct.pack("<IIB", 1, 0xdeadbeef, 2)
                    phase_errors.append(index)
            else:
                assert status == 4 and length == 0 and not payload
                stalls.append(index)
    if packet["type"] == 100 and commands:
        fields = struct.unpack_from("<BBBBHHH", body)
        if fields[2] in (0x21, 0x02):
            assert len(body) == 10
            controls.append((index, packet["direction"], packet["id"], fields))
assert len(commands) == 1
if args.phase_error:
    assert not stalls and len(data) == len(phase_errors) == 1
    assert commands[0] < data[0] < phase_errors[0]
    fault = phase_errors[0]
else:
    assert len(stalls) == 1 and not data and not phase_errors
    fault = stalls[0]
assert [(direction, fields) for _, direction, _, fields in controls] == [
    ("qemu-to-device", (0, 255, 0x21, 0, 0, 0, 0)),
    ("device-to-qemu", (0, 255, 0x21, 0, 0, 0, 0)),
    ("qemu-to-device", (0, 1, 0x02, 0, 0, 129, 0)),
    ("device-to-qemu", (0, 1, 0x02, 4, 0, 129, 0)),
]
assert controls[0][2] == controls[1][2] and controls[2][2] == controls[3][2]
assert controls[0][2] != controls[2][2]
assert commands[0] < fault < controls[0][0]
assert not any(packet["type"] == 101 for packet in packets[fault + 1:])
print("failed-clear=true fresh-read=false result=false zero-allocation=true retired=true")
