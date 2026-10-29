"""Injected CPU transport/private-file proof; no actual USB or Wi-Fi is used."""
import contextlib
import importlib.util
import io
import json
import os
from pathlib import Path
import tempfile
import socket
import threading
import unittest
from unittest import mock

spec = importlib.util.spec_from_file_location("green_carrier", Path(__file__).parents[1] / "carrier.py")
c = importlib.util.module_from_spec(spec)
spec.loader.exec_module(c)
PASSWORD = b"synthetic-test-key"
PEER = "10.0.0.182"


def frame(kind, identity, payload):
    return c.HEADER.pack(b"CPBR", 1, kind, identity, len(payload)) + payload


def status(**changes):
    value = {"role": c.ROLE, "build": "a" * 64, "configuration": "fixture-green-config", "body_member": False,
             "radio_ready": True, "link_up": True, "ip": [10, 0, 0, 181], "attachment_generation": 7, "peer_ready": False}
    value.update(changes)
    return value


def information(identity, **changes):
    return frame(c.INFORMATION, identity, json.dumps(status(**changes)).encode())


def joined():
    return information(1) + information(2) + information(3, peer_ready=True)


def response(identity, raw):
    return frame(c.DATA, identity, raw[:7]) + frame(c.DATA, identity, raw[7:]) + frame(c.DONE, identity, len(raw).to_bytes(4, "little"))


def outgoing(stream):
    data, result = bytes(stream.outgoing), []
    while data:
        magic, version, kind, identity, length = c.HEADER.unpack(data[:c.HEADER.size])
        assert (magic, version) == (b"CPBR", 1)
        result.append((kind, identity, data[c.HEADER.size:c.HEADER.size + length]))
        data = data[c.HEADER.size + length:]
    return result


class Stream:
    def __init__(self, incoming):
        self.incoming, self.outgoing = bytearray(incoming), bytearray()
        self.events, self.closed = [], False
    @property
    def dtr(self):
        return self._dtr
    @dtr.setter
    def dtr(self, value):
        self._dtr = value
        self.events.append(("dtr", value))
    def reset_input_buffer(self):
        self.events.append(("discard-stale",))
    def close(self):
        self.closed = True
    def read(self, length):
        if not self.incoming:
            raise OSError("synthetic USB interruption")
        result = self.incoming[:min(length, 3)]
        del self.incoming[:len(result)]
        return result
    def write(self, value):
        n = min(7, len(value))
        self.outgoing.extend(value[:n])
        return n


