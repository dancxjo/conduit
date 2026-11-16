from pathlib import Path
import base64,subprocess,shlex,json,time
root=Path(__file__).resolve().parent.parent
checks=[('blue','victus.local','/dev/serial/by-id/usb-Silicon_Labs_CP2102_USB_to_UART_Bridge_Controller_0001-if00-port0'),('yellow','motherbrain.local','/dev/serial/by-id/usb-Conduit_RP2040_Host_e6639c758f335f34-if00')]
for color,host,port in checks:
 code='''import serial,subprocess,time,json
path=PORT
result={'query':'Root Inspect only; no provisioning/admission/Start'}
try:
 if subprocess.run(['fuser',path],capture_output=True).returncode==0:raise RuntimeError('Serial device has active owner; no query')
 p=serial.Serial(port=None,baudrate=115200,timeout=.2,write_timeout=2,exclusive=True);p.dtr=False;p.rts=False;p.port=path;p.open()
 with p:
  p.reset_input_buffer();p.dtr=True;p.write(b'\\0\\1?');deadline=time.monotonic()+8;raw=b'';size=None
  while time.monotonic()<deadline:
   raw+=p.read(4096)
   if len(raw)>=2:
    size=int.from_bytes(raw[:2],'big')
    if not 0<size<=16384:raise RuntimeError('Unexpected framing')
    if len(raw)>=size+2:break
  if size is None or len(raw)<size+2:raise RuntimeError('No complete Root response within8seconds')
  result['inspection']=raw[2:size+2].decode(errors='strict')
except Exception as error:result['refusal']=str(error)
print(json.dumps(result))
'''.replace('PORT',repr(port),1)
 command='/usr/bin/python3 -c '+shlex.quote('import base64;exec(base64.b64decode('+repr(base64.b64encode(code.encode()).decode())+'))')
 result=subprocess.run(['ssh','-o','BatchMode=yes','-o','ConnectTimeout=5','dancxjo@'+host,command],capture_output=True,text=True,timeout=20)
 receipt={'captured_unix':time.time(),'host':host,'device':color,'exit':result.returncode}
 if result.returncode==0:receipt.update(json.loads(result.stdout))
 else:receipt['refusal']=result.stderr.strip()[:512]
 (root/'outputs'/f'{color}-root-inspection-20261115.json').write_text(json.dumps(receipt,indent=2)+'\n');print(json.dumps(receipt),flush=True)
