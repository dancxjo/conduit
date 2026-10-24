import os,subprocess,pathlib,json,hashlib,gzip,tempfile
root=pathlib.Path.cwd();repo=root/'work/6393-permanent-cord-bound';source='1cf7f4e7de59240a5d84fd736557d06664811dda';branch='evidence/reviewed-open-prs-omnibus-20261024';prefix='docs/audits/evidence/reviewed-open-prs-omnibus-20261024'
def git(*a,input=None,env=None):return subprocess.check_output(['git','-C',str(repo),*a],input=input,env=env).decode().strip()
def sha(b):return hashlib.sha256(b).hexdigest()
assert git('rev-parse','HEAD')==source and not git('status','--porcelain');final=json.loads((root/'outputs/reviewed-omnibus-environment-delta-proof.json').read_bytes());assert final['status']=='passed'and final['source']==source
names=['outputs/reviewed-omnibus-final-local-proof.json','outputs/reviewed-omnibus-final-continuation-proof.json','outputs/reviewed-omnibus-environment-delta-proof.json'];payload={};inputs=[]
def add(path):
 p=pathlib.Path(path);p=p if p.is_absolute()else root/p;raw=p.read_bytes();name=p.name;data=raw
 if name.endswith('.log'):name+='.gz';data=gzip.compress(raw,mtime=0)
 if name in payload:assert payload[name]==data;return
 payload[name]=data;inputs.append({'file':name,'raw_bytes':len(raw),'raw_sha256':sha(raw),'stored_bytes':len(data),'stored_sha256':sha(data)})
for n in names:
 r=json.loads((root/n).read_bytes());add(n)
 for s in r['steps']:
  assert sha(pathlib.Path(s['log']).read_bytes())==s['log_sha256'];add(s['log'])
for n in ['outputs/1cf-reviewed-omnibus-environment-delta-test-custody.json','outputs/6422-current-source-custody-and-ancestry.json','outputs/6422-exact-log-attributes-repair-publication.json','outputs/6422-candidate-quick-whitespace-diagnosis.json','outputs/6eb-completed-before-environment-lock-fix-test-custody.json','outputs/6eb-completed-continuation-before-environment-delta-test-custody.json','work/run-reviewed-omnibus-final-proof.py','work/run-reviewed-omnibus-continuation-proof.py','work/run-reviewed-omnibus-environment-delta-proof.py',__file__]:add(n)
payload['README.md']=b'''# Qualified local proof for omnibus #6422

The final receipt executes clean Source1cf7f4e7de59240a5d84fd736557d06664811dda: Core116, constructor/projection32, full supported cargo xtask prove environment face-publication (runtime16, standalone Mask16,4custody compile-fail docs, strict owning checks), owning library strict lint, CLI compile and113SDK tests. The fifth display-mask custody case actually executes. Local proof does not replace Candidate or merged-source Integration.

Earlier6eb36f67d3057346061e69b70ed20fa3f801d965 receipts preserve their own identity: native scene144 plus7compact cards and1explicit opt-in CPU cross-Mask case; Quantity math10/pressure5, planner86, catalog12; Face62 with2existingignored; browseroffer9/Patchbay3; supported Native custody505 with1existingignored; reference15, phone10including84kernelrequests, Source-laws24 and compositorwrappers22. Both strict lint stages and CLI/SDK passed there. The first6eb run stopped on an external lockfile failure BEFORE environment tests; that negative is retained, not relabeled successful. The final supported CLI supplies the corrected combined-source proof.

All original five PR heads, both required repairs and currentdev5b266161926f920bbf266288fe0f1f757d419632 remain ancestors. All19 owning changed source inputs from the independent environment coverage lane match. The final attributes-only repair preserves all60 captured log Git blobs and ordinary source/prose whitespace guards. Its exact-parent ancestry and real PR-base diff check are retained.

Raw logs are losslessly gzip compressed with raw/stored hashes. Actual executed ELF hashes remain in the receipts; verified whole archives/copies remain LOCAL at the recorded custody paths and are not hosted in this packet. The two6eb archives contain51 and25 unique executed files respectively; overlap across receipts is not added into a unique total. The exact final-source actual ELF files were checked when execution completed.

Registered Patchbay DOM evidence remains synthetic Face plus simulated acknowledgement. CPU retained-Face pages/Frames do not establish initialized Mask/Show, live Body, current QEMU, physical execution, whole-runtime memory bounds or parent-issue completion. Historical authentic Gear/Cord/optional-custody negative Source identities remain preserved separately in ancestor/recovery refs and published repair packets.
'''
payload['manifest.json']=(json.dumps({'source':source,'tree':git('rev-parse',source+'^{tree}'),'raw_inputs':inputs,'files':[{'path':n,'bytes':len(b),'sha256':sha(b)}for n,b in payload.items()]},indent=2)+'\n').encode()
assert not git('ls-remote','origin','refs/heads/'+branch)
with tempfile.TemporaryDirectory(prefix='6422-evidence-index-',dir=root/'work')as tmp:
 env=dict(os.environ,GIT_INDEX_FILE=str(pathlib.Path(tmp)/'index'));git('read-tree',source,env=env)
 for n,b in payload.items():git('update-index','--add','--cacheinfo','100644',git('hash-object','-w','--stdin',input=b,env=env),prefix+'/'+n,env=env)
 commit=git('commit-tree',git('write-tree',env=env),'-p',source,input=b'Retain source-qualified omnibus local proof and exact raw log custody\n',env=env);git('update-ref','refs/heads/'+branch,commit,'0'*40)
 subprocess.run(['git','-C',str(repo),'push','origin','refs/heads/'+branch+':refs/heads/'+branch],check=True)
 assert git('ls-remote','origin','refs/heads/'+branch).split()[0]==commit
r={'source':source,'evidence_commit':commit,'branch':branch,'prefix':prefix,'files':len(payload),'url':'https://github.com/dancxjo/conduit/tree/'+commit+'/'+prefix,'remote_exact_verified':True};(root/'outputs/6422-qualified-local-proof-publication.json').write_text(json.dumps(r,indent=2));print(json.dumps(r,indent=2))
