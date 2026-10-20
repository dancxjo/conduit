from pathlib import Path
import json,hashlib,subprocess
T=Path('/home/dancxjo/Documents/Codex/2026-10-19/goal-6038'); W=T/'work/native-typography'; H='ffcdb1a8ea7caaf9fbdaa4e077c8b04b620dd2b7'
def read(p):return json.loads(p.read_text())
def digest(p):return hashlib.sha256(p.read_bytes()).hexdigest()
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=W,text=True).strip()==H
assert not subprocess.check_output(['git','status','--porcelain'],cwd=W,text=True).strip()
product=T/'outputs/5992-product-ffcdb1a8e';m=read(product/'build-manifest.json');assert m['source_identity']==H
for key in ['image','kernel']:assert digest(product/m[key]['file'])==m[key]['sha256']
stages=[];dirs=[]
normal=T/'outputs/5992-normal-journey-ffcdb1a8e/x86_64';n=read(normal/'x86_64-product-journey-proof.json');assert n['source_commit']==H and n['image_sha256']==m['image']['sha256'];dirs.append(normal)
stages.append({'name':'supported-normal-journey','receipt':str((normal/'x86_64-product-journey-proof.json').relative_to(T)),'steps':len(n['steps']),'physical':False,'human':False})
for family in ['panels-wide','panels-narrow','pointer-wide','pointer-narrow','details-wide','details-narrow']:
 suffix='-cold-birth' if family.startswith('pointer') else '';p=T/f'outputs/5992-{family}-ffcdb1a8e{suffix}';j=read(p/'receipt.json');dirs.append(p)
 assert j['status']=='passed' and j['source']==H and j['image_sha256']==m['image']['sha256'] and j['exit_code']==0
 for c in j['captures']:
  assert digest(p/c['file'])==c['sha256'];f=c['focus'];v=f['display_bounds'];b=f['hit_bounds']
  if b is None:assert f['action_id'] is None and f['argument_name'] is None
  else:assert 0<=b['x'] and 0<=b['y'] and b['x']+b['width']<=v['width'] and b['y']+b['height']<=v['height']
 s={'name':family,'receipt':str((p/'receipt.json').relative_to(T)),'captures':len(j['captures']),'edits':len(j['edits'])}
 if family.startswith('details'):
  assert j['source_effects']==0 and j['typed_identity_properties'];a=j['captures'][0]['focus']
  for c in j['captures']:
   for key in ['face_id','face_revision','show_id']:assert c['focus'][key]==a[key]
  s.update(details_pages=j['details_pages'],source_effects=0)
 if family.startswith('pointer'):s.update(hits=len(j['pointer_hits']),boundary=j['boundary'])
 stages.append(s)
h=T/'outputs/5992-headless-ffcdb1a8e';hm=read(h/'build-manifest.json');hr=read(h/'headless-proof.json');assert hm['source_identity']==H;dirs.append(h)
for key in ['image','kernel']:assert digest(h/hm[key]['file'])==hm[key]['sha256']==hr[key+'_sha256']
for k in ['first','second']:
 sign=hr[k]['sign'];assert sign['status']=='unsupported' and sign['image_binding']==hm['resolved_build']['image_id'];assert sign['initialized_capabilities']==sign['presenters']==sign['facilities']==sign['face_surface_bytes']==sign['allocated_bytes']==0
assert hr['first']['boot_id']!=hr['second']['boot_id'] and hr['first']['host_id']!=hr['second']['host_id']
files=[{'path':str(p.relative_to(T)),'bytes':p.stat().st_size,'sha256':digest(p)} for d in dirs for p in sorted(d.rglob('*')) if p.is_file()]
r={'source':H,'tree':subprocess.check_output(['git','rev-parse','HEAD^{tree}'],cwd=W,text=True).strip(),'status':'local-normal-and-headless-proof-passed','proof_class':'freestanding-emulator','normal_product':m,'normal_stages':stages,'headless_receipt':str((h/'headless-proof.json').relative_to(T)),'headless_proof_sha256':digest(h/'headless-proof.json'),'files':files,'boundary':'Seven normal proof runs and two unsupported-startup headless boots. No physical/human/stable acceptance or two-Look proof. Scalar rendering is not full shaping. Earlier failed receipts retained separately. Current complete-library CI remains required.'}
o=T/'outputs/5992-ffcdb1a8e/receipt.json';o.write_text(json.dumps(r,indent=2)+'\n');print(json.dumps({'stages':stages,'raw_files':len(files),'receipt_sha256':digest(o)},indent=2))
