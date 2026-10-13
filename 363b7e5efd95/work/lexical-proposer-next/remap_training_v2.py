"""Mechanical TRAIN+22teaching ABI remap; no new supervision or heldout input."""
from pathlib import Path
import struct,json,hashlib
root=Path(__file__).resolve().parent
source=root/'proposal-446-training-v1/samples.bin'
b=source.read_bytes(); assert b[:8]==b'C27TRAIN'; out=bytearray(b[:8]); cursor=8; samples=tuples=0

def remap():
 global cursor,tuples
 values=list(struct.unpack_from('<27H',b,cursor));cursor+=54;assert all(x<446 for x in values)
 pair=values[2]-36; assert 0<=pair<324
 top,nxt=divmod(pair,18); values[2]=36+min(top,16)*17+min(nxt,16)
 for i in range(3,27):
  assert values[i]>=360
  values[i]-=35
 assert all(0<=x<411 for x in values)
 out.extend(struct.pack('<27H',*values)); tuples+=1
while cursor<len(b):
 remap(); target,count=struct.unpack_from('<HH',b,cursor);cursor+=4
 assert target<76 and 1<=count<=16;out.extend(struct.pack('<HH',target,count))
 for _ in range(count):remap()
 samples+=1
assert cursor==len(b)
destination=root/'proposal-411-training-v1';destination.mkdir(exist_ok=False);(destination/'samples.bin').write_bytes(out)
manifest={'scope':'Mechanical ABI remap of exact frozen TRAIN+22reviewed teaching; no heldout inputs or new gold labels','source_samples_sha256':hashlib.sha256(b).hexdigest(),'samples_sha256':hashlib.sha256(out).hexdigest(),'samples':samples,'feature_tuples':tuples,'categories':411,'classes':76,'lookups':27,'policy':'Other16 and boundary17 coalesce only in17x17 pair interaction; exact18way unigrams/history/origins preserved','source_training_manifest':json.loads((root/'proposal-446-training-v1/manifest.json').read_text())}
(destination/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n');print(json.dumps({k:v for k,v in manifest.items() if k!='source_training_manifest'}))
