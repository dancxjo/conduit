#!/usr/bin/env python3
"""Loopback HTTP through an explicitly selected Pico W carrier.

This is transport only: no Body membership, discovery, HTTP replay, or host NIC
changes. Credentials come exclusively from an owned private known-wifi file.
"""
import argparse
import importlib.util
import ipaddress
import json
import os
from pathlib import Path
import re
import secrets
import stat
import struct
import threading
import time
from http.server import HTTPServer

# Reuse the existing bounded, nonlogging HTTP front end, never its old-role USB
# parser. The common exception type keeps its fail-closed 502 behavior intact.
_spec = importlib.util.spec_from_file_location("conduit_legacy_http_frontend", Path(__file__).parents[1] / "usb-http-relay" / "relay.py")
_http = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(_http)
CarrierError = _http.RelayError
HEADER = struct.Struct("<4sBBIH")
ROLE = "conduit.environment-wifi-carrier@1"
JOIN, REQUEST, STATUS, SENSOR_STATUS, DIAGNOSTICS, MDNS_QUERY, SELECT_PEER = 1, 2, 3, 6, 7, 10, 11
DATA, DONE, ERROR, INFORMATION = 128, 129, 130, 131
MAX_REQUEST, MAX_RESPONSE, CHUNK = 8192, 65536, 512


def private_ipv4(value):
    try:
        address = ipaddress.IPv4Address(value)
    except (ipaddress.AddressValueError, ValueError):
        raise CarrierError("Expected explicit private IPv4 peer") from None
    if not any(address in network for network in (
        ipaddress.IPv4Network("10.0.0.0/8"), ipaddress.IPv4Network("172.16.0.0/12"),
        ipaddress.IPv4Network("192.168.0.0/16"),
    )):
        raise CarrierError("Expected explicit RFC1918 IPv4 peer")
    return address


def _unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise CarrierError("Duplicate private configuration field")
        result[key] = value
    return result


