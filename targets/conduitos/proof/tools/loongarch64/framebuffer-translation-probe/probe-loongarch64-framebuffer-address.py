from pathlib import Path
import json,hashlib,subprocess,shutil,tempfile,os,time,socket,re
root=Path(__file__).resolve().parent;out=root/'loongarch64-framebuffer-address-probe-g0';out.mkdir(exist_ok=False)
hist=Path('/home/dancxjo/Documents/Codex/2026-10-06/goal-please-finish-and-close-5113/work/conduit/target/conduitos/live/loongarch64-virt');manifest=json.loads((hist/'build-manifest.json').read_text());assert manifest['artifact_role']=='product-host'
sha=lambda p:hashlib.file_digest(Path(p).open('rb'),'sha256').hexdigest()
for name in ['image','kernel']:assert sha(hist/manifest[name]['file'])==manifest[name]['sha256']
fw=Path('/home/dancxjo/Documents/Codex/2026-10-06/goal-please-finish-and-close-5113/work/conduit/target/conduitos/toolchain/QEMU_EFI.loongarch64-qemu8.fd');template_sha=sha(fw);assert template_sha=='b4c32ce95a54346c8bc180241a21ad763c5fcf43310e95aa93ed58bf0cc0864e'
cache=root/'conduit/target/conduitos/toolchain/loongarch64-domain-qemu/225a0b2f8dc09b3cf8fc174a6cbcaf961f5df9997326cceb7ee902540250938c'
reviewed=json.loads((cache/'receipt.json').read_text());assert reviewed['proof_class']=='diagnostic-emulator-tool-only';qemu=cache/'qemu-system-loongarch64';assert sha(qemu)==reviewed['executable_sha256']=='ad9ed950138b7fc2e2c2e86dd258c94216b1116becbf76126b4e429eabe4c477';env=os.environ.copy();env['QEMU_AUDIO_DRV']='none'
state={'classification':'historical-normal-product-read-only-paging-CSR-observation; not graphical Frame proof','firmware_provenance':'existing Conduit reviewed Debian2025.02-8+deb13u1 4MiB ROM pin','historical_image_manifest':manifest,'reviewed_diagnostic_emulator_receipt':reviewed,'graphical_product_proved':False,'physical_scanout_proved':False,'gpu_acceleration_proved':False,'inputs':[{'path':str(p),'bytes':p.stat().st_size,'sha256':sha(p)}for p in [qemu,fw,hist/manifest['image']['file']]]}
with tempfile.TemporaryDirectory(prefix='c6057-')as temp:
 endpoint=Path(temp)/'qmp';secret_path=Path(temp)/'entropy.bin';secret=bytearray(os.getrandom(4096,os.GRND_NONBLOCK));assert len(secret)==4096
 fd=os.open(secret_path,os.O_CREAT|os.O_EXCL|os.O_WRONLY,0o600)
 try:assert os.write(fd,secret)==4096
 finally:
  os.close(fd)
  for index in range(len(secret)):secret[index]=0
 state['virtual_entropy']={'provider':'Linux getrandom GRND_NONBLOCK; fresh4096bytes; ephemeral0600 file','fw_cfg_name':'opt/conduitos/cryptographic-entropy@1','bytes_retained':False}
 cmd=[str(qemu),'-fw_cfg',f'name=opt/conduitos/cryptographic-entropy@1,file={secret_path}','-machine','virt','-cpu','la464','-m','2G','-smp','1','-accel','tcg,thread=single','-bios',str(fw),'-drive',f'file={hist/manifest["image"]["file"]},format=raw,if=none,id=boot,readonly=on','-device','virtio-blk-pci,drive=boot,bootindex=0','-device','ramfb','-display','none','-monitor','none','-serial',f'file:{out/"serial.log"}','-net','none','-qmp',f'unix:{endpoint},server=on,wait=off','-no-reboot']
 state['arguments']=cmd;(out/'launch.json').write_text(json.dumps(state,indent=2)+'\n');state['environment']={'QEMU_AUDIO_DRV':env['QEMU_AUDIO_DRV']};state['emulator_version']=subprocess.check_output([str(qemu),'--version'],env=env,text=True);started=time.monotonic()
 with (out/'stderr.log').open('wb')as err:
  p=subprocess.Popen(cmd,env=env,stdin=subprocess.DEVNULL,stdout=subprocess.DEVNULL,stderr=err)
  try:
   while p.poll()is None and time.monotonic()-started<45:
    text=(out/'serial.log').read_text(errors='replace')if(out/'serial.log').exists()else''
    if 'CONDUIT_LOONGARCH64_PRODUCT_REFUSAL 'in text or 'CONDUIT_BOOT_STAGE loongarch64-product-ready'in text or ('CONDUIT_LOONGARCH64_PRODUCT {'in text and '"status":"ready"'in text):break
    time.sleep(.2)
   if p.poll()is None and endpoint.exists():
    with socket.socket(socket.AF_UNIX)as sock:
     sock.settimeout(5);sock.connect(str(endpoint));stream=sock.makefile('rwb');trace=[]
     def receive(sensitive=False):
      line=stream.readline(256*1024);assert line and len(line)<256*1024;value=json.loads(line);trace.append({'register_response_filtered':True,'nonwhitelisted_register_data_retained':False}if sensitive and 'return'in value else value);return value
     receive()
     def request(name,args=None,sensitive=False):
      stream.write(json.dumps({'execute':name,**({'arguments':args}if args else{})}).encode()+b'\n');stream.flush()
      while True:
       value=receive(sensitive)
       if 'return'in value or 'error'in value:return value
     request('qmp_capabilities')
     dump=request('human-monitor-command',{'command-line':'info registers'},True)
     assert isinstance(dump.get('return'),str)
     wanted=['CRMD','PWCL','PWCH','STLBPS','PGDL','PGDH','DMW0','DMW1','DMW2','DMW3','RVACFG']
     observed={}
     for name in wanted:
      match=re.search(r'(?<![A-Za-z0-9_])'+name+r'\s+([0-9a-fA-F]{1,16})(?![0-9a-fA-F])',dump['return'])
      if match:observed[name]=int(match.group(1),16)
     del dump
     state['paging_csrs']=observed
     state['raw_register_dump_retained']=False
     (out/'csr-observation.json').write_text(json.dumps({'paging_csrs':observed,'guest_paused':False,'general_registers_retained':False,'scope':'read-only architectural register observation after historical guest refusal, not table/scanout custody'},indent=2)+'\n')
     memory_reads=[]
     def words(address,count):
      assert 0xffff800000000000 <= address <= 0xffffffffffffffff and 0<count<=8
      response=request('human-monitor-command',{'command-line':f'x /{count}gx 0x{address:x}'})
      assert isinstance(response.get('return'),str),response
      values=[]
      for line in response['return'].splitlines():
       if ':' in line:
        values.extend(int(v,16)for v in re.findall(r'0x([0-9a-fA-F]{16})',line.split(':',1)[1]))
      assert len(values)==count,response
      memory_reads.append({'virtual_address':address,'words':values})
      return values
     symbols=subprocess.check_output(['nm','-a',str(hist/manifest['kernel']['file'])],text=True)
     selected={}
     for name in ['HHDM','FRAMEBUFFERS']:
      matches=[int(line.split()[0],16)for line in symbols.splitlines()if 'boot6limine'in line and line.endswith(name)]
      assert len(matches)==1
      selected[name]=matches[0]
     hhdm_response=words(selected['HHDM'],6)[5]
     hhdm=words(hhdm_response,2)[1]
     framebuffer_response=words(selected['FRAMEBUFFERS'],6)[5]
     framebuffer_header=words(framebuffer_response,3)
     assert framebuffer_header[1]==1
     framebuffer_pointer=words(framebuffer_header[2],1)[0]
     framebuffer=words(framebuffer_pointer,5)
     state['display_address_observation']={'hhdm_offset':hhdm,'framebuffer_virtual':framebuffer[0],'framebuffer_physical':framebuffer[0]-hhdm,'width':framebuffer[1],'height':framebuffer[2],'pitch':framebuffer[3],'packed_format_word':framebuffer[4],'selected_request_symbols':selected,'memory_reads':memory_reads,'scope':'historical normal-product read-only Limine response addresses; no reserved custody or graphical Frame proof'}
     (out/'display-address-observation.json').write_text(json.dumps(state['display_address_observation'],indent=2)+'\n')
     state['screendump_result']=request('screendump',{'filename':str(out/'display.ppm')});(out/'qmp.json').write_text(json.dumps(trace,indent=2)+'\n')
   state['elapsed_ms']=int((time.monotonic()-started)*1000)
  finally:
   if p.poll()is None:p.terminate()
   try:p.wait(timeout=5)
   except subprocess.TimeoutExpired:p.kill();p.wait()
 state['exit_code']=p.returncode
assert sha(fw)==template_sha
state['template_vars_unchanged']=True
state['files']=[{'path':x.name,'bytes':x.stat().st_size,'sha256':sha(x)}for x in out.iterdir()if x.is_file()and x.name!='vars.fd']
(out/'receipt.json').write_text(json.dumps(state,indent=2)+'\n')
assert 'CONDUIT_LOONGARCH64_PRODUCT_REFUSAL observatory-framebuffer-unrepresented'in(out/'serial.log').read_text(errors='replace');print(json.dumps(state.get('paging_csrs',{}),indent=2));print('CSR observation only; no new graphical Frame claimed')
