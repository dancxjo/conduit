#!/usr/bin/env python3
"""Finite external UAC1 full-speed usbredir device; never a Conduit provider.

USB-IF Audio1.0/Formats1.0; SPICE usbredir0.15.0 protocol. OUT ISO packets
have no per-packet usbredir response. Native QEMU completion is separate evidence
from this external device transcript; neither establishes audible sound.
"""
import argparse
import importlib.util
import os
from pathlib import Path
import socket
import struct
import time

_COMMON = Path(__file__).resolve().parents[2] / 'evidence/selected-serial-53b0baa1c/cdc_device.py'
_spec = importlib.util.spec_from_file_location('bounded_usbredir_wire', _COMMON)
wire = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(wire)
DEVICE = struct.pack('<BBHBBBBHHHBBBB', 18, 1, 0x0200, 0, 0, 0, 8,
                     0xffff, 0xa117, 0x0100, 0, 0, 0, 1)
CONFIG = bytes.fromhex(
    '09 02 64 00 02 01 00 80 32 '
    '09 04 00 00 00 01 01 00 00 '
    '09 24 01 00 01 1e 00 01 01 '
    '0c 24 02 01 01 01 00 02 03 00 00 00 '
    '09 24 03 02 01 03 00 01 00 '
    '09 04 01 00 00 01 02 00 00 '
    '09 04 01 01 01 01 02 00 00 '
    '07 24 01 01 01 01 00 '
    '0b 24 02 01 02 02 10 01 80 bb 00 '
    '09 05 01 0d c0 00 01 00 00 '
    '07 25 01 01 00 00 00')
assert len(CONFIG) == 100