class Transport(unittest.TestCase):
    def connection(self, streams):
        secrets, opens = [], []
        queue = iter(streams)
        def opener(*args, **kwargs):
            opens.append((args, kwargs))
            return next(queue)
        def loader(filename, ssid):
            self.assertEqual((filename, ssid), ("private-fixture", "Troys"))
            password = bytearray(PASSWORD)
            secrets.append(password)
            return password
        connection = c.CarrierConnection("/dev/fixture-green", "private-fixture", "Troys", PEER, opener=opener,
                                         loader=loader, sleep=lambda seconds: streams[len(opens)-1].events.append(("hold", seconds)))
        return connection, secrets, opens

    def test_new_role_always_joins_canonical_profile_and_selects_peer_before_http(self):
        raw = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}"
        stream = Stream(joined() + response(4, raw))
        connection, secrets, opens = self.connection([stream])
        self.assertEqual(connection.exchange(c.REQUEST, b"GET /body HTTP/1.1\r\n\r\n"), raw)
        sent = outgoing(stream)
        self.assertEqual([row[0] for row in sent], [c.STATUS, c.JOIN, c.SELECT_PEER, c.REQUEST])
        self.assertEqual(sent[1][2], b"CNJ1\x01\x05" + bytes([len(PASSWORD)]) + b"Troys" + PASSWORD)
        self.assertEqual(sent[2][2], bytes([10, 0, 0, 182]))
        self.assertEqual(stream.events, [("dtr", False), ("hold", 0.65), ("discard-stale",), ("dtr", True)])
        self.assertTrue(opens[0][1]["exclusive"])
        connection.shutdown()
        self.assertEqual(secrets[0], bytearray(len(PASSWORD)))
        self.assertTrue(stream.closed)

    def test_old_role_and_member_impersonation_never_send_join(self):
        for changes in ({"role": "conduit.pico-usb-http-relay@1"}, {"body_member": True}):
            stream = Stream(information(1, **changes))
            connection, secrets, _ = self.connection([stream])
            with self.assertRaises(c.CarrierError):
                connection.exchange(c.REQUEST, b"GET / HTTP/1.1\r\n\r\n")
            self.assertEqual([row[0] for row in outgoing(stream)], [c.STATUS])
            self.assertTrue(stream.closed)
            self.assertEqual(secrets[0], bytearray(len(PASSWORD)))

    def test_ambiguous_http_is_not_replayed_and_later_request_gets_fresh_session(self):
        first = Stream(joined() + frame(c.DATA, 4, b"HTTP/1.1 200 incomplete"))
        raw = b"HTTP/1.1 202 Accepted\r\nContent-Length: 2\r\n\r\n{}"
        second = Stream(joined() + response(4, raw))
        connection, secrets, _ = self.connection([first, second])
        initial = b"POST /mutating HTTP/1.1\r\nContent-Length: 1\r\n\r\na"
        later = b"GET /later HTTP/1.1\r\n\r\n"
        with self.assertRaises(OSError):
            connection.exchange(c.REQUEST, initial)
        self.assertTrue(first.closed)
        self.assertEqual(secrets[0], bytearray(len(PASSWORD)))
        self.assertEqual(connection.exchange(c.REQUEST, later), raw)
        self.assertEqual([p for k, _, p in outgoing(first) if k == c.REQUEST], [initial])
        self.assertEqual([p for k, _, p in outgoing(second) if k == c.REQUEST], [later])
        connection.shutdown()
        self.assertEqual(secrets[1], bytearray(len(PASSWORD)))
        with self.assertRaises(c.CarrierError):
            connection.exchange(c.REQUEST, later)

    def test_selected_peer_scope_identity_and_frame_extents_refuse(self):
        for changes in ({"attachment_generation": 8}, {"peer_ready": False}, {"ip": [10,0,0,180]}, {"build": "b"*64}):
            stream = Stream(information(1) + information(2) + information(3, **dict(changes, peer_ready=changes.get("peer_ready", True))))
            connection, _, _ = self.connection([stream])
            with self.assertRaises(c.CarrierError):
                connection.exchange(c.REQUEST, b"never")
            self.assertNotIn(c.REQUEST, [row[0] for row in outgoing(stream)])
        for peer in ("8.8.8.8", "127.0.0.1", "169.254.1.1", "224.0.0.1", "10.0.0.2:80", "hostname.local"):
            with self.assertRaises(c.CarrierError):
                c.private_ipv4(peer)
        for data in (frame(c.DONE, 1, b"\0"*4), frame(c.INFORMATION, 2, b"{}"), frame(c.DATA, 1, b"x"*513), frame(c.DATA, 1, b"HTTP") + frame(c.DONE, 1, (5).to_bytes(4,"little"))):
            with self.assertRaises(c.CarrierError):
                c.Carrier(Stream(data)).exchange(c.REQUEST, b"GET / HTTP/1.1\r\n\r\n")
        stream = Stream(frame(c.ERROR, 1, PASSWORD))
        with self.assertRaises(c.CarrierError) as refusal:
            c.Carrier(stream).exchange(c.JOIN, b"fixture")
        self.assertNotIn(PASSWORD.decode(), str(refusal.exception))

    def test_mdns_requires_selected_current_peer_and_exact_nonce_answer(self):
        hostname, nonce = "conduit-climate-b0a732286178.local", bytes(range(1,9))
        info = {"role": c.ROLE,"build":"a"*64,"configuration":"fixture-green-config","body_member":False,
                "observation":"selected-peer-mdns-a","nonce":nonce.hex(),"query_id":int.from_bytes(nonce[:2],"big"),
                "hostname":hostname,"address":[10,0,0,182],"source":[10,0,0,182],"source_port":5353,"attachment_generation":7}
        stream=Stream(joined()+frame(c.INFORMATION,4,json.dumps(info).encode()))
        connection,_,_=self.connection([stream])
        self.assertEqual(connection.observe_mdns(hostname,nonce),info)
        self.assertEqual(outgoing(stream)[-1],(c.MDNS_QUERY,4,nonce+hostname.encode()))
        connection.shutdown()
        with self.assertRaises(c.CarrierError):
            c.Carrier(Stream(b"")).observe_mdns(hostname,PEER,7,nonce)

    def test_large_scoped_mdns_information_does_not_expand_normal_status_limit(self):
        hostname, nonce = "conduit-climate-b0a732286178.local", bytes(range(1,9))
        info = {"role":c.ROLE,"build":"a"*64,"configuration":"fixture-green-config","body_member":False,
                "observation":"selected-peer-mdns-a","nonce":nonce.hex(),"query_id":int.from_bytes(nonce[:2],"big"),
                "hostname":hostname,"address":[10,0,0,182],"source":[10,0,0,182],"source_port":5353,"attachment_generation":7}
        encoded=json.dumps(info).encode()
        # Whitespace remains valid JSON and independently crosses the old bound.
        encoded+=b" "*max(0,600-len(encoded))
        self.assertGreater(len(encoded),512)
        self.assertLessEqual(len(encoded),768)
        stream=Stream(joined()+frame(c.INFORMATION,4,encoded))
        connection,_,_=self.connection([stream])
        self.assertEqual(connection.observe_mdns(hostname,nonce),info)
        connection.shutdown()
        oversized=json.dumps(status()).encode()
        oversized+=b" "*max(0,600-len(oversized))
        self.assertGreater(len(oversized),512)
        self.assertLessEqual(len(oversized),768)
        for kind in (c.STATUS,c.JOIN,c.SELECT_PEER):
            with self.assertRaises(c.CarrierError):
                c.Carrier(Stream(frame(c.INFORMATION,1,oversized))).exchange(kind)
        with self.assertRaises(c.CarrierError):
            c.Carrier(Stream(frame(c.INFORMATION,1,encoded+b" "*769))).exchange(c.MDNS_QUERY)

    def test_environment_uses_owned_session_without_join_or_peer_request(self):
        info = {"role":c.ROLE,"build":"a"*64,"configuration":"fixture-green-config",
                "body_member":False,"attempts":10,"samples":9,"frames":20,"acks":19,"heap_peak":300,
                "display_frame":[0,1,2,3]}
        stream=Stream(joined()+frame(c.INFORMATION,4,json.dumps(info).encode()))
        connection,_,opens=self.connection([stream])
        with self.assertRaises(c.CarrierError): connection.environment()
        self.assertEqual(opens,[])
        connection.connect()
        self.assertEqual(connection.environment(),info)
        self.assertEqual([k for k,_,_ in outgoing(stream)],[c.STATUS,c.JOIN,c.SELECT_PEER,c.SENSOR_STATUS])
        self.assertEqual(len(opens),1)
        connection.shutdown()
        with self.assertRaises(c.CarrierError): connection.environment()
        self.assertEqual(len(opens),1)

    def test_environment_deadline_and_sequence_exhaustion_are_finite(self):
        stream=Stream(b"")
        client=c.Carrier(stream,clock=iter([0,6]).__next__)
        client.identity=("a"*64,"fixture-green-config")
        with self.assertRaisesRegex(c.CarrierError,"deadline"):
            client.exchange(c.SENSOR_STATUS,timeout=5)
        self.assertEqual([k for k,_,_ in outgoing(stream)],[c.SENSOR_STATUS])
        client.sequence=0xFFFFFFFF
        with self.assertRaisesRegex(c.CarrierError,"exhausted"):
            client.exchange(c.SENSOR_STATUS)

    def test_environment_refuses_identity_counter_capacity_and_framing_errors(self):
        info={"role":c.ROLE,"build":"a"*64,"configuration":"fixture-green-config",
              "body_member":False,"attempts":10,"samples":9,"frames":20,"acks":19,"heap_peak":300}
        bad=[]
        for changes in ({"body_member":True},{"build":"b"*64},{"configuration":"other"},
                        {"samples":11},{"acks":21},{"frames":True},{"heap_peak":-1}):
            bad.append(frame(c.INFORMATION,4,json.dumps(dict(info,**changes)).encode()))
        bad.extend([frame(c.INFORMATION,5,json.dumps(info).encode()),frame(c.INFORMATION,4,b" "*1537),
                    frame(c.DATA,4,b"x"),frame(c.DONE,4,b"\0"*4),frame(c.ERROR,4,PASSWORD),
                    frame(c.INFORMATION,4,b'{"role":1,"role":2}')])
        for reply in bad:
            stream=Stream(joined()+reply)
            connection,_,opens=self.connection([stream]);connection.connect()
            with self.assertRaises(c.CarrierError): connection.environment()
            self.assertTrue(stream.closed)
            with self.assertRaises(c.CarrierError): connection.environment()
            self.assertEqual(len(opens),1)
        large=json.dumps(info).encode()+b" "*900
        client=c.Carrier(Stream(frame(c.INFORMATION,1,large)))
        client.identity=(info["build"],info["configuration"])
        self.assertEqual(client.exchange(c.SENSOR_STATUS),info)
        with self.assertRaises(c.CarrierError):
            c.Carrier(Stream(b"")).exchange(c.SENSOR_STATUS,b"not empty")


