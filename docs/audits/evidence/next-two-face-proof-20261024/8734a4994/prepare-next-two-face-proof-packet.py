"""Prepare a local immutable-candidate packet only. No network or Git ref writes."""
import pathlib,json,subprocess,tomllib,posixpath,re,hashlib,gzip
ROOT=pathlib.Path.cwd();R=ROOT/'work/6267-review';S='8734a4994af36d8b4c28d3b2bd39cc197d65e1ae';T='d63189bd7311480c447bbc8128fbf09098577fd2';D=ROOT/'work/next-two-face-proof-packet'
def git(*a):return subprocess.check_output(['git','-C',str(R),*a],text=True).strip()
def raw(ref,p):return subprocess.check_output(['git','-C',str(R),'show',ref+':'+p])
def sha(b):return hashlib.sha256(b).hexdigest()
def closure():
 roots={'bodies/environment/runtime','bodies/environment/face','tools/xtask'};todo=list(roots);seen=set()
 while todo:
  p=todo.pop()
  if p in seen:continue
  seen.add(p);m=tomllib.loads(raw(S,p+'/Cargo.toml').decode())
  def walk(d):
   for k,v in d.items():
    if k in ('dependencies','build-dependencies')or k=='dev-dependencies'and p in roots:
     for dep in v.values():
      if isinstance(dep,dict)and'path'in dep:todo.append(posixpath.normpath(p+'/'+dep['path']))
    elif isinstance(v,dict):walk(v)
  walk(m)
 return sorted(seen)
def main():
 r=json.loads((ROOT/'outputs/next-two-face-proof-local-proof.json').read_bytes());assert r['source']==S and r['tree']==T and r['status']=='passed'and r['clean_after']and r['protected856_verified_before']and r['protected856_verified_after'];assert len(r['steps'])==6
 for step,n in zip(r['steps'],[3,4,9,0,0,0]):
  b=(ROOT/step['log']).read_bytes();assert sha(b)==step['sha256']and step['exit_code']==0 and step['actual_passed']==n and step['before_available_bytes']>=7*1024**3;assert sum(map(int,re.findall(rb'test result: ok\. (\d+) passed; 0 failed; 0 ignored;',b)))==n
 assert sum(s['actual_passed']for s in r['steps'])==16
 for e in r['executables']:
  b=pathlib.Path(e['path']).read_bytes();assert b[:4]==b'\x7fELF'and sha(b)==e['sha256']and len(b)==e['bytes']
 executable_paths={str(pathlib.Path(e['path']).resolve()):e for e in r['executables']}
 for step in r['steps']:
  for path in step['cargo_running_executables']:assert str(pathlib.Path(path).resolve())in executable_paths
  for row in step['cargo_run_backtick_executables']:assert row['sha256']==executable_paths[str(pathlib.Path(row['path']).resolve())]['sha256']
 assert sum(len(step['cargo_running_executables'])for step in r['steps'])==5 and sum(len(step['cargo_run_backtick_executables'])for step in r['steps'])==1
 assert git('rev-parse',S+'^{tree}')==T
 source=json.loads((ROOT/'outputs/next-two-face-proof-source.json').read_bytes());assert source['source']==S and source['tree']==T;last=source['base'];paths=set()
 for row in source['rows']:
  assert row['actual_parent']==last and git('show','-s','--format=%P',row['merge']).split()==[last,row['head']];assert git('diff','--raw','--no-abbrev',row['original_base'],row['head'])==git('diff','--raw','--no-abbrev',last,row['merge']);assert not paths&set(row['paths']);paths|=set(row['paths']);subprocess.run(['git','-C',str(R),'merge-base','--is-ancestor',row['head'],S],check=True);last=row['merge']
 assert last==S and len(paths)==11
 correspondence={'source':S,'tree':T,'exact11_disjoint_source_blobs':{p:{'blob':git('rev-parse',S+':'+p),'sha256':sha(raw(S,p))}for p in sorted(paths)},'full_transitive_normal_build_and_owningrootdev_package_trees':{p:git('rev-parse',S+':'+p)for p in closure()},'root4_inputs':{p:sha(raw(S,p))for p in ['Cargo.toml','Cargo.lock','rust-toolchain.toml','.cargo/config.toml']},'limits':['Actual frozen8734/d631 combinedCPUfeatureproof only; no Candidate/Integration/currentdev/global acceptance','No new Native, fullruntime93, rootplanlowering59, physicalHost/provider/recorder/parent proof campaign']}
 D.mkdir(exist_ok=True);payload={}
 for p in ['outputs/next-two-face-proof-local-proof.json','outputs/next-two-face-proof-source.json','work/run-next-two-face-proof.py','work/resume-next-two-face-proof.py','outputs/next-two-face-proof-headroom-stop.json','work/finalize-next-two-face-running-custody.py',__file__,*[x['log']for x in r['steps']]]:
  path=pathlib.Path(p);b=path.read_bytes();name=path.name;payload[name+('.gz'if name.endswith('.log')else'')]=gzip.compress(b,mtime=0)if name.endswith('.log')else b
 payload['source-dependency-correspondence.json']=(json.dumps(correspondence,indent=2)+'\n').encode();payload['README.md']=b'''# New two-feature Face composition proof

Actual clean8734/d631 normal composition of capturedb3a and exactoriginal6395/6396heads,11disjointpaths. Exactlysixcommands: supportedface-publication3+focusedstrict; featurecombineddisplay_packets4; completeFace9; runtime/Facealltargetstrict; installedthumbv6m no_stdlibfeaturecheck. No physical/runtimeinstallation/globalCandidate/Integration or parent completion inferred. ActualELFs retainedlocally with CargoRunning argv/path/SHA; no baremetalexecutable execution claim. All856 prior actualELFs rehashedunchanged before/after.
'''
 payload['manifest.json']=(json.dumps({'source':S,'tree':T,'files':[{'path':n,'bytes':len(b),'sha256':sha(b)}for n,b in payload.items()]},indent=2)+'\n').encode()
 for n,b in payload.items():(D/n).write_bytes(b)
 (ROOT/'outputs/next-two-face-proof-packet-preparation.json').write_text(json.dumps({'source':S,'tree':T,'packet':str(D),'files':len(payload),'actual_tests':16,'publication_held_for_independent_review':True},indent=2)+'\n');print('Prepared local packet',len(payload),'files; NO publication')
if __name__=='__main__':main()
