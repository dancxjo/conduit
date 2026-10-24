import pathlib,subprocess,os,json,hashlib,gzip,tempfile,argparse
ROOT=pathlib.Path.cwd();REPO=ROOT/'work/6267-review';SOURCE='0872c37862e62d657fcd57a7740e4655008ebea1';PARENTS=['40e63b6a7a79bb5691f98f0706396630bf168047','efc427b1926159c9d93a55f6591c76d024798b5e'];TREE='05986adf4c0ad6daf2fecd83a0b0452c1d29eba7';RUNTIME='f21a561749a3bbacf15770fcaece51337e19d60e';RECOVERY='prep/6380-6377-combined-proof-20261024';BRANCH='evidence/6380-6377-combined-proof-20261024';DEST='bodies/environment/runtime/evidence/2026-10-24-borrowed-observation-source-combined/0872c3786'
def sha(b):return hashlib.sha256(b).hexdigest()
def git(*a,input=None,env=None):return subprocess.check_output(['git','-C',str(REPO),*a],input=input,env=env).decode().strip()
def main():
 p=argparse.ArgumentParser();p.add_argument('--publish',action='store_true');a=p.parse_args();assert a.publish
 r=json.loads((ROOT/'outputs/6380-6377-combined-local-proof.json').read_text());assert r['source']==SOURCE and r['parents']==PARENTS and r['tree']==TREE and r['runtime_tree']==RUNTIME and r['clean_after']and r['finished'];assert len(r['steps'])==4
 for step,name,count in zip(r['steps'],['runtime-default','strict-lint-default','runtime-capture','strict-lint-capture'],[35,0,36,0]):assert step['name']==name and step['exit_code']==0 and step['actual_passed']==count and sha((ROOT/step['log']).read_bytes())==step['sha256']
 for e in r['executables']:assert sha((ROOT/e['path']).read_bytes())==e['sha256']
 assert git('show','-s','--format=%P',SOURCE).split()==PARENTS and git('rev-parse',SOURCE+'^{tree}')==TREE and git('rev-parse',SOURCE+':bodies/environment/runtime')==RUNTIME
 paths=['tests/observation_kernel.rs','tests/support/assignment.rs'];assert git('diff','--name-only','5b97ebbcbcbca860de344785e4a6d0a671438173',RUNTIME).splitlines()==paths;rows=[]
 for pth in paths:
  full='bodies/environment/runtime/'+pth;blob=git('rev-parse',SOURCE+':'+full);assert blob==git('rev-parse',PARENTS[0]+':'+full);rows.append({'path':full,'blob':blob,'sha256':sha(subprocess.check_output(['git','-C',str(REPO),'show',SOURCE+':'+full]))})
 checkout=ROOT/'work/6380-6377-combined-proof';assert not subprocess.check_output(['git','status','--porcelain'],cwd=checkout,text=True).strip();assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=checkout,text=True).strip()==SOURCE
 for branch in [RECOVERY,BRANCH]:assert not git('ls-remote','origin','refs/heads/'+branch)
 audit={'source':SOURCE,'parents':PARENTS,'tree':TREE,'runtime_tree':RUNTIME,'bridge_files':rows,'runtime_delta_against_actual_prior_combined_5b97':paths,'actual_proof':'35default and36capture full runtime suites, strictalltargetClippy both configurations, exact source/clean state/rawlogs/retainedlocalELFs.','qualification':['OriginalPR6380head40e63 stays unchanged; no combinedCandidate/Integration inferred.','Actual component/runtime proof may only qualify future tree when entire runtime subtree equals f21a.','Source/program/build/runtime/test helpers all included by whole-runtime tree identity.','Physical fixture origin unverified/no native/firmware/physical/parent6295 completion.']};payload={};inputs=[]
 for name in ['outputs/6380-6377-combined-local-proof.json','outputs/6313-current-dev-c-source-audit.json','outputs/6313-current-dev-c-publication.json','outputs/6377-efc-independent-proof-publication.json','work/run-6380-6377-combined-proof.py',__file__,*[s['log']for s in r['steps']]]:
  path=pathlib.Path(name);path=path if path.is_absolute()else ROOT/path;raw=path.read_bytes();n=path.name;data=raw
  if n.endswith('.log'):n+='.gz';data=gzip.compress(raw,mtime=0)
  payload[n]=data;inputs.append({'file':n,'raw_sha256':sha(raw),'stored_sha256':sha(data),'raw_bytes':len(raw)})
 payload['combined-source-correspondence.json']=(json.dumps(audit,indent=2)+'\n').encode();payload['README.md']=b'''# Borrowed observation bridge plus checked Source actual runtime proof

Actual execution source0872c3786 normally merges original PR6380head40e63 with exact PR6377sourceefc. The complete tree is05986adf4 and runtime subtreef21a56174. The runtime differs from previously combined-tested5b97 only by the same two bridge files; both retain their original reviewed blobs. No PR source branch was changed.

Full owning runtime passed35 default tests and36 source-capture tests, each with strict all-target Clippy. Raw complete logs are losslessly compressed with raw/stored hashes. The proof receipt contains exact commands, environment, clean source/tree, named successes and retained local actual executable size/hash custody; binaries are not hosted here. This is actual component execution, not merely a prospective static composition.

Any future delivery using this proof must establish entire runtime subtree correspondence, including build programs, manifests, production code, tests and fixtures. No combined Candidate/Integration, native installation, firmware, physical capture, parent6295 completion or issue closure is inferred. Fixed octet fixtures retain unverified physical origin; earlier packets retain original source identities.
''';payload['manifest.json']=(json.dumps({'source':SOURCE,'tree':TREE,'runtime_tree':RUNTIME,'raw_inputs':inputs,'files':[{'path':n,'bytes':len(b),'sha256':sha(b)}for n,b in payload.items()]},indent=2)+'\n').encode()
 with tempfile.TemporaryDirectory(prefix='6380-6377-index-',dir=ROOT/'work')as tmp:
  env=dict(os.environ,GIT_INDEX_FILE=str(pathlib.Path(tmp)/'index'));git('read-tree',SOURCE,env=env)
  for n,b in payload.items():git('update-index','--add','--cacheinfo','100644',git('hash-object','-w','--stdin',input=b,env=env),DEST+'/'+n,env=env)
  commit=git('commit-tree',git('write-tree',env=env),'-p',SOURCE,input=b'Retain actual borrowed bridge and checked Source combined runtime proof\n',env=env);git('update-ref','refs/heads/'+BRANCH,commit,'0'*40);subprocess.run(['git','-C',str(REPO),'push','origin','refs/heads/'+RECOVERY+':refs/heads/'+RECOVERY,'refs/heads/'+BRANCH+':refs/heads/'+BRANCH],check=True)
  for b,want in [(RECOVERY,SOURCE),(BRANCH,commit)]:assert git('ls-remote','origin','refs/heads/'+b).split()[0]==want
 out={'source':SOURCE,'tree':TREE,'runtime_tree':RUNTIME,'source_branch':RECOVERY,'branch':BRANCH,'commit':commit,'prefix':DEST,'files':len(payload),'remote_verified':True,'url':'https://github.com/dancxjo/conduit/tree/'+commit+'/'+DEST};(ROOT/'outputs/6380-6377-combined-proof-publication.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(out,indent=2))
if __name__=='__main__':main()
