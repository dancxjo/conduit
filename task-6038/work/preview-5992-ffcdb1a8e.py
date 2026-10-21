from pathlib import Path
import re,zlib,struct,hashlib,json
T=Path('/home/dancxjo/Documents/Codex/2026-10-19/goal-6038');O=T/'outputs/5992-visual-ffcdb1a8e';O.mkdir(exist_ok=True);assert not (O/"receipt.json").exists()
def sha(b):return hashlib.sha256(b).hexdigest()
def chunk(tag,b):return struct.pack('>I',len(b))+tag+b+struct.pack('>I',zlib.crc32(tag+b)&0xffffffff)
rows=[]
for label in ['wide','narrow']:
 for chapter in ['panels','pointer','details']:
  D=T/(f'outputs/5992-{chapter}-{label}-ffcdb1a8e'+('-cold-birth' if chapter=='pointer' else ''));r=json.loads((D/'receipt.json').read_text());assert r['status']=='passed'
  for c in r['captures']:
   p=D/c['file'];b=p.read_bytes();assert sha(b)==c['sha256'];m=re.match(rb'P6\s+(\d+)\s+(\d+)\s+255\n',b);assert m
   w,h=map(int,m.groups());pixels=b[m.end():];assert len(pixels)==w*h*3
   raw=b''.join(b'\0'+pixels[y*w*3:(y+1)*w*3] for y in range(h));png=b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>2I5B',w,h,8,2,0,0,0))+chunk(b'IDAT',zlib.compress(raw))+chunk(b'IEND',b'')
   dest=O/f'{chapter}-{label}-{p.stem}.png';dest.write_bytes(png);
   from PIL import Image
   assert Image.open(dest).convert('RGB').tobytes()==pixels
   rows.append({'source':str(p.relative_to(T)),'source_sha256':sha(b),'pixels_sha256':sha(pixels),'preview':dest.name,'preview_sha256':sha(png),'width':w,'height':h,'transform':'Lossless RGB P6 to PNG; no scaling/crop/edit','focus':c.get('focus')})
(O/'receipt.json').write_text(json.dumps({'source':'ffcdb1a8ea7caaf9fbdaa4e077c8b04b620dd2b7','previews':rows,'boundary':'Format conversion only; visual inspection recorded separately'},indent=2)+'\n');print(len(rows),'lossless previews')
