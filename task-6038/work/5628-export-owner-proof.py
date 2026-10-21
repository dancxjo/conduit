"""Export bounded public native evidence; keys/routes/private owner state excluded."""
from pathlib import Path
import hashlib,json,re,struct,sys,zlib
T=Path('/home/dancxjo/Documents/Codex/2026-10-19/goal-6038');d=json.loads((T/'work/5628-private-owner-paths.json').read_text());n=int(sys.argv[1]);D=Path(d['directory']);I=D/f'look-proof-{n}';N=I/'native';r=json.loads((I/'receipt.json').read_text());assert r['status']!='running'
source=json.loads((I/'browser/browser-joined.json').read_text())['source'];assert re.fullmatch('[a-f0-9]{40}',source);assert r.get('source',source)==source
O=T/f'outputs/5628-owner-look-{n}';O.mkdir();rows=[]
def sha(x):return hashlib.sha256(x).hexdigest()
def chunk(t,x):return struct.pack('>I',len(x))+t+x+struct.pack('>I',zlib.crc32(t+x)&0xffffffff)
for p in sorted(N.glob('*.ppm')):
 b=p.read_bytes();m=re.match(rb'P6\s+(\d+)\s+(\d+)\s+255\s',b);assert m;w,h=map(int,m.groups());rgb=b[m.end():];assert len(rgb)==w*h*3 and w<=4096 and h<=4096
 png=b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',w,h,8,2,0,0,0))+chunk(b'IDAT',zlib.compress(b''.join(b'\0'+rgb[y*w*3:(y+1)*w*3] for y in range(h)),9))+chunk(b'IEND',b'');out=O/(p.stem+'.png');out.write_bytes(png);rows.append(dict(file=out.name,width=w,height=h,ppm_sha256=sha(b),png_sha256=sha(png),rgb_sha256=sha(rgb)))
serial=(N/'serial.log').read_bytes();records=[]
for l in serial.decode().splitlines():
 if any(l.startswith(p+' ') for p in ['CONDUIT_NATIVE_COMPONENT_REALIZATION','CONDUIT_NATIVE_OWNER_FACE','CONDUIT_CRECHE_FOCUS','CONDUIT_NATIVE_COMPONENT_SELECTION']):
  prefix,payload=l.split(' ',1);value=json.loads(payload)
  # Owner ACKs and Show/focus/realization truth only; no admission secret or endpoint.
  records.append(dict(prefix=prefix,value=value))
assert len(records)<=512
(O/'native-observations.json').write_text(json.dumps(records,indent=2)+'\n')
bindings=[]
for observation in r['observations']:
 p=I/'browser'/observation['browser_artifact']['file'];b=p.read_bytes();assert sha(b)==observation['browser_artifact']['sha256'];x=json.loads(b);assert x['source']==source;report=x['report'];binding=x['revision_binding'];assert report['show_id']==observation['show'];assert binding['face_id']==observation['face'] and binding['face_revision_decimal']==observation['revision'];bindings.append(dict(label=observation['label'],original_private_artifact_sha256=sha(b),source=x['source'],workload=x['workload'],revision_binding=binding,owner_plan_id=report['owner_plan_id'],show_id=report['show_id'],selected=report['selected']))
(O/'independent-owner-bindings.json').write_text(json.dumps(bindings,indent=2)+'\n')
public=dict(source=source,status=r['status'],matrix=r.get('matrix'),refresh_budget=r.get('refresh_budget'),capture_review='pending',native_serial_sha256=sha(serial),private_controller_receipt_sha256=sha((I/'receipt.json').read_bytes()),captures=rows,observations=[{k:v for k,v in x.items() if k!='browser_artifact'} for x in r['observations']],boundaries=['No stable, physical or human claim','Raw private logs/state/media/URLs omitted; public typed observations and independent owner bindings retained'])
(O/'receipt.json').write_text(json.dumps(public,indent=2)+'\n');print(json.dumps({'status':r['status'],'matrix':r.get('matrix'),'captures':len(rows),'owner_bindings':len(bindings)}))
