import pathlib,re,json,hashlib
project=pathlib.Path('/home/dancxjo/Documents/Codex/2026-10-06/goal-please-finish-and-close-4907')
p=project/'work/production-custody/window8-complete-custody-draft-source.conduit';text=p.read_text()
blocks={m.group(1):m.group(2)for m in re.finditer(r'^type\s+(\w+)\s*=([\s\S]*?)(?=^(?:type|plot)\s|\Z)',text,re.M)}
deps={n:set(re.findall(r'\b[A-Z][A-Za-z0-9_]*\b',b))&blocks.keys() for n,b in blocks.items()}
def closure(roots):
 seen=set();todo=list(roots)
 while todo:
  n=todo.pop()
  if n in seen or n not in blocks:continue
  seen.add(n);todo.extend(deps[n]-seen)
 return sorted(seen)
files=['parser_window8_dependency_custody','parser_window8_independent_commit','parser_window8_dependency_forest','parser_window8_numeric_custody'];result=[];roots=[]
for name in files:
 q=project/'work/production-custody'/f'{name}.conduit';s=q.read_text();types=re.findall(r'^type\s+(\w+)\s*=',s,re.M);ports=[]
 for m in re.finditer(r'^plot\s+([\w-]+)\s*\(\s*input:\s*(\w+)\.\.\.\|\s*>>\s*output:\s*(\w+)\.\.\.\|',s,re.M):
  entry={'name':m[1],'input':m[2],'output':m[3]};ports.append(entry);roots.extend([m[2],m[3]])
 result.append({'path':str(q),'sha256':hashlib.sha256(q.read_bytes()).hexdigest(),'types':types,'ports':ports,'declaration_closure_count':len(closure(types)),'port_closure_count':len(closure([v for x in ports for v in [x['input'],x['output']]]))})
roots=sorted(set(roots));r={'scope':'Read-only lexical declaration references only; not checked descriptor/resource readiness. Imported numerical aliases excluded from named declared closure. No Source mutations.','snapshot_sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'declared_types_in_snapshot':len(blocks),'files':result,'port_roots':roots,'port_root_count':len(roots),'per_root_declared_closure':{n:len(closure([n]))for n in roots},'combined_port_named_union':closure(roots),'combined_port_named_union_count':len(closure(roots)),'unresolved_port_aliases':[n for n in roots if n not in blocks]}
pathlib.Path('work/lexical-proposer-next/window8-added-custody-text-inventory.json').write_text(json.dumps(r,indent=2)+'\n');print(json.dumps({k:v for k,v in r.items()if k not in ['files','combined_port_named_union','scope']}));print([(x['path'].split('/')[-1],len(x['types']),len(x['ports']),x['port_closure_count'])for x in result])
