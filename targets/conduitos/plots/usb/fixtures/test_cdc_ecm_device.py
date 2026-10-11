"""Device-only protocol tests, below QEMU and Conduit Plan/Play proof."""
import struct
import unittest
import cdc_ecm_device as fixture


def frame(length=64):
    return bytes.fromhex('ffffffffffff0200000000010800') + bytes([0x5a]) * (length - 14)


class Device(fixture.Device):
    def __init__(self):
        super().__init__(None, None, 0, frame(14))
        self.sent = []
        self.configuration, self.packet_filter, self.alternate = 1, 15, 1

    def send(self, kind, packet_id, body=b''):
        self.sent.append((kind, packet_id, body))


class Contract(unittest.TestCase):
    def test_pinned_geometry_and_mac(self):
        self.assertEqual(len(fixture.CONFIG), 80)
        self.assertEqual(fixture.CONFIG[2:4], b'\x50\x00')
        self.assertEqual(fixture.STRINGS[4][2:].decode('utf-16le'), '020000000001')
        self.assertEqual(fixture.CONFIG[-14:], bytes.fromhex('0705810240000007050202400000'))

    def test_filter_unknown_and_wrong_interface_really_stall(self):
        device = Device()
        for value, index, status in [(15, 0, 0), (16, 0, 4), (15, 1, 4)]:
            device.control(7, fixture.CONTROL.pack(0, 0x43, 0x21, 0, value, index, 0))
            self.assertEqual(fixture.CONTROL.unpack(device.sent[-1][2])[3], status)
        device.control(8, fixture.CONTROL.pack(0, 0x20, 0x21, 0, 0, 0, 0))
        self.assertEqual(fixture.CONTROL.unpack(device.sent[-1][2])[3], fixture.STALL)

    def test_full_packet_needs_zlp_and_receives_identical_distinct_frame(self):
        device = Device()
        device.bulk(1, fixture.BULK.pack(2, 0, 64, 0, 0) + frame())
        self.assertEqual(len(device.partial_tx), 64)
        self.assertFalse(device.rx)
        device.bulk(2, fixture.BULK.pack(2, 0, 0, 0, 0))
        self.assertFalse(device.partial_tx)
        self.assertEqual(device.rx, frame(14))
        device.bulk(3, fixture.BULK.pack(129, 0, 1514, 0, 0))
        self.assertEqual(device.sent[-1][2][fixture.BULK.size:], frame(14))
        self.assertFalse(device.rx)

    def test_zero_short_oversized_and_pressure_do_not_invent_frames(self):
        device = Device()
        device.bulk(1, fixture.BULK.pack(2, 0, 0, 0, 0))
        self.assertEqual(fixture.BULK.unpack(device.sent[-1][2])[1], fixture.STALL)
        device.bulk(2, fixture.BULK.pack(2, 0, 13, 0, 0) + b'x' * 13)
        self.assertEqual(fixture.BULK.unpack(device.sent[-1][2])[1], fixture.STALL)
        device.bulk(3, fixture.BULK.pack(2, 0, 1515, 0, 0) + b'x' * 1515)
        self.assertEqual(fixture.BULK.unpack(device.sent[-1][2])[1], fixture.IOERROR)
        device.bulk(4, fixture.BULK.pack(2, 0, 14, 0, 0) + frame(14))
        device.bulk(5, fixture.BULK.pack(2, 0, 14, 0, 0) + frame(14))
        self.assertEqual(fixture.BULK.unpack(device.sent[-1][2])[1], fixture.IOERROR)
        self.assertEqual(device.rx, frame(14))

    def test_pending_read_cancel_has_one_exact_terminal_response(self):
        device = Device()
        device.bulk(11, fixture.BULK.pack(129, 0, 1514, 0, 0))
        device.dispatch(21, 11, b'')
        self.assertIsNone(device.pending_in)
        self.assertEqual(device.sent[-1][1], 11)
        self.assertEqual(fixture.BULK.unpack(device.sent[-1][2])[1], fixture.CANCELLED)
        count = len(device.sent)
        device.dispatch(21, 11, b'')
        self.assertEqual(len(device.sent), count)


if __name__ == '__main__':
    unittest.main()
