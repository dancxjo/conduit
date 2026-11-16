from pathlib import Path
import base64,subprocess,shlex,json,time,sys
root=Path(__file__).resolve().parent.parent
checks=[('blue','victus.local','/dev/serial/by-id/usb-Silicon_Labs_CP2102_USB_to_UART_Bridge_Controller_0001-if00-port0'),('yellow','motherbrain.local','/dev/serial/by-id/usb-Conduit_RP2040_Host_e6639c758f335f34-if00')]
for color,host,port in checks:
 if len(sys.argv)>1 and color not in sys.argv[1:]:continue
 code='''import serial,subprocess,time,json
path=PORT
result={'query':'Root Inspect only; no provisioning/admission/Start'}
try:
 if subprocess.run(['fuser',path],capture_output=True).returncode==0:raise RuntimeError('Serial device has active owner; no query')
 p=serial.Serial(port=None,baudrate=115200,timeout=.2,write_timeout=2,exclusive=True);p.dtr=True;p.rts=not IS_CDC;p.port=path;p.open()
 with p:
  time.sleep(3 if not IS_CDC else .2)
  p.reset_input_buffer();p.write(b'\\0\\1?');deadline=time.monotonic()+8;raw=b'';size=None
  while time.monotonic()<deadline:
   raw+=p.read(4096)
   if len(raw)>=2:
    size=int.from_bytes(raw[:2],'big')
    if not 0<size<=16384:raise RuntimeError('Unexpected framing prefix='+raw[:16].hex())
    if len(raw)>=size+2:break
  if size is None or len(raw)<size+2:raise RuntimeError('No complete Root response within8seconds')
  result['inspections']=[{'elapsed_seconds':0,'value':raw[2:size+2].decode(errors='strict')}]
  for delay in (30,30):
   time.sleep(delay);p.reset_input_buffer();p.write(b'\\0\\1?');raw=b'';deadline=time.monotonic()+8;size=None
   while time.monotonic()<deadline:
    raw+=p.read(4096)
    if len(raw)>=2:
     size=int.from_bytes(raw[:2],'big')
     if not 0<size<=16384:raise RuntimeError('Unexpected monitoring framing')
     if len(raw)>=size+2:break
   if size is None or len(raw)<size+2:raise RuntimeError('Monitoring response timeout')
   result['inspections'].append({'elapsed_seconds':30*(len(result['inspections'])),'value':raw[2:size+2].decode(errors='strict')})
except Exception as error:result['refusal']=str(error)
print(json.dumps(result))
'''.replace('PORT',repr(port),1).replace('IS_CDC',repr(color!='blue'))
 command='/usr/bin/python3 -c '+shlex.quote('import base64;exec(base64.b64decode('+repr(base64.b64encode(code.encode()).decode())+'))')
 result=subprocess.run(['ssh','-o','BatchMode=yes','-o','ConnectTimeout=5','dancxjo@'+host,command],capture_output=True,text=True,timeout=90)
 receipt={'captured_unix':time.time(),'host':host,'device':color,'exit':result.returncode}
 if result.returncode==0:receipt.update(json.loads(result.stdout))
 else:receipt['refusal']=result.stderr.strip()[:512]
 (root/'outputs'/f'{color}-root-continuity-20261115.json').write_text(json.dumps(receipt,indent=2)+'\n');print(json.dumps(receipt),flush=True)
