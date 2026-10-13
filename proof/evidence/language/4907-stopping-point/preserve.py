from pathlib import Path
import subprocess,json,tarfile,hashlib,re,time,os
root=Path('/home/dancxjo/Documents/Codex/2026-10-06/goal-please-finish-and-close-4907')
out=root/'work/stopping-point';out.mkdir(exist_ok=True)
rows=json.loads((root/'work/stopping-point-worktree-inventory.json').read_text())
items={}
for base in ['work','outputs']:
 for p in (root/base).rglob('*'):
  if out in p.parents or not p.is_file():continue
  items[str(p)]='chat/'+str(p.relative_to(root))
for row in rows:
 p=Path(row['path'])
 if p.name=='conduit-4907-native-reuse':continue
 raw=subprocess.check_output(['git','-C',str(p),'ls-files','--others','--exclude-standard','-z']).split(b'\0')
 for b in raw:
  if not b:continue
  f=p/os.fsdecode(b)
  if f.is_file():items[str(f)]='worktrees/'+p.name+'/'+str(f.relative_to(p))
excluded=[];selected=[]
for path,name in sorted(items.items()):
 p=Path(path);st=p.stat();reason=None
 if p.is_symlink():reason='local symlink; original target retained locally'
 elif p.suffix in {'.rlib','.rmeta','.o','.a','.so','.pyc','.d'}:reason='regenerable build/cache artifact'
 elif p.name in {'.env','credentials','credentials.json','id_rsa','id_ed25519'}:reason='credential or environment file retained locally'
 else:
  with p.open('rb') as f:magic=f.read(8)
  if magic.startswith(b'\x7fELF') or magic==b'!<arch>\n':reason='regenerable executable/library artifact'
 if reason:excluded.append({'path':path,'bytes':st.st_size,'classification':reason})
 else:selected.append((p,name,st))
print(json.dumps({'selected_files':len(selected),'selected_bytes':sum(s.st_size for _,_,s in selected),'excluded_files':len(excluded),'excluded_bytes':sum(e['bytes'] for e in excluded)}),flush=True)
records=[];secret=re.compile(rb'(?:ghp_[A-Za-z0-9]{30,}|github_pat_[A-Za-z0-9_]{50,}|sk-proj-[A-Za-z0-9_-]{40,})')
class Reader:
 def __init__(self,f,p):self.f=f;self.p=p;self.h=hashlib.sha256();self.tail=b''
 def read(self,n):
  b=self.f.read(n);self.h.update(b)
  if secret.search(self.tail+b):raise RuntimeError('credential-shaped content: '+str(self.p))
  self.tail=(self.tail+b)[-256:];return b
archive=out/'local-authored-work-and-evidence.tar.gz'
with tarfile.open(archive,'w:gz',compresslevel=3) as tar:
 for p,name,st in selected:
  info=tar.gettarinfo(str(p),arcname=name)
  with p.open('rb') as f:
   r=Reader(f,p);tar.addfile(info,r)
  after=p.stat()
  if (after.st_size,after.st_mtime_ns)!=(st.st_size,st.st_mtime_ns):raise RuntimeError('input changed during snapshot: '+str(p))
  records.append({'path':str(p),'archive_path':name,'bytes':st.st_size,'sha256':r.h.hexdigest()})
manifest={'created_unix':time.time(),'scope':'This goal chat work and outputs plus untracked work in related 4898/4907/5212 worktrees. Native-reuse snapshot is separately published in PR5313. Tracked heads were already remote-contained except final custody checkpoint. No files removed.','included':records,'excluded_preserved_locally':excluded,'archive_bytes':archive.stat().st_size,'archive_sha256':hashlib.file_digest(archive.open('rb'),'sha256').hexdigest()}
(out/'preservation-manifest.json').write_text(json.dumps(manifest,indent=2))
print(json.dumps({'archive_bytes':manifest['archive_bytes'],'archive_sha256':manifest['archive_sha256'],'included_files':len(records),'excluded_files':len(excluded)}),flush=True)
