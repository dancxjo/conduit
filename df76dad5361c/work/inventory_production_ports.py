from pathlib import Path
import re,json,hashlib
root=Path('/home/dancxjo/conduit-4907-production-custody'); lang=root/'semantics/language'; build=(lang/'build.rs').read_text()
# Include the actual concatenated Source list, with all remaining semantic files
# only to resolve named children. This remains lexical, not checked readiness.
source_block=build.split('    let source = [',1)[1].split('    ]',1)[0]
files=[lang/name for name in re.findall(r'include_str!\("([^"]+)"\)',source_block)]
types={};ports={};sources={}
for file in files:
 s=file.read_text()
 for m in re.finditer(r'^type\s+(\w+)\s*=\s*(.*?)(?=^(?:type|plot|form|value|import|use|information)\s|\Z)',s,re.M|re.S):
  types[m.group(1)]=set(re.findall(r'\b[A-Z]\w*\b',m.group(2)))-{m.group(1)}
 for m in re.finditer(r'^plot\s+(language-parser-[\w-]+)\s*\(([^)]*)\)\s*(?:=|\{)',s,re.M|re.S):
  ns=re.findall(r':\s*((?:Language|Linguistic)\w+)\s*\.\.\.\|',m.group(2))
  if ns:ports[m.group(1)]=ns;sources[m.group(1)]=str(file.relative_to(root))
known=set(types)
types={k:v & known for k,v in types.items()}
def closure(roots):
 found=set();pending=list(roots)
 while pending:
  n=pending.pop()
  if n in found:continue
  found.add(n);pending.extend(types.get(n,set())-found)
 return sorted(found)
entries=list(dict.fromkeys(re.findall(r'"(language-parser-[\w-]+)"',build)+['language-parser-joint-branch','language-parser-decode-complete']))
report={'boundary':'Read-only lexical declaration inventory, not checked descriptor readiness. Type aliases/nesting and production port paths require checked followup.','head_inputs':{},'entries':{},'missing_entries':[]}
for entry in entries:
 if entry not in ports:report['missing_entries'].append(entry);continue
 ns=ports[entry];report['entries'][entry]={'io_roots':ns,'named_union_count':len(closure(ns)),'named_union':closure(ns),'source':sources[entry]}
roots=sorted({n for e in report['entries'].values() for n in e['io_roots']})
report['all_selected_entry_roots']=roots;report['all_selected_entry_union_count']=len(closure(roots));report['all_selected_entry_named_union']=closure(roots)
for p in [lang/'build.rs',lang/'tests/common/parser_joint_v2_runtime.rs',lang/'tests/common/parser_retained_v2.rs',lang/'tests/common/parser_session_protection.rs']:
 report['head_inputs'][str(p.relative_to(root))]=hashlib.sha256(p.read_bytes()).hexdigest()
Path('work/production-port-root-inventory.json').write_text(json.dumps(report,indent=2)+'\n')
for name,e in report['entries'].items(): print(name,'->',','.join(e['io_roots']),';',e['named_union_count'])
print('ALL',len(roots),'roots;',report['all_selected_entry_union_count'],'named union; missing',report['missing_entries'])
groups={
'derivation':['language-parser-session-seed','language-parser-joint-branch','language-parser-v2-pos','language-parser-v2-model-features','language-parser-legal-mask','language-parser-score-proposal','language-parser-transition','language-parser-joint-expansion','language-parser-joint-runtime-merge','language-parser-decode-complete'],
'retained':['language-parser-availability','language-parser-revision-reset','language-parser-joint-consensus','language-parser-wait-state','language-parser-joint-score-band-1000','language-parser-joint-stable-fact','language-parser-joint-commit','language-parser-joint-rebase'],
'protected':['language-parser-protected-set-initialize','language-parser-protected-origin-edge','language-parser-protected-set-insert','language-parser-protected-set-rebase','language-parser-protection-forest-projection','language-parser-independent-mask','language-parser-independent-branch','language-parser-independent-commit','language-parser-independent-commit-rebase','language-parser-independent-commit-rebase-sets','language-parser-retained-commit-anchor']}
extra={
'derivation':['LanguageParserJointRuntimeBeam'],
'retained':['LanguageParserJointRuntimeBeam','LanguageParserJointStableFact','LanguageParserStableDependencyAdmission','LanguageParserRetainedCommitReceipt','LanguageParserRetainedFactReceipt','LanguageParserRetainedSnapshotReceipt'],
'protected':['LanguageParserIndependentProtectedAdmission','LanguageParserProtectedRebaseReceipt','LanguageParserProtectedHypothesisCompatibility']}
groups['protected_execution']=groups['protected'][:7]
groups['protected_commit']=groups['protected'][7:]
extra['protected_execution']=extra['protected']
extra['protected_commit']=['LanguageParserRetainedCommitReceipt','LanguageParserIndependentProtectedAdmission']
groups['protected_origins_rebase']=groups['protected'][:4]
groups['protected_branch_mask']=groups['protected'][4:7]
extra['protected_origins_rebase']=['LanguageParserIndependentProtectedAdmission','LanguageParserProtectedRebaseReceipt']
extra['protected_branch_mask']=['LanguageParserProtectedHypothesisCompatibility']
report['groups']={}
for name,entries in groups.items():
 rs=set(n for entry in entries for n in report['entries'][entry]['io_roots'])|set(extra[name])
 missing=sorted(rs-known)
 minimal=sorted(n for n in rs if not any(n in set(closure([other])) for other in rs-{n}))
 report['groups'][name]={'entries':entries,'extra_native_admission_roots':extra[name],'roots':sorted(rs),'minimal_roots':minimal,'minimal_root_count':len(minimal),'named_union_count':len(closure(rs)),'named_union':closure(rs),'missing_named_declarations':missing}
 print('GROUP',name,len(minimal),'minimalRoots;',len(closure(rs)),'namedTypes;',missing)
