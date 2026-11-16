from pathlib import Path
import subprocess,shlex,base64,json,hashlib,os,time
root=Path(__file__).resolve().parent.parent
host='dancxjo@victus.local'
stage='.local/state/conduit/blue-admitted-recovery-20261107-a3a1b5b943/readonly-backup-20261115'
local=root/'work/private/blue-readonly-backup-20261115'
local.mkdir(mode=0o700);os.chmod(local,0o700)
code='''from pathlib import Path
import subprocess,hashlib,json,os
os.umask(0o077)
r=Path.home()/STAGE; r.mkdir(mode=0o700)
tool=r.parent/'espflash';port='/dev/serial/by-id/usb-Silicon_Labs_CP2102_USB_to_UART_Bridge_Controller_0001-if00-port0'
assert hashlib.sha256(tool.read_bytes()).hexdigest()=='03869e52b4ab6433720087eb8b895a94852e0c6f1e44b07c184b1dfeaab2f6ae'
assert subprocess.run(['fuser',port],capture_output=True).returncode==1
with (r/'read.log').open('xb') as log:
 subprocess.run([str(tool),'read-flash','--chip','esp32','--port',port,'--baud','921600','--after','hard-reset','--non-interactive','--skip-update-check','0','0x400000',str(r/'flash.bin')],stdout=log,stderr=subprocess.STDOUT,check=True,timeout=240)
raw=(r/'flash.bin').read_bytes();assert len(raw)==0x400000
assert 'b0:a7:32:28:61:78' in (r/'read.log').read_text().lower()
print(json.dumps({'bytes':len(raw),'sha256':hashlib.sha256(raw).hexdigest(),'physical_mac':'b0:a7:32:28:61:78','operation':'ROM read-flash only, hard reset into existing application afterward','flash_writes':False}))
'''.replace('STAGE',repr(stage))
command='python3 -c '+shlex.quote('import base64;exec(base64.b64decode('+repr(base64.b64encode(code.encode()).decode())+'))')
p=subprocess.run(['ssh','-o','BatchMode=yes','-o','ConnectTimeout=5',host,command],capture_output=True,text=True,timeout=260)
if p.returncode:
 (local/'remote-error.log').write_text(p.stderr);raise RuntimeError('Read-only backup failed; private error retained, no retry/reflash')
proof=json.loads(p.stdout)
for name in ['flash.bin','read.log']:
 subprocess.run(['scp','-q',host+':'+stage+'/'+name,str(local/name)],check=True)
raw=(local/'flash.bin').read_bytes();assert len(raw)==proof['bytes'] and hashlib.sha256(raw).hexdigest()==proof['sha256']
with (local/'flash.bin').open('rb') as f:os.fsync(f.fileno())
proof.update({'local_copy_sha256_verified':True,'captured_unix':time.time()})
(root/'outputs/blue-readonly-backup-20261115.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps(proof))
