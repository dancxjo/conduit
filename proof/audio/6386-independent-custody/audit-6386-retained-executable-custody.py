import pathlib,json,hashlib,gzip,subprocess,re
R=pathlib.Path.cwd();G=R/'work/6267-review';D=pathlib.Path('/home/dancxjo/Documents/Codex/2026-10-21/goal-find-the-first-available-github/work/6386-completed-cache-proof-custody');m=json.loads((D/'manifest.json').read_text());c=json.loads((D/'compression-custody.json').read_text());assert c['state'].startswith('completed');assert len(c['records'])==m['tested_elf_count']==30;elf={};rows=[]
for x in c['records']:
 p=pathlib.Path(x['compressed']);assert hashlib.sha256(p.read_bytes()).hexdigest()==x['compressed_sha256'];h=hashlib.sha256();n=0;magic=b''
 with gzip.open(p,'rb')as f:
  while b:=f.read(1024*1024):
   if n==0:magic=b[:4]
   n+=len(b);h.update(b)
 assert magic==b'\x7fELF' and n==x['decoded_bytes'] and h.hexdigest()==x['decoded_sha256'];original=next(v for v in m['retained'] if v['retained']==x['original']);assert original['sha256']==h.hexdigest() and original['bytes']==n;elf[original['original']]=original;rows.append({'original':original['original'],'compressed':str(p),'decoded_sha256':h.hexdigest(),'decoded_bytes':n})
S='a3117127a9d4eb3e6946d63c5534e9720fb92e66';prefix='proof/audio/6386-common-basis-linear/'
def raw(p):return subprocess.check_output(['git','-C',str(G),'show',S+':'+prefix+p])
p=json.loads(raw('results.json'));supported=gzip.decompress(raw('supported-acoustic-control.log.gz'));assert hashlib.sha256(supported).hexdigest()==p['logs']['supported-acoustic-control.log.gz']['raw_sha256'];t=supported.decode();counts=list(map(int,re.findall(r'test result: ok\. (\d+) passed; 0 failed; 0 ignored;',t)));assert sum(counts)==105 and len([n for n in counts if n])==27;bindings=re.findall(r'Running[^\n]*\(([^)]+)\)',t)
for path in bindings:assert path in elf
out={'status':'passed','actual_execution_source':p['source_commit'],'packet_delivery':S,'custody_manifest_head_is_documentary':m['head'],'supported_tests':105,'positive_test_binaries':27,'test_running_bindings':len(bindings),'actual_ELFs_losslessly_rehashed':len(rows),'executables':rows,'scope':'Independent byte custody of original owning-suite executables and source-bound raw log. No new execution, moving dev/Candidate identity or full Integration acceptance. Owner compressed artifacts remain untouched.'};(R/'outputs/6386-retained-executable-custody-independent-review.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps({k:v for k,v in out.items() if k!='executables'},indent=2))
