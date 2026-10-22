import hashlib,json,shutil,subprocess
from pathlib import Path
root=Path('/home/dancxjo/Documents/Codex/2026-10-21/please-finish-and-close-any-conduit')
checkout=root/'work/publication';source='cf1ee8acd38dc180ad040d24d83d52bda1abcf6e'
checks=json.loads((root/'work/6181-cf1-remaining-checks.json').read_text())
assert checks['source_commit']==source and checks['status']=='complete'
assert all(c['exit_code']==0 for c in checks['checks'])
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=checkout,text=True).strip()==source
assert not subprocess.check_output(['git','status','--porcelain'],cwd=checkout,text=True).strip()
packet=root/'work/6181-cf1-backend-proof';assert not packet.exists()
target=checkout/'target/conduitos/x86_64'
read=lambda p:json.loads(p.read_text())
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
build=read(target/'build.json');image=read(target/'image.json')
assert build['base_commit']==source and build['architecture']=='x86_64' and build['artifact_role']=='architecture-proof-appliance'
assert sha(target/'conduitos')==build['elf_sha256']
assert sha(target/'conduitos.iso')==image['iso_sha256']
transcript=sha(target/'ordinary-domain-proof.log')
receipts=['service-memory-fixture-proof.json','compositor-guest-fixture-proof.json','compositor-hostile-broker-fixture-proof.json','compositor-copy-recovery-fixture-proof.json']
for name in receipts:
    d=read(target/name)
    assert d['build']==build and d['transcript_sha256']==transcript and d['fixture'] is True
    assert d['proof_class']=='freestanding-emulator' and d['observations']['status']=='completed'
    assert d['machine']=={'cpu':'max','emulator':'qemu-system-x86_64','memory_mib':256,'model':'q35','vcpus':1}
memory=read(target/receipts[0])['observations']
assert memory['hostile_entries']==26 and memory['privilege']=='ring3'
assert memory['restart_boundary_samples_zero'] is True and memory['retained_bytes']==memory['scratch_bytes']==8388608
assert memory['production_compositor'] is False and memory['dma_isolation'] is False and memory['driver_isolation'] is False
inventory=target/f"privilege-inventory-{build['elf_sha256']}-architecture-proof-appliance.json"
d=read(inventory);assert d['build']==build and d['working_tree_status']==''
linkmap=target/d['root_link_map'];assert sha(linkmap)==d['root_link_map_sha256']
files=[target/'conduitos',target/'conduitos.iso',target/'build.json',target/'image.json',target/'ordinary-domain-proof.log',inventory,linkmap]+[target/n for n in receipts]
packet.mkdir()
for p in files:shutil.copy2(p,packet/p.name)
shutil.copy2(root/'work/6181-cf1-ordinary-domain-proof.log',packet/'build-proof.log')
retention={'source_commit':source,'command':'cargo +stable xtask make conduitos ordinary-domain-proof --locked','command_exit_code':0,'proof_class':'freestanding-emulator','fixture':True,'files':[]}
for p in sorted(packet.iterdir()):
    retention['files'].append({'path':p.name,'bytes':p.stat().st_size,'sha256':sha(p)})
(packet/'retention.json').write_text(json.dumps(retention,indent=2)+'\n')
for f in retention['files']:
    p=packet/f['path'];assert p.stat().st_size==f['bytes'] and sha(p)==f['sha256']
(root/'outputs/6181-cf1-backend-proof-receipt.json').write_text(json.dumps(retention,indent=2)+'\n')
print(json.dumps({'sealed':str(packet),'source':source,'elf_sha256':build['elf_sha256'],'iso_sha256':image['iso_sha256'],'hostile_entries':26,'files':len(retention['files'])}))
