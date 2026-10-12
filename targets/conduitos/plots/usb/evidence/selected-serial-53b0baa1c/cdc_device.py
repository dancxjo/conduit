#!/usr/bin/env python3
"""Bounded external emulated USB CDC-ACM DEVICE, never a Conduit provider.

Wire authority: SPICE usbredir usb-redirection-protocol.txt + usbredirproto.h.
No libusb, TTY, personal device, guest class driver, or Conduit execution code.
"""
import argparse
import json
import os
import socket
import struct
import time
from pathlib import Path

CAPS = (1 << 1) | (1 << 4) | (1 << 5) | (1 << 6)
MAX_DATA = 2048
MAX_FRAME = MAX_DATA + 256
MAX_MESSAGES = 256
MAX_WIRE = 512 * 1024
CONTROL = struct.Struct('<BBBBHHH')
BULK = struct.Struct('<BBHIH')
SUCCESS, CANCELLED, INVALID, IOERROR, STALL = range(5)
DEVICE = struct.pack('<BBHBBBBHHHBBBB', 18, 1, 0x0200, 2, 0, 0, 8,
                     0xffff, 0xf117, 0x0100, 1, 2, 3, 1)
# CDC 1.10 ACM: communications interface 0, data interface 1; CDC functional
# Header, Call Management, ACM (line coding/state), Union; actual serial wires.
CONFIG = bytes.fromhex(
    '09 02 43 00 02 01 00 80 32 '
    '09 04 00 00 01 02 02 01 00 '
    '05 24 00 10 01 05 24 01 00 01 04 24 02 02 05 24 06 00 01 '
    '07 05 83 03 10 00 10 '
    '09 04 01 00 02 0a 00 00 00 '
    '07 05 81 02 40 00 00 07 05 02 02 40 00 00')
STRINGS = {0: bytes.fromhex('04 03 09 04')}
for number, text in [(1, 'Conduit external device fixture'),
                     (2, 'Emulated CDC-ACM serial device'), (3, 'bounded-cdc-1')]:
    encoded = text.encode('utf-16le')
    STRINGS[number] = bytes([len(encoded) + 2, 3]) + encoded


