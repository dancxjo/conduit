import pathlib,re,json,hashlib
p=pathlib.Path('/home/dancxjo/conduit-4907-production-custody/semantics/language')
files=sorted(p.glob('*.conduit'));blocks={};owner={};ports=[]
selected=[f for f in files if f.name.startswith('parser_window8')]
for f in files:
 s=f.read_text()
 for m in re.finditer(r'^type\s+(\w+)\s*=([\s\S]*?)(?=^(?:type|plot)\s|\Z)',s,re.M):blocks[m[1]]=m[2];owner[m[1]]=f.name
for f in selected:
 s=f.read_text()
 for m in re.finditer(r'^plot\s+([\w-]+)\s*\(([\s\S]*?)\)\s*=',s,re.M):
  io=re.findall(r':\s*([A-Za-z][A-Za-z0-9_]*)\s*\.\.\.\|',m[2])
  ports.append({'name':m[1],'file':f.name,'io':io,'signature':m[2].strip()})
deps={n:set(re.findall(r'\b[A-Z][A-Za-z0-9_]*\b',b))&blocks.keys()for n,b in blocks.items()}
def closure(roots):
 seen=set();todo=list(roots)
 while todo:
  n=todo.pop()
  if n in seen or n not in blocks:continue
  seen.add(n);todo.extend(deps[n]-seen)
 return seen
def minimal(roots):
 roots=set(roots)
 return sorted(n for n in roots if not any(n!=o and n in closure([o]) for o in roots))
# All declaration roots include final guarded admissions even when Source emits law-free proposals.
roots=minimal([n for n in blocks if owner[n] in {f.name for f in selected}]+[n for x in ports for n in x['io'] if n in blocks])
groups=[]
for n in sorted(roots,key=lambda n:(-len(closure([n])),n)):
 c=closure([n]);options=[(len(c-g['closure']),i)for i,g in enumerate(groups)if len(g['roots'])<16 and len(c|g['closure'])<=64]
 if options:
  _,i=min(options);groups[i]['roots'].append(n);groups[i]['closure']|=c
 else:groups.append({'roots':[n],'closure':c})
for g in groups:g['root_count']=len(g['roots']);g['type_count']=len(g['closure']);g['closure']=sorted(g['closure'])
r={'scope':'Read-only lexical named-declaration dependency estimate. All canonical parser_window8 files included, including old and successor feature versions. Greedy finite partition, NOT proven minimum/checker/readiness/storage acceptance. All declaration roots cover final guarded admissions; raw numerical aliases require separate bounded primitive capability.','ports':ports,'files':{f.name:hashlib.sha256(f.read_bytes()).hexdigest()for f in selected},'minimal_maximal_roots':roots,'named_union_count':len(closure(roots)),'maximum_single_root_closure':max(map(lambda n:len(closure([n])),roots)),'unresolved_io':sorted({n for x in ports for n in x['io'] if n not in blocks}),'groups':groups}
out=pathlib.Path('work/lexical-proposer-next/complete-window8-port-text-inventory.json');out.write_text(json.dumps(r,indent=2)+'\n');print(json.dumps({k:v for k,v in r.items()if k not in ['ports','files','groups','minimal_maximal_roots']}));print('ports',len(ports),'roots',len(roots),'groups',[(g['root_count'],g['type_count'])for g in groups])
