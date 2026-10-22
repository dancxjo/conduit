"""Independent documentary checks over a sealed normal ProductHost QEMU run."""
from pathlib import Path
import argparse,hashlib,json
from PIL import Image

def strict(raw):
 def pairs(items):
  d={}
  for k,v in items:
   if k in d:raise ValueError('duplicate JSON key '+k)
   d[k]=v
  return d
 def bad(x):raise ValueError('non-finite JSON '+x)
 return json.loads(raw,object_pairs_hook=pairs,parse_constant=bad)
def sha(p):
 h=hashlib.sha256()
 with p.open('rb') as f:
  for b in iter(lambda:f.read(1048576),b''):h.update(b)
 return h.hexdigest()
def inside(root,name):
 p=(root/name).resolve();assert p.is_relative_to(root.resolve()),name;return p
ap=argparse.ArgumentParser();ap.add_argument('packet',type=Path);ap.add_argument('source');ap.add_argument('output',type=Path);a=ap.parse_args();p=a.packet
s=strict((p/'seal.json').read_bytes());assert s['exit_code']==0 and s['source_commit']==a.source and s['source_clean_after'] and s['source_unchanged']
for name,digest in s['hashes'].items():assert sha(inside(p,name))==digest,name
c=p/'live-images/x86_64-pc';m=strict((c/'build-manifest.json').read_bytes());j=strict((p/'journey-proof.json').read_bytes());g=strict((p/'graphical-profile-proof.json').read_bytes());f=strict((p/'journey-frames/manifest.json').read_bytes())
assert m['artifact_role']=='product-host' and m['source_identity']==j['source_commit']==g['source_commit']==a.source
assert sha(inside(c,m['image']['file']))==m['image']['sha256']==j['image_sha256']==g['image_sha256']
assert sha(inside(c,m['kernel']['file']))==m['kernel']['sha256']
assert f['status']=='complete' and f['failure'] is None and f['context']['source_commit']==a.source and f['context']['image_sha256']==j['image_sha256']
assert f['proof_class']=='freestanding-emulator' and j['physical_evidence'] is False and j['human_enactment'] is False
assert len(f['checkpoints'])==24
serial=(p/'journey-serial.log').read_bytes();complete=serial[:serial.rfind(b'\n')+1];records=[];offset=0
for line in complete.splitlines(keepends=True):
 offset+=len(line);line=line.rstrip(b'\r\n')
 if line.startswith(b'CONDUIT_') and b' ' in line:
  tag,raw=line.split(b' ',1)
  if raw.startswith(b'{'):records.append((offset,strict(raw)))
 assert not line.startswith((b'CONDUIT_PANIC ',b'CONDUIT_FAULT ')),line[:120]
frames=[];seen=set();last=0;boot_ids=set()
for cp in f['checkpoints']:
 name=cp['checkpoint'];assert name not in seen;seen.add(name);assert cp['health_refusal'] is None
 z=cp['frame'];q=inside(p/'journey-frames',z['png']);assert q.stat().st_size==z['png_bytes'] and sha(q)==z['png_sha256']
 with Image.open(q) as im:
  rgba=im.convert('RGBA');assert rgba.size==(z['width'],z['height']);assert hashlib.sha256(rgba.tobytes()).hexdigest()==z['pixel_sha256'];assert len(rgba.getcolors(rgba.width*rgba.height))>1
 end=cp['serial_byte_end'];assert last<=end<=len(complete) and complete[end-1:end]==b'\n';last=end
 assert any(v==cp['guest_record'] for n,v in records if n<=end),'guest record not in captured serial prefix'
 b=cp['guest_boot_record'];assert any(v==b for n,v in records if n<=end),'boot record not in captured serial prefix'
 assert b['status']=='accepted' and b['framebuffers']>0 and b['boot_id']==j['boot_id'] and b['host_id']==j['host_id'];boot_ids.add(b['boot_id'])
 frames.append({'checkpoint':name,'width':z['width'],'height':z['height'],'png_sha256':z['png_sha256'],'serial_byte_end':end})
