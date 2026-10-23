# Internal documentary verifier; product proof enters through cargo xtask.
# Reuses the existing retained stdlib PNG decoder, without new dependencies.
import json,hashlib,struct,zlib,gzip
from pathlib import Path
import sys
root=Path(__file__).resolve().parents[4]
archive=Path(__file__).resolve().parent
def images(value):
 if isinstance(value,dict):
  if all(k in value for k in ['png','png_sha256','pixel_sha256','width','height']):yield value
  else:
   for v in value.values():yield from images(v)
 elif isinstance(value,list):
  for v in value:yield from images(v)
def verify(directory):
 reports=[]
 for manifest in directory.rglob('manifest.json*'):
  records=list(images(json.loads(gzip.decompress(manifest.read_bytes()) if manifest.suffix=='.gz' else manifest.read_bytes())))
  for row in records:
   p=manifest.parent/row['png'];assert p.resolve().is_relative_to(directory.resolve())
   raw=p.read_bytes();assert hashlib.sha256(raw).hexdigest()==row['png_sha256'],p
   assert len(raw)==row['png_bytes'],p
   assert raw[:8]==b'\x89PNG\r\n\x1a\n';i=8;chunks=0
   while i<len(raw):
    n=struct.unpack('>I',raw[i:i+4])[0];tag=raw[i+4:i+8];data=raw[i+8:i+8+n];crc=struct.unpack('>I',raw[i+8+n:i+12+n])[0]
    assert zlib.crc32(tag+data)&0xffffffff==crc,p
    i+=12+n;chunks+=1
    if tag==b'IEND':break
   assert i==len(raw),p
   decoder=(root/'docs/audits/evidence/native-stale-show-qemu-20261023/verify-png.py').read_text().split('receipt=json.loads',1)[0]
   previous_args=sys.argv;sys.argv=['verify-png',str(p)]
   try:
    state={};exec(compile(decoder,'existing-stdlib-png-decoder','exec'),state)
   finally:sys.argv=previous_args
   assert (state['w'],state['h'])==(row['width'],row['height']),p
   assert hashlib.sha256(state['rgba']).hexdigest()==row['pixel_sha256'],p
   reports.append({'path':str(p.relative_to(archive)),'png_sha256':row['png_sha256'],'pixel_sha256':row['pixel_sha256'],'width':row['width'],'height':row['height'],'crc_chunks':chunks})
 return reports

# Only these curated current-run directories are part of this verification.
counts={}
for case in ['narrow','wide','journey','search']:
 records=verify(archive/f'802-{case}');counts[case]=len(records)
assert counts=={'narrow':17,'wide':15,'journey':54,'search':11},counts
print(json.dumps({'source_commit':'80260a13fbc7f611f8f7dda7804d195adfa6b2ec','counts':counts,'checks':'original PNG bytes, CRCs, dimensions and independently decoded RGBA hashes'},indent=2))
