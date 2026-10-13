"""Bounded exact product media observation with one owned external USB device."""
import hashlib,json,pathlib,subprocess,tempfile,time
base=pathlib.Path(__file__).resolve().parent
out=base/'run';out.mkdir()
media=base/'media-prepared';image=media/'conduitos.iso';r=json.loads((media/'protocol-receipt.json').read_text())
assert hashlib.sha256(image.read_bytes()).hexdigest()==r['image_sha256']
with tempfile.TemporaryDirectory(prefix='cdc-product-') as tmp:
 socket=pathlib.Path(tmp)/'device.sock';device=None;qemu=None
 with (out/'device.log').open('x') as dl,(out/'qemu.log').open('x') as ql:
  try:
   device=subprocess.Popen(['python3',str(base.parent/'usb-cdc-fixture/cdc_device.py'),'--socket',str(socket),'--transcript',str(out/'wire.jsonl'),'--seconds','60'],stdout=dl,stderr=dl)
   deadline=time.monotonic()+5
   while not socket.exists() and time.monotonic()<deadline:
    if device.poll() is not None:raise RuntimeError('device exited before socket')
    time.sleep(.02)
   if not socket.exists():raise RuntimeError('owned device socket timeout')
   args=['qemu-system-x86_64','-M','q35','-cpu','max','-m','512M','-smp','1','-display','none','-vga','std','-monitor','none','-serial',f'file:{out}/serial.log','-no-reboot','-net','none','-device','isa-debug-exit,iobase=0xf4,iosize=0x04','-cdrom',str(image),'-boot','d','-device','qemu-xhci,id=protocol-usb,p2=1,p3=0','-chardev',f'socket,id=cdc,path={socket}','-device','usb-redir,chardev=cdc,bus=protocol-usb.0,port=1,msos-desc=off,streams=off,debug=4']
   qemu=subprocess.Popen(args,stdin=subprocess.DEVNULL,stdout=subprocess.DEVNULL,stderr=ql)
   deadline=time.monotonic()+55;outcome='observation-timeout'
   while time.monotonic()<deadline:
    serial=out/'serial.log';raw=serial.read_bytes() if serial.exists() else b''
    if len(raw)>262144 or (out/'qemu.log').stat().st_size>262144:raise RuntimeError('observation exceeded transcript bound')
    if b'CONDUIT_PROTOCOL_RETIRED complete' in raw:outcome='play-retired';break
    if b'"status":"refused"' in raw or b'CONDUIT_PROTOCOL_REFUSAL ' in raw:outcome='refused';break
    if qemu.poll() is not None:outcome='emulator-ended';break
    time.sleep(.05)
   receipt={'schema':'conduit.usb/cdc-control-observation@1','proof_class':'freestanding-emulator','scope':'control GET_DESCRIPTOR only; no CDC class execution claim','outcome':outcome,'image_sha256':r['image_sha256'],'qemu_arguments':args,'serial_sha256':hashlib.sha256(raw).hexdigest(),'physical_compatibility':'unverified'}
   (out/'receipt.json').write_text(json.dumps(receipt,indent=2)+'\n');print(outcome)
  finally:
   for p in (qemu,device):
    if p is not None and p.poll() is None:
     p.terminate()
     try:p.wait(timeout=2)
     except subprocess.TimeoutExpired:p.kill();p.wait(timeout=2)