Path('work/production-port-root-inventory.json').write_text(json.dumps(report,indent=2)+'\n')
report['runtime_native_wrappers']={}
wrappers={
'derivation':['parser_joint_v2_runtime.rs','parser_joint_v2_fixture.rs','parser_joint_flows.rs'],
'retained':['parser_retained_v2.rs'],
'protected':['parser_session_protection.rs']}
for group,filenames in wrappers.items():
 rs=set(report['groups'][group]['roots'])
 refs=set()
 for file in filenames:
  s=(lang/'tests/common'/file).read_text();refs |= set(re.findall(r'\b(?:Language|Linguistic)\w+\b',s)) & known
 rs |= refs
 minimal=sorted(n for n in rs if not any(n in set(closure([other])) for other in rs-{n}))
 report['runtime_native_wrappers'][group]={'files':filenames,'additional_referenced_native_types':sorted(refs-set(report['groups'][group]['named_union'])),'minimal_roots':minimal,'minimal_root_count':len(minimal),'named_union_count':len(closure(rs)),'named_union':closure(rs)}
 print('WITH RUNTIME NATIVE WRAPPERS',group,len(minimal),'minimalRoots;',len(closure(rs)),'namedTypes; additional',report['runtime_native_wrappers'][group]['additional_referenced_native_types'])
Path('work/production-port-root-inventory.json').write_text(json.dumps(report,indent=2)+'\n')
partitions={
'retained_lifecycle':(['language-parser-availability','language-parser-revision-reset','language-parser-wait-state','language-parser-joint-rebase'],['LanguageParserAvailability','LanguageParserWaitState']),
'retained_stabilize_commit':(['language-parser-joint-consensus','language-parser-joint-score-band-1000','language-parser-joint-stable-fact','language-parser-joint-commit'],['LanguageParserJointRuntimeBeam','LanguageParserJointRuntimeRawBeam','LanguageParserJointStableFact','LanguageParserStableDependencyAdmission','LanguageParserRetainedCommitReceipt','LanguageParserRetainedFactReceipt','LanguageParserRetainedSnapshotReceipt'])}
for name,(entries,extra_roots) in partitions.items():
 rs=set(n for entry in entries for n in report['entries'][entry]['io_roots'])|set(extra_roots)
 minimal=sorted(n for n in rs if not any(n in set(closure([other])) for other in rs-{n}))
 report['groups'][name]={'entries':entries,'extra_native_admission_roots':extra_roots,'roots':sorted(rs),'minimal_roots':minimal,'minimal_root_count':len(minimal),'named_union_count':len(closure(rs)),'named_union':closure(rs),'missing_named_declarations':sorted(rs-known)}
 print('PARTITION',name,len(minimal),'minimalRoots;',len(closure(rs)),'namedTypes')
report['source_files_sha256']={str(file.relative_to(root)):hashlib.sha256(file.read_bytes()).hexdigest() for file in files}
report['source_concatenated_sha256']=hashlib.sha256('\n'.join(file.read_text() for file in files).encode()).hexdigest()
Path('work/production-port-root-inventory.json').write_text(json.dumps(report,indent=2)+'\n')
