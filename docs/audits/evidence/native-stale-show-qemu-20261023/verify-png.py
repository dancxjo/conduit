import sys,struct,zlib,hashlib,json
from pathlib import Path
p=Path(sys.argv[1]);data=p.read_bytes();assert data[:8]==b'\x89PNG\r\n\x1a\n'
o=8;compressed=b'';ended=False
while o<len(data):
 n=struct.unpack('>I',data[o:o+4])[0];kind=data[o+4:o+8];chunk=data[o+8:o+8+n];crc=struct.unpack('>I',data[o+8+n:o+12+n])[0]
 assert zlib.crc32(kind+chunk)&0xffffffff==crc
 if kind==b'IHDR': w,h,depth,color,comp,filt,interlace=struct.unpack('>IIBBBBB',chunk);assert depth==8 and color in [2,6] and comp==filt==interlace==0
 if kind==b'IDAT':compressed+=chunk
 o+=12+n
 if kind==b'IEND':ended=True;break
assert ended and o==len(data)
bpp=4 if color==6 else 3;stride=w*bpp;raw=zlib.decompress(compressed);assert len(raw)==h*(stride+1)
prev=bytearray(stride);rgba=bytearray()
def paeth(a,b,c):
 p=a+b-c; ds=[abs(p-a),abs(p-b),abs(p-c)];return [a,b,c][ds.index(min(ds))]
for y in range(h):
 start=y*(stride+1);kind=raw[start];row=bytearray(raw[start+1:start+1+stride]);assert kind<=4
 for x in range(stride):
  a=row[x-bpp] if x>=bpp else 0;b=prev[x];c=prev[x-bpp] if x>=bpp else 0
  row[x]=(row[x]+[0,a,b,(a+b)//2,paeth(a,b,c)][kind])&255
 if color==6:rgba.extend(row)
 else:
  for x in range(0,stride,3):rgba.extend(row[x:x+3]);rgba.append(255)
 prev=row
receipt=json.loads((p.parent/'compositor-semantic-copy-recovery-fixture-proof.json').read_text())['stale_show_capture']['capture']
assert receipt['png_bytes']==len(data) and receipt['width']==w and receipt['height']==h
assert receipt['png_sha256']==hashlib.sha256(data).hexdigest()
assert receipt['pixel_sha256']==hashlib.sha256(rgba).hexdigest()
print(json.dumps({'png':p.name,'bytes':len(data),'width':w,'height':h,'png_sha256':hashlib.sha256(data).hexdigest(),'pixel_sha256':hashlib.sha256(rgba).hexdigest(),'checks':'independent stdlib PNG signature, complete chunks, CRCs, zlib extent, all filters, RGBA hash and retained receipt match'},indent=2))
