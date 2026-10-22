import hashlib, json, struct, zlib
from pathlib import Path
base=Path(__file__).resolve().parent
p=base/'proof.json';report=json.loads(p.read_text())
for frame in report['captures']:
 encoded=(base/frame['png']).read_bytes()
 assert encoded[:8]==b'\x89PNG\r\n\x1a\n'
 assert len(encoded)==frame['bytes'] and hashlib.sha256(encoded).hexdigest()==frame['sha256']
 offset=8;parts=[]
 while offset<len(encoded):
  length=struct.unpack('>I',encoded[offset:offset+4])[0];kind=encoded[offset+4:offset+8];payload=encoded[offset+8:offset+8+length]
  crc=struct.unpack('>I',encoded[offset+8+length:offset+12+length])[0]
  assert zlib.crc32(kind+payload)==crc
  if kind==b'IHDR': width,height,depth,color,compression,filtering,interlace=struct.unpack('>IIBBBBB',payload)
  if kind==b'IDAT':parts.append(payload)
  offset+=12+length
 assert offset==len(encoded) and (width,height)==(frame['width'],frame['height'])
 assert depth==8 and color in (2,6) and compression==filtering==interlace==0
 channels=3 if color==2 else 4;stride=width*channels;raw=zlib.decompress(b''.join(parts))
 assert len(raw)==height*(stride+1)
 previous=bytearray(stride);rgba=bytearray();offset=0
 for _ in range(height):
  mode=raw[offset];row=bytearray(raw[offset+1:offset+stride+1]);offset+=stride+1
  for i in range(stride):
   left=row[i-channels] if i>=channels else 0;above=previous[i];corner=previous[i-channels] if i>=channels else 0
   if mode==0:prediction=0
   elif mode==1:prediction=left
   elif mode==2:prediction=above
   elif mode==3:prediction=(left+above)//2
   else:
    assert mode==4
    estimate=left+above-corner;distances=[abs(estimate-left),abs(estimate-above),abs(estimate-corner)]
    prediction=[left,above,corner][distances.index(min(distances))]
   row[i]=(row[i]+prediction)&255
  if channels==4:rgba.extend(row)
  else:
   for i in range(0,stride,3):rgba.extend(row[i:i+3]);rgba.append(255)
  previous=row
 assert frame['decoded_rgba_sha256']==hashlib.sha256(rgba).hexdigest()
report['capture_verification']={'status':'passed','captures':len(report['captures']),'checks':['PNG byte length and SHA256','PNG chunk CRC','dimensions','stdlib PNG decode','retained RGBA SHA256']}
print(json.dumps(report['capture_verification']))
