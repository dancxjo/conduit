from pathlib import Path
import re,hashlib,json
base=Path('/home/dancxjo/conduit-4907-evaluation-identity/target/debug/build/conduit-language-70b796a2da20ac23/out');out=Path('work/native-child-rank').resolve();s=(base/'semantic_types.rs').read_text()
def block(start):
 depth=1;i=start
 while depth:
  depth+=(s[i]=='{')-(s[i]=='}');i+=1
 return s[start:i-1]
descriptors={}
for m in re.finditer(r'pub static (\w+)_PREPARED_NATIVE_DESCRIPTOR:[^\n]+\{',s):descriptors[m[1]]=block(m.end())
names=set();queue=['LanguageParserWindow8RawBeam']
while queue:
 name=queue.pop()
 if name in names:continue
 names.add(name);body=descriptors[name];children=re.search(r'children: &\[([^\]]*)\]',body)[1]
 queue+=re.findall(r'&(\w+)_PREPARED_NATIVE_DESCRIPTOR',children)
manifest={'header':hashlib.sha256(s.encode()).hexdigest(),'names':sorted(names),'blobs':{}}
def preserve(m):
 name=m[1];b=(base/name).read_bytes();(out/name).write_bytes(b);manifest['blobs'][name]=hashlib.sha256(b).hexdigest();return 'include_bytes!('+json.dumps(str(out/name))+')'
def literals(body):return re.sub(r'include_bytes!\(concat!\(env!\("OUT_DIR"\), "/([^\"]+)"\)\)',preserve,body)
def method(name,method):
 for m in re.finditer(r'impl '+re.escape(name)+r' \{',s):
  b=block(m.end());mm=re.search(r'fn '+method+r'\([^\n]*\{',b)
  if mm:
   dep=1;i=mm.end()
   while dep:dep+=(b[i]=='{')-(b[i]=='}');i+=1
   return literals(b[mm.end():i-1])
 raise Exception((name,method))
rs=['use conduit_plot::rust_binding::{generate_rust_bindings,RustBindingOptions,NativeBindingRefusal};use conduit_plot::rust_binding::semantic_core as conduit_core;use conduit_plot::CheckedNativeType;fn main(){let mut types=Vec::new();']
for name in sorted(names):
 ident=re.search(r'pub const (\w+)_IDENTITY: &str = "(type/'+name+r'@[^\"]+)";',s);assert ident,name
 blob=re.search(r'pub const '+ident[1]+r': &\[u8\] = ([^;]+);',s)[1]
 laws_static=re.search(r'static '+re.escape(name)+r'_PREPARED_NATIVE_LAWS: &\[&\[u8\]\] = (.*?);',s,re.S);assert laws_static,name
 rs.append('static '+name+'_PREPARED_NATIVE_LAWS: &[&[u8]] = '+literals(laws_static[1])+';')
 rs.append('{let value_contracts={'+method(name,'value_contracts')+'};let invariants:Result<Vec<conduit_plot::PortableExpressionProgram>,NativeBindingRefusal>=(||{'+method(name,'invariants')+'})();types.push(CheckedNativeType{name:'+json.dumps(name)+'.into(),identity:conduit_core::kind_id('+json.dumps(ident[2])+'),value_type:conduit_core::StructuredInfoType::from_canonical_bytes('+literals(blob)+').unwrap(),value_contracts,invariants:invariants.unwrap()});}')
rs.append('let generated=generate_rust_bindings(&types,&RustBindingOptions{prepared_family_roots:["LanguageParserWindow8RawBeam".into()].into(),..Default::default()}).unwrap();std::fs::write("work/native-child-rank/bindings.rs",generated.source).unwrap();println!("types={} laws={}",types.len(),types.iter().map(|t|t.invariants.len()).sum::<usize>());}')
(out/'recover.rs').write_text('\n'.join(rs));(out/'original-metadata-manifest.json').write_text(json.dumps(manifest,indent=2));print(len(names),len(manifest['blobs']))
