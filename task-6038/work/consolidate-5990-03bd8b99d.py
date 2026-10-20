from pathlib import Path
import json,hashlib,subprocess
T=Path('/home/dancxjo/Documents/Codex/2026-10-19/goal-6038');O=T/'outputs';head='03bd8b99d1018544f7df9bae159e8f416c859689';tag=head[:9]
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=T/'work/native-panels',text=True).strip()==head
assert not subprocess.check_output(['git','status','--porcelain'],cwd=T/'work/native-panels',text=True).strip()
def load(p):return json.loads(p.read_text())
product=O/f'5990-product-{tag}';m=load(product/'build-manifest.json');assert m['source_identity']==head and m['artifact_role']=='product-host'
image=m['image']['sha256'];assert hashlib.sha256((product/m['image']['file']).read_bytes()).hexdigest()==image
normal=O/f'5990-normal-journey-{tag}'
j=load(normal/'x86_64/journey-proof.json');assert j['source_commit']==head and j['image_sha256']==image and len(j['steps'])==22
summary={};dirs=[product,normal]
for family in ['panels','pointer']:
 for label in ['wide','narrow']:
  name=f'5990-{family}-{label}-{tag}'+('-cold-birth' if family=='pointer' else '')
  d=O/name;r=load(d/'receipt.json');assert r['source']==head and r['image_sha256']==image and r['status']=='passed' and r['exit_code']==0
  dirs.append(d)
  for c in r['captures']:assert hashlib.sha256((d/c['file']).read_bytes()).hexdigest()==c['sha256']
  if family=='panels':
   assert len(r['initial_focus_cycle'])==12 and len(r['edits'])==5 and len(r['source_plot_candidates'])==5 and len(r['filtered_selected_subjects'])==4
   summary[name]={'controls':12,'accepted_source_edits':5,'captures':len(r['captures'])}
  else:
   assert len(r['pointer_hits'])==12 and len(r['dispatches'])==2 and r['birth']['receipt']['accepted_effects']==1
   summary[name]={'physical_control_hits':12,'boolean_typed_commits':2,'birth_accepted_effects':1,'boundary':r['boundary'],'post_birth_release':r['birth_attempt']['release']}
files={}
for d in dirs:
 for p in sorted(d.rglob('*')):
  if p.is_file():assert not p.is_symlink();files[str(p.relative_to(O))]={'sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'bytes':p.stat().st_size}
receipt={'source':head,'base':'7c6defe20d5c0ef344d96d9b5abf0fb6131ade49','image_sha256':image,'artifact_role':'product-host','proof_class':'freestanding-emulator','physical_hardware':False,'human_enactment':False,'normal_journey_checkpoints':22,'candidate_integration_stable':'pending for this refreshed Source','supplements':summary,'files':files}
out=O/f'5990-stage05-{tag}';out.mkdir(exist_ok=False);(out/'receipt.json').write_text(json.dumps(receipt,indent=2)+'\n');print(json.dumps({'files':len(files),'receipt':str(out/'receipt.json'),'source':head,'image':image}))