class PrivateConfiguration(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.directory=Path(self.temp.name)
        self.directory.chmod(0o700)
        self.file=self.directory/"known.json"
        self.document={"schema":"conduit.private/known-wifi@1","profiles":[{"ssid":"Troys","password":PASSWORD.decode(),"security":"wpa2-personal","priority":0}]}
    def save(self,document=None):
        self.file.write_text(json.dumps(self.document if document is None else document))
        self.file.chmod(0o600)
    def test_exact_profile_private_custody_bounds_and_no_secret_logging(self):
        self.save()
        self.assertEqual(c.load_profile(self.file,"Troys"),bytearray(PASSWORD))
        with self.assertRaises(c.CarrierError): c.load_profile(self.file,"Troy")
        with self.assertRaises(c.CarrierError): c.load_profile(self.file,"Troys",uid=os.getuid()+1)
        self.file.chmod(0o644)
        with self.assertRaises(c.CarrierError): c.load_profile(self.file,"Troys")
        self.file.chmod(0o600)
        link=self.directory/"symlink";link.symlink_to(self.file)
        with self.assertRaises(c.CarrierError): c.load_profile(link,"Troys")
        self.file.write_bytes(b"x"*8193)
        with self.assertRaises(c.CarrierError): c.load_profile(self.file,"Troys")
        for value in ({"schema":"other","profiles":[]},dict(self.document,profiles=self.document["profiles"]*2),dict(self.document,profiles=[])):
            self.save(value)
            with self.assertRaises(c.CarrierError) as refusal: c.load_profile(self.file,"Troys")
            self.assertNotIn(PASSWORD.decode(),str(refusal.exception))
    def test_cli_has_no_password_argument_and_refusals_do_not_echo_device_errors(self):
        self.save()
        stderr=io.StringIO()
        with mock.patch.object(c.CarrierConnection,"connect",side_effect=c.CarrierError(PASSWORD.decode())),contextlib.redirect_stderr(stderr):
            result=c.main(["--port","/dev/fixture","--network-file",str(self.file),"--ssid","Troys","--peer",PEER])
        self.assertEqual(result,1)
        self.assertNotIn(PASSWORD.decode(),stderr.getvalue())
        self.assertIsNot(c.handler,c._http.handler)


class HttpBoundary(unittest.TestCase):
    def roundtrip(self,request,diagnostic_failure=False,nonlocal_client=False):
        class SelectedTransport:
            peer_host=PEER
            def __init__(self): self.requests=[]
            def environment(self):
                self.requests.append((c.SENSOR_STATUS,b""))
                if diagnostic_failure: raise c.CarrierError("private-device-error")
                return {"role":c.ROLE,"build":"a"*64,"configuration":"yellow-fixture","body_member":False,
                        "frames":12,"acks":11}
            def exchange(self,kind,payload):
                self.requests.append((kind,payload))
                return b"HTTP/1.1 202 Accepted\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}"
        transport=SelectedTransport()
        selected_handler=c.handler(transport)
        if nonlocal_client:
            class NonlocalHandler(selected_handler):
                def setup(self):
                    super().setup()
                    self.client_address=("192.168.0.2",self.client_address[1])
            selected_handler=NonlocalHandler
        server=c.HTTPServer(("127.0.0.1",0),selected_handler)
        worker=threading.Thread(target=server.handle_request)
        stderr=io.StringIO()
        try:
            with contextlib.redirect_stderr(stderr):
                worker.start()
                with socket.create_connection(server.server_address,timeout=2) as client:
                    client.sendall(request)
                    client.shutdown(socket.SHUT_WR)
                    received=bytearray()
                    while True:
                        chunk=client.recv(65536)
                        if not chunk: break
                        received.extend(chunk)
                worker.join(2)
            self.assertFalse(worker.is_alive())
            self.assertEqual(stderr.getvalue(),"")
            return bytes(received),transport.requests
        finally: server.server_close()
    def test_real_loopback_frontend_forwards_once_to_only_selected_host_without_logs(self):
        body=b"synthetic-private-body"
        request=b"POST /private-test HTTP/1.1\r\nHost: localhost\r\nContent-Length: "+str(len(body)).encode()+b"\r\n\r\n"+body
        response,requests=self.roundtrip(request)
        self.assertTrue(response.startswith(b"HTTP/1.1 202 Accepted"))
        self.assertEqual(len(requests),1)
        self.assertEqual(requests[0][0],c.REQUEST)
        self.assertIn(b"Host: "+PEER.encode()+b"\r\n",requests[0][1])
        self.assertTrue(requests[0][1].endswith(body))
    def test_local_environment_route_retains_provenance_and_never_forwards_http(self):
        reply,requests=self.roundtrip(b"GET /__carrier/environment HTTP/1.1\r\nHost: localhost\r\n\r\n")
        self.assertTrue(reply.startswith(b"HTTP/1.1 200"))
        self.assertEqual(requests,[(c.SENSOR_STATUS,b"")])
        self.assertIn(b"Cache-Control: no-store",reply)
        document=json.loads(reply.split(b"\r\n\r\n",1)[1])
        self.assertFalse(document["body_member"])
        self.assertEqual(document["transport"],"owned-usb-session")
        self.assertEqual(document["environment"]["configuration"],"yellow-fixture")
        reply,requests=self.roundtrip(b"GET /__carrier/environment HTTP/1.1\r\n\r\n",True)
        self.assertTrue(reply.startswith(b"HTTP/1.1 502"))
        self.assertNotIn(b"private-device-error",reply)
        reply,requests=self.roundtrip(b"GET /__carrier/environment HTTP/1.1\r\n\r\n",nonlocal_client=True)
        self.assertTrue(reply.startswith(b"HTTP/1.1 403"))
        self.assertEqual(requests,[])
        for request,code in ((b"POST /__carrier/environment HTTP/1.1\r\n\r\n",405),
                             (b"GET /__carrier/environment?x HTTP/1.1\r\n\r\n",400),
                             (b"GET /__carrier/environment HTTP/1.1\r\nContent-Length: 1\r\n\r\nx",400)):
            reply,requests=self.roundtrip(request)
            self.assertTrue(reply.startswith(b"HTTP/1.1 "+str(code).encode()))
            self.assertEqual(requests,[])

    def test_duplicate_length_chunked_and_total_request_capacity_never_reach_usb(self):
        for headers,expected in ((b"Content-Length: 1\r\nContent-Length: 1\r\n",400),(b"Transfer-Encoding: chunked\r\n",400),(b"Content-Length: 8192\r\n",413),(b"X-Large: "+b"x"*8192+b"\r\n",413)):
            response,requests=self.roundtrip(b"POST / HTTP/1.1\r\nHost: localhost\r\n"+headers+b"\r\n")
            self.assertTrue(response.startswith(b"HTTP/1.1 "+str(expected).encode()))
            self.assertEqual(requests,[])


if __name__ == "__main__": unittest.main()
