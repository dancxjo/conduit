"""Synthetic external device protocol tests; no native-controller proof."""
import importlib.util
from pathlib import Path
import struct
import unittest

spec = importlib.util.spec_from_file_location('audio_device', Path(__file__).with_name('audio_device.py'))
audio = importlib.util.module_from_spec(spec)
spec.loader.exec_module(audio)

class Fixture(audio.Device):
    def __init__(self):
        super().__init__(None, None, 0, 2)
        self.sent = []
    def send(self, kind, packet_id, body=b''):
        self.sent.append((kind, packet_id, body))

class AudioDeviceTests(unittest.TestCase):
    def test_descriptor_walk_matches_declared_extent_and_exact_format(self):
        data, offset = audio.CONFIG, 0
        self.assertEqual(len(data), int.from_bytes(data[2:4], 'little'))
        while offset < len(data):
            self.assertGreaterEqual(data[offset], 2)
            offset += data[offset]
        self.assertEqual(offset, 100)
        self.assertIn(bytes.fromhex('0b2402010202100180bb00'), data)
        self.assertIn(bytes.fromhex('0905010dc000010000'), data)
    def test_negotiation_and_wrong_alternate_rate_stall(self):
        device = Fixture()
        device.dispatch(6, 1, b'\x01')
        device.dispatch(9, 2, bytes([1,2]))
        self.assertEqual(device.sent[-1][2][0], audio.wire.STALL)
        self.assertEqual(device.alt, 0)
        device.dispatch(9, 3, bytes([1,1]))
        self.assertEqual(device.alt, 1)
        def rate(value):
            return audio.wire.CONTROL.pack(0, 1, 0x22, 0, 0x100, 1, 3)+value.to_bytes(3, 'little')
        device.control(4, rate(44100))
        self.assertEqual(device.sent[-1][2][3], audio.wire.STALL)
        device.control(5, rate(48000))
        self.assertEqual(device.sent[-1][2][3], audio.wire.SUCCESS)
        device.control(6, audio.wire.CONTROL.pack(128,0x81,0xa2,0,0x100,1,3))
        self.assertEqual(device.sent[-1][2][-3:], bytes.fromhex('80bb00'))
    def test_iso_has_no_invented_ack_and_pressure_refuses(self):
        device = Fixture()
        device.configuration, device.alt = 1,1
        device.dispatch(12, 1, bytes([1,8,2]))
        before = len(device.sent)
        payload = bytes((i*17+3)%256 for i in range(192))
        packet = struct.pack('<BBH',1,0,192)+payload
        device.dispatch(102, 2, packet)
        device.dispatch(102, 3, packet)
        self.assertEqual(len(device.sent), before)
        self.assertEqual(device.packets, 2)
        with self.assertRaises(ValueError):device.dispatch(102, 4, packet)
        self.assertEqual(device.packets, 2)
        device.dispatch(13, 5, b'\x01')
        with self.assertRaises(ValueError):device.dispatch(102, 6, packet)
    def test_malformed_packet_refuses_before_counter_mutation(self):
        device=Fixture();device.streaming=True
        for packet in [b'',struct.pack('<BBH',1,0,192),struct.pack('<BBH',2,0,192)+bytes(192)]:
            with self.assertRaises(ValueError):device.dispatch(102, 1, packet)
        self.assertEqual(device.packets,0)

if __name__ == '__main__': unittest.main()
