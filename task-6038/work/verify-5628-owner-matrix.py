from pathlib import Path
import hashlib,json,struct,zlib
T=Path('/home/dancxjo/Documents/Codex/2026-10-19/goal-6038')
result=[]
for n in (8,9,10):
 O=T/f'outputs/5628-owner-look-{n}';p=O/'receipt.json';r=json.loads(p.read_text());assert r['source']=='253dcb985eb8890ddf873d672d3a282f2366e6db'
 for c in r['captures']:
  b=(O/c['file']).read_bytes();assert hashlib.sha256(b).hexdigest()==c['png_sha256'];assert b[:8]==b'\x89PNG\r\n\x1a\n';i=8;blocks=[]
  while i<len(b):
   size=struct.unpack('>I',b[i:i+4])[0];tag=b[i+4:i+8];data=b[i+8:i+8+size];assert zlib.crc32(tag+data)&0xffffffff==struct.unpack('>I',b[i+8+size:i+12+size])[0];i+=12+size
   if tag==b'IHDR':
    w,h,depth,color,compression,filtering,interlace=struct.unpack('>IIBBBBB',data);assert (depth,color,compression,filtering,interlace)==(8,2,0,0,0)
   if tag==b'IDAT':blocks.append(data)
  assert i==len(b) and (w,h)==(c['width'],c['height']);raw=zlib.decompress(b''.join(blocks));assert len(raw)==h*(w*3+1);rows=[]
  for y in range(h):
   off=y*(w*3+1);assert raw[off]==0;rows.append(raw[off+1:off+1+w*3])
  assert hashlib.sha256(b''.join(rows)).hexdigest()==c['rgb_sha256']
 records=json.loads((O/'native-observations.json').read_text());real=[x['value'] for x in records if x['prefix']=='CONDUIT_NATIVE_COMPONENT_REALIZATION'];assert len(real)==3
 assert all(x['face']==real[0]['face'] and all(x['configuration'][k]==real[0]['configuration'][k] for k in ('component','recipe')) for x in real)
 shows=[x['acknowledged_show']['show_id'] for x in real];assert len(set(shows))==3
 for s in shows:assert any(x['prefix']=='CONDUIT_NATIVE_OWNER_FACE' and x['value'].get('status')=='acknowledged' and x['value'].get('show_id')==s for x in records)
 assert any(x['prefix']=='CONDUIT_NATIVE_COMPONENT_SELECTION' and x['value'].get('code')=='owner-look-provisional-input' for x in records)
 assert not any(x['value'].get('code')=='native-owner-refresh-unavailable' for x in records)
 bindings=json.loads((O/'independent-owner-bindings.json').read_text());assert len(bindings)==5
 for x in bindings:assert x['revision_binding']['face_id']==real[0]['face']['identity'] and x['revision_binding']['face_revision_decimal']==str(real[0]['face']['revision'])
 result.append(dict(attempt=n,matrix=r['matrix'],png_file_crc_and_decoded_rgb_hashes=len(r['captures']),equal_full_face_component_and_recipe=True,fresh_acknowledged_configured_shows=3,independent_owner_reads=5,refresh_budget=4))
(T/'outputs/5628-owner-matrix-verification.json').write_text(json.dumps({'source':r['source'],'journeys':result,'review':'visual review recorded in per-journey receipts; unsupported Ŵ glyph remains explicit fallback','stable':False,'physical':False,'human':False},indent=2)+'\n');print(json.dumps(result))
