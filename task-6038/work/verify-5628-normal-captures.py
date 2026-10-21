from pathlib import Path
import json,hashlib,struct,zlib
T=Path('/home/dancxjo/Documents/Codex/2026-10-19/goal-6038');P=T/'outputs/5628-normal-journey-253dcb985/x86_64'
def decode(p):
 b=p.read_bytes();assert b[:8]==b'\x89PNG\r\n\x1a\n' and len(b)<8*1024**2;i=8;chunks=[];size=None
 while i<len(b):
  n=struct.unpack('>I',b[i:i+4])[0];tag=b[i+4:i+8];v=b[i+8:i+8+n];crc=struct.unpack('>I',b[i+8+n:i+12+n])[0];assert zlib.crc32(tag+v)&0xffffffff==crc;i+=n+12
  if tag==b'IHDR':
   w,h,depth,color,compression,filtering,interlace=struct.unpack('>IIBBBBB',v);assert depth==8 and color==6 and compression==filtering==interlace==0;assert w*h<=4*1024**2;size=(w,h)
  if tag==b'IDAT':chunks.append(v)
  if tag==b'IEND':break
 assert i==len(b) and size
 stride=w*4;raw=zlib.decompress(b''.join(chunks));assert len(raw)==h*(stride+1);out=bytearray();prior=bytearray(stride)
 for y in range(h):
  off=y*(stride+1);kind=raw[off];row=bytearray(raw[off+1:off+1+stride]);assert kind<=4
  if kind:
   for x in range(stride):
    a=row[x-4] if x>=4 else 0;up=prior[x];c=prior[x-4] if x>=4 else 0
    if kind==1:prediction=a
    elif kind==2:prediction=up
    elif kind==3:prediction=(a+up)//2
    else:
     v=a+up-c;da=abs(v-a);db=abs(v-up);dc=abs(v-c);prediction=a if da<=db and da<=dc else up if db<=dc else c
    row[x]=(row[x]+prediction)&255
  out.extend(row);prior=row
 return size,bytes(out)
r=json.loads((P/'journey-proof.json').read_text());m=json.loads((P/'journey-frames/manifest.json').read_text());assert r['source_commit']=='253dcb985eb8890ddf873d672d3a282f2366e6db';assert r['image_sha256']=='02743a01e04a3d5f066fae7c1c7d4ecb8d5a14acf983a21a0aaa842f65dd51eb';assert m['status']=='complete' and m['failure'] is None and len(m['checkpoints'])==26
for cp in m['checkpoints']:
 f=cp['frame'];p=P/'journey-frames'/f['png'];assert p.stat().st_size==f['png_bytes'];assert hashlib.sha256(p.read_bytes()).hexdigest()==f['png_sha256'];size,pixels=decode(p);assert size==(f['width'],f['height']);assert hashlib.sha256(pixels).hexdigest()==f['pixel_sha256']
l=r['look_selection'];assert l['status']=='passed' and l['body_actions']==0 and len(l['transitions'])==4
record={'source':r['source_commit'],'iso':r['image_sha256'],'canonical_command_exit':0,'checkpoints':26,'independent_png_file_and_decoded_RGBA_hash_checks':26,'local_look_transitions':4,'body_actions_during_look_swaps':0,'owner_backed_typography_proof':False,'physical':False,'human':False,'reviewed_frame':'creche-editorial-spacious.png','remaining':'Owner-backed independent Title profile swaps, full workspace, combined/stable and native fallback/Patchbay remain separate'};(P/'root-verification.json').write_text(json.dumps(record,indent=2)+'\n');print(json.dumps(record))
