import pathlib,re,json,hashlib
b=pathlib.Path('work/lexical-proposer-next');production=pathlib.Path('/home/dancxjo/conduit-4907-production-custody');project=pathlib.Path('/home/dancxjo/Documents/Codex/2026-10-06/goal-please-finish-and-close-4907')
files=list((production/'semantics/language').glob('*.conduit'))+list((project/'work/production-custody').glob('parser_window8_*.conduit'))+[b/'proposal_window8_features.conduit',b/'qualified_analysis.conduit']
blocks={};hashes={}
for f in files:
 s=f.read_text();hashes[str(f)]=hashlib.sha256(f.read_bytes()).hexdigest()
 for m in re.finditer(r'^type\s+(\w+)\s*=([\s\S]*?)(?=^(?:type|plot)\s|\Z)',s,re.M):blocks[m[1]]=m[2]
deps={n:set(re.findall(r'\b[A-Z][A-Za-z0-9_]*\b',s))&blocks.keys()for n,s in blocks.items()}
def closure(roots):
 seen=set();todo=list(roots)
 while todo:
  n=todo.pop()
  if n in seen or n not in blocks:continue
  seen.add(n);todo.extend(deps[n]-seen)
 return sorted(seen)
roots=set()
for f in [production/'semantics/language/build.rs',pathlib.Path('semantics/language/build.rs')]:
 s=f.read_text().split('prepared_family_roots:')[1].split('..RustBindingOptions')[0]
 roots.update(re.findall(r'"(Language\w+)"',s))
new=[n for n in blocks if n.startswith('LanguageParserProposalWindow8')or 'QualifiedLexical' in n]
custody=[]
for f in (project/'work/production-custody').glob('parser_window8_*.conduit'):custody.extend(re.findall(r'^type\s+(\w+)\s*=',f.read_text(),re.M))
roots.update(custody);roots.update(new)
r={'scope':'Text declaration dependency estimate only; all matching work custody drafts included conservatively, not checked generation/readiness/resources','generation_named_union_count':len(closure(roots)),'maximum_single_root_closure':max(map(lambda n:len(closure([n])),roots)),'new_types':new,'new_type_closures':{n:len(closure([n]))for n in new},'custody_types':sorted(set(custody)),'generation_roots':sorted(roots),'named_union':closure(roots),'inputs':hashes}
(b/'final-proposal-custody-text-inventory.json').write_text(json.dumps(r,indent=2)+'\n');print(json.dumps({k:v for k,v in r.items()if k not in ['generation_roots','named_union','inputs']}))
