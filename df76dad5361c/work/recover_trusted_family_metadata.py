from pathlib import Path
import re,hashlib,json,shutil
trusted=Path('/home/dancxjo/conduit-4907-text-revisions/target/debug/build/conduit-language-66b63170b7b5d838/out')
raw=(trusted/'semantic_types.rs').read_bytes()
assert hashlib.sha256(raw).hexdigest()=='ee3a34933e5648bb012031d5097ea818c528e1bb5f2a6f2305382271eef816f1'
s=raw.decode(); names=Path('work/stablelex-family-rust-types.txt').read_text().splitlines()
out=Path('work/actual-family-snapshot');out.mkdir(exist_ok=True)
(out/'semantic_types.rs').write_bytes(raw)
manifest={'semantic_types.rs':hashlib.sha256(raw).hexdigest()}
def preserve(match):
    name=match.group(1);blob=(trusted/name).read_bytes();(out/name).write_bytes(blob)
    manifest[name]=hashlib.sha256(blob).hexdigest()
    return 'include_bytes!('+json.dumps(str((out/name).resolve()))+')'
def method(name,method):
    for match in re.finditer(r'impl '+re.escape(name)+r' \{',s):
        start=match.end();depth=1;i=start
        while depth:
            depth+=(s[i]=='{')-(s[i]=='}');i+=1
        block=s[start:i-1]
        m=re.search(r'fn '+method+r'\([^\n]*\{',block)
        if m:
            start=m.end();depth=1;j=start
            while depth:
                depth+=(block[j]=='{')-(block[j]=='}');j+=1
            return re.sub(r'include_bytes!\(concat!\(env!\("OUT_DIR"\), "/([^\"]+)"\)\)',preserve,block[start:j-1])
    raise Exception((name,method))
rs=['use conduit_plot::rust_binding::{generate_rust_bindings, RustBindingOptions, NativeBindingRefusal};','use conduit_plot::rust_binding::semantic_core as conduit_core;','use conduit_plot::CheckedNativeType;','fn main() { let mut types = Vec::new();']
for name in names:
    ident=re.search(r'pub const (\w+)_IDENTITY: &str = "(type/'+re.escape(name)+r'@[^\"]+)";',s)
    assert ident,name
    const=ident.group(1)
    blob=re.search(r'pub const '+const+r': &\[u8\] = (include_bytes!\(concat!\(env!\("OUT_DIR"\), "/[^\"]+"\)\));',s).group(1)
    blob=re.sub(r'include_bytes!\(concat!\(env!\("OUT_DIR"\), "/([^\"]+)"\)\)',preserve,blob)
    contracts=method(name,'value_contracts');laws=method(name,'invariants')
    rs.append('{ let value_contracts = {'+contracts+'}; let invariants: Result<Vec<conduit_plot::PortableExpressionProgram>, NativeBindingRefusal> = (|| {'+laws+'})(); types.push(CheckedNativeType { name: '+json.dumps(name)+'.into(), identity: conduit_core::kind_id('+json.dumps(ident.group(2))+'), value_type: conduit_core::StructuredInfoType::from_canonical_bytes('+blob+').unwrap(), value_contracts, invariants: invariants.unwrap() }); }')
rs += ['assert_eq!(types.len(),35); assert_eq!(types.iter().map(|ty|ty.invariants.len()).sum::<usize>(),118);','let options = RustBindingOptions { prepared_family_roots: ["LanguageParserWindow8StableLexicalFact".into()].into(), ..RustBindingOptions::default() };','let generated = generate_rust_bindings(&types, &options).unwrap(); std::fs::write("work/actual-family-generated.rs",generated.source).unwrap();','for ty in &types { eprintln!("{} {} {}",ty.name,ty.invariants.len(),ty.value_contracts.len()); }','}']
Path('work/recover_trusted_family_metadata.rs').write_text('\n'.join(rs))
(out/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
print(len(names),'Types;',len(manifest)-1,'immutable blobs;',sum((out/name).stat().st_size for name in manifest),'bytes')
