import pathlib,subprocess,os,json,hashlib,gzip,tempfile,argparse,re
ROOT=pathlib.Path.cwd();REPO=ROOT/'work/6267-review';SOURCE='e9f8787ccbf3a0fda086a5e4d568cb7f597d34e1';TREE='d35051b7b0b512ef82e76fd37902479a65c43a06';RECOVERY='prep/ready-four-combined-proof-20261024';BRANCH='evidence/ready-four-combined-proof-20261024';DEST='docs/audits/evidence/ready-four-combined-20261024/e9f8787cc'
def sha(b):return hashlib.sha256(b).hexdigest()
def git(*a,input=None,env=None):return subprocess.check_output(['git','-C',str(REPO),*a],input=input,env=env).decode().strip()
def main():
 p=argparse.ArgumentParser();p.add_argument('--publish',action='store_true');a=p.parse_args();assert a.publish
 r=json.loads((ROOT/'outputs/ready-four-combined-local-proof.json').read_text());assert r['source']==SOURCE and r['tree']==TREE and r['status']=='passed'and r['clean_after']and r['finished']and len(r['steps'])==10 and sum(s['actual_passed']for s in r['steps'])==102
 expected=[35,0,36,0,13,0,9,0,9,0];running=[]
 for step,count in zip(r['steps'],expected):
  raw=(ROOT/step['log']).read_bytes();assert step['exit_code']==0 and step['actual_passed']==count and sha(raw)==step['sha256']and step['before_available_bytes']>=7*1024**3;assert sum(map(int,re.findall(r'test result: ok\. (\d+) passed; 0 failed; 0 ignored;',raw.decode())))==count;running+=step['cargo_running_executables']
 for e in r['executables']:assert len((ROOT/e['path']).read_bytes())==e['bytes']and sha((ROOT/e['path']).read_bytes())==e['sha256']
 for path in set(running):
  q=pathlib.Path(path);assert q.is_file();assert any((ROOT/e['path']).resolve()==q.resolve()for e in r['executables']),path
 composition=json.loads((ROOT/'outputs/ready-four-combined-source.json').read_text());assert composition['source']==SOURCE and git('rev-parse',SOURCE+'^{tree}')==TREE;paths=[]
 for row in composition['rows']:
  assert git('show','-s','--format=%P',row['local_merge']).split()==[row['actual_parent'],row['head']];assert git('diff','--raw','--no-abbrev',row['original_base'],row['head'])==git('diff','--raw','--no-abbrev',row['actual_parent'],row['local_merge']);subprocess.run(['git','-C',str(REPO),'merge-base','--is-ancestor',row['head'],SOURCE],check=True);paths+=row['paths']
 assert len(set(paths))==32 and sorted(set(paths))==git('diff','--name-only',composition['base'],SOURCE).splitlines();source_rows=[]
 for path in sorted(set(paths)):source_rows.append({'path':path,'blob':git('rev-parse',SOURCE+':'+path),'sha256':sha(subprocess.check_output(['git','-C',str(REPO),'show',SOURCE+':'+path]))})
 checkout=ROOT/'work/ready-four-combined-proof';assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=checkout,text=True).strip()==SOURCE;assert not subprocess.check_output(['git','status','--porcelain'],cwd=checkout,text=True).strip()
 for branch in [RECOVERY,BRANCH]:assert not git('ls-remote','origin','refs/heads/'+branch)
 correspondence={'source':SOURCE,'tree':TREE,'components':{p:git('rev-parse',SOURCE+':'+p)for p in ['bodies/environment/runtime','plots/seven-segment-mask','architecture/assigned-plan','architecture/core','architecture/plot','semantics/face','semantics/language','bodies/environment/face']},'all_four_heads_ancestors':True,'exact_original32_path_deltas':source_rows,'actual_execution_source':SOURCE,'actual_tests':102,'actual_cargo_running_executables':sorted(set(running)),'scope':'Actual proposed combined source, not currentdev/Candidate/Integration/native/physical acceptance.'};payload={};inputs=[]
 for name in ['outputs/ready-four-combined-local-proof.json','outputs/ready-four-combined-source.json','work/run-ready-four-combined-proof.py',__file__,*[s['log']for s in r['steps']]]:
  pth=pathlib.Path(name);pth=pth if pth.is_absolute()else ROOT/pth;raw=pth.read_bytes();n=pth.name;data=raw
  if n.endswith('.log'):n+='.gz';data=gzip.compress(raw,mtime=0)
  assert n not in payload;payload[n]=data;inputs.append({'file':n,'raw_sha256':sha(raw),'stored_sha256':sha(data),'raw_bytes':len(raw)})
 payload['combined-source-correspondence.json']=(json.dumps(correspondence,indent=2)+'\n').encode();payload['README.md']=b'''# Four reviewed changes: actual proposed mixed-source proof

Exact execution sourcee9f8787ccbf3a0fda086a5e4d568cb7f597d34e1/tree d35051b7b0b512ef82e76fd37902479a65c43a06 normally merges the four reviewed owner heads6380/6382/6384(90a)/6385. All heads remain ancestors, all32 original before/after file deltas are exact, and no manual source edits were introduced. Source and evidence recovery branches are create-only.

Ten sequential existing focused Cargo commands passed102 actual tests: owning runtime35default+36source-capture and strictalltarget lint both; Mask13+strictalltarget lint; assigned-plan Fore9default+9allfeatures and strictalltarget lint both. Each command had at least7GiB available before launch. Complete raw logs are gzip compressed with raw/stored hashes, exactargv/environment/exit, clean source/tree and named results. Actual Cargo Running executable paths are joined to independently rehashed retained local executable inventories. Binaries are local, not hosted here.

This is actual proposed-source CPU/component integration evidence, including changed Plot/Language/Mask/Fore dependencies. It is not currentdev delivery, a new Candidate run, accepted Integration, native scheduling/installation, physical capture/display, firmware, parent6378/6295 completion or issueclosure. Original proofs retain their original source identities. No PR or owner source branch is changed by this evidence publication.
''';payload['manifest.json']=(json.dumps({'source':SOURCE,'tree':TREE,'raw_inputs':inputs,'files':[{'path':n,'bytes':len(b),'sha256':sha(b)}for n,b in payload.items()]},indent=2)+'\n').encode()
 with tempfile.TemporaryDirectory(prefix='ready-four-index-',dir=ROOT/'work')as tmp:
  env=dict(os.environ,GIT_INDEX_FILE=str(pathlib.Path(tmp)/'index'));git('read-tree',SOURCE,env=env)
  for n,b in payload.items():git('update-index','--add','--cacheinfo','100644',git('hash-object','-w','--stdin',input=b,env=env),DEST+'/'+n,env=env)
  commit=git('commit-tree',git('write-tree',env=env),'-p',SOURCE,input=b'Retain actual proposed four-head mixed-source owning proof\n',env=env);git('update-ref','refs/heads/'+BRANCH,commit,'0'*40);subprocess.run(['git','-C',str(REPO),'push','origin','refs/heads/'+RECOVERY+':refs/heads/'+RECOVERY,'refs/heads/'+BRANCH+':refs/heads/'+BRANCH],check=True)
  for branch,want in [(RECOVERY,SOURCE),(BRANCH,commit)]:assert git('ls-remote','origin','refs/heads/'+branch).split()[0]==want
 out={'source':SOURCE,'tree':TREE,'source_branch':RECOVERY,'branch':BRANCH,'commit':commit,'prefix':DEST,'files':len(payload),'actual_tests':102,'remote_verified':True,'url':'https://github.com/dancxjo/conduit/tree/'+commit+'/'+DEST};(ROOT/'outputs/ready-four-combined-proof-publication.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(out,indent=2))
if __name__=='__main__':main()
