"""Verify compact documentary bytes; omitted executable payloads are not verified."""
import gzip,hashlib,json,pathlib
p=pathlib.Path(__file__).resolve().parent
manifest=json.loads((p/'original-custody.json').read_text())
for row in manifest['files']:
 f=(p/row['stored']).resolve()
 assert f.is_relative_to(p)
 data=f.read_bytes()
 assert len(data)==row['stored_bytes'] and hashlib.sha256(data).hexdigest()==row['stored_sha256']
 original=gzip.decompress(data) if row['encoding']=='gzip' else data
 assert len(original)==row['original_bytes'] and hashlib.sha256(original).hexdigest()==row['original_sha256']
for line in (p/'SHA256SUMS').read_text().splitlines():
 digest,name=line.split('  ',1);f=(p/name).resolve();assert f.is_relative_to(p)
 assert hashlib.sha256(f.read_bytes()).hexdigest()==digest,name
print(json.dumps({'status':'verified','documentary_files':len(manifest['files']),
 'omitted_payloads_not_verified':len(manifest['omitted']),
 'scope':'Stored bytes/lossless gzip only; not execution or omitted-image verification'},indent=2))
