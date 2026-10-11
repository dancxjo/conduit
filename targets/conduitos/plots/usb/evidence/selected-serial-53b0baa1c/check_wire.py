"""Validate actual USBredir pairs independently of checked serial outputs."""
import json,struct,sys
pending={}; controls=[]; tx=[]; rx=[]; bulk_order=[]
for line in open(sys.argv[1]):
 event=json.loads(line)
 if event['type'] not in (6,8,100,101):continue
 raw=bytes.fromhex(event['wire_hex']);length=struct.unpack_from('<I',raw,4)[0]
 header=len(raw)-length
 assert header in (12,16)
 body=raw[header:]
 if event['direction']=='qemu-to-device':
  assert event['id'] not in pending
  pending[event['id']]=(event['type'],body)
  continue
 kind,request=pending.pop(event['id'])
 assert event['type']==(8 if kind==6 else kind)
 if kind==6:
  assert request==bytes([1]) and body==bytes([0,1])
  controls.append((9,0,1,0,0))
 elif kind==100:
  req=struct.unpack_from('<BBBBHHH',request)
  result=struct.unpack_from('<BBBBHHH',body)
  assert result[3]==0
  if req[1:3]==(32,33):
   assert request[10:]==bytes([0,194,1,0,0,0,8]) and result[6]==7 and body[10:]==b''
  if req[1:3]==(33,161):
   assert result[6]==7 and body[10:]==bytes([0,194,1,0,0,0,8])
  if req[1:3] in ((6,128),(9,0),(32,33),(33,161),(34,33)):
   if req[1]==6 and (req[4],req[6])!=(512,67):continue
   controls.append((req[1],req[2],req[4],req[5],req[6]))
 else:
  ep,status,actual,stream,high=struct.unpack_from('<BBHIH',body)
  requested_ep,_,requested,_,_=struct.unpack_from('<BBHIH',request)
  assert status==0 and ep==requested_ep and high==0
  bulk_order.append(ep)
  if ep==2:
   assert request[10:]==b'cdc-plot-tx\n' and requested==12 and actual==12
   tx.append(actual)
  elif ep==129:
   assert requested==64 and actual==17 and body[10:]==b'cdc-device-reply\n'
   rx.append(actual)
  else:raise AssertionError('unexpected bulk endpoint')
assert not pending
# Firmware enumeration differs; require its explicit architecture-specific prefix.
architecture=sys.argv[2] if len(sys.argv)>2 else 'aarch64'
assert architecture in ('aarch64','x86_64')
bootstrap=[(6,128,512,0,67)]+([(9,0,1,0,0)] if architecture=='aarch64' else [])
source=[(6,128,512,0,67),(9,0,1,0,0),(32,33,0,0,7),(33,161,0,0,7),(34,33,3,0,0)]
assert controls==bootstrap+source
assert len(tx)==len(rx)==16
assert bulk_order==[2,129]*16
print(json.dumps({'source_controls':5,'actual_transmit':16,'actual_receive':16,'status_success':True,'exact_octets':True}))