def load_profile(filename, selected_ssid, uid=None):
    """Read bounded private JSON; retain only the exact selected credential."""
    uid = os.getuid() if uid is None else uid
    raw = bytearray()
    directory = descriptor = None
    try:
        path = Path(filename)
        if not path.is_absolute():
            raise CarrierError("Private network file must use an absolute path")
        directory = os.open(path.parent, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
        parent = os.fstat(directory)
        if parent.st_uid != uid or parent.st_mode & 0o077:
            raise CarrierError("Network file parent must be owned and private")
        descriptor = os.open(path.name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=directory)
        before = os.fstat(descriptor)
        if not stat.S_ISREG(before.st_mode) or before.st_uid != uid or stat.S_IMODE(before.st_mode) != 0o600 or before.st_size > 8192:
            raise CarrierError("Network file must be an owned bounded 0600 regular file")
        while len(raw) <= 8192:
            part = os.read(descriptor, 8193 - len(raw))
            if not part:
                break
            raw.extend(part)
            if len(raw) > 8192:
                raise CarrierError("Private network file capacity")
        after = os.fstat(descriptor)
        if (before.st_size, before.st_mtime_ns, before.st_ctime_ns) != (after.st_size, after.st_mtime_ns, after.st_ctime_ns):
            raise CarrierError("Network file changed while reading")
        document = json.loads(raw, object_pairs_hook=_unique_object)
        if set(document) != {"schema", "profiles"} or document["schema"] != "conduit.private/known-wifi@1":
            raise CarrierError("Private network schema refused")
        profiles = document["profiles"]
        if not isinstance(profiles, list) or not 1 <= len(profiles) <= 4:
            raise CarrierError("Private network profile capacity")
        seen = set()
        selected = None
        for profile in profiles:
            if not isinstance(profile, dict) or set(profile) != {"ssid", "password", "security", "priority"}:
                raise CarrierError("Private network profile fields refused")
            ssid, password = profile["ssid"], profile["password"]
            if not isinstance(ssid, str) or not 1 <= len(ssid.encode("utf-8")) <= 32 or "\0" in ssid or ssid in seen:
                raise CarrierError("Private network SSID refused")
            seen.add(ssid)
            if profile["security"] != "wpa2-personal" or type(profile["priority"]) is not int or not -(1 << 31) <= profile["priority"] < (1 << 31):
                raise CarrierError("Unsupported private network policy")
            if not isinstance(password, str) or not 8 <= len(password) <= 63 or any(not 32 <= ord(c) <= 126 for c in password):
                raise CarrierError("Private network credential refused")
            if ssid == selected_ssid:
                selected = password
        if selected is None:
            raise CarrierError("Exact selected SSID is not in the private file")
        return bytearray(selected.encode("ascii"))
    except CarrierError:
        raise
    except (OSError, ValueError, TypeError, UnicodeError):
        raise CarrierError("Private network file refused") from None
    finally:
        raw[:] = b"\0" * len(raw)
        if descriptor is not None:
            os.close(descriptor)
        if directory is not None:
            os.close(directory)


def validate_status(value):
    if not isinstance(value, dict) or value.get("role") != ROLE or value.get("body_member") is not False:
        raise CarrierError("Not the selected Pico carrier role")
    if not isinstance(value.get("build"), str) or not re.fullmatch(r"[0-9a-f]{64}", value["build"]):
        raise CarrierError("Carrier build identity refused")
    configuration = value.get("configuration")
    if not isinstance(configuration, str) or not configuration or len(configuration) > 128 or any(ord(c) < 32 for c in configuration):
        raise CarrierError("Carrier configuration identity refused")
    if any(type(value.get(key)) is not bool for key in ("radio_ready", "link_up", "peer_ready")):
        raise CarrierError("Carrier readiness metadata refused")
    generation = value.get("attachment_generation")
    if type(generation) is not int or not 0 <= generation < (1 << 64):
        raise CarrierError("Carrier attachment generation refused")
    ip = value.get("ip")
    if ip is not None and (not isinstance(ip, list) or len(ip) != 4 or any(type(b) is not int or not 0 <= b <= 255 for b in ip)):
        raise CarrierError("Carrier IPv4 metadata refused")
    return value


class Carrier:
    def __init__(self, stream, clock=time.monotonic):
        self.stream, self.clock = stream, clock
        self.sequence = 0
        self.lock = threading.Lock()
        self.identity = None
        self.selected = None

    def _read(self, length, deadline):
        result = bytearray()
        while len(result) < length:
            if self.clock() >= deadline:
                raise CarrierError("USB reply deadline")
            chunk = self.stream.read(length - len(result))
            if len(chunk) > length - len(result):
                raise CarrierError("USB read capacity")
            result.extend(chunk)
        return bytes(result)

    def exchange(self, kind, payload=b"", timeout=40):
        if type(kind) is not int or kind not in (JOIN, REQUEST, STATUS, SELECT_PEER, MDNS_QUERY, SENSOR_STATUS, DIAGNOSTICS) or not isinstance(payload, (bytes, bytearray, memoryview)) or len(payload) > MAX_REQUEST:
            raise CarrierError("Carrier command capacity")
        if kind in (SENSOR_STATUS, DIAGNOSTICS) and (payload or self.identity is None):
            raise CarrierError("Environment query requires an identified session and no payload")
        with self.lock:
            if self.sequence == 0xFFFFFFFF:
                raise CarrierError("USB session sequence exhausted")
            self.sequence += 1
            identity = self.sequence
            wire = bytearray(HEADER.pack(b"CPBR", 1, kind, identity, len(payload)))
            wire.extend(payload)
            try:
                at = 0
                while at < len(wire):
                    count = self.stream.write(memoryview(wire)[at:])
                    if not count or count > len(wire) - at:
                        raise CarrierError("USB write incomplete")
                    at += count
                deadline = self.clock() + timeout
                result = bytearray()
                while True:
                    magic, version, response, response_id, length = HEADER.unpack(self._read(HEADER.size, deadline))
                    if magic != b"CPBR" or version != 1 or response_id != identity:
                        raise CarrierError("USB response scope")
                    limit = {DATA: CHUNK, DONE: 4, ERROR: 128, INFORMATION: 1536 if kind in (SENSOR_STATUS, DIAGNOSTICS) else (768 if kind == MDNS_QUERY else 512)}.get(response)
                    if limit is None or length > limit:
                        raise CarrierError("USB response capacity")
                    data = self._read(length, deadline)
                    if response == ERROR:
                        raise CarrierError("Carrier command refused")  # Never echo untrusted bytes.
                    if response == INFORMATION:
                        if kind not in (STATUS, JOIN, SELECT_PEER, MDNS_QUERY, SENSOR_STATUS, DIAGNOSTICS) or result:
                            raise CarrierError("Unexpected information response")
                        try:
                            info = json.loads(data, object_pairs_hook=_unique_object)
                        except (ValueError, UnicodeError):
                            raise CarrierError("Invalid carrier information") from None
                        if not isinstance(info, dict) or info.get("role") != ROLE:
                            raise CarrierError("Not the Pico carrier role")
                        if kind in (SENSOR_STATUS, DIAGNOSTICS):
                            if info.get("body_member") is not False or (info.get("build"), info.get("configuration")) != self.identity:
                                raise CarrierError("Environment session identity changed")
                            for key in ("attempts", "samples", "frames", "acks", "heap_peak"):
                                if type(info.get(key)) is not int or not 0 <= info[key] < (1 << 64):
                                    raise CarrierError("Environment counter refused")
                            if info["samples"] > info["attempts"] or info["acks"] > info["frames"]:
                                raise CarrierError("Environment counter ordering refused")
                        elif kind != MDNS_QUERY:
                            validate_status(info)
                            identity = (info["build"], info["configuration"])
                            if self.identity is not None and identity != self.identity:
                                raise CarrierError("Carrier session identity changed")
                            self.identity = identity
                        return info
                    if response == DATA:
                        if kind != REQUEST or not data or len(result) + length > MAX_RESPONSE:
                            raise CarrierError("HTTP response capacity")
                        result.extend(data)
                    elif response == DONE:
                        if kind != REQUEST or length != 4 or not result or int.from_bytes(data, "little") != len(result):
                            raise CarrierError("HTTP completion extent")
                        return bytes(result)
            finally:
                wire[:] = b"\0" * len(wire)

    def join(self, ssid, password):
        try:
            name = ssid.encode("utf-8")
        except (AttributeError, UnicodeError):
            raise CarrierError("SSID encoding refused") from None
        if not 1 <= len(name) <= 32 or b"\0" in name or not 8 <= len(password) <= 63 or any(not 32 <= b <= 126 for b in password):
            raise CarrierError("Protected network credential capacity")
        payload = bytearray(b"CNJ1\x01") + bytes([len(name), len(password)]) + name + password
        try:
            status = self.exchange(JOIN, payload, timeout=60)
        finally:
            payload[:] = b"\0" * len(payload)
        if not status["radio_ready"] or not status["link_up"] or status["ip"] is None or status["attachment_generation"] == 0:
            raise CarrierError("Chosen network has no current attachment")
        private_ipv4(bytes(status["ip"]))
        return status

    def select_peer(self, peer, attached):
        address = private_ipv4(peer)
        if list(address.packed) == attached["ip"]:
            raise CarrierError("Selected peer is the carrier itself")
        status = self.exchange(SELECT_PEER, address.packed, timeout=10)
        if not status["peer_ready"] or not status["radio_ready"] or not status["link_up"] or status["ip"] != attached["ip"] or status["attachment_generation"] != attached["attachment_generation"]:
            raise CarrierError("Selected peer attachment changed or refused")
        # Firmware validates its actual DHCP prefix, network/broadcast exclusion,
        # and on-link unicast scope; the status intentionally omits that prefix.
        self.selected = (str(address), status["attachment_generation"])
        return status

    def observe_mdns(self, hostname, peer, generation, nonce=None):
        if self.selected != (str(private_ipv4(peer)), generation):
            raise CarrierError("mDNS requires this session's explicitly selected peer")
        if not re.fullmatch(r"conduit-climate-[0-9a-f]{12}(?:-[23])?\.local", hostname):
            raise CarrierError("Selected MCU hostname refused")
        nonce = secrets.token_bytes(8) if nonce is None else nonce
        if not isinstance(nonce, bytes) or len(nonce) != 8 or nonce == bytes(8):
            raise CarrierError("mDNS nonce refused")
        info = self.exchange(MDNS_QUERY, nonce + hostname.encode("ascii"), timeout=5)
        address = list(private_ipv4(peer).packed)
        if info.get("body_member") is not False or (info.get("build"), info.get("configuration")) != self.identity or info.get("observation") != "selected-peer-mdns-a" or info.get("nonce") != nonce.hex() or info.get("query_id") != max(1, int.from_bytes(nonce[:2], "big")) or info.get("hostname") != hostname or info.get("address") != address or info.get("source") != address or info.get("source_port") != 5353 or info.get("attachment_generation") != generation:
            raise CarrierError("Selected peer mDNS observation scope")
        return info


class CarrierConnection:
    """A failed/ambiguous HTTP command is never retried, even after reconnection."""
    def __init__(self, port, network_file, ssid, peer, opener=None, loader=load_profile, sleep=time.sleep):
        self.port, self.network_file, self.ssid = port, network_file, ssid
        self.peer_host = str(private_ipv4(peer))
        self.opener, self.loader, self.sleep = opener, loader, sleep
        self.stream = self.client = self.password = None
        self.generation = None
        self.identity = None
        self.closed = False
        self.lock = threading.Lock()

    def _close(self):
        stream, self.stream, self.client = self.stream, None, None
        if self.password is not None:
            self.password[:] = b"\0" * len(self.password)
            self.password = None
        self.generation = self.identity = None
        if stream is not None:
            try:
                stream.close()
            except OSError:
                pass

    def connect(self):
        if self.closed:
            raise CarrierError("Carrier connection is closed")
        if self.stream is not None:
            raise CarrierError("Carrier session already owned")
        self.password = self.loader(self.network_file, self.ssid)
        if not isinstance(self.password, bytearray):
            self.password = None
            raise CarrierError("Private loader must return wipeable credential bytes")
        try:
            if self.opener is None:
                try:
                    import serial
                except ImportError:
                    raise CarrierError("pyserial is required for USB transport") from None
                opener = serial.Serial
            else:
                opener = self.opener
            self.stream = opener(self.port, 115200, timeout=0.5, write_timeout=2, exclusive=True)
            self.stream.dtr = False
            self.sleep(0.65)
            self.stream.reset_input_buffer()
            self.stream.dtr = True
            self.client = Carrier(self.stream)
            status = self.client.exchange(STATUS, timeout=45)
            if not status["radio_ready"]:
                raise CarrierError("Carrier radio is not ready")
            # A prior link is insufficient: STATUS does not disclose its SSID.
            attached = self.client.join(self.ssid, self.password)
            selected = self.client.select_peer(self.peer_host, attached)
            if (selected["build"], selected["configuration"]) != (status["build"], status["configuration"]):
                raise CarrierError("Carrier identity changed during attachment")
            self.identity = (selected["build"], selected["configuration"])
            self.generation = selected["attachment_generation"]
        except (CarrierError, OSError):
            self._close()
            raise

    def exchange(self, kind, payload=b"", timeout=40):
        if kind != REQUEST:
            raise CarrierError("HTTP front end accepts REQUEST only")
        with self.lock:
            try:
                if self.client is None:
                    self.connect()
                return self.client.exchange(REQUEST, payload, timeout)
            except (CarrierError, OSError):
                self._close()
                raise

    def environment(self):
        """Observe this owned session only; never open, rejoin, select, or retry."""
        with self.lock:
            if self.closed or self.client is None or self.identity is None:
                raise CarrierError("No identified carrier session")
            try:
                return self.client.exchange(SENSOR_STATUS, timeout=5)
            except (CarrierError, OSError):
                self._close()
                raise

    def observe_mdns(self, hostname, nonce=None):
        with self.lock:
            try:
                if self.client is None:
                    self.connect()
                return self.client.observe_mdns(hostname, self.peer_host, self.generation, nonce)
            except (CarrierError, OSError):
                self._close()
                raise

    def shutdown(self):
        with self.lock:
            self.closed = True
            self._close()


def handler(connection):
    class LocalDiagnostics(_http.handler(connection)):
        def forward(self):
            if self.path.split("?", 1)[0] != "/__carrier/environment":
                return super().forward()
            # This diagnostic never crosses USB as HTTP or reaches the peer.
            if self.client_address[0] != "127.0.0.1":
                self.send_error(403)
            elif self.command != "GET":
                self.send_error(405)
            elif self.path != "/__carrier/environment" or self.headers.get_all("Content-Length", []) or self.headers.get("Transfer-Encoding"):
                self.send_error(400)
            else:
                try:
                    document = {"schema": "conduit.carrier/environment-observation@1",
                                "transport": "owned-usb-session", "body_member": False,
                                "environment": connection.environment()}
                    raw = json.dumps(document, separators=(",", ":"), allow_nan=False).encode()
                    if len(raw) > 2048:
                        raise CarrierError("Environment HTTP capacity")
                    self.send_response(200)
                    self.send_header("Content-Type", "application/json")
                    self.send_header("Cache-Control", "no-store")
                    self.send_header("Content-Length", str(len(raw)))
                    self.send_header("Connection", "close")
                    self.end_headers()
                    self.wfile.write(raw)
                except (CarrierError, OSError, ValueError):
                    self.send_error(502, "Carrier environment observation refused")
            self.close_connection = True

        do_GET = do_POST = do_HEAD = do_PUT = do_DELETE = do_OPTIONS = forward
    return LocalDiagnostics


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--port", required=True, help="explicit Pico CDC device; never the ESP UART")
    parser.add_argument("--network-file", required=True, help="owned private known-wifi@1 JSON; never a password argument")
    parser.add_argument("--ssid", required=True, help="exact profile name, for example Troys")
    parser.add_argument("--peer", required=True, help="explicit actual Root private on-link IPv4; fixed TCP port 80")
    parser.add_argument("--http-port", type=int, default=8767)
    args = parser.parse_args(argv)
    if not 1 <= args.http_port <= 65535:
        parser.error("HTTP port extent")
    connection = None
    try:
        connection = CarrierConnection(args.port, args.network_file, args.ssid, args.peer)
        connection.connect()
        server = HTTPServer(("127.0.0.1", args.http_port), handler(connection))
        print(f"Pico carrier loopback ready at http://127.0.0.1:{args.http_port}/ (selected peer {connection.peer_host}:80; transport only)", flush=True)
        try:
            server.serve_forever()
        finally:
            server.server_close()
        return 0
    except (CarrierError, OSError, KeyboardInterrupt):
        print("Pico carrier stopped or refused; no HTTP request was replayed", file=__import__('sys').stderr)
        return 1
    finally:
        if connection is not None:
            connection.shutdown()


if __name__ == "__main__":
    raise SystemExit(main())
