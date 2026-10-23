#!/usr/bin/env python3
"""Bounded physical Root provisioning proof; only ephemeral fixture credentials.
No flash, Body installation, UART credential ingress or Host authority is made.
"""
import argparse
import hashlib
import http.client
import json
from pathlib import Path
import secrets
import time
import urllib.parse
import serial

parser = argparse.ArgumentParser()
parser.add_argument("--port", default="/dev/ttyUSB0")
parser.add_argument("--output", required=True)
args = parser.parse_args()
out = Path(args.output)
out.mkdir(parents=True, exist_ok=False)
password = "fixture-only-" + secrets.token_hex(8)
body = urllib.parse.urlencode({"ssid": "conduit-absent-" + secrets.token_hex(8), "password": password}).encode()
raw = bytearray()
pending = bytearray()
offers = []
result = {"proof_class": "physical-installed-root-negative-network-fixture", "completed": False,
          "body_installation_claimed": False, "station_success_claimed": False,
          "fixture_sha256": hashlib.sha256(body).hexdigest(), "post_acknowledged": False}
port = serial.Serial(port=None, baudrate=115200, timeout=.1)
port.port = args.port
# Set both before opening: false/false resets this ESP32 development board.
port.dtr = True
port.rts = True
started = time.monotonic()

def offer(timeout):
    port.write(b"offer\n")
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        pending.extend(port.read(8192))
        while b"\n" in pending:
            line, _, tail = pending.partition(b"\n")
            pending[:] = tail
            raw.extend(line + b"\n")
            try:
                current = json.loads(line)
            except (ValueError, TypeError):
                continue
            if not isinstance(current, dict) or current.get("event") != "native-root-offer":
                continue
            offers.append(current)
            return current
    raise RuntimeError("no complete Root offer before deadline")

try:
    port.open()
    baseline = offer(8)
    boot = baseline["advertisement"]["boot_id"]
    result["observed_boot_id"] = boot
    assert baseline["network_configuration"] == "uninstalled", "requires a fresh uninstalled Root setup control"
    connection = http.client.HTTPConnection("192.168.4.1", 80, timeout=5)
    try:
        connection.request("POST", "/network", body, {"Content-Type": "application/x-www-form-urlencoded"})
        response = connection.getresponse()
        status = response.status
        assert password.encode() not in response.read(), "credential echoed in HTTP response"
    finally:
        connection.close()
    result["post_status"] = status
    assert status == 202, "private setup submission was not acknowledged"
    result["post_acknowledged"] = True
    print("Fixture acknowledged; no UART requests for 30 seconds", flush=True)
    quiet = time.monotonic() + 30
    while time.monotonic() < quiet:
        pending.extend(port.read(8192))
    result["uart_quiet_after_post_seconds"] = 30
    stable = 0
    while time.monotonic() - started < 90:
        current = offer(5)
        assert current["advertisement"]["boot_id"] == boot, "device reset during network attempt"
        print(json.dumps({key: current.get(key) for key in ["network_configuration", "network_control_dispatches", "network_control_failure_phase", "network_control_failure_reason", "native_root_heap_peak_bytes", "native_sdk_heap_peak_bytes"]}), flush=True)
        if current["network_configuration"] == "setup-denied":
            result["terminal_refusal"] = current
            raise RuntimeError("Root refused network control preparation")
        if current["network_configuration"] == "access-point-restored":
            assert current["network_control_installed"]
            assert current["network_control_dispatches"] == [1, 1, 0]
            assert current["network_control_failure_phase"] == "associate"
            assert current["network_control_failure_reason"] in ["rejected", "deadline"]
            assert not current["network_control_receipt"]
            assert not current["body_installed"] and not current["native_plan_installed"]
            assert current["network_control_binding"]["boot_id"] == boot
            stable += 1
            if stable == 2:
                result["completed"] = True
                break
        pause = time.monotonic() + 3
        while time.monotonic() < pause:
            pending.extend(port.read(8192))
    assert result["completed"], "bounded join failure and native AP restoration were not verified"
    assert password.encode() not in raw + pending, "credential echoed over UART"
    print("PASS: one failed join and acknowledged AP restoration in the same Boot", flush=True)
except Exception as error:
    result["failure"] = str(error)
    print("INCOMPLETE: " + str(error), flush=True)
finally:
    port.close()
    result["elapsed_seconds"] = time.monotonic() - started
    (out / "serial.log").write_bytes((raw + pending).replace(password.encode(), b"[redacted fixture credential]"))
    (out / "offers.json").write_text(json.dumps(offers, indent=2) + "\n")
    (out / "result.json").write_text(json.dumps(result, indent=2) + "\n")
raise SystemExit(0 if result["completed"] else 1)
