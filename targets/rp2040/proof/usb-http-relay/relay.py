#!/usr/bin/env python3
"""Loopback HTTP access to the fixed ESP AP through a native USB Pico relay.

This transport creates no Body or membership evidence. It forwards complete
HTTP responses and body bytes unchanged, and never retries an HTTP command.
"""
import argparse
import json
import struct
import threading
import time
from http.server import BaseHTTPRequestHandler, HTTPServer

HEADER = struct.Struct("<4sBBIH")
ROLE = "conduit.pico-usb-http-relay@1"
MAX_REQUEST, MAX_RESPONSE, CHUNK = 8192, 65536, 512
JOIN, REQUEST, STATUS, SENSOR_STATUS, SENSOR_DIAGNOSTIC = 1, 2, 3, 6, 7
DATA, DONE, ERROR, INFORMATION = 128, 129, 130, 131
BOOTSEL_QUERY, BOOTSEL_REQUEST, BOOTSEL_CHALLENGE, BOOTSEL_ACK = 4, 5, 132, 133


class RelayError(Exception):
    pass


class Relay:
    def __init__(self, stream):
        self.stream = stream
        self.sequence = 0
        self.lock = threading.Lock()

    def _read(self, length, deadline):
        result = bytearray()
        while len(result) < length:
            if time.monotonic() >= deadline:
                raise RelayError("USB reply deadline")
            chunk = self.stream.read(length - len(result))
            if len(chunk) > length - len(result):
                raise RelayError("invalid USB read")
            result.extend(chunk)
        return bytes(result)

    def exchange(self, kind, payload=b"", timeout=40):
        if len(payload) > MAX_REQUEST:
            raise RelayError("request capacity")
        with self.lock:
            if self.sequence == 0xFFFFFFFF:
                raise RelayError("USB session sequence exhausted")
            self.sequence += 1
            identity = self.sequence
            wire = bytearray(HEADER.pack(b"CPBR", 1, kind, identity, len(payload)))
            wire.extend(payload)
            try:
                offset = 0
                while offset < len(wire):
                    written = self.stream.write(memoryview(wire)[offset:])
                    if not written or written > len(wire) - offset:
                        raise RelayError("USB write incomplete")
                    offset += written
                deadline = time.monotonic() + timeout
                result = bytearray()
                while True:
                    magic, version, response, response_id, length = HEADER.unpack(
                        self._read(HEADER.size, deadline)
                    )
                    if magic != b"CPBR" or version != 1 or response_id != identity:
                        raise RelayError("USB response scope")
                    limit = {DATA: CHUNK, DONE: 4, INFORMATION: 512, ERROR: 128, BOOTSEL_CHALLENGE: 64, BOOTSEL_ACK: 64}.get(response)
                    if limit is None or length > limit:
                        raise RelayError("USB response frame capacity")
                    data = self._read(length, deadline)
                    if response in (BOOTSEL_CHALLENGE, BOOTSEL_ACK):
                        expected = BOOTSEL_CHALLENGE if kind == BOOTSEL_QUERY else BOOTSEL_ACK if kind == BOOTSEL_REQUEST else None
                        if response != expected or length != 64 or any(c not in b"0123456789abcdef" for c in data):
                            raise RelayError("invalid BOOTSEL build scope")
                        if kind == BOOTSEL_REQUEST and data != payload:
                            raise RelayError("BOOTSEL acknowledgement build mismatch")
                        return data
                    if response == ERROR:
                        # Firmware errors are fixed codes, never echoed request data.
                        raise RelayError(data.decode("ascii", "replace"))
                    if response == INFORMATION:
                        if kind not in (STATUS, JOIN, SENSOR_STATUS, SENSOR_DIAGNOSTIC) or result:
                            raise RelayError("unexpected information reply")
                        try:
                            status = json.loads(data)
                        except (ValueError, UnicodeError) as error:
                            raise RelayError("invalid device status") from error
                        if not isinstance(status, dict) or status.get("role") != ROLE:
                            raise RelayError("not the native Pico HTTP relay")
                        return status
                    if response == DATA:
                        if kind != REQUEST or not data or len(result) + length > MAX_RESPONSE:
                            raise RelayError("HTTP response capacity")
                        result.extend(data)
                    elif response == DONE:
                        if kind != REQUEST or length != 4 or not result:
                            raise RelayError("unexpected completion")
                        if int.from_bytes(data, "little") != len(result):
                            raise RelayError("HTTP response length")
                        return bytes(result)
            finally:
                wire[:] = b"\0" * len(wire)

    def reboot_bootsel(self, expected_build):
        """Explicit developer action, never called by HTTP forwarding/reconnect."""
        if len(expected_build) != 64:
            raise RelayError("expected exact build identity")
        actual = self.exchange(BOOTSEL_QUERY)
        if actual != expected_build:
            raise RelayError("BOOTSEL challenge build mismatch")
        return self.exchange(BOOTSEL_REQUEST, actual)

    def join(self, ssid, password=b""):
        ssid = ssid.encode("utf-8")
        if not 1 <= len(ssid) <= 32 or len(password) > 63 or (password and len(password) < 8):
            raise RelayError("credential capacity")
        if b"\0" in ssid or b"\0" in password:
            raise RelayError("invalid credentials")
        request = bytearray([len(ssid), len(password)]) + ssid + password
        try:
            status = self.exchange(JOIN, request, timeout=60)
        finally:
            request[:] = b"\0" * len(request)
        if not status.get("link") or not status.get("configured") or not status.get("peer_ready"):
            raise RelayError("Wi-Fi has no configured link")
        return status


