from pathlib import Path
import json,sys,hashlib,subprocess
arm=sys.argv[1]; root=Path(__file__).resolve().parent;p=root/arm/'native/x86_64';meta=json.loads((root/arm/'run.json').read_text());assert meta['exit_code']==0 and meta['status_after']==''
def digest(p):return hashlib.sha256(p.read_bytes()).hexdigest()
b=json.loads((p/'build.json').read_text());im=json.loads((p/'image.json').read_text());assert b['base_commit']==meta['source'] and b['elf_sha256']==digest(p/'conduitos') and im['iso_sha256']==digest(p/'conduitos.iso')
raw=p/'ordinary-domain-proof.log';trans=raw.read_text();receipts={}
for f in p.glob('*fixture-proof.json'):
 r=json.loads(f.read_text());assert r['build']==b and r['transcript_sha256']==digest(raw) and r['fixture'] is True and r['proof_class']=='freestanding-emulator';assert r['machine']=={'cpu':'max','emulator':'qemu-system-x86_64','memory_mib':256,'model':'q35','vcpus':1};receipts[f.name]=r
assert len(receipts)==4
s=receipts['service-memory-fixture-proof.json']['observations'];assert s['hostile_entries']==26 and s['restart_boundary_samples_zero'] is True and s['retained_bytes']==8388608 and s['scratch_bytes']==8388608
nm='CONDUIT_X87_NM_CONTROL vector=7 origin=1 terminal=true root-sibling-survived=true';assert nm in trans.splitlines()
result={'source':meta['source'],'scope':meta['scope'],'elf_sha256':digest(p/'conduitos'),'iso_sha256':digest(p/'conduitos.iso'),'transcript_sha256':digest(raw),'receipts':receipts,'raw_nm_control':nm,'protected_markers':[x for x in trans.splitlines() if x.startswith(('CONDUIT_DOMAIN_NEGATIVES ','CONDUIT_DOMAIN_GATE_NEGATIVES ','CONDUIT_DOMAIN_TIMER_COEXISTENCE '))]}
(root/arm/'verified.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps({k:v for k,v in result.items() if k!='receipts'},indent=2))
