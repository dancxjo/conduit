from pathlib import Path
import re,json,hashlib
custody=Path('/home/dancxjo/conduit-4907-production-custody/semantics/language')
build=(custody/'build.rs').read_text(); block=build.split('    let source = [',1)[1].split('    ]',1)[0]
files=[custody/n for n in re.findall(r'include_str!\("([^"]+)"\)',block)]
types={}
for p in files:
 s=re.sub(r'#[^\n]*','',p.read_text())
 for m in re.finditer(r'^type\s+(\w+)\s*=\s*(.*?)(?=^(?:type|plot|form|value|import|use|information)\s|\Z)',s,re.M|re.S):types[m[1]]=set(re.findall(r'\b[A-Z]\w*\b',m[2]))-{m[1]}
known=set(types);types={n:refs&known for n,refs in types.items()}
def closure(roots):
 found=set();pending=list(roots)
 while pending:
  n=pending.pop()
  if n in found:continue
  found.add(n);pending.extend(types.get(n,set())-found)
 return found
production=re.findall(r'"(\w+)"',build.split('prepared_family_roots: [',1)[1].split(']',1)[0])
own=Path('semantics/language/build.rs').read_text();model=re.findall(r'"(\w+)"',own.split('prepared_family_roots: [',1)[1].split(']',1)[0])
p=closure(production);m=closure(model);report={'boundary':'Lexical authored declaration closure, not checked generation or runtime readiness','production_roots':production,'production_named_types':sorted(p),'production_count':len(p),'model_roots':model,'model_count':len(m),'shared_count':len(p&m),'combined_count':len(p|m),'combined_named_types':sorted(p|m),'maximum_individual_root':max((len(closure([n])),n) for n in production+model),'missing_authored_roots':sorted(set(production+model)-known),'source_hashes':{str(f):hashlib.sha256(f.read_bytes()).hexdigest() for f in files},'build_hash':hashlib.sha256(build.encode()).hexdigest()}
Path('work/production-window8-combined-current.json').write_text(json.dumps(report,indent=2)+'\n');print({k:v for k,v in report.items() if k.endswith('count') or k in ['maximum_individual_root','missing_authored_roots']})
