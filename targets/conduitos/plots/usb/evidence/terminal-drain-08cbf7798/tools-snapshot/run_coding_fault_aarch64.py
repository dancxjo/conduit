"""Bounded exact product media observation with one owned external USB device."""
import argparse,hashlib,json,pathlib,subprocess,tempfile,time
base=pathlib.Path(__file__).resolve().parent
serial_tools=base.parent/'usb-serial-product-proof'
parser=argparse.ArgumentParser()
parser.add_argument('--media',type=pathlib.Path,required=True)
parser.add_argument('--output',type=pathlib.Path,required=True)
parser.add_argument('--build-record',type=pathlib.Path,required=True)
parser.add_argument('--expected-commit',required=True)
parser.add_argument('--expected-package',required=True)
parser.add_argument('--verdict',choices=['supported','unsupported'],required=True)
parser.add_argument('--fault-mode',choices=['short-coding','wrong-coding'],required=True)
parser.add_argument('--device-profile',choices=['valid','wrong-interface'])
parser.add_argument('--receive-hex',default='6364632d6465766963652d7265706c790a')
options=parser.parse_args()
device_profile=options.device_profile or ('valid' if options.verdict=='supported' else 'wrong-interface')
out=options.output.resolve();out.mkdir()
media=options.media.resolve();image=media/'conduitos.iso';r=json.loads((media/'protocol-receipt.json').read_text())
assert r['architecture']=='aarch64'
build=json.loads(options.build_record.read_text())
assert build['schema']=='conduit.conduitos.build/v2' and build['artifact_role']=='product-host'
assert build['architecture']=='aarch64' and build['base_commit']==options.expected_commit
assert build['elf_sha256']==r['kernel_sha256']
assert hashlib.sha256((media/'iso-root/boot/conduitos').read_bytes()).hexdigest()==build['elf_sha256']
package=media/'iso-root/boot/conduit-protocol-source.json'
assert hashlib.sha256(package.read_bytes()).hexdigest()==options.expected_package
root_bytes=(media/'iso-root/boot/conduit-protocol-root.json').read_bytes()
assert hashlib.sha256(root_bytes).hexdigest()==r['root_request_sha256']
root=json.loads(root_bytes)
assert root['package_sha256']==options.expected_package
assert root['entry']=='cdc-serial-duplex-demonstration' and root['root_port']==1
assert root['maximum_control_operations']==5
assert root['preflight']=={'entry':'cdc-serial-profile-preflight','maximum_discovery_operations':2,'maximum_steps':4096}
assert root['endpoint_read']=={'address':129,'transfer_type':2,'maximum_packet':root['endpoint_read']['maximum_packet'],'interval':0,'endpoint_epoch':1,'maximum_operations':16}
assert root['endpoint_read']['maximum_packet'] in (32,64)
if options.verdict=='supported':assert root['endpoint_read']['maximum_packet']==64
assert root['endpoint_write']=={'address':2,'transfer_type':2,'maximum_packet':root['endpoint_write']['maximum_packet'],'interval':0,'endpoint_epoch':1,'maximum_operations':16}
assert root['endpoint_write']['maximum_packet'] in (32,64)
if options.verdict=='supported':assert root['endpoint_write']['maximum_packet']==64
assert r['root_kind']=='usb' and r['root_module_command']=='conduit.protocol/usb-root-request@1'
assert hashlib.sha256(image.read_bytes()).hexdigest()==r['image_sha256']
with tempfile.TemporaryDirectory(prefix='cdc-product-') as tmp:
 socket=pathlib.Path(tmp)/'device.sock';device=None;qemu=None
 with (out/'device.log').open('x') as dl,(out/'qemu.log').open('x') as ql:
  try:
   assert options.verdict=='supported' and device_profile=='valid'
   device_args=['python3',str(base.parent/'usb-serial-product-faults/device.py'),'--fault-mode',options.fault_mode]
   device=subprocess.Popen(device_args+['--socket',str(socket),'--transcript',str(out/'wire.jsonl'),'--seconds','60','--reply-hex',options.receive_hex],stdout=dl,stderr=dl)
   deadline=time.monotonic()+5
   while not socket.exists() and time.monotonic()<deadline:
    if device.poll() is not None:raise RuntimeError('device exited before socket')
    time.sleep(.02)
   if not socket.exists():raise RuntimeError('owned device socket timeout')
   firmware=base.parent/'usb-aarch64-tools/root/usr/share/qemu-efi-aarch64/QEMU_EFI.fd'
   emulator=base.parent/'usb-aarch64-tools/root/usr/bin/qemu-system-aarch64'
   args=[str(emulator),'-M','virt-10.2,highmem=off,gic-version=2,iommu=none','-cpu','neoverse-n2','-m','512M','-smp','1','-display','none','-monitor','none','-serial',f'file:{out}/serial.log','-no-reboot','-net','none','-bios',str(firmware),'-cdrom',str(image),'-boot','d','-device','qemu-xhci,id=protocol-usb,p2=1,p3=0','-chardev',f'socket,id=cdc,path={socket}','-device','usb-redir,chardev=cdc,bus=protocol-usb.0,port=1,msos-desc=off,streams=off,debug=4']
   qemu=subprocess.Popen(args,stdin=subprocess.DEVNULL,stdout=subprocess.DEVNULL,stderr=ql)
   deadline=time.monotonic()+55;outcome='observation-timeout'
   while time.monotonic()<deadline:
    serial=out/'serial.log';raw=serial.read_bytes() if serial.exists() else b''
    if len(raw)>262144 or (out/'qemu.log').stat().st_size>262144:raise RuntimeError('observation exceeded transcript bound')
    if b'CONDUIT_PROTOCOL_RETIRED complete' in raw:outcome='play-retired';break
    if b'"status":"refused"' in raw or b'CONDUIT_PROTOCOL_REFUSAL ' in raw or b'CONDUIT_AARCH64_PRODUCT_REFUSAL ' in raw:outcome='refused';break
    if qemu.poll() is not None:outcome='emulator-ended';break
    time.sleep(.05)
   receipt={'schema':'conduit.usb/cdc-control-observation@1','proof_class':'freestanding-emulator','scope':'pure descriptor preflight retirement before selected serial Back; unsupported profile refuses class effects; wire gate independent','outcome':outcome,'image_sha256':r['image_sha256'],'qemu_arguments':args,'serial_sha256':hashlib.sha256(raw).hexdigest(),'physical_compatibility':'unverified','architecture':'aarch64','protocol_image_receipt':r,'product_commit':options.expected_commit,'build_record':build,'firmware_sha256':hashlib.sha256(firmware.read_bytes()).hexdigest(),'emulator_sha256':hashlib.sha256(emulator.read_bytes()).hexdigest()}
   receipt['fault_mode']=options.fault_mode
   receipt['scope']='actual checked class refuses wrong or short observed line coding before SET_CONTROL_STATE and bulk; pure descriptor preflight supported'
   receipt['device_profile']=device_profile
   receipt['proof_runner_sha256']=hashlib.sha256(pathlib.Path(__file__).read_bytes()).hexdigest()
   receipt['validation_tools_sha256']={name:hashlib.sha256((base/name).read_bytes()).hexdigest() for name in ('check_discovery.py','check_coding_fault_wire.py','decoder','coding_fault_decoder')}
   receipt['validation']='not-run'
   validation_error=None
   expected_outcome='play-retired' if options.verdict=='supported' else 'refused'
   if outcome==expected_outcome:
    try:
     discovery=subprocess.run(['python3',str(base/'check_discovery.py'),str(out/'serial.log'),str(package),options.verdict,'--root',str(media/'iso-root/boot/conduit-protocol-root.json')],check=True,capture_output=True,text=True,timeout=10)
     receipt['discovery']=json.loads(discovery.stdout)
     decoded=subprocess.run([str(base/'coding_fault_decoder'),str(out/'serial.log'),'observation',options.receive_hex],check=True,capture_output=True,text=True,timeout=10) if options.verdict=='supported' else None
     wire=subprocess.run(['python3',str(base/'check_coding_fault_wire.py'),str(out/'wire.jsonl'),'aarch64',options.verdict,options.receive_hex,device_profile,options.fault_mode],check=True,capture_output=True,text=True,timeout=10)
     receipt['decoded_serial']=json.loads(decoded.stdout) if decoded else None
     receipt['independent_wire']=json.loads(wire.stdout)
     canonical=subprocess.run([str(base/'decoder'),str(out/'serial.log')],check=True,capture_output=True,text=True,timeout=10)
     observation=json.loads(canonical.stdout)
     assert observation['configuration']['input_hex']==receipt['independent_wire']['discovery_configuration_hex']
     assert observation['attachment_epoch']==root['attachment_epoch']
     for endpoint in ('endpoint_read','endpoint_write'):
      selected=root.get(endpoint)
      geometry={key:value for key,value in selected.items() if key!='maximum_operations'} if selected else None
      assert observation[endpoint]==geometry
     receipt['canonical_discovery']=observation
     receipt['validation']='passed'
    except (subprocess.CalledProcessError, AssertionError) as error:
     validation_error=str(error)
     receipt['validation']='failed'
     receipt['validation_error']={'command':error.cmd,'returncode':error.returncode,'stdout':error.stdout,'stderr':error.stderr} if isinstance(error,subprocess.CalledProcessError) else {'assertion':str(error)}
   if outcome!=expected_outcome:
    validation_error='unexpected native outcome: '+outcome
    receipt['validation']='failed'
    receipt['validation_error']=validation_error
   (out/'receipt.json').write_text(json.dumps(receipt,indent=2)+'\n');print(outcome)
   if validation_error is not None:raise RuntimeError(validation_error)
  finally:
   for p in (qemu,device):
    if p is not None and p.poll() is None:
     p.terminate()
     try:p.wait(timeout=2)
     except subprocess.TimeoutExpired:p.kill();p.wait(timeout=2)
