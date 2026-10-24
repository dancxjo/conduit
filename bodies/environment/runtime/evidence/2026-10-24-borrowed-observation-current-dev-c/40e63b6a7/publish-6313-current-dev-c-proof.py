import pathlib,subprocess,os,json,hashlib,gzip,tempfile,argparse
ROOT=pathlib.Path.cwd();REPO=ROOT/'work/6267-review';SOURCE='40e63b6a7a79bb5691f98f0706396630bf168047';RECOVERY='prep/6313-current-dev-20261024-c';BRANCH='evidence/6313-current-dev-c-20261024';DEST='bodies/environment/runtime/evidence/2026-10-24-borrowed-observation-current-dev-c/40e63b6a7'
def sha(b):return hashlib.sha256(b).hexdigest()
def git(*a,input=None,env=None):return subprocess.check_output(['git','-C',str(REPO),*a],input=input,env=env).decode().strip()
def main():
 p=argparse.ArgumentParser();p.add_argument('--publish',action='store_true');a=p.parse_args();assert a.publish
 r=json.loads((ROOT/'outputs/6313-current-dev-c-proof.json').read_text());assert r['source']==SOURCE and r['clean_after'] and r['finished']and len(r['steps'])==2 and r['steps'][0]['actual_passed']==32
 for step in r['steps']:assert step['exit_code']==0 and sha((ROOT/step['log']).read_bytes())==step['sha256']
 for e in r['executables']:assert sha((ROOT/e['path']).read_bytes())==e['sha256']
 audit=json.loads((ROOT/'outputs/6313-current-dev-c-source-audit.json').read_text());assert audit['source']==SOURCE and audit['production_runtime_source_exact_base'];assert git('rev-parse',SOURCE+'^{tree}')==r['tree']
 checkout=ROOT/'work/6313-current-dev-c';assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=checkout,text=True).strip()==SOURCE;assert not subprocess.check_output(['git','status','--porcelain'],cwd=checkout,text=True).strip();assert git('rev-parse',RECOVERY)==SOURCE
 for branch in [RECOVERY,BRANCH]:assert not git('ls-remote','origin','refs/heads/'+branch)
 payload={};inputs=[]
 for name in ['outputs/6313-current-dev-c-proof.json','outputs/6313-current-dev-c-source-audit.json','outputs/6313-current-requirements-0640.json','outputs/6313-open-pr-ownership-0640.json','work/run-6313-current-dev-c-proof.py','work/audit-6313-current-dev-c-source.py',__file__,*[step['log']for step in r['steps']]]:
  path=pathlib.Path(name);path=path if path.is_absolute()else ROOT/path;raw=path.read_bytes();n=path.name;data=raw
  if n.endswith('.log'):n+='.gz';data=gzip.compress(raw,mtime=0)
  payload[n]=data;inputs.append({'file':n,'raw_sha256':sha(raw),'stored_sha256':sha(data),'raw_bytes':len(raw)})
 payload['README.md']=b'''# Held borrowed-observation bridge current-dev refresh

Actual clean source40e63b6a7 is a normal merge of captured currentdev91430c2 and prior heldc4914d source. The complete product delta remains exactly observation_kernel.rs and its assignment helper, with both bridge blobs unchanged. Production runtime files are exact captureddev. All32 current owning runtime tests and strict all-target Clippy passed with a private cache; full raw logs, commands/environment, source audit and local actual ELF hashes are retained. Binaries are local, not hosted here.

The existing sensor and seven-scenario observation tests consume returned owned borrowed-input tables after preparation image/descriptors drop. Original negative variants, earlier29-test and preflight proofs retain their original source identities. No duplicate broad preflight was run in this refresh because tooling and production source did not change. Current display tests are included in the32.

Forthcoming Source decoderefc composes cleanly; its runtime differs from previously combined-tested5b97 only by these same two bridge files. This static correspondence is not actual future combined bridge execution. The preparation remains held outside the PR queue until Root releases it. No Candidate, actual dev delivery, accepted Integration, closure, firmware, physical or stable-release capability is claimed.
''';payload['manifest.json']=(json.dumps({'source':SOURCE,'tree':r['tree'],'inputs':inputs,'files':[{'path':n,'bytes':len(b),'sha256':sha(b)}for n,b in payload.items()]},indent=2)+'\n').encode()
 with tempfile.TemporaryDirectory(prefix='6313-c-evidence-index-',dir=ROOT/'work')as tmp:
  env=dict(os.environ,GIT_INDEX_FILE=str(pathlib.Path(tmp)/'index'));git('read-tree',SOURCE,env=env)
  for n,b in payload.items():git('update-index','--add','--cacheinfo','100644',git('hash-object','-w','--stdin',input=b,env=env),DEST+'/'+n,env=env)
  commit=git('commit-tree',git('write-tree',env=env),'-p',SOURCE,input=b'Retain current-dev held borrowed-observation bridge proof\n',env=env);git('update-ref','refs/heads/'+BRANCH,commit,'0'*40);subprocess.run(['git','-C',str(REPO),'push','origin','refs/heads/'+RECOVERY+':refs/heads/'+RECOVERY,'refs/heads/'+BRANCH+':refs/heads/'+BRANCH],check=True)
  for b,want in [(RECOVERY,SOURCE),(BRANCH,commit)]:assert git('ls-remote','origin','refs/heads/'+b).split()[0]==want
 out={'source':SOURCE,'source_branch':RECOVERY,'branch':BRANCH,'commit':commit,'prefix':DEST,'files':len(payload),'remote_verified':True,'url':'https://github.com/dancxjo/conduit/tree/'+commit+'/'+DEST};(ROOT/'outputs/6313-current-dev-c-publication.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(out,indent=2))
if __name__=='__main__':main()
