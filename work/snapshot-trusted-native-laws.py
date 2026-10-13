import pathlib,re,hashlib,json
root=pathlib.Path('/home/dancxjo/conduit-4907-text-revisions/target/debug/build/conduit-language-66b63170b7b5d838/out')
header=root/'semantic_types.rs'; data=header.read_bytes(); text=data.decode()
names=['LanguageParserIndependentProtectedAdmission','LanguageParserStableDependencyAdmission','LanguageParserWindow8StableLexicalFact','LanguageParserWindow8Snapshot','LanguageParserWindow8CheckedHypothesis','LanguageParserJointStableFact','LanguageParserJointRuntimeHypothesis']
assert hashlib.sha256(data).hexdigest()=='ee3a34933e5648bb012031d5097ea818c528e1bb5f2a6f2305382271eef816f1'
out=pathlib.Path('work/trusted-law-snapshot');out.mkdir(exist_ok=True)
manifest={'source_header':str(header),'header_sha256':hashlib.sha256(data).hexdigest(),'banks':[]}
for name in names:
 pieces=re.findall(r'^impl '+name+r' \{(.*?)(?=^impl |^pub struct|^pub enum|\Z)',text,re.M|re.S)
 laws=[]
 for piece in pieces:
  if 'fn invariants()' not in piece:continue
  part=piece.split('fn invariants()',1)[1].split('])',1)[0]
  laws+=re.findall(r'PortableExpressionProgram::from_canonical_bytes\(include_bytes!\(concat!\(env!\("OUT_DIR"\), "/(native_binding_bytes_\d+.bin)"',part)
 bank={'name':name,'laws':[]}
 for index,f in enumerate(laws):
  raw=(root/f).read_bytes(); dest=out/(name+'-'+str(index)+'.bin');dest.write_bytes(raw)
  bank['laws'].append({'source_path':str((root/f).resolve()),'path':str(dest.resolve()),'sha256':hashlib.sha256(raw).hexdigest(),'bytes':len(raw)})
 manifest['banks'].append(bank)
 assert len(laws)>0,name
assert header.read_bytes()==data
for b in manifest['banks']:
 for p in b['laws']:
  assert hashlib.sha256(pathlib.Path(p['source_path']).read_bytes()).hexdigest()==p['sha256']
(out/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
(out/'laws.tsv').write_text(''.join(b['name']+'\t'+str(i)+'\t'+p['path']+'\n' for b in manifest['banks'] for i,p in enumerate(b['laws'])))
print(json.dumps({b['name']:len(b['laws']) for b in manifest['banks']}))
