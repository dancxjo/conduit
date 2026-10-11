#!/usr/bin/env python3
"""Bounded external USB CDC DEVICE fault fixtures, never Conduit providers.

Modes alter one observed device behavior. The Host/class policy stays in the
checked Source and the ordinary product binary. No personal hardware or libusb.
"""
import argparse
import importlib.util
from pathlib import Path
import sys

parser=argparse.ArgumentParser(add_help=False)
parser.add_argument('--fault-mode',required=True,choices=['wrong-interface','short-coding','wrong-coding','short-in','disconnect-in','hold-in','late-after-cancel'])
options,remaining=parser.parse_known_args()
sys.argv=[sys.argv[0]]+remaining
spec=importlib.util.spec_from_file_location('bounded_cdc_device',Path(__file__).resolve().parent.parent/'usb-cdc-fixture/cdc_device.py')
module=importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
mode=options.fault_mode
original_send=module.Device.send
original_bulk=module.Device.bulk
original_control=module.Device.control
original_dispatch=module.Device.dispatch

if mode=='wrong-interface':
    descriptor=bytearray(module.CONFIG)
    assert descriptor[11]==descriptor[35]==0
    descriptor[11]=descriptor[35]=2  # communications interface + Union master
    module.CONFIG=bytes(descriptor)

def send(self,kind,packet_id,body=b''):
    if mode=='wrong-interface' and kind==5:
        metadata=bytearray(body)
        metadata[64+19]=2  # IRQ83 belongs to the actual communications interface
        body=bytes(metadata)
    elif mode=='wrong-interface' and kind==4:
        metadata=bytearray(body)
        metadata[4]=2
        body=bytes(metadata)
    if kind==100 and mode in ('short-coding','wrong-coding'):
        ep,request,request_type,status,value,index,length=module.CONTROL.unpack_from(body)
        if request==0x21 and request_type==0xa1 and status==module.SUCCESS:
            assert length==7
            payload=body[module.CONTROL.size:]
            if mode=='short-coding':
                payload=payload[:6]
            else:
                payload=module.struct.pack('<IBBB',9600,0,0,8)
            body=module.CONTROL.pack(ep,request,request_type,status,value,index,len(payload))+payload
            print('FAULT_APPLIED '+mode,flush=True)
    return original_send(self,kind,packet_id,body)

def control(self,packet_id,body):
    ep,request,request_type,_,value,index,length=module.CONTROL.unpack_from(body)
    if mode=='wrong-interface' and request_type in (0x21,0xa1) and index!=2:
        return self.send(100,packet_id,module.CONTROL.pack(ep,request,request_type,module.STALL,value,index,0))
    return original_control(self,packet_id,body)

def bulk(self,packet_id,body):
    ep,_,length,stream,high=module.BULK.unpack_from(body)
    if ep==129 and mode in ('disconnect-in','hold-in','late-after-cancel'):
        assert not high and not stream and 1<=length<=module.MAX_DATA
        assert len(body)==module.BULK.size and self.pending_in is None
        self.pending_in=(packet_id,length)
        print('FAULT_RETAINED_FIRST_IN '+mode,flush=True)
        if mode=='disconnect-in':
            self.send(2,0)  # actual usbredir device disconnect
            self.connected=False
        return
    return original_bulk(self,packet_id,body)

def dispatch(self,kind,packet_id,body):
    if mode=='late-after-cancel' and kind==21 and not body and self.pending_in is not None:
        pending,length=self.pending_in
        assert pending==packet_id
        self.pending_in=None
        data=self.reply[:length]
        # Exactly one device response, intentionally after the observed cancel.
        # QEMU may suppress it before any guest xHCI event; this is NOT evidence
        # of a late admitted Native provider completion in Conduit.
        print('FAULT_LATE_RESPONSE_AFTER_CANCEL '+str(packet_id),flush=True)
        self.bulk_reply(packet_id,129,len(data),data=data)
        return
    return original_dispatch(self,kind,packet_id,body)

module.Device.send=send
module.Device.control=control
module.Device.bulk=bulk
module.Device.dispatch=dispatch
if mode=='short-in':
    sys.argv+=['--reply-hex',bytes([255,0,128]).hex()]
module.main()
