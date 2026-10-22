import subprocess,os,json,time,hashlib,pathlib,shutil,sys,datetime
from PIL import Image
root=pathlib.Path.cwd();w=root/'work';cwd=w/'6158-bounded-captures';source='eb77aad03dae3654709e8004f617d48550e95a9d';mode=sys.argv[1];assert mode in ['1280x800','640x480']
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=cwd,text=True).strip()==source
assert not subprocess.check_output(['git','status','--porcelain'],cwd=cwd)
checks=json.loads((w/'6158-eb77aad03-validation.json').read_text());assert checks['status']=='passed' and checks['source_commit']==source
target=w/f'6158-eb77aad03-controls-{mode}-target';packet=w/f'6158-eb77aad03-controls-{mode}-preserved';assert not target.exists();packet.mkdir()
env=os.environ.copy();env.pop('RUST_MIN_STACK',None);env.update(CARGO_TARGET_DIR='/home/dancxjo/Documents/Codex/2026-10-21/goal-find-the-first-available-github/work/lint-target',CARGO_BUILD_JOBS='4',CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='0',CONDUIT_CONDUITOS_TARGET_ROOT=str(target))
cmd=['cargo','+stable','xtask','make','conduitos','creche-controls-proof','--viewport',mode,'--locked'];start=time.monotonic();print('START',mode,flush=True)
(packet/'context.json').write_text(json.dumps({'source_commit':source,'requested_viewport':mode,'command':cmd,'started_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'boundary':'Separate canonical live-image controls proof. Actual source-owned naming/control evidence, no default journey or full6158 acceptance. EDID preference, never crop; capture count is actual.'},indent=2)+'\n')
with (packet/'supported-command.log').open('wb') as f:result=subprocess.run(cmd,cwd=cwd,env=env,stdout=f,stderr=subprocess.STDOUT)
print('TERMINAL',mode,result.returncode,flush=True);artifacts=target/'x86_64'
for p in artifacts.iterdir() if artifacts.exists() else []:
 if p.is_file():shutil.copy2(p,packet/p.name)
 elif p.name=='journey-frames':shutil.copytree(p,packet/p.name)
live=cwd/'target/conduitos/live'
for host in live.iterdir() if live.exists() else []:
 if host.is_dir() and (host/'build-manifest.json').exists():shutil.copytree(host,packet/'live-images'/host.name)
receipt={'source_commit':source,'requested_viewport':mode,'exit_code':result.returncode,'seconds':time.monotonic()-start,'source_clean_after':not subprocess.check_output(['git','status','--porcelain'],cwd=cwd),'source_unchanged':subprocess.check_output(['git','rev-parse','HEAD'],cwd=cwd,text=True).strip()==source,'hashes':{},'verified_frames':[]}
def digest(p):
 h=hashlib.sha256()
 with p.open('rb') as f:
  for chunk in iter(lambda:f.read(1024*1024),b''):h.update(chunk)
 return h.hexdigest()
for p in sorted(packet.rglob('*')):
 if p.is_file():receipt['hashes'][str(p.relative_to(packet))]=digest(p)
receipt['generic_target_snapshot_boundary']='Generic target files may be builder/headless outputs; canonical ProductHost ELF/ISO custody comes exclusively from live-images/x86_64-pc manifest.'
canonical=packet/'live-images/x86_64-pc'
if (canonical/'build-manifest.json').exists():
 manifest=json.loads((canonical/'build-manifest.json').read_text());receipt['canonical_manifest_source']=manifest['source_identity'];receipt['artifact_role']=manifest['artifact_role'];receipt['canonical_current_source']=manifest['source_identity']==source
 if manifest['source_identity']==source:
  assert manifest['artifact_role']=='product-host'
  assert digest(canonical/manifest['image']['file'])==manifest['image']['sha256']
  assert digest(canonical/manifest['kernel']['file'])==manifest['kernel']['sha256']
  if (packet/'graphical-profile-proof.json').exists():
   proof=json.loads((packet/'graphical-profile-proof.json').read_text());assert proof['source_commit']==source and proof['image_sha256']==manifest['image']['sha256']
   receipt['headless_evidence_boundary']='Four headless build/image asset-absence checks only; no headless guest runtime claim.'
  if (packet/'journey-proof.json').exists():
   journey=json.loads((packet/'journey-proof.json').read_text());assert journey['source_commit']==source and journey['image_sha256']==manifest['image']['sha256']
 else:assert result.returncode,'successful command cannot use a stale canonical image'
if (packet/'journey-frames/manifest.json').exists():
 manifest=json.loads((packet/'journey-frames/manifest.json').read_text());receipt['frame_manifest_status']=manifest['status'];receipt['frame_failure']=manifest['failure'];receipt['viewport_context']=manifest['context']
 for c in manifest['checkpoints']:
  frame=c['frame'];p=packet/'journey-frames'/frame['png'];assert p.stat().st_size==frame['png_bytes'] and digest(p)==frame['png_sha256']
  with Image.open(p) as im:
   rgba=im.convert('RGBA');assert rgba.size==(frame['width'],frame['height']);assert hashlib.sha256(rgba.tobytes()).hexdigest()==frame['pixel_sha256']
  receipt['verified_frames'].append({'checkpoint':c['checkpoint'],'width':frame['width'],'height':frame['height'],'png_sha256':frame['png_sha256'],'pixel_sha256':frame['pixel_sha256']})
if (packet/'creche-controls-proof.json').exists():
 control=json.loads((packet/'creche-controls-proof.json').read_text());manifest=json.loads((canonical/'build-manifest.json').read_text())
 assert control['source_commit']==source and control['image_sha256']==manifest['image']['sha256']
 assert control['canonical_build_metadata']['target_build_manifest']==manifest
 assert control['profile_id']==manifest['profile_id'] and control['build_id']==manifest['build_id']
 assert control['logical_image_binding']==manifest['resolved_description_binding']
 assert control['final_image_id']==manifest['image_id']
 assert control['captures']==len(receipt['verified_frames'])<=27
 receipt['controls']={'status':control['status'],'captures':control['captures'],'forward_focus_records':len(control['forward_focus']),'reverse_focus_records':len(control['reverse_focus']),'primary_pages':len(control['primary_pages']),'details_pages':len(control['details_pages']),'full6158_acceptance':control['full_6158_acceptance']}
(packet/'seal.json').write_text(json.dumps(receipt,indent=2)+'\n');print('SEALED',packet,'frames',len(receipt['verified_frames']),flush=True)
sys.exit(result.returncode)