class Device:
    def __init__(self, peer, transcript, deadline, reply):
        self.peer, self.transcript, self.deadline = peer, transcript, deadline
        self.reply = reply
        self.wide = False
        self.messages = self.wire = 0
        self.configuration = self.control_state = 0
        self.line_coding = struct.pack('<IBBB', 9600, 0, 0, 8)
        self.rx = bytearray()
        self.pending_in = None
        self.notification = False
        self.connected = False

    def record(self, direction, raw, kind, packet_id):
        self.messages += 1
        self.wire += len(raw)
        if self.messages > MAX_MESSAGES or self.wire > MAX_WIRE:
            raise ValueError('fixture wire envelope exhausted')
        self.transcript.write(json.dumps({'direction': direction, 'type': kind,
            'id': packet_id, 'wire_hex': raw.hex()}, separators=(',', ':')) + '\n')
        self.transcript.flush()

    def exact(self, count):
        data = bytearray()
        while len(data) < count:
            remaining = self.deadline - time.monotonic()
            if remaining <= 0:
                raise TimeoutError('fixture deadline')
            self.peer.settimeout(min(remaining, 1))
            try:
                chunk = self.peer.recv(count - len(data))
            except socket.timeout:
                continue
            if not chunk:
                raise EOFError('owned QEMU peer closed')
            data.extend(chunk)
        return bytes(data)

    def receive(self):
        header = self.exact(16 if self.wide else 12)
        kind, length, packet_id = struct.unpack('<IIQ' if self.wide else '<III', header)
        if length > MAX_FRAME:
            raise ValueError('oversized frame before allocation')
        body = self.exact(length)
        self.record('qemu-to-device', header + body, kind, packet_id)
        return kind, packet_id, body

    def send(self, kind, packet_id, body=b''):
        if len(body) > MAX_FRAME:
            raise ValueError('oversized outbound frame')
        header = struct.pack('<IIQ' if self.wide else '<III', kind, len(body), packet_id)
        raw = header + body
        self.record('device-to-qemu', raw, kind, packet_id)
        self.peer.settimeout(max(0.001, min(1, self.deadline - time.monotonic())))
        self.peer.sendall(raw)

    def metadata(self):
        types, intervals, interfaces = [255] * 32, [0] * 32, [0] * 32
        sizes = [0] * 32
        for address, kind, packet, interval, interface in [
            (0, 0, 8, 0, 0), (128, 0, 8, 0, 0),
            (129, 2, 64, 0, 1), (2, 2, 64, 0, 1), (131, 3, 16, 16, 0)]:
            index = ((address & 128) >> 3) | (address & 15)
            types[index], intervals[index], interfaces[index], sizes[index] = (
                kind, interval, interface, packet)
        self.send(5, 0, bytes(types + intervals + interfaces) + struct.pack('<32H', *sizes))
        self.send(4, 0, struct.pack('<I', 2) + bytes([0, 1] + [0] * 30)
            + bytes([2, 10] + [0] * 30) + bytes([2, 0] + [0] * 30)
            + bytes([1, 0] + [0] * 30))

    def handshake(self):
        self.send(0, 0, b'bounded-external-cdc-acm-device@1'.ljust(64, b'\0')
                  + struct.pack('<I', CAPS))
        kind, packet_id, body = self.receive()
        if kind != 0 or packet_id != 0 or len(body) < 68 or len(body) % 4:
            raise ValueError('invalid hello')
        peer_caps = struct.unpack_from('<I', body, 64)[0]
        if peer_caps & CAPS != CAPS:
            raise ValueError('xHCI-required capabilities unavailable')
        self.wide = True  # Hello alone always uses the original 32-bit id header.
        self.metadata()
        self.send(1, 0, struct.pack('<BBBBHHH', 1, 2, 0, 0, 0xffff, 0xf117, 0x0100))
        self.connected = True
        print('CDC_DEVICE_CONNECTED full-speed ffff:f117 IN81 OUT02 IRQ83', flush=True)

    def control(self, packet_id, body):
        if len(body) < CONTROL.size:
            raise ValueError('short control header')
        ep, request, request_type, _, value, index, length = CONTROL.unpack_from(body)
        payload = body[CONTROL.size:]
        inbound = bool(request_type & 128)
        if length > MAX_DATA or ep != (128 if inbound else 0) or (
                len(payload) != (0 if inbound else length)):
            raise ValueError('invalid control envelope')
        status, result = STALL, b''
        if request_type == 0x80 and request == 6:
            descriptor = {1: DEVICE, 2: CONFIG}.get(value >> 8)
            if value >> 8 == 3:
                descriptor = STRINGS.get(value & 255)
            if descriptor is not None:
                status, result = SUCCESS, descriptor[:length]
        elif request_type in (0x80, 0x81, 0x82) and request == 0 and length == 2:
            status, result = SUCCESS, b'\0\0'
        elif request_type == 0x21 and request == 0x20 and index == 0 and length == 7:
            rate, stop, parity, bits = struct.unpack('<IBBB', payload)
            if rate > 0 and stop <= 2 and parity <= 4 and bits in (5, 6, 7, 8, 16):
                self.line_coding, status = payload, SUCCESS
        elif request_type == 0xa1 and request == 0x21 and index == 0 and length == 7:
            status, result = SUCCESS, self.line_coding
        elif request_type == 0x21 and request == 0x22 and index == 0 and length == 0 and value <= 3:
            self.control_state, status = value, SUCCESS
            self.notify()
        # The device advertises only supported ACM behavior. Unknown requests
        # really STALL; they are never reported as a successful zero-byte call.
        actual = len(result) if inbound else (length if status == SUCCESS else 0)
        self.send(100, packet_id, CONTROL.pack(ep, request, request_type, status,
                                             value, index, actual) + result)

    def notify(self):
        if self.notification:
            # CDC SERIAL_STATE: carrier bits mirror the emulator's asserted DTR.
            state = 3 if self.control_state & 1 else 0
            wire = struct.pack('<BBHHHH', 0xa1, 0x20, 0, 0, 2, state)
            self.send(103, 0, struct.pack('<BBH', 131, SUCCESS, len(wire)) + wire)

    def bulk_reply(self, packet_id, ep, length, status=SUCCESS, data=b''):
        self.send(101, packet_id, BULK.pack(ep, status, length, 0, 0) + data)

    def flush_in(self):
        if self.pending_in is not None and self.rx:
            packet_id, length = self.pending_in
            data = bytes(self.rx[:length])
            del self.rx[:len(data)]
            self.pending_in = None
            self.bulk_reply(packet_id, 129, len(data), data=data)

    def bulk(self, packet_id, body):
        if len(body) < BULK.size:
            raise ValueError('short bulk header')
        ep, _, length, stream, high = BULK.unpack_from(body)
        payload = body[BULK.size:]
        if high or stream or length > MAX_DATA or len(payload) != (0 if ep & 128 else length):
            raise ValueError('invalid bulk envelope')
        if self.configuration != 1 or not self.control_state & 1 or ep not in (129, 2):
            self.bulk_reply(packet_id, ep, 0, STALL)
        elif ep == 2:
            # Physical serial fixture supplies a distinct deterministic receive
            # message after each nonempty transmit; Plot owns the serial policy.
            if payload:
                if len(self.rx) + len(self.reply) > MAX_DATA:
                    self.bulk_reply(packet_id, ep, 0, IOERROR)
                    return
                self.rx.extend(self.reply)
            self.bulk_reply(packet_id, ep, length)
            self.flush_in()
        elif length == 0:
            self.bulk_reply(packet_id, ep, 0)
        elif self.pending_in is not None:
            raise ValueError('more than one outstanding IN transfer')
        else:
            self.pending_in = (packet_id, length)
            self.flush_in()

    def dispatch(self, kind, packet_id, body):
        if kind == 100:
            self.control(packet_id, body)
        elif kind == 101:
            self.bulk(packet_id, body)
        elif kind == 6 and len(body) == 1:
            config = body[0]
            status = SUCCESS if config in (0, 1) else STALL
            if status == SUCCESS:
                self.cancel_pending()
                self.configuration = config
                self.notification = False
                self.metadata()
            self.send(8, packet_id, bytes([status, self.configuration]))
        elif kind == 7 and not body:
            self.send(8, packet_id, bytes([SUCCESS, self.configuration]))
        elif kind in (9, 10) and len(body) == (2 if kind == 9 else 1):
            interface, alt = body[0], body[1] if kind == 9 else 0
            self.send(11, packet_id, bytes([SUCCESS if interface < 2 and alt == 0 else STALL,
                                          interface, alt]))
        elif kind == 3 and not body:
            self.cancel_pending()
            self.configuration = self.control_state = 0
            self.notification = False
            self.rx.clear()
            self.line_coding = struct.pack('<IBBB', 9600, 0, 0, 8)
        elif kind == 21 and not body:
            if self.pending_in is not None and self.pending_in[0] == packet_id:
                self.cancel_pending()
            # Already-completed packets have their matching response in flight;
            # do not invent a second completion for the same packet id.
        elif kind in (15, 16) and body == bytes([131]):
            self.notification = kind == 15
            self.send(17, packet_id, bytes([SUCCESS, 131]))
            self.notify()
        else:
            raise ValueError(f'unsupported usbredir packet {kind}/{len(body)}')

    def cancel_pending(self):
        if self.pending_in is not None:
            packet_id, _ = self.pending_in
            self.pending_in = None
            self.bulk_reply(packet_id, 129, 0, CANCELLED)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--socket', required=True)
    parser.add_argument('--transcript', required=True)
    parser.add_argument('--seconds', type=int, default=60, choices=range(1, 121))
    parser.add_argument('--reply-hex', default=b'cdc-device-reply\n'.hex())
    args = parser.parse_args()
    reply = bytes.fromhex(args.reply_hex)
    if not 1 <= len(reply) <= MAX_DATA:
        parser.error('reply must contain 1..2048 bytes')
    path = Path(args.socket)
    if path.exists() or path.is_symlink():
        parser.error('socket path already exists; existing resources are never replaced')
    deadline = time.monotonic() + args.seconds
    with open(args.transcript, 'x', encoding='utf8') as transcript, socket.socket(socket.AF_UNIX) as listener:
        listener.bind(str(path))
        owned_inode = path.lstat().st_ino
        os.chmod(path, 0o600)
        listener.listen(1)
        listener.settimeout(args.seconds)
        print('CDC_DEVICE_LISTENING ' + str(path), flush=True)
        try:
            peer, _ = listener.accept()
            with peer:
                device = Device(peer, transcript, deadline, reply)
                device.handshake()
                while True:
                    device.dispatch(*device.receive())
        except EOFError:
            print('CDC_DEVICE_PEER_CLOSED', flush=True)
        except (ValueError, TimeoutError, socket.timeout) as error:
            print('CDC_DEVICE_REFUSED ' + str(error), flush=True)
            raise SystemExit(1)
        finally:
            # Only this exact freshly-created socket is owned by this process.
            if path.exists() and not path.is_symlink() and path.lstat().st_ino == owned_inode:
                path.unlink()


if __name__ == '__main__':
    main()
