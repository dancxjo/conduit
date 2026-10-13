import pathlib,re,hashlib,json
root=pathlib.Path('/home/dancxjo/conduit-4907-text-revisions/target/debug/build/conduit-language-1f6d70d68216eeb4/out')
header=root/'semantic_types.rs'; data=header.read_bytes(); text=data.decode()
names=['LanguageParserIndependentProtectedAdmission','LanguageParserStableDependencyAdmission','LanguageParserWindow8StableLexicalFact','LanguageParserWindow8Snapshot','LanguageParserWindow8CheckedHypothesis','LanguageParserJointStableFact','LanguageParserJointRuntimeHypothesis']
out=pathlib.Path('work/actual-law-snapshot');out.mkdir(exist_ok=True)
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
  bank['laws'].append({'path':str(dest.resolve()),'sha256':hashlib.sha256(raw).hexdigest(),'bytes':len(raw)})
 manifest['banks'].append(bank)
 assert len(laws)>0,name
assert header.read_bytes()==data
(out/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
(out/'laws.tsv').write_text(''.join(b['name']+'\t'+str(i)+'\t'+p['path']+'\n' for b in manifest['banks'] for i,p in enumerate(b['laws'])))
print(json.dumps({b['name']:len(b['laws']) for b in manifest['banks']}))
