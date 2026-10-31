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
    def test_ap_transition_is_explicit_and_does_not_send_join_or_http(self):
        for command in (r.TEST_AP_START, r.TEST_AP_STOP):
            stream = Stream(frame(r.INFORMATION, 1, b'{"role":"conduit.pico-usb-http-relay@1","test_ap":true}'))
            peer = bytes.fromhex("b0a732286178") if command == r.TEST_AP_START else b""
            self.assertTrue(r.Relay(stream).exchange(command, peer)["test_ap"])
            self.assertEqual(bytes(stream.outgoing), frame(command, 1, peer))
        for invalid in (b"", b"\0" * 6, b"\xff" * 6, b"\x01" * 6):
            stream = Stream(b"")
            self.assertRaises(r.RelayError, r.Relay(stream).start_test_ap, invalid)
            self.assertEqual(bytes(stream.outgoing), b"")
        peer = bytes.fromhex("b0a732286178")
        for accepted in (True, False):
            status = (b'{"role":"conduit.pico-usb-http-relay@1","test_ap":true,'
                      b'"dhcp_serving":' + str(accepted).lower().encode() +
                      b',"peer":"192.168.4.2:80"}')
            client = r.Relay(Stream(frame(r.INFORMATION, 1, status)))
            if accepted:
                self.assertTrue(client.start_test_ap(peer)["dhcp_serving"])
            else:
                self.assertRaises(r.RelayError, client.start_test_ap, peer)

    def test_ap_proxy_reconnect_never_starts_ap_or_joins_implicitly(self):
        class Port(Stream):
            def reset_input_buffer(self): pass
            def close(self): pass
        for serving in (True, False):
            status = (b'{"role":"conduit.pico-usb-http-relay@1","test_ap":true,'
                      b'"dhcp_serving":' + str(serving).lower().encode() +
                      b',"peer_ready":true,"peer":"192.168.4.2:80"}')
            port = Port(frame(r.INFORMATION, 1, status))
            connection = r.PicoConnection("unused", None, bytearray(),
                opener=lambda *args, **kwargs: port, test_ap=True)
            if serving:
                connection.connect()
                self.assertEqual(connection.peer_host, "192.168.4.2")
            else:
                self.assertRaises(r.RelayError, connection.connect)
            self.assertEqual(bytes(port.outgoing), frame(r.STATUS, 1, b""))
            connection.shutdown()

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

    def test_missing_usb_startup_recovers_with_role_and_link_checks_before_http(self):
        class Port(Stream):
            def reset_input_buffer(self): pass
            def close(self): pass
        status = b'{"role":"conduit.pico-usb-http-relay@1","link":true,"configured":true,"peer_ready":true}'
        reply = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok"
        port = Port(frame(r.INFORMATION, 1, status) + frame(r.INFORMATION, 2, status)
                    + frame(r.DATA, 3, reply) + frame(r.DONE, 3, len(reply).to_bytes(4, "little")))
        attempts = []
        def opener(*args, **kwargs):
            attempts.append(1)
            if len(attempts) == 1:
                raise FileNotFoundError("USB absent")
            return port
        connection = r.PicoConnection("unused", "actual", bytearray(), opener=opener)
        self.assertFalse(connection.connect_initial(wait_for_device=True))
        self.assertIsNone(connection.client)
        request = b"GET / HTTP/1.0\r\n\r\n"
        self.assertEqual(connection.exchange(r.REQUEST, request), reply)
        self.assertEqual(bytes(port.outgoing), frame(r.STATUS, 1, b"")
                         + frame(r.JOIN, 2, b"\x06\0actual") + frame(r.REQUEST, 3, request))
        connection.shutdown()
        refused = r.PicoConnection("unused", "actual", bytearray(),
            opener=lambda *args, **kwargs: Port(frame(r.INFORMATION, 1, b'{"role":"other"}')))
        self.assertRaises(r.RelayError, refused.connect_initial, True)
        refused.shutdown()

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

    def test_sensor_diagnostics_do_not_require_wifi_association(self):
        for kind in (r.SENSOR_STATUS,r.SENSOR_DIAGNOSTIC):
            stream=Stream(frame(r.INFORMATION,1,b'{"role":"conduit.pico-usb-http-relay@1","status":"sensor-failed"}'))
            self.assertEqual(r.Relay(stream).exchange(kind)["status"],"sensor-failed")
            self.assertEqual(bytes(stream.outgoing),frame(kind,1,b""))



class MdnsProof(unittest.TestCase):
    def test_explicit_query_correlates_nonce_host_source_and_address(self):
        import json
        nonce = bytes.fromhex("1234567890abcdef")
        host = "conduit-climate-b0a732286178.local"
        reply = {"role":r.ROLE,"observation":"legacy-mdns-a","nonce":nonce.hex(),"query_id":0x1234,"hostname":host,"address":[192,168,4,2],"source":[192,168,4,2],"source_port":5353}
        stream = Stream(frame(r.INFORMATION,1,json.dumps(reply).encode()))
        self.assertEqual(r.Relay(stream).observe_mdns(host,nonce),reply)
        self.assertEqual(bytes(stream.outgoing),frame(r.MDNS_QUERY,1,nonce+host.encode()))
        for key,value in [("nonce","00"*8),("hostname",host+".evil"),("query_id",1),("source",[192,168,4,3]),("address",[192,168,4,3]),("source_port",53),("observation","fixture")]:
            altered = dict(reply);altered[key]=value
            self.assertRaises(r.RelayError,r.Relay(Stream(frame(r.INFORMATION,1,json.dumps(altered).encode()))).observe_mdns,host,nonce)
        for name in ("outside.local",host.upper(),host+".evil"):
            stream=Stream(b"")
            self.assertRaises(r.RelayError,r.Relay(stream).observe_mdns,name,nonce)
            self.assertEqual(bytes(stream.outgoing),b"")

if __name__ == "__main__":
    unittest.main()
