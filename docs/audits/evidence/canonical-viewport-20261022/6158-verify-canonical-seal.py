import argparse,json,pathlib,hashlib
from PIL import Image
ap=argparse.ArgumentParser();ap.add_argument('packet');ap.add_argument('output');a=ap.parse_args();p=pathlib.Path(a.packet);s=json.loads((p/'seal.json').read_text());assert s['exit_code']==0 and s['source_clean_after'] and s['source_unchanged']
def sha(q):
 h=hashlib.sha256()
 with q.open('rb') as f:
  for b in iter(lambda:f.read(1024*1024),b''):h.update(b)
 return h.hexdigest()
for name,digest in s['hashes'].items():
 q=(p/name).resolve();assert q.is_relative_to(p.resolve()) and sha(q)==digest,name
c=p/'live-images/x86_64-pc';m=json.loads((c/'build-manifest.json').read_text());g=json.loads((p/'graphical-profile-proof.json').read_text());j=json.loads((p/'journey-proof.json').read_text());f=json.loads((p/'journey-frames/manifest.json').read_text())
assert m['artifact_role']=='product-host' and m['source_identity']==g['source_commit']==j['source_commit']==s['source_commit']
assert m['image']['sha256']==g['image_sha256']==j['image_sha256']==sha(c/m['image']['file'])
assert m['kernel']['sha256']==sha(c/m['kernel']['file'])
assert f['status']=='complete' and f['failure'] is None and len(f['checkpoints'])==24
width,height=map(int,s['requested_viewport'].split('x'))
for cp in f['checkpoints']:
 z=cp['frame'];q=p/'journey-frames'/z['png'];assert sha(q)==z['png_sha256'] and q.stat().st_size==z['png_bytes']
 with Image.open(q) as im:
  rgba=im.convert('RGBA');assert rgba.size==(width,height)==(z['width'],z['height']);assert hashlib.sha256(rgba.tobytes()).hexdigest()==z['pixel_sha256']
e=f['context']['viewport_guest_evidence'];assert e['evidence']=='guest Mask display bounds with prior acknowledged Show'
for focus in e['focus_records']:assert focus['display_bounds']=={'x':0,'y':0,'width':width,'height':height}
out={'schema':'conduit.audit/canonical-viewport-seal@1','status':'verified','source':s['source_commit'],'viewport':s['requested_viewport'],'packet':str(p),'original_files_rehashed':len(s['hashes']),'pngs_verified':len(f['checkpoints']),'focus_extent_records':len(e['focus_records']),'artifact_role':m['artifact_role'],'image_sha256':m['image']['sha256'],'kernel_sha256':m['kernel']['sha256'],'scope':'Canonical ProductHost emulator journey, actual Mask extent and QMP pixels; headless asset checks only, no naming/all-page/before-after/physical acceptance'}
pathlib.Path(a.output).write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(out,indent=2))
