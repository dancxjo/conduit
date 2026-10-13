from pathlib import Path
import hashlib,json,os,subprocess
root=Path('work').resolve(); candidates=[]
for p in root.rglob('*'):
 if p.is_file() and 'immutable-sdk' in str(p) and p.suffix in ['.rlib','.so']:
  s=p.stat(); candidates.append((p,s.st_size,s.st_ino,s.st_nlink,s.st_blocks))
groups={}; changed=[]; recovered=0
for p,size,ino,links,blocks in candidates:
 h=hashlib.sha256(p.read_bytes()).hexdigest();key=(p.name,h); first=groups.get(key)
 if first is None: groups[key]=p;continue
 if first.stat().st_ino==p.stat().st_ino:continue
 if subprocess.run(['fuser','-s',str(p)],capture_output=True).returncode==0 or subprocess.run(['fuser','-s',str(first)],capture_output=True).returncode==0: continue
 s=p.stat();temp=p.with_suffix(p.suffix+'.dedup');os.link(first,temp);os.replace(temp,p)
 assert hashlib.sha256(p.read_bytes()).hexdigest()==h
 saved=s.st_blocks*512 if s.st_nlink==1 else 0;recovered+=saved;changed.append({'path':str(p),'same_bytes_at':str(first),'sha256':h,'reclaimed_blocks_bytes':saved})
receipt={'scope':'own immutable identical SDK payload dedup only; all original paths and bytes preserved; fuser checked both before replace','recovered_physical_bytes':recovered,'files':changed};Path('work/retained-factory-storage/sdk-deduplication.json').write_text(json.dumps(receipt,indent=2)+'\n');print(recovered,len(changed))
