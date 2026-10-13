"""External device state tests; no Conduit/native execution claim."""
import importlib.util
from pathlib import Path
import struct
import sys
import unittest

class RecoveryDeviceTests(unittest.TestCase):
    def device(self, mode='read-reentry'):
        previous = sys.argv
        sys.argv = ['bot_device.py', '--fault-mode', mode]
        try:
            spec = importlib.util.spec_from_file_location('bounded_bot_test', Path(__file__).with_name('bot_device.py'))
            module = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(module)
        finally:
            sys.argv = previous
        device = module.Device.__new__(module.Device)
        device.stage, device.tag, device.commands, device.recovery_step = 'terminal', 1, 1, 0
        replies = []
        device.send = lambda kind, packet, body: replies.append((kind, packet, body))
        device.bulk_reply = lambda *args, **kwargs: replies.append((args, kwargs))
        return module, device, replies

    def control(self, module, device, kind, request, index):
        device.control(7, module.wire.CONTROL.pack(0, request, kind, 0, 0, index, 0))

    def cbw(self, module, device, tag):
        command = bytearray(31)
        command[:4] = b'USBC'
        struct.pack_into('<II', command, 4, tag, 512)
        command[12:16] = bytes([128, 0, 10, 0x28])
        command[22:24] = bytes([0, 1])
        device.bulk(8, module.wire.BULK.pack(2, 0, 31, 0, 0) + command)

    def test_new_command_waits_for_exact_three_acknowledgements_and_new_tag(self):
        module, device, replies = self.device()
        with self.assertRaises(ValueError):
            self.cbw(module, device, 2)
        self.control(module, device, 0x02, 1, 0x81)
        self.assertEqual(device.recovery_step, 0)
        self.control(module, device, 0x21, 0xff, 0)
        self.control(module, device, 0x02, 1, 0x81)
        with self.assertRaises(ValueError):
            self.cbw(module, device, 2)
        self.control(module, device, 0x02, 1, 0x02)
        self.assertEqual((device.recovery_step, device.stage), (3, None))
        with self.assertRaises(ValueError):
            self.cbw(module, device, 1)
        self.cbw(module, device, 2)
        self.assertEqual((device.commands, device.tag, device.stage), (2, 2, 'data'))

    def test_firmware_inquiry_refuses_profile_without_consuming_reentry_budget(self):
        module, device, replies = self.device()
        device.stage, device.commands = None, 0
        command = bytearray(31)
        command[:4] = b'USBC'
        struct.pack_into('<II', command, 4, 999, 36)
        command[12:16] = bytes([128, 0, 12, 0x12])
        device.bulk(8, module.wire.BULK.pack(2, 0, 31, 0, 0) + command)
        self.assertEqual((device.commands, device.recovery_step, device.stage), (0, 0, None))
        self.assertEqual(replies[-1][0], (8, 2, 0, module.wire.STALL))

    def test_failed_clear_in_never_unlocks_a_second_command(self):
        module, device, replies = self.device('read-reentry-stall-clear-in')
        self.control(module, device, 0x21, 0xff, 0)
        self.control(module, device, 0x02, 1, 0x81)
        self.control(module, device, 0x02, 1, 0x02)
        self.assertEqual((device.recovery_step, device.stage), (1, 'recovery'))
        with self.assertRaises(ValueError):
            self.cbw(module, device, 2)
        self.assertEqual(device.commands, 1)

if __name__ == '__main__':
    unittest.main()