assert len(boot_ids)==1 and 'body-playing' in seen and 'body-stopped' in seen
text=complete.decode('utf-8');lines=text.splitlines();stages=[(n,l.split()[1:]) for n,l in enumerate(lines,1) if l.startswith('CONDUIT_STARTUP_STAGE ')]
assert [v for _,v in stages]==[[x,y] for x in ['wake','plan','play'] for y in ['begin','accepted']]
assert lines.count('CONDUIT_CRECHE_CHECKPOINT body-born-lulled')==1 and lines.count('CONDUIT_CRECHE_CHECKPOINT body-playing')==1
born=lines.index('CONDUIT_CRECHE_CHECKPOINT body-born-lulled');active=lines.index('CONDUIT_CRECHE_CHECKPOINT body-playing');projections=[]
for n,l in enumerate(lines[:active+1]):
 if l.startswith('CONDUIT_PRODUCT_JOURNEY '):
  v=strict(l.split(' ',1)[1])
  if v['status'] in ['born-lulled','awake','planned','quiescent-awaiting-input'] and (n>=born or v['status']=='born-lulled'):projections.append(v)
unique=[]
for v in projections:
 reduced={k:v[k] for k in ['status','revision','body_id','wake_id','plan_id','active_play_id']}
 if not unique or reduced!=unique[-1]:unique.append(reduced)
assert [v['status'] for v in unique]==['born-lulled','awake','planned','quiescent-awaiting-input']
assert all(y['revision']==x['revision']+1 for x,y in zip(unique,unique[1:]))
assert len({v['body_id'] for v in unique})==1 and unique[0]['body_id']==j['body_id']
assert len({v['plan_id'] for v in unique[1:]})==1 and unique[-1]['active_play_id'] and all(v['active_play_id'] is None for v in unique[:-1])
assert len(j['steps'])>=20 and j['screenshots']=='journey-frames/manifest.json'
commands=[];clock=None;last_ns=-1
for n,line in enumerate((p/'journey-qmp.log').read_text().splitlines(),1):
 event=strict(line);assert event['observer_clock']=='std::time::Instant'
 stamp=event['observer_elapsed_ns'];assert isinstance(stamp,int) and stamp>=last_ns;last_ns=stamp
 if event['direction']=='command':commands.append((n,stamp,strict(event['message'])))
captures=[i for i,(_,_,cmd) in enumerate(commands) if cmd.get('execute')=='screendump' and cmd.get('arguments',{}).get('filename','').endswith('/body-playing.ppm')];assert len(captures)==1
stop=captures[0];previous=max(i for i,(_,_,cmd) in enumerate(commands[:stop]) if cmd.get('execute')=='screendump')
keys=[(n,t,cmd) for n,t,cmd in commands[previous+1:stop] if cmd.get('execute')=='input-send-event' and cmd.get('id','').endswith(':journey-lifecycle')]
assert len(keys)==2
for (_,_,cmd),down in zip(keys,[True,False]):
 events=cmd['arguments']['events'];assert len(events)==1 and events[0]['type']=='key' and events[0]['data']=={'down':down,'key':{'data':'ret','type':'qcode'}}
ns=commands[stop][1]-keys[0][1];assert ns>0
interval={'start_qmp_line':keys[0][0],'end_qmp_line':commands[stop][0],'nanoseconds':ns,'seconds':ns/1e9,'scope':'QMP Birth Enter key-down command to body-playing screendump command; includes observer overhead, not ACK-to-ACK guest latency, capture end or speedup.'}
assert len(g['headless_images'])==4 and all(x['graphical_assets'] is False for x in g['headless_images'])
out={'schema':'conduit.audit/normal-startup-originals@1','status':'verified','source':a.source,'packet':str(p),'sealed_original_files':len(s['hashes']),'pngs_verified':len(frames),'frames':frames,'stage_lines':stages,'lifecycle':unique,'observer_command_interval':interval,'headless_asset_exclusion_checks':g['headless_images'],'image_sha256':m['image']['sha256'],'kernel_sha256':m['kernel']['sha256'],'scope':'Normal ProductHost emulator originals, canonical image/source, captured serial prefixes and one-Birth startup identities. No independent Face digest recomputation, physical/human evidence, calibrated latency, CI or full issue closure.'}
a.output.write_text(json.dumps(out,indent=2)+'\n');print(json.dumps({'status':'verified','source':a.source,'pngs':len(frames),'originals':len(s['hashes'])}))
