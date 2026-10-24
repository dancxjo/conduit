import pathlib,subprocess,os,json,hashlib,gzip,tempfile,argparse,re,tomllib,posixpath
ROOT=pathlib.Path.cwd();REPO=ROOT/'work/6267-review';SOURCE='f7a959cdc7304f74d6b058f69f139af94037bab5';TREE='be722c7c85e0553256f1cb4c42ac6919081f3755';BRANCH='evidence/6389-be722-runtime-supported-20261024';PREFIX='docs/audits/evidence/6389-be722-runtime-supported-20261024/f7a959cdc'
def sha(b):return hashlib.sha256(b).hexdigest()
def git(*a,input=None,env=None):return subprocess.check_output(['git','-C',str(REPO),*a],input=input,env=env).decode().strip()
def raw(ref,p):return subprocess.check_output(['git','-C',str(REPO),'show',ref+':'+p])
def dependency_paths():
 roots={'bodies/environment/runtime','plots/seven-segment-mask','architecture/assigned-plan','mechanisms/io/tm1637','make/embedded-build','tools/xtask-dispatch'};seen=set();pending=list(roots)
 while pending:
  package=pending.pop()
  if package in seen:continue
  seen.add(package);manifest=tomllib.loads(raw(SOURCE,package+'/Cargo.toml').decode())
  def walk(value):
   if not isinstance(value,dict):return
   for name,table in value.items():
    if name in ('dependencies','build-dependencies')or name=='dev-dependencies'and package in roots:
     for dep in table.values():
      if isinstance(dep,dict)and'path'in dep:pending.append(posixpath.normpath(package+'/'+dep['path']))
    elif isinstance(table,dict):walk(table)
  walk(manifest)
 return sorted(seen)
