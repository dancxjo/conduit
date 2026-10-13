from pathlib import Path
import re,json,hashlib
custody=Path('/home/dancxjo/conduit-4907-production-custody/semantics/language')
build=(custody/'build.rs').read_text(); block=build.split('    let source = [',1)[1].split('    ]',1)[0]
files=[custody/n for n in re.findall(r'include_str!\("([^"]+)"\)',block)]
extra_files=[custody/(n+".conduit") for n in ["parser_window8_continuation","parser_window8_empty_grow","parser_window8_reanalysis","parser_window8_dependency_facts","parser_window8_lexical_ledger","parser_window8_lexical_protection"]]
baseline_files=files[:]
files+=extra_files
extra_roots=[]
for f in extra_files:
 extra_roots+=re.findall(r"^type\s+(\w+)\s*=",f.read_text(),re.M)
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
p=closure(production);m=closure(model);e=closure(extra_roots);report={'boundary':'Lexical authored declaration closure, not checked generation or runtime readiness','production_roots':production,'production_named_types':sorted(p),'production_count':len(p),'model_roots':model,'model_count':len(m),'shared_count':len(p&m),'combined_count':len(p|m),'combined_named_types':sorted(p|m),'maximum_individual_root':max((len(closure([n])),n) for n in production+model),'missing_authored_roots':sorted(set(production+model)-known),'source_hashes':{str(f):hashlib.sha256(f.read_bytes()).hexdigest() for f in files},'lifecycle_roots':extra_roots,'lifecycle_count':len(e),'lifecycle_additional_types':sorted(e-(p|m)),'all_union_count':len(e|p|m),'lifecycle_per_root_counts':{n:len(closure([n])) for n in extra_roots},'maximum_lifecycle_root':max((len(closure([n])),n) for n in extra_roots),'build_hash':hashlib.sha256(build.encode()).hexdigest()}
ports=[]
for file in extra_files:
 for match in re.finditer(r'^plot\s+([\w-]+)\s*\((.*?)\)',file.read_text(),re.M|re.S):
  fields=re.findall(r'(\w+)\s*:\s*(\w+)\.\.\.\|',match[2])
  ports.append({'file':str(file),'plot':match[1],'ports':fields})
maximal=[n for n in extra_roots if not any(n in closure([other]) for other in extra_roots if other!=n)]
port_roots={ty for port in ports for _,ty in port['ports']}
required=set(extra_roots)|port_roots
cover=[n for n in required if not any(n in closure([other]) for other in required if other!=n)]
report.update({'lifecycle_plot_ports':ports,'minimal_complete_roots':sorted(cover),'minimal_root_count':len(cover),'minimal_union_count':len(closure(cover)),'all_declarations_and_ports_covered':required<=closure(cover)})
Path('work/production-window8-lifecycle-text-inventory.json').write_text(json.dumps(report,indent=2)+'\n');print({k:v for k,v in report.items() if k.endswith('count') or k in ['maximum_individual_root','maximum_lifecycle_root','missing_authored_roots','lifecycle_additional_types']})
