from pathlib import Path
import re,json,hashlib
header=Path('target/debug/build/conduit-language-a41c865d7c39adcc/out/semantic_types.rs')
s=header.read_text();assert hashlib.sha256(header.read_bytes()).hexdigest()=='5b89415fb684d09776ca72b5ac215586a06f1d35eaa3824333c75b8434bac202'
inventory=json.loads(Path('work/bank-root-inventory.json').read_text());names=inventory['schema_name_inventory'];roots=inventory['roots'];out=Path('work/bank-family-snapshot');out.mkdir(exist_ok=True)
manifest={'semantic_types.rs':hashlib.sha256(header.read_bytes()).hexdigest()};(out/'semantic_types.rs').write_bytes(header.read_bytes())
pattern=r'include_bytes!\(concat!\(env!\("OUT_DIR"\), "/([^\"]+)"\)\)'
def preserve(m):
 name=m.group(1);data=(header.parent/name).read_bytes();(out/name).write_bytes(data);manifest[name]=hashlib.sha256(data).hexdigest()
 return 'include_bytes!('+json.dumps(str((out/name).resolve()))+')'
def method(name,which):
 for m in re.finditer(r'impl '+re.escape(name)+r' \{',s):
  start=m.end();depth=1;i=start
  while depth:depth+=(s[i]=='{')-(s[i]=='}');i+=1
  block=s[start:i-1];m=re.search(r'fn '+which+r'\([^\n]*\{',block)
  if m:
   start=m.end();depth=1;j=start
   while depth:depth+=(block[j]=='{')-(block[j]=='}');j+=1
   return block[start:j-1]
 raise RuntimeError((name,which))
rs=['use conduit_plot::rust_binding::{generate_rust_bindings,RustBindingOptions,NativeBindingRefusal};','use conduit_plot::rust_binding::semantic_core as conduit_core;','use conduit_plot::CheckedNativeType;','fn main(){let mut types=Vec::new();'];lawcounts={}
for name in names:
 ident=re.search(r'pub const (\w+)_IDENTITY: &str = "(type/'+re.escape(name)+r'@[^\"]+)";',s);assert ident,name
 blob=re.search(r'pub const '+ident.group(1)+r': &\[u8\] = (include_bytes!\(concat!\(env!\("OUT_DIR"\), "/[^\"]+"\)\));',s).group(1);blob=re.sub(pattern,preserve,blob)
 table=re.search(r'static '+re.escape(name)+r'_PREPARED_NATIVE_LAWS: &\[&\[u8\]\] = &\[(.*?)\n\];',s,re.S)
 lawfiles=re.findall(pattern,table.group(1) if table else method(name,'invariants'));lawcounts[name]=len(lawfiles)
 programs=[]
 for file in lawfiles:
  encoded=preserve(re.match('(.*)',file));programs.append('conduit_plot::PortableExpressionProgram::from_canonical_bytes('+encoded+').unwrap()')
 contracts=method(name,'value_contracts')
 rs.append('{let value_contracts={'+contracts+'};types.push(CheckedNativeType{name:'+json.dumps(name)+'.into(),identity:conduit_core::kind_id('+json.dumps(ident.group(2))+'),value_type:conduit_core::StructuredInfoType::from_canonical_bytes('+blob+').unwrap(),value_contracts,invariants:vec!['+','.join(programs)+']});}')
rs+=['assert_eq!(types.len(),53);assert_eq!(types.iter().map(|t|t.invariants.len()).sum::<usize>(),146);','let options=RustBindingOptions{prepared_family_roots:['+','.join(json.dumps(n)+'.into()' for n in roots)+'].into(),..RustBindingOptions::default()};','let generated=generate_rust_bindings(&types,&options).unwrap();std::fs::write("work/bank-family-generated.rs",generated.source).unwrap();','for ty in types{eprintln!("{} {} {}",ty.name,ty.invariants.len(),ty.value_contracts.len());}','}']
Path('work/recover_bank_family_metadata.rs').write_text('\n'.join(rs));(out/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n');(out/'law-counts.json').write_text(json.dumps(lawcounts,indent=2)+'\n')
print(len(names),'Types',sum(lawcounts.values()),'laws',len(manifest)-1,'resources')