class Device(wire.Device):
    def __init__(self, peer, transcript, deadline, maximum_packets=128, mode="normal"):
        super().__init__(peer, transcript, deadline, b'')
        if mode not in ("normal", "wrong-rate", "stall-rate"):
            raise ValueError("unknown bounded audio fixture mode")
        self.mode = mode
        self.alt = 0
        self.rate = 48000
        self.streaming = False
        self.packets = 0
        self.maximum_packets = maximum_packets

    def handshake(self):
        self.send(0, 0, b'bounded-external-uac1-device@1'.ljust(64, b'\0')
                  + struct.pack('<I', wire.CAPS))
        kind, packet_id, body = self.receive()
        if kind != 0 or packet_id != 0 or len(body) < 68 or len(body) % 4:
            raise ValueError('invalid hello')
        if struct.unpack_from('<I', body, 64)[0] & wire.CAPS != wire.CAPS:
            raise ValueError('required usbredir capabilities unavailable')
        self.wide = True
        self.metadata()
        self.send(1, 0, struct.pack('<BBBBHHH', 1, 0, 0, 0, 0xffff, 0xa117, 0x0100))
        print('AUDIO_DEVICE_CONNECTED UAC1 FS PCM16LE stereo48000 OUT01 max192 interval1', flush=True)

    def metadata(self):
        types, intervals, interfaces, sizes = [255]*32, [0]*32, [0]*32, [0]*32
        for index in (0, 16):
            types[index], sizes[index] = 0, 8
        if self.alt == 1:
            types[1], intervals[1], interfaces[1], sizes[1] = 1, 1, 1, 192
        self.send(5, 0, bytes(types+intervals+interfaces)+struct.pack('<32H', *sizes))
        self.send(4, 0, struct.pack('<I', 2)+bytes([0,1]+[0]*30)
                  +bytes([1,1]+[0]*30)+bytes([1,2]+[0]*30)+bytes(32))

    def control(self, packet_id, body):
        if len(body) < wire.CONTROL.size:
            raise ValueError('short control')
        ep, request, request_type, _, value, index, length = wire.CONTROL.unpack_from(body)
        payload = body[wire.CONTROL.size:]
        inbound = bool(request_type & 128)
        if ep != (128 if inbound else 0) or length > 256 or len(payload) != (0 if inbound else length):
            raise ValueError('control extent')
        status, result = wire.STALL, b''
        if request_type == 0x80 and request == 6:
            descriptor = {1: DEVICE, 2: CONFIG}.get(value >> 8)
            if descriptor is not None:
                status, result = wire.SUCCESS, descriptor[:length]
        elif request_type == 0x22 and request == 1 and value == 0x100 and index == 1 and length == 3:
            if self.mode != "stall-rate" and self.configuration == 1 and int.from_bytes(payload, 'little') == 48000:
                self.rate, status = 48000, wire.SUCCESS
        elif request_type == 0xa2 and request == 0x81 and value == 0x100 and index == 1 and length == 3:
            if self.configuration == 1:
                status, result = wire.SUCCESS, (44100 if self.mode == 'wrong-rate' else self.rate).to_bytes(3, 'little')
        actual = len(result) if inbound else (length if status == wire.SUCCESS else 0)
        self.send(100, packet_id, wire.CONTROL.pack(ep, request, request_type, status, value, index, actual)+result)

    def dispatch(self, kind, packet_id, body):
        if kind == 100:
            self.control(packet_id, body)
        elif kind == 6 and len(body) == 1:
            status = wire.SUCCESS if body[0] in (0,1) else wire.STALL
            if status == wire.SUCCESS:
                self.configuration, self.alt, self.streaming = body[0], 0, False
                self.metadata()
            self.send(8, packet_id, bytes([status, self.configuration]))
        elif kind == 7 and not body:
            self.send(8, packet_id, bytes([wire.SUCCESS, self.configuration]))
        elif kind in (9,10) and len(body) == (2 if kind == 9 else 1):
            interface, alt = body[0], body[1] if kind == 9 else self.alt
            valid = self.configuration == 1 and ((interface == 0 and alt == 0) or (interface == 1 and alt in (0,1)))
            if valid and kind == 9:
                self.alt = alt if interface == 1 else self.alt
                self.streaming = False
                self.metadata()
            self.send(11, packet_id, bytes([wire.SUCCESS if valid else wire.STALL, interface, alt]))
        elif kind == 12 and len(body) == 3:
            endpoint, packets, urbs = body
            valid = self.configuration == 1 and self.alt == 1 and endpoint == 1 and packets > 0 and urbs > 0 and packets*urbs <= 256
            self.streaming = valid
            self.send(14, packet_id, bytes([wire.SUCCESS if valid else wire.STALL, endpoint]))
        elif kind == 13 and body == b'\x01':
            self.streaming = False
            self.send(14, packet_id, bytes([wire.SUCCESS, 1]))
        elif kind == 102:
            if len(body) < 4:
                raise ValueError('short ISO packet')
            endpoint, _, length = struct.unpack_from('<BBH', body)
            if not self.streaming or endpoint != 1 or length != 192 or len(body) != length+4:
                raise ValueError('ISO packet outside exact admitted profile')
            if self.packets >= self.maximum_packets:
                raise ValueError('finite packet envelope exhausted')
            self.packets += 1
            # OUT ISO has no per-packet reply in usbredir. Raw PCM is retained in
            # record(); never invent an acknowledgement or repair/retry bytes.
        elif kind == 3 and not body:
            self.configuration, self.alt, self.streaming = 0, 0, False
        elif kind == 21 and not body:
            pass  # No pending usbredir OUT packet acknowledgement exists.
        else:
            raise ValueError(f'unsupported usbredir packet {kind}/{len(body)}')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--socket', required=True)
    parser.add_argument('--transcript', required=True)
    parser.add_argument('--seconds', type=int, default=60, choices=range(1,121))
    parser.add_argument('--maximum-packets', type=int, default=128, choices=range(1,129))
    parser.add_argument('--mode', choices=('normal', 'wrong-rate', 'stall-rate'), default='normal')
    args = parser.parse_args()
    path = Path(args.socket)
    if path.exists() or path.is_symlink():
        parser.error('existing socket never replaced')
    deadline = time.monotonic()+args.seconds
    with open(args.transcript, 'x', encoding='utf8') as transcript, socket.socket(socket.AF_UNIX) as listener:
        listener.bind(str(path))
        inode = path.lstat().st_ino
        os.chmod(path, 0o600)
        listener.listen(1)
        listener.settimeout(args.seconds)
        print('AUDIO_DEVICE_LISTENING '+str(path), flush=True)
        try:
            peer, _ = listener.accept()
            with peer:
                device = Device(peer, transcript, deadline, args.maximum_packets, args.mode)
                device.handshake()
                while True:
                    device.dispatch(*device.receive())
        except EOFError:
            print('AUDIO_DEVICE_PEER_CLOSED', flush=True)
        except (ValueError, TimeoutError, socket.timeout) as error:
            print('AUDIO_DEVICE_REFUSED '+str(error), flush=True)
            raise SystemExit(1)
        finally:
            if path.exists() and not path.is_symlink() and path.lstat().st_ino == inode:
                path.unlink()


if __name__ == '__main__':
    main()
