import pathlib,subprocess,json,hashlib,socket,time,datetime,tempfile,os
root=pathlib.Path.cwd();out=root/'outputs/6057-aarch64-first-frame-06bf2d320';out.mkdir(exist_ok=True)
tool=root/'work/6057-tools/extracted/usr/bin/qemu-system-aarch64';fw=root/'work/6057-tools/extracted/usr/share/qemu-efi-aarch64/QEMU_EFI.fd'
images=root/'work/6057-normal-graphical-host';manifest=json.loads((images/'build-manifest.json').read_text());image=images/manifest['image']['file'];assert manifest['source_identity']=='06bf2d3207bc17794e0b18336e56c824ce08e973';assert manifest['artifact_role']=='product-host';assert manifest['target']=='conduitos/aarch64/virt'
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
assert sha(image)==manifest['image']['sha256']
assert sha(images/'conduitos-kernel.elf')==manifest['kernel']['sha256']
socket_dir=tempfile.TemporaryDirectory(prefix='c6057-');qmp=pathlib.Path(socket_dir.name)/'qmp.sock';serial=out/'serial.log';err=out/'stderr.log'
args=[str(tool),'-M','virt','-cpu','neoverse-n2','-m','512M','-smp','1','-display','none','-monitor','none','-serial',f'file:{serial}','-net','none','-no-reboot','-bios',str(fw),'-cdrom',str(image),'-boot','d','-device','ramfb','-qmp',f'unix:{qmp},server=on,wait=off']
receipt={'started_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'issue':6057,'classification':'Normal PROFILE product first-frame experiment; acceptance requires separate serial, image, Show and PNG audit','command':args,'source':manifest['source_identity'],'image_sha256':sha(image),'kernel_sha256':sha(images/'conduitos-kernel.elf'),'firmware_sha256':sha(fw),'emulator_sha256':sha(tool),'version':subprocess.check_output([str(tool),'--version'],text=True).splitlines()[0],'product_show_proved':False}
try:
 with err.open('wb') as ef:
  modules=root/'work/6057-tools/extracted/usr/lib/x86_64-linux-gnu/qemu';modules.mkdir(exist_ok=True)
  env=dict(os.environ,QEMU_MODULE_DIR=str(modules))
  receipt['module_path']=str(modules)
  child=subprocess.Popen(args,stdout=subprocess.DEVNULL,stderr=ef,env=env)
  deadline=time.monotonic()+45
  while time.monotonic()<deadline and child.poll() is None:
   raw=serial.read_text(errors='replace') if serial.exists() else ''
   complete=raw.rsplit('\n',1)[0]
   if 'CONDUIT_AARCH64_PRODUCT {' in complete or 'CONDUIT_AARCH64_PRODUCT_REFUSAL ' in complete:break
   time.sleep(.2)
  receipt['process_live_at_capture']=child.poll() is None
  receipt['serial_complete_product_record']='CONDUIT_AARCH64_PRODUCT {' in complete if serial.exists() else False
  if child.poll() is None and qmp.exists():
   with socket.socket(socket.AF_UNIX) as sock:
    sock.settimeout(5);sock.connect(str(qmp));f=sock.makefile('rwb',buffering=0);receipt['qmp_greeting']=json.loads(f.readline())
    def call(execute,arguments=None):
     d={'execute':execute}
     if arguments:d['arguments']=arguments
     f.write((json.dumps(d)+'\n').encode())
     while True:
      r=json.loads(f.readline())
      if 'return' in r or 'error' in r:return r
    receipt['qmp_capabilities']=call('qmp_capabilities');receipt['screendump']=call('screendump',{'filename':str(out/'display.ppm')})
  receipt['serial_sha256']=sha(serial) if serial.exists() else None
  receipt['records']={}
  for prefix in ['CONDUIT_NATIVE_FIRST_FRAME ', 'CONDUIT_AARCH64_PRODUCT ']:
   lines=[line[len(prefix):] for line in complete.splitlines() if line.startswith(prefix)]
   receipt['records'][prefix.strip()]=[json.loads(line) for line in lines]
  if (out/'display.ppm').exists():
   from PIL import Image,ImageStat
   with Image.open(out/'display.ppm') as capture:
    capture.save(out/'display.png')
    receipt['png']={'sha256':sha(out/'display.png'),'size':list(capture.size),'extrema':capture.getextrema(),'nonuniform':any(lo!=hi for lo,hi in capture.convert('RGB').getextrema())}

finally:
 if 'child' in locals() and child.poll() is None:child.terminate();child.wait(timeout=10)
 receipt['exit_after_owned_cleanup']=child.returncode if 'child' in locals() else None
 receipt['stderr']=err.read_text(errors='replace') if err.exists() else ''
 (out/'receipt.json').write_text(json.dumps(receipt,indent=2)+'\n')
 print(json.dumps(receipt,indent=2))