def main():
 p=argparse.ArgumentParser();p.add_argument('--publish',action='store_true');a=p.parse_args();assert a.publish
 r=json.loads((ROOT/'outputs/6389-be722-runtime-supported-local-proof.json').read_bytes());assert r['source']==SOURCE and r['tree']==TREE and r['status']=='passed'and r['clean_after'] and r['protected426_verified_before']and r['protected426_verified_after'];assert len(r['steps'])==12
 for s,count in zip(r['steps'],[35,0,36,0,13,0,9,0,9,0,17,7]):
  b=(ROOT/s['log']).read_bytes();assert s['exit_code']==0 and s['actual_passed']==count and sha(b)==s['sha256']and s['before_available_bytes']>=7*1024**3;assert sum(map(int,re.findall(rb'test result: ok\. (\d+) passed; 0 failed; 0 ignored;',b)))==count
 assert sum(s['actual_passed']for s in r['steps'])==126
 for e in r['executables']:assert sha(pathlib.Path(e['path']).read_bytes())==e['sha256']
 for s in r['steps']:
  for path in s['cargo_running_executables']:assert any(pathlib.Path(e['path']).resolve()==pathlib.Path(path).resolve()for e in r['executables'])
 assert git('rev-parse',SOURCE+'^{tree}')==TREE;checkout=pathlib.Path('/tmp/conduit-6389-be722-source');assert not subprocess.check_output(['git','status','--porcelain'],cwd=checkout,text=True).strip()
 original=json.loads((ROOT/'outputs/ready-four-combined-source.json').read_text());paths=set()
 for row in original['rows']:
  assert git('diff','--raw','--no-abbrev',row['original_base'],row['head'])==git('diff','--raw','--no-abbrev',row['actual_parent'],row['local_merge']);paths|=set(row['paths']);subprocess.run(['git','-C',str(REPO),'merge-base','--is-ancestor',row['head'],SOURCE],check=True)
 assert len(paths)==32;makepaths=git('diff','--name-only','e9f8787ccbf3a0fda086a5e4d568cb7f597d34e1',SOURCE).splitlines();assert len(makepaths)==8 and not paths&set(makepaths);paths|=set(makepaths);assert sorted(paths)==git('diff','--name-only',original['base'],SOURCE).splitlines()
 native=json.loads((ROOT/'outputs/6384-native-merge-helper-preparation-review.json').read_text());nativeobjects={p:git('rev-parse',SOURCE+':'+p)for p in native['transitive_normal_build_dependencies']}
 for p,oid in nativeobjects.items():assert oid==git('rev-parse','e9f8787ccbf3a0fda086a5e4d568cb7f597d34e1:'+p)
 correspondence={'source':SOURCE,'tree':TREE,'all_five_owner_heads_ancestors':True,'exact40pathblobs':[{'path':p,'blob':git('rev-parse',SOURCE+':'+p),'sha256':sha(raw(SOURCE,p))}for p in sorted(paths)],'components':{p:git('rev-parse',SOURCE+':'+p)for p in ['make/embedded-build','bodies/environment/runtime','plots/seven-segment-mask','architecture/assigned-plan','architecture/core','architecture/plot','semantics/language','semantics/face','bodies/environment/face']},'fresh_Native_proof_source':'e9f8787ccbf3a0fda086a5e4d568cb7f597d34e1','fresh_Native_packet':'5514e2b950887dc70bdf5fa5d8c4b2df9b93311c','Native11_dependency_objects_unchanged':nativeobjects,'owning_transitive_dependency_objects':{p:git('rev-parse',SOURCE+':'+p)for p in dependency_paths()},'workspace_inputs':{p:sha(raw(SOURCE,p))for p in ['Cargo.toml','Cargo.lock','rust-toolchain.toml','.cargo/config.toml']},'qualification':'126actual owning/public-route test executions atf7a/be722. NativeBeam notrerun; existingfreshNativee9f proof has11exactnormal/build dependencies, explicit sourcequalified carryforward. No Candidate/Integration/physical or parentclosure inferred.'}
 assert not git('ls-remote','origin','refs/heads/'+BRANCH);payload={};inputs=[]
 for n in ['outputs/6389-be722-runtime-supported-local-proof.json','outputs/6389-be722-proof-publication.json','outputs/ready-four-native-proof-publication.json','outputs/6389-actual-admission-proof-scope.json','work/run-6389-be722-runtime-supported-proof.py',__file__,*[s['log']for s in r['steps']]]:
  path=pathlib.Path(n);path=path if path.is_absolute()else ROOT/path;b=path.read_bytes();name=path.name;stored=b
  if name.endswith('.log'):name+='.gz';stored=gzip.compress(b,mtime=0)
  payload[name]=stored;inputs.append({'file':name,'raw_bytes':len(b),'raw_sha256':sha(b),'stored_sha256':sha(stored)})
 payload['source-dependency-correspondence.json']=(json.dumps(correspondence,indent=2)+'\n').encode();payload['README.md']=b'''# Actual environmental runtime and public checks after Make Fore composition

Exact cleanf7a959cdc/treebe722 includes the five reviewed original heads with40exact file deltas. Following separatefreshMake26+strict proof, twelve sequential commands passed126actualtests: runtime35+36withstrictboth, Mask13+strict, Fore9+9withstrictboth, supportedTM17(wire-only/contract+Sourceencoderstrict) and supportedSource3+4withstrictboth. Everycommand had at least7GiB inprivate/tmp beforelaunch. Completeargv/exit/rawlogs/namedtests/RunningELFs/source/components/retainedlocal hashes are explicit.

The original323executables and stablecopiesof103Makeowning executables remain hash-identical before/after. Prior102and24proofs ate9f/d350 remain immutable and are not relabeled. NativeBeam was not rebuilt: its fresh5514packet executed ate9f, and all11normal/builddependencyobjects remainexact atf7a, as separately recorded. Actual6389Candidate26 checkout5140 predatesNativePlot and retainsitsown identity. Binaries are retainedlocally, not hostedbythispacket.

This actualcomponent/public-route proof is not devdelivery, fullCandidate/Integration, physicalfixtureprovenance, realdisplayHostCalls/cancellation/MCUfirmware, recorder/overnight or parent6378/6295 completion. No unknownwhole-tree or runtime-only dependencywaiver is granted. Acceptancehelpers must explicitlyreview exactbe722 and originalscopes before use.
''';payload['manifest.json']=(json.dumps({'source':SOURCE,'tree':TREE,'raw_inputs':inputs,'files':[{'path':n,'bytes':len(b),'sha256':sha(b)}for n,b in payload.items()]},indent=2)+'\n').encode()
 with tempfile.TemporaryDirectory(prefix='6389-runtime-index-',dir=ROOT/'work')as tmp:
  env=dict(os.environ,GIT_INDEX_FILE=str(pathlib.Path(tmp)/'index'));git('read-tree',SOURCE,env=env)
  for n,b in payload.items():git('update-index','--add','--cacheinfo','100644',git('hash-object','-w','--stdin',input=b,env=env),PREFIX+'/'+n,env=env)
  commit=git('commit-tree',git('write-tree',env=env),'-p',SOURCE,input=b'Retain actual runtime and public supported checks after Make Fore composition\n',env=env);git('update-ref','refs/heads/'+BRANCH,commit,'0'*40);subprocess.run(['git','-C',str(REPO),'push','origin','refs/heads/'+BRANCH+':refs/heads/'+BRANCH],check=True);assert git('ls-remote','origin','refs/heads/'+BRANCH).split()[0]==commit
 out={'source':SOURCE,'tree':TREE,'branch':BRANCH,'commit':commit,'prefix':PREFIX,'files':len(payload),'actual_tests':126,'remote_verified':True,'url':'https://github.com/dancxjo/conduit/tree/'+commit+'/'+PREFIX};(ROOT/'outputs/6389-be722-runtime-supported-publication.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(out,indent=2))
if __name__=='__main__':main()