class PicoConnection:
    """Reconnect only for a later request; an ambiguous HTTP command is never replayed."""
    def __init__(self, port, ssid, password, opener=None):
        self.port, self.ssid, self.password = port, ssid, password
        self.opener = opener
        self.stream = self.client = None
        self.lock = threading.Lock()

    def _close(self):
        stream = self.stream
        self.stream = self.client = None
        if stream is not None:
            try:
                stream.close()
            except OSError:
                pass

    def connect(self):
        if self.opener is None:
            import serial
            opener = serial.Serial
        else:
            opener = self.opener
        self.stream = opener(self.port, 115200, timeout=0.5, write_timeout=2, exclusive=True)
        self.stream.dtr = False
        time.sleep(0.1)
        self.stream.reset_input_buffer()
        self.stream.dtr = True
        self.client = Relay(self.stream)
        self.client.exchange(STATUS, timeout=45)
        self.client.join(self.ssid, self.password)

    def exchange(self, *args, **kwargs):
        with self.lock:
            try:
                if self.client is None:
                    self.connect()
                return self.client.exchange(*args, **kwargs)
            except (RelayError, OSError):
                self._close()
                raise

    def shutdown(self):
        with self.lock:
            self._close()
            self.password[:] = b"\0" * len(self.password)


def handler(relay):
    class Proxy(BaseHTTPRequestHandler):
        protocol_version = "HTTP/1.1"

        def setup(self):
            super().setup()
            self.connection.settimeout(10)

        def log_message(self, *_):
            pass  # Paths, invitations and provisioning bodies must not be logged.

        def forward(self):
            try:
                if not self.path.startswith("/") or "\r" in self.path or "\n" in self.path:
                    self.send_error(400)
                    return
                lengths = self.headers.get_all("Content-Length", [])
                if len(lengths) > 1 or self.headers.get("Transfer-Encoding"):
                    self.send_error(400)
                    return
                length = int(lengths[0]) if lengths else 0
                if not 0 <= length <= MAX_REQUEST:
                    self.send_error(413)
                    return
                lines = [f"{self.command} {self.path} HTTP/1.1", "Host: 192.168.4.1", "Connection: close"]
                for name, value in self.headers.items():
                    if name.lower() in ("host", "connection", "proxy-connection"):
                        continue
                    if "\r" in value or "\n" in value:
                        self.send_error(400)
                        return
                    lines.append(f"{name}: {value}")
                head = ("\r\n".join(lines) + "\r\n\r\n").encode("latin-1")
                if len(head) + length > MAX_REQUEST:
                    self.send_error(413)
                    return
                body = self.rfile.read(length)
                if len(body) != length:
                    self.send_error(400)
                    return
                raw = relay.exchange(REQUEST, head + body)
                if not raw.startswith((b"HTTP/1.0 ", b"HTTP/1.1 ")):
                    raise RelayError("invalid HTTP peer reply")
                # Wait for complete DONE before exposing any body to the browser.
                self.connection.sendall(raw)
                self.close_connection = True
            except (RelayError, OSError, ValueError):
                self.send_error(502, "Pico to ESP transport refused; HTTP request was not replayed")
                self.close_connection = True

        do_GET = do_POST = do_HEAD = do_PUT = do_DELETE = do_OPTIONS = forward
    return Proxy


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--port", required=True, help="explicit Pico relay CDC device, never the ESP UART")
    parser.add_argument("--ssid", help="actual ESP access-point SSID")
    parser.add_argument("--ask-password", action="store_true", help="prompt without writing credentials to disk")
    parser.add_argument("--http-port", type=int, default=8766)
    parser.add_argument("--bootsel-build", help="explicitly reboot only the exact 64-hex firmware build; no Wi-Fi join")
    args = parser.parse_args()
    if args.bootsel_build:
        import serial
        with serial.Serial(args.port, 115200, timeout=0.2, write_timeout=2) as stream:
            stream.dtr = False
            time.sleep(0.1)
            stream.dtr = True
            Relay(stream).reboot_bootsel(args.bootsel_build.encode("ascii"))
        print("Exact-build BOOTSEL acknowledged")
        return
    if not args.ssid:
        parser.error("--ssid is required for HTTP proxy mode")
    password = bytearray()
    if args.ask_password:
        import getpass
        password.extend(getpass.getpass("ESP AP password: ").encode("utf-8"))
    relay = PicoConnection(args.port, args.ssid, password)
    try:
        relay.connect()  # Actual role, association and DHCP must pass before ready.
        server = HTTPServer(("127.0.0.1", args.http_port), handler(relay))
        print(f"Native USB relay ready at http://127.0.0.1:{args.http_port}/ (fixed ESP AP peer)")
        try:
            server.serve_forever()
        finally:
            server.server_close()
    finally:
        relay.shutdown()


if __name__ == "__main__":
    main()
