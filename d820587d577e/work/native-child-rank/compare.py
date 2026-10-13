import re,json,hashlib
from pathlib import Path
base=Path('/home/dancxjo/conduit-4907-evaluation-identity/target/debug/build/conduit-language-70b796a2da20ac23/out'); old=(base/'semantic_types.rs').read_text();new=Path('work/native-child-rank/bindings.rs').read_text();m=json.loads(Path('work/native-child-rank/original-metadata-manifest.json').read_text())
for name in m['names']:
 const=name.upper()+'_SEMANTIC_TYPE'
 ob=re.search(r'pub const '+const+r': &\[u8\] = include_bytes!\(concat!\(env!\("OUT_DIR"\), "/([^\"]+)"\)\);',old)[1]
 nb=re.search(r'pub const '+const+r': &\[u8\] = &\[([^\]]*)\]',new)[1]
 assert (base/ob).read_bytes()==bytes(int(x.strip(),16)for x in nb.split(',')if x.strip()),name
 def descriptor(s):
  b=re.search(r'pub static '+name+r'_PREPARED_NATIVE_DESCRIPTOR:[^\n]+\{(.*?)\n\};',s,re.S)[1]
  b=re.sub(r'external_edges: &\[\],','',b);return re.sub(r'\s+','',b)
 assert descriptor(old)==descriptor(new),name
print('PASS exact 10 original full Types, ordered law references, contract recipes, child edges and layouts')
row=json.loads(Path('work/corrected411-runtime/initial-vocative-completed-row.json').read_text())
for e in row['epochs']:Path('work/native-child-rank/epoch-'+str(e['epoch'])+'.bin').write_bytes(bytes.fromhex(e['beam_canonical']))
