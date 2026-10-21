from pathlib import Path
import json,os,subprocess,socket,hashlib,sys,time
T=Path('/home/dancxjo/Documents/Codex/2026-10-19/goal-6038');W=T/'work/selected-look-current-owner';P=T/'work/5628-private-owner-paths.json';H='db1f41fcc4fe2dea2fe9eaedf799855661542eab'
d=json.loads(P.read_text());attempt=d.get('attempt',3);assert isinstance(attempt,int) and 3<=attempt<=32;D=Path(d['directory']);assert D.stat().st_mode&0o077==0 and D.stat().st_uid==os.getuid()
mode=sys.argv[1] if len(sys.argv)==2 else ''
if mode in ('--prepare','--invite','--provision'):
 assert json.loads((Path(d['state'])/'installation.json').read_text())['release_source_identity']==H
 assert json.loads((W/'target/conduitos/live/x86_64-pc/build-manifest.json').read_text())['source_identity']==H
if mode=='--prepare':
 s=socket.socket();s.bind((d['owner_forward_ip'],0));port=s.getsockname()[1];s.close();d.update(owner_forward=f"{d['owner_forward_ip']}:{port}",invitation=str(D/f'invitation-{attempt}.json'),spore=str(D/f'spore-{attempt}.iso'),body_description=str(D/f'native-{attempt}.body.conduit'),provision_ready=str(D/f'provision-ready-{attempt}.json'));P.write_text(json.dumps(d,indent=2)+'\n');print('Private route paths prepared; no listener/invitation created')
elif mode=='--invite':
 fd=os.open(d['invitation'],os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600);os.dup2(fd,1);os.close(fd);fd=os.open(D/f'invitation-listener-{attempt}.log',os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600);os.dup2(fd,2);os.close(fd)
 args=[d['installed_owner'],'body','invite','--state-dir',d['state'],'--ttl-seconds','600','--route-bind',d['owner_forward'],'--route-url','wss://10.0.2.42:4433/conduit','--route-tls-cert',d['certificate'],'--route-tls-key',d['key'],'--authorize-route'];os.execv(args[0],args)
elif mode=='--provision':
 assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=W,text=True).strip()==H
 assert not subprocess.check_output(['git','status','--porcelain'],cwd=W,text=True).strip()
 deadline=time.monotonic()+30
 while True:
  try:
   raw=Path(d['invitation']).read_bytes();assert len(raw)<=64*1024;inv=json.loads(raw);break
  except (FileNotFoundError,json.JSONDecodeError):
   assert time.monotonic()<deadline;time.sleep(.1)
 assert inv['schema']=='conduit.body/spawn-invitation@2' and inv.get('rendezvous')
 claim=inv['claim'];body=claim['body_id'];invitation=claim['invitation_id'];profile=str(W/'targets/conduitos/profiles/conduitos-native.host.conduit');text='body shared {\n schema = 2\n id = '+json.dumps(body)+'\n host = {name: "native", configuration: '+json.dumps(profile)+', spore: {join_mode: "self-joining", invitation: '+json.dumps(invitation)+', output: "disk-image"}}\n}\n';fd=os.open(d['body_description'],os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600)
 with os.fdopen(fd,'w') as f:f.write(text)
 env=dict(os.environ,CARGO_TARGET_DIR=str(T/'work/native-panels-host-target'),CARGO_BUILD_JOBS='4',CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='0');commands=[['cargo','+stable','xtask','make','body','check',d['body_description']],['cargo','+stable','xtask','make','body','provision-conduitos',d['body_description'],'--host','native','--build',str(W/'target/conduitos/live/x86_64-pc'),'--invitation',d['invitation'],'--route-tls-cert',d['certificate'],'--output',d['spore']]]
 fd=os.open(D/f'provision-{attempt}.log',os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600)
 with os.fdopen(fd,'wb') as log:
  for args in commands:
   r=subprocess.run(args,cwd=W,env=env,stdout=log,stderr=subprocess.STDOUT,timeout=120);assert r.returncode==0,'supported provisioning refused; inspect private log'
 p=Path(d['spore']);assert p.stat().st_mode&0o077==0;receipt={'source':H,'iso_sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'membership_admitted':False,'candidate_id':next(c['candidate_id'] for c in inv['rendezvous']['candidates'] if c['line_family']=='authenticated-tls-stream')};fd=os.open(d['provision_ready'],os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600)
 with os.fdopen(fd,'w') as f:json.dump(receipt,f);f.write('\n')
 print('Private normal product spore provisioned; invitation/media remain private; membership not yet admitted')
else:
 print('Preparation helper only. Explicit --prepare/--invite/--provision required.')
