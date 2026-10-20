import pathlib,json,subprocess,tempfile,os,hashlib,re,stat,time
T=pathlib.Path('/home/dancxjo/Documents/Codex/2026-10-19/goal-6038');B=T/'work/stewardship-20261020';R='/home/dancxjo/conduit';ref='refs/heads/recovery/local-sync-20261020/auxiliary-proof-source';parent='fc18ddb28fcabbad3cb4aac546993b87edc52440'
files=[('work/native-pagination-user-bug.md', 'task-6038/work/native-pagination-user-bug.md'), ('work/pagination-user-report-origin.json', 'task-6038/work/pagination-user-report-origin.json'), ('work/mechanical-layout-current-gap-audit.md', 'task-6038/work/mechanical-layout-current-gap-audit.md'), ('work/mechanical-layout-issue.md', 'task-6038/work/mechanical-layout-issue.md'), ('work/mechanical-layout-issue-v2.md', 'task-6038/work/mechanical-layout-issue-v2.md'), ('work/mechanical-layout-issue-v2.json', 'task-6038/work/mechanical-layout-issue-v2.json'), ('work/6158-general-layout-coordination.md', 'task-6038/work/6158-general-layout-coordination.md'), ('work/6038-general-layout-scope-update.md', 'task-6038/work/6038-general-layout-scope-update.md')]
def g(*a,env=None,data=None):return subprocess.check_output(['git',*a],cwd=R,env=env,input=data)
def remote():return g('ls-remote','--heads','origin',ref).decode().split()[0]
if remote()!=parent:raise RuntimeError('Remote parent drift')
sigs=[re.compile(rb'-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----'),re.compile(rb'\b(?:gh[pousr]_[A-Za-z0-9]{30,}|github_pat_[A-Za-z0-9_]{40,})\b'),re.compile(rb'\bAKIA[A-Z0-9]{16}\b'),re.compile(rb'(?i)(?:api[_-]?key|password|bearer|access[_-]?token|client[_-]?secret)\s*[=:]\s*[\"\'][^\"\']{12,}[\"\']')];batch=[]
for source,target in files:
 p=T/source
 if p.is_symlink() or not p.is_file() or not p.resolve().is_relative_to(T):raise RuntimeError('Source path bounds')
 data=p.read_bytes()
 if b'\0' in data or len(data)>2**20 or any(s.search(data) for s in sigs):raise RuntimeError('Unsafe source scan '+source)
 data.decode();batch.append({'source':source,'tree_path':target,'bytes':len(data),'sha256':hashlib.sha256(data).hexdigest(),'mode':'100755' if p.stat().st_mode&stat.S_IXUSR else '100644','content':data})
fd,idx=tempfile.mkstemp(prefix='aux-update-index-',dir=B);os.close(fd);os.unlink(idx);env=dict(os.environ,GIT_INDEX_FILE=idx)
try:
 g('read-tree',parent,env=env)
 for f in batch:
  blob=g('hash-object','-w','--stdin',data=f['content']).decode().strip();f['blob']=blob;g('update-index','--add','--cacheinfo',f['mode'],blob,f['tree_path'],env=env)
 manifest={'parent':parent,'scope':'Read-only native capacity seam plan and public Source measurement correction; no implementation or platform proof','files':[{k:v for k,v in f.items() if k!='content'} for f in batch]};manifestbytes=(json.dumps(manifest,indent=2)+'\n').encode();mblob=g('hash-object','-w','--stdin',data=manifestbytes).decode().strip();g('update-index','--add','--cacheinfo','100644',mblob,'mechanical-layout-user-scope-update-manifest.json',env=env);tree=g('write-tree',env=env).decode().strip();commit=g('commit-tree',tree,'-p',parent,data=b'Preserve user general mechanical layout scope, ontology and CSS lessons audit\n\nRecovery metadata only; raw products and proof logs remain local.\n').decode().strip()
finally:pathlib.Path(idx).unlink(missing_ok=True)
for f in batch:
 if hashlib.sha256((T/f['source']).read_bytes()).hexdigest()!=f['sha256']:raise RuntimeError('Source drift before push')
if remote()!=parent:raise RuntimeError('Remote changed before push')
p=subprocess.run(['git','push','origin',commit+':'+ref],cwd=R,text=True,stdout=subprocess.PIPE,stderr=subprocess.PIPE);(B/'mechanical-layout-user-scope-update-push.log').write_text(p.stdout+p.stderr)
if p.returncode:raise RuntimeError('Normal FF push failed')
if remote()!=commit:raise RuntimeError('Origin mismatch')
if g('rev-parse',commit+'^').decode().strip()!=parent:raise RuntimeError('Parent mismatch')
for f in batch:
 if hashlib.sha256(g('show',commit+':'+f['tree_path'])).hexdigest()!=f['sha256']:raise RuntimeError('Committed byte mismatch')
manifest.update(commit=commit,tree=tree,ref=ref,origin_verified=True,finished=time.strftime('%FT%TZ',time.gmtime()));(B/'mechanical-layout-user-scope-update-result.json').write_text(json.dumps(manifest,indent=2)+'\n');print(json.dumps({'files':len(batch),'parent':parent,'commit':commit,'tree':tree,'origin_verified':True},indent=2))
