import pathlib,subprocess,json,hashlib,gzip,tempfile,os,time
root=pathlib.Path.cwd();source=root/'work/6410-tape-branch-repair';anchor=root/'work/6267-review';base='ebf9894139d747154d64e2e26ad8d2af749df5d7';head='6adc488e20284a93d980b91563b94b6e51f758ba';branch='repair/6410-tape-branch-join-20261024';ebranch='evidence/6410-tape-branch-join-20261024';prefix='docs/audits/evidence/6410-tape-branch-join-20261024/'+head[:9]
def git(*a,input=None,env=None):return subprocess.check_output(['git','-C',str(anchor),*a],input=input,env=env).decode().strip()
def sha(b):return hashlib.sha256(b).hexdigest()
r=json.loads((root/'outputs/6410-branch-join-positive.json').read_bytes());neg=json.loads((root/'outputs/6410-first-negative-independent-audit.json').read_bytes());assert r['source']==head and r['status']=='passed'and r['clean_after'];assert sum(r['steps'][0]['actual_passed_counts'])==15 and all(s['exit_code']==0 for s in r['steps']);assert not subprocess.check_output(['git','-C',str(source),'status','--porcelain'],text=True);assert git('rev-parse','refs/heads/'+branch)==head
inventory={str(pathlib.Path(e['path']).resolve()):e for e in r['executables']}
for e in r['executables']:
 b=pathlib.Path(e['path']).read_bytes();assert len(b)==e['bytes'] and sha(b)==e['sha256']
for s in r['steps']:
 assert sha(pathlib.Path(s['log']).read_bytes())==s['sha256']
 for p in s['actual_running']:assert str(pathlib.Path(p).resolve())in inventory
b=pathlib.Path(neg['retained_actual_failed_elf']).read_bytes();assert sha(b)==neg['sha256']
paths=['outputs/6410-branch-join-first-negative.json','outputs/6410-first-negative-independent-audit.json','outputs/6410-branch-join-positive.json','work/6410-branch-join-first-negative.log','work/6410-branch-join-owning-regressions.log','work/6410-branch-join-scoped-strict.log','work/run-6410-branch-join-negative.py','work/run-6410-branch-join-positive.py','work/publish-6410-branch-join-repair.py'];payload={}
for p in paths:
 data=(root/p).read_bytes();name=pathlib.Path(p).name;payload[name+('.gz'if name.endswith('.log')else'')]=gzip.compress(data,mtime=0)if name.endswith('.log')else data
for i,p in enumerate([neg['retained_actual_failed_elf'],*r['steps'][0]['actual_running']]):payload[('negative'if i==0 else'positive-'+str(i))+'.actual-test.elf.gz']=gzip.compress(pathlib.Path(p).read_bytes(),mtime=0)
payload['repair.patch']=subprocess.check_output(['git','-C',str(anchor),'diff',base,head]);payload['source-closure.json']=(json.dumps({'source':head,'tree':r['tree'],'base':base,'negative_source':neg['source'],'objects':{p:git('rev-parse',head+':'+p)for p in ['Cargo.toml','Cargo.lock','rust-toolchain.toml','architecture/plot','architecture/core','architecture/kernel','architecture/assigned-plan']},'executed_positive_ELF_bindings':r['steps'][0]['actual_running'],'all_positive_ELFs_rehashed':len(r['executables'])},indent=2)+'\n').encode()
payload['README.md']=('''# Structured tape conditional join repair

At unmodified production Source ebf989413, a new ordinary/prepared parity control fails: true branch output omits the following right=33 record field. The four existing tape tests pass. Exact negative Source85f0a4 and rawfailedlog/actualELF are retained.

Repair6adc488 prevents constant-byte coalescing across an existing jump destination. No runtime instruction or admission/bounds rule is added. Both branch values and repeated reuse match ordinary canonical bytes with zero allocations measured. Fifteen actual tests across four owning targets pass; strict Plot library/four-target Clippy passes. This does not claim the author's separate27-case campaign or fullCI acceptance.

Clean source/tree, fullargv/exits/logs, allpositiveELFhashes,4actualpositiveRunningELFs andnegativeELF retained. Recoverybranch andevidence only; existingPR owner remains unchanged. Fresh Candidate and applicable accepted Integration remain required after adoption. No MCU installation, physical output, authority or whole-pathno-growth claim.
''').encode();payload['manifest.json']=(json.dumps({'source':head,'base':base,'files':[{'path':n,'bytes':len(b),'sha256':sha(b)}for n,b in payload.items()]},indent=2)+'\n').encode()
assert not git('ls-remote','origin','refs/heads/'+branch);subprocess.run(['git','-C',str(anchor),'push','--force-with-lease=refs/heads/'+branch+':','origin','refs/heads/'+branch],check=True);assert git('ls-remote','origin','refs/heads/'+branch).split()[0]==head;assert not git('ls-remote','origin','refs/heads/'+ebranch)
with tempfile.TemporaryDirectory(dir=root/'work',prefix='6410-evidence-index-')as td:
 env=dict(os.environ,GIT_INDEX_FILE=td+'/index');git('read-tree',head,env=env)
 for n,b in payload.items():git('update-index','--add','--cacheinfo','100644',git('hash-object','-w','--stdin',input=b,env=env),prefix+'/'+n,env=env)
 commit=git('commit-tree',git('write-tree',env=env),'-p',head,input=b'Retain actual conditional join failure and repaired constructor proof\n',env=env);git('update-ref','refs/heads/'+ebranch,commit,'0'*40);subprocess.run(['git','-C',str(anchor),'push','--force-with-lease=refs/heads/'+ebranch+':','origin','refs/heads/'+ebranch],check=True);assert git('ls-remote','origin','refs/heads/'+ebranch).split()[0]==commit
out={'at':time.time(),'source':head,'base':base,'evidence':commit,'branch':branch,'evidence_branch':ebranch,'remote_verified':True,'files':len(payload),'actual_passed_tests':15,'actual_negative':{'passed':4,'failed':1},'positive_ELFs_rehashed':len(r['executables']),'url':'https://github.com/dancxjo/conduit/tree/'+commit+'/'+prefix,'owner_branch_modified':False,'new_PR_created':False};(root/'outputs/6410-branch-join-repair-publication.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(out,indent=2))
