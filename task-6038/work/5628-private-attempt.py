"""Private test-state setup; exact installed product, explicit checkpoint residence.
No runtime identity, invitation, or private path is printed. No retries.
"""
from pathlib import Path
import json, os, signal, subprocess, sys
T=Path('/home/dancxjo/Documents/Codex/2026-10-19/goal-6038')
P=T/'work/5628-private-owner-paths.json'
d=json.loads(P.read_text()); D=Path(d['directory']); W=T/'work/selected-look-reconciled'
os.umask(0o077)
assert D.stat().st_uid==os.getuid() and D.stat().st_mode&0o077==0
mode=sys.argv[1]
if mode=='prepare':
 n=int(sys.argv[2]);assert n==d['attempt']+1
 backup=D/f"attempt-{d['attempt']}-paths.json";assert not backup.exists();backup.write_text(json.dumps(d,indent=2)+'\n')
 state=D/f'state-{n}';checkpoint=D/f'checkpoint-{n}';state.mkdir(mode=0o700);checkpoint.mkdir(mode=0o700)
 with (D/f'install-{n}.log').open('xb') as log:
  r=subprocess.run([str(W/'target/5628-linux-release/conduit-linux-x86_64'),'host','service','install',str(W/'target/5628-linux-release/hosted-linux-x86_64.json'),'--state-dir',str(state),'--selected-todo-checkpoint-root',str(checkpoint),'--no-start'],stdout=log,stderr=subprocess.STDOUT);assert r.returncode==0
 owner=json.loads((state/'installation.json').read_text())['product_executable']
 with (D/f'own-body-{n}.log').open('xb') as log:
  r=subprocess.run([owner,'body','own',str(W/'plots/todo/checkpoint-once.conduit'),'--state-dir',str(state),'--name','Typography proof'],input=b'',stdout=log,stderr=subprocess.STDOUT,cwd=checkpoint);assert r.returncode==0
 d.update(state=str(state),checkpoint=str(checkpoint),installed_owner=owner,attempt=n);P.write_text(json.dumps(d,indent=2)+'\n');print('Fresh isolated test state prepared; no browser or domain action started.')
elif mode=='service':
 fd=os.open(D/f"owner-service-attempt-{d['attempt']}.log",os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600);os.dup2(fd,1);os.dup2(fd,2);os.close(fd);os.chdir(d['checkpoint']);os.execv(d['installed_owner'],[d['installed_owner'],'host','service','run','--state-dir',d['state']])
elif mode=='journey':
 n=d['attempt'];c=dict(matrix=sys.argv[2] if len(sys.argv)==3 else 'typography',repo=str(W),owner=d['installed_owner'],state=d['state'],handbook=str(W/'target/5628-handbook'),playwright='/home/dancxjo/conduit/proof/browser/node_modules/@playwright/test/index.mjs',out=str(D/f'look-proof-{n}'),source='253dcb985eb8890ddf873d672d3a282f2366e6db',seed_text='Crème brûlée · Ŵ — portable typography',spore=d['spore'],owner_forward=d['owner_forward'],qmp=str(D/f'look{n}.qmp'),ready_file=str(D/f'seeded-ready-{n}.json'),provision_ready=d['provision_ready'],xvfb=True)
 p=D/f'look-config-{n}.json';fd=os.open(p,os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600)
 with os.fdopen(fd,'w') as f:json.dump(c,f,indent=2)
 os.environ.update(CARGO_TARGET_DIR=str(T/'work/native-panels-host-target'),CARGO_BUILD_JOBS='4',CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='0');os.execvp('python3',['python3',str(T/'work/5628-owner-look-journey.py'),'--run',str(p)])
elif mode=='stop':
 n=0
 for proc in Path('/proc').iterdir():
  if not proc.name.isdigit():continue
  try:
   a=(proc/'cmdline').read_bytes().split(b'\0');exe=os.readlink(proc/'exe')
   if exe==d['installed_owner'] and d['state'].encode() in a and (a[1:4]==[b'host',b'service',b'run'] or a[1:3]==[b'body',b'invite']):os.kill(int(proc.name),signal.SIGTERM);n+=1
  except (OSError,IndexError):pass
 print('Stopped exact owned service/listener:',n)
else:raise ValueError('Use prepare N, service, journey or stop')
