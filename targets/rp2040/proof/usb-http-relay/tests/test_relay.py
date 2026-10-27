import importlib.util
import pathlib
import unittest

spec = importlib.util.spec_from_file_location("relay", pathlib.Path(__file__).parents[1] / "relay.py")
r = importlib.util.module_from_spec(spec)
spec.loader.exec_module(r)


def frame(kind, identity, data):
    return r.HEADER.pack(b"CPBR", 1, kind, identity, len(data)) + data


class Stream:
    def __init__(self, incoming):
        self.incoming = bytearray(incoming)
        self.outgoing = bytearray()
    def read(self, length):
        data = self.incoming[:min(length, 3)]
        del self.incoming[:len(data)]
        return data
    def write(self, data):
        self.outgoing.extend(data[:7])
        return min(len(data), 7)


class Proof(unittest.TestCase):
    def test_complete_raw_http_bytes_and_request_body_are_unchanged(self):
        body = b'{"actual":"canonical\\u00e9","bytes":[0,255]}'
        raw = b"HTTP/1.1 200 OK\r\nContent-Length: " + str(len(body)).encode() + b"\r\n\r\n" + body
        incoming = frame(r.DATA, 1, raw[:20]) + frame(r.DATA, 1, raw[20:]) + frame(r.DONE, 1, len(raw).to_bytes(4, "little"))
        stream = Stream(incoming)
        request = b"POST /actual HTTP/1.1\r\nConnection: close\r\n\r\n" + body
        self.assertEqual(r.Relay(stream).exchange(r.REQUEST, request), raw)
        self.assertEqual(bytes(stream.outgoing), frame(r.REQUEST, 1, request))

    def test_wrong_role_identity_kind_completion_and_capacity_refuse(self):
        for data in [frame(r.INFORMATION, 1, b'{"role":"other"}'), frame(r.INFORMATION,1,b"[]"), frame(r.DATA, 2, b"x"), frame(255, 1, b""), frame(r.DONE, 1, b"\0"*4), frame(r.DATA, 1, b"x") + frame(r.DONE, 1, (2).to_bytes(4,"little")), r.HEADER.pack(b"CPBR", 1, r.DATA, 1, 513)]:
            with self.assertRaises(r.RelayError):
                r.Relay(Stream(data)).exchange(r.REQUEST, b"GET / HTTP/1.0\r\n\r\n", timeout=0.01)
        self.assertRaises(r.RelayError, r.Relay(Stream(b"")).exchange, r.REQUEST, b"x" * 8193)

    def test_credential_input_and_status_scope_are_bounded(self):
        status = frame(r.INFORMATION,1,b'{"role":"conduit.pico-usb-http-relay@1","link":true,"configured":true,"peer_ready":true}')
        relay = r.Relay(Stream(status))
        self.assertTrue(relay.join("Actual ESP AP")["link"])
        for ssid, password in [("", b""), ("s"*33, b""), ("a", b"short"), ("a", b"x"*64), ("a\0", b"")]:
            self.assertRaises(r.RelayError, r.Relay(Stream(b"")).join, ssid, password)

    def test_no_mutating_request_is_automatically_replayed_after_transport_error(self):
        stream = Stream(frame(r.ERROR,1,b"tcp-read"))
        relay = r.Relay(stream)
        self.assertRaises(r.RelayError, relay.exchange, r.REQUEST, b"POST /commit HTTP/1.0\r\n\r\n")
        self.assertEqual(bytes(stream.outgoing).count(b"POST /commit"), 1)

    def test_transmit_backing_is_wiped_even_when_reply_refuses(self):
        class ViewStream(Stream):
            def __init__(self, data):
                super().__init__(data)
                self.views = []
            def write(self, data):
                self.views.append(data)
                return super().write(data)
        stream = ViewStream(frame(r.ERROR,1,b"association-failed"))
        self.assertRaises(r.RelayError, r.Relay(stream).join, "SSID", bytearray(b"password"))
        self.assertTrue(all(not any(view) for view in stream.views))

    def test_reconnect_occurs_only_for_a_later_distinct_request(self):
        calls = []
        class Client:
            def __init__(self, fail): self.fail = fail
            def exchange(self, kind, payload):
                calls.append(payload)
                if self.fail: raise OSError("USB removed")
                return b"HTTP/1.1 200 OK\r\n\r\nnext"
        class Connection(r.PicoConnection):
            def connect(self): self.client = Client(False)
        connection = Connection("unused", "actual", bytearray())
        connection.client = Client(True)
        self.assertRaises(OSError, connection.exchange, r.REQUEST, b"POST first")
        self.assertIsNone(connection.client)
        self.assertEqual(connection.exchange(r.REQUEST,b"GET second"),b"HTTP/1.1 200 OK\r\n\r\nnext")
        self.assertEqual(calls,[b"POST first",b"GET second"])
        connection.shutdown()

    def test_bootsel_is_explicit_and_bound_to_exact_challenged_build(self):
        build = b"a" * 64
        stream = Stream(frame(r.BOOTSEL_CHALLENGE,1,build)+frame(r.BOOTSEL_ACK,2,build))
        self.assertEqual(r.Relay(stream).reboot_bootsel(build), build)
        self.assertEqual(bytes(stream.outgoing), frame(r.BOOTSEL_QUERY,1,b"")+frame(r.BOOTSEL_REQUEST,2,build))
        stream = Stream(frame(r.BOOTSEL_CHALLENGE,1,b"b"*64))
        self.assertRaises(r.RelayError,r.Relay(stream).reboot_bootsel,build)
        self.assertEqual(bytes(stream.outgoing),frame(r.BOOTSEL_QUERY,1,b""))
        for kind, data in [(r.BOOTSEL_CHALLENGE,b"z"*64),(r.BOOTSEL_ACK,build)]:
            self.assertRaises(r.RelayError,r.Relay(Stream(frame(kind,1,data))).exchange,r.BOOTSEL_QUERY)


if __name__ == "__main__":
    unittest.main()
