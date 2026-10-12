#!/usr/bin/env python3
"""External bounded BOT DEVICE fault fixture; never a Conduit provider."""
import argparse
import struct
import sys
import usbredir_transport as wire

parser = argparse.ArgumentParser(add_help=False)
parser.add_argument('--fault-mode', choices=['none', 'stall-in', 'disconnect-in', 'stale-tag', 'bad-residue', 'command-failed', 'phase-error', 'short-in', 'hold-in', 'late-after-cancel'], required=True)
options, remaining = parser.parse_known_args()
sys.argv = [sys.argv[0]] + remaining
wire.CONFIG = bytes.fromhex('09 02 20 00 01 01 05 c0 00 09 04 00 00 02 08 06 50 00 07 05 81 02 00 02 00 07 05 02 02 00 02 00')
wire.DEVICE = struct.pack('<BBHBBBBHHHBBBB', 18, 1, 0x0200, 0, 0, 0, 64, 0xffff, 0xf117, 0x0100, 0, 0, 0, 1)
wire.MAX_MESSAGES = 512  # finite budget includes the retained AArch64 firmware prefix
Base = wire.Device

class Device(Base):
    def __init__(self, *args):
        super().__init__(*args)
        self.stage = None
        self.tag = None
        self.commands = 0

    def metadata(self):
        types, intervals, interfaces, sizes = [255]*32, [0]*32, [0]*32, [0]*32
        for address, kind, packet in [(0,0,64), (128,0,64), (129,2,512), (2,2,512)]:
            index = ((address & 128) >> 3) | (address & 15)
            types[index], sizes[index] = kind, packet
        self.send(5,0,bytes(types+intervals+interfaces)+struct.pack('<32H',*sizes))
        self.send(4,0,struct.pack('<I',1)+bytes(32)+bytes([8]+[0]*31)+bytes([6]+[0]*31)+bytes([80]+[0]*31))

    def handshake(self):
        self.send(0,0,b'bounded-external-bot-device@1'.ljust(64,b'\0')+struct.pack('<I',wire.CAPS))
        kind, packet, body = self.receive()
        if kind != 0 or packet != 0 or len(body)<68 or struct.unpack_from('<I',body,64)[0]&wire.CAPS != wire.CAPS:
            raise ValueError('invalid hello')
        self.wide = True
        self.metadata()
        self.send(1,0,struct.pack('<BBBBHHH',2,0,0,0,0xffff,0xf117,0x0100))
        self.connected = True

    def bulk(self, packet_id, body):
        if len(body)<wire.BULK.size:
            raise ValueError('short bulk')
        ep, _, length, stream, high = wire.BULK.unpack_from(body)
        payload = body[wire.BULK.size:]
        if stream or high or length>wire.MAX_DATA or len(payload)!=(0 if ep&128 else length):
            raise ValueError('invalid bulk envelope')
        if ep==2:
            if self.stage is not None or length!=31 or payload[:4]!=b'USBC':
                return self.bulk_reply(packet_id,ep,0,wire.STALL)
            tag, extent = struct.unpack_from('<II',payload,4)
            if payload[12:16]!=bytes([128,0,10,0x28]) or extent!=512 or payload[17:21]!=bytes(4) or payload[22:24]!=bytes([0,1]):
                return self.bulk_reply(packet_id,ep,0,wire.STALL)
            self.commands+=1
            if self.commands>16:
                raise ValueError('command bound')
            self.tag, self.stage = tag, 'data'
            print('BOT_CBW '+str(tag),flush=True)
            return self.bulk_reply(packet_id,ep,31)
        if ep!=129 or self.stage is None:
            return self.bulk_reply(packet_id,ep,0,wire.STALL)
        if self.stage=='data':
            if length!=512:
                raise ValueError('DATA length')
            if options.fault_mode in ('hold-in','late-after-cancel'):
                if self.pending_in is not None:raise ValueError('duplicate held IN')
                self.pending_in=(packet_id,length)
                print('BOT_HELD_DATA '+str(packet_id),flush=True)
                return
            if options.fault_mode=='disconnect-in':
                self.pending_in=(packet_id,length)
                self.send(2,0)
                self.connected=False
                print('BOT_DISCONNECT_AFTER_CBW '+str(self.tag),flush=True)
                return
            if options.fault_mode=='stall-in':
                self.stage='terminal'
                print('BOT_STALL_AFTER_CBW '+str(self.tag),flush=True)
                return self.bulk_reply(packet_id,ep,0,wire.STALL)
            self.stage='status'
            data=bytes((i*37+11)%256 for i in range(512))
            if options.fault_mode=='short-in':data=data[:500]
            return self.bulk_reply(packet_id,ep,len(data),data=data)
        if self.stage!='status' or length!=13:
            return self.bulk_reply(packet_id,ep,0,wire.STALL)
        tag=self.tag-1 if options.fault_mode=='stale-tag' else self.tag
        residue=1 if options.fault_mode=='bad-residue' else 0
        status={'command-failed':1,'phase-error':2}.get(options.fault_mode,0)
        data=b'USBS'+struct.pack('<IIB',tag,residue,status)
        self.stage=None
        self.bulk_reply(packet_id,ep,13,data=data)

    def dispatch(self, kind, packet_id, body):
        if options.fault_mode=='late-after-cancel' and kind==21 and not body and self.pending_in is not None:
            pending,length=self.pending_in
            if pending!=packet_id:raise ValueError('cancel identity mismatch')
            self.pending_in=None
            data=bytes((i*37+11)%256 for i in range(length))
            print('BOT_LATE_DEVICE_RESPONSE_AFTER_CANCEL '+str(packet_id),flush=True)
            self.bulk_reply(packet_id,129,len(data),data=data)
            return
        return super().dispatch(kind,packet_id,body)

wire.Device=Device
if __name__=='__main__':
    wire.main()
