import os,subprocess,pathlib,json,hashlib,gzip,tempfile
root=pathlib.Path.cwd();repo=root/'work/6393-permanent-cord-bound';source='4e9cad06aaa21a59257d9d42181395ff17ed0b52';branch='evidence/reviewed-open-prs-omnibus-locks-20261024';prefix='docs/audits/evidence/reviewed-open-prs-omnibus-locks-20261024'
def git(*a,input=None,env=None):return subprocess.check_output(['git','-C',str(repo),*a],input=input,env=env).decode().strip()
def sha(b):return hashlib.sha256(b).hexdigest()
a=json.loads((root/'outputs/6422-complete-local-preflight.json').read_bytes());r=json.loads((root/'outputs/6422-all-standalone-locks-refresh.json').read_bytes());assert a['status']=='passed'and a['source']==source and r['status']=='committed_full_preflight_passed_and_remote_verified';assert git('rev-parse','HEAD')==source and not git('status','--porcelain');payload={};inputs=[]
for n in ['outputs/6422-complete-local-preflight.json','outputs/6422-all-standalone-locks-refresh.json','work/repair-6422-all-standalone-locks.py','work/finish-6422-all-standalone-locks.py','work/run-6422-complete-preflight.py',__file__,*[s['log']for s in a['steps']],*[s['log']for s in r['steps']]]:
 p=pathlib.Path(n);p=p if p.is_absolute()else root/p;raw=p.read_bytes();name=p.name;data=raw
 if name.endswith('.log'):name+='.gz';data=gzip.compress(raw,mtime=0)
 if name in payload:assert payload[name]==data;continue
 payload[name]=data;inputs.append({'file':name,'raw_bytes':len(raw),'raw_sha256':sha(raw),'stored_sha256':sha(data)})
payload['README.md']=b'''# Final standalone-lock and complete preflight receipt for #6422

Source4e9cad06aaa21a59257d9d42181395ff17ed0b52 is a13-Cargo.lock-only descendant of the fully locally tested1cf7f4e7de59240a5d84fd736557d06664811dda. Production code, root Cargo.lock, environment runtime and standalone Mask lockfiles are unchanged from1cf. The previous source-qualified execution packet is c95f46953ed55424c1ba0294cd6c8fab71e545b0/docs/audits/evidence/reviewed-open-prs-omnibus-20261024 and retains its own Source identity; those tests are not relabeled4e9 execution.

The actual full supported cargo +stable xtask ci pipeline preflight5b266161926f920bbf266288fe0f1f757d419632 passed on clean4e9 Source. It checks all29 standalone locked graphs, actual-base diff whitespace, complete workspace formatting, locked metadata, generated canonical definitions,345CI pipeline tests (zero failed/skipped), and pinned actionlint. Supported setup-ci verified its pinned tool archive/executable.

The thirteen standalone lock graphs add only required adler2.0.1/miniz_oxide0.8.9 entries or shared Face dependency edges. Every pre-existing registry version and checksum is preserved. An initial offline DHT11 resolution attempted unrelated downgrades; the guard stopped it, its known edit was restored from1cf, and online workspace resolution preserved all original pins. The initial1cf full standalone-lock refusal remains in the receipt and raw logs.

This is local preflight/metadata proof, not board compilation, firmware execution, physical delivery, Candidate success or accepted Integration. No workflow guard was bypassed. Raw logs are losslessly gzip compressed with hashes. All original PR heads and repairs remain ancestors through1cf.
'''
payload['manifest.json']=(json.dumps({'source':source,'tree':git('rev-parse',source+'^{tree}'),'raw_inputs':inputs,'files':[{'path':n,'bytes':len(b),'sha256':sha(b)}for n,b in payload.items()]},indent=2)+'\n').encode();assert not git('ls-remote','origin','refs/heads/'+branch)
with tempfile.TemporaryDirectory(prefix='6422-lock-proof-index-',dir=root/'work')as tmp:
 env=dict(os.environ,GIT_INDEX_FILE=str(pathlib.Path(tmp)/'index'));git('read-tree',source,env=env)
 for n,b in payload.items():git('update-index','--add','--cacheinfo','100644',git('hash-object','-w','--stdin',input=b,env=env),prefix+'/'+n,env=env)
 commit=git('commit-tree',git('write-tree',env=env),'-p',source,input=b'Retain complete omnibus preflight and version-preserving standalone-lock proof\n',env=env);git('update-ref','refs/heads/'+branch,commit,'0'*40);subprocess.run(['git','-C',str(repo),'push','origin','refs/heads/'+branch+':refs/heads/'+branch],check=True);assert git('ls-remote','origin','refs/heads/'+branch).split()[0]==commit
out={'source':source,'evidence_commit':commit,'branch':branch,'prefix':prefix,'files':len(payload),'url':'https://github.com/dancxjo/conduit/tree/'+commit+'/'+prefix,'remote_exact_verified':True};(root/'outputs/6422-complete-preflight-publication.json').write_text(json.dumps(out,indent=2));print(json.dumps(out,indent=2))
