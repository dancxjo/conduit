from pathlib import Path
from collections import Counter
import re, json, hashlib, sys
header=Path(sys.argv[1]); source=header.read_text(); names=json.loads(Path('work/bank-root-inventory.json').read_text())['schema_name_inventory']
resources=[]; laws={}
pattern=r'include_bytes!\(concat!\(env!\("OUT_DIR"\), "/([^\"]+)"\)\)'
for name in names:
    table=re.search(r'static '+re.escape(name)+r'_PREPARED_NATIVE_LAWS: &\[&\[u8\]\] = &\[(.*?)\n\];',source,re.S)
    assert table,name
    files=re.findall(pattern,table.group(1)); laws[name]=len(files); resources+=files
    ident=re.search(r'pub const (\w+)_IDENTITY: &str = "type/'+re.escape(name)+r'@',source)
    assert ident,name
    row=re.search(r'pub const '+ident.group(1)+r': &\[u8\] = (.*);',source)
    resources+=re.findall(pattern,row.group(1))
actual=Counter(hashlib.sha256((header.parent/file).read_bytes()).hexdigest() for file in resources)
trusted=json.loads(Path('work/bank-family-snapshot/manifest.json').read_text()); expected=Counter(v for k,v in trusted.items() if k!='semantic_types.rs')
assert actual==expected,(actual-expected,expected-actual)
assert len(names)==53 and sum(laws.values())==146 and len(resources)==199
receipt={'header':str(header.resolve()),'sha256':hashlib.sha256(header.read_bytes()).hexdigest(),'types':len(names),'ordered_laws':sum(laws.values()),'resources':len(resources),'trusted_resource_multiset_equal':True,'law_counts':laws}
print(json.dumps(receipt,indent=2))
