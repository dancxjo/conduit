"""Preserve explicit MCU source and public qualification evidence without moving checkout/index."""
from pathlib import Path
import os, subprocess, tempfile, json
root=Path(__file__).resolve().parent.parent
repo=root/'work/environment-extraction-integration-20261111'
branch='checkpoint/greenhouse-source-preparation-20261114'
base='docs/checkpoints/greenhouse-source-preparation-20261114'
def run(*a,**kw):return subprocess.check_output(['git',*a],cwd=repo,**kw)
old=run('rev-parse',branch).decode().strip();head=run('rev-parse','HEAD').decode().strip()
fd,index=tempfile.mkstemp(prefix='conduit-checkpoint-index-');os.close(fd);os.unlink(index)
env=dict(os.environ,GIT_INDEX_FILE=index)
def git(*a,input=None):return run(*a,input=input,env=env).decode().strip()
def put(path,data):
 blob=git('hash-object','-w','--stdin',input=data)
 git('update-index','--add','--cacheinfo',f'100644,{blob},{base}/{path}')
try:
 git('read-tree',old)
 paths=set(git('diff','--name-only',old,head).splitlines())|set(git('diff','--name-only','HEAD').splitlines())|set(git('ls-files','--others','--exclude-standard').splitlines())
 paths=sorted(p for p in paths if p=='Cargo.lock' or p.startswith(('targets/mcu/','targets/rp2040/','targets/conduitos/plots/usb/','architecture/')))
 for p in paths:
  if (repo/p).is_file():git('add','--',p)
 names=['positive-shared-stock-factory-20261114.log','positive-shared-stock-factory-20261114.json','rp2040-shared-stock-build-receipt-20261114.json','rp2040-shared-stock-footprint-20261114.json','rp2040-shared-stock-link-20261114.log','rp2040-shared-stock-inference-failure-20261114.log','shared-stock-runtime-tests-20261114.log','shared-stock-runtime-clippy-20261114.log','shared-stock-topology-tests-20261114.log','shared-stock-topology-clippy-20261114.log','prepared-owner-retention-audit-20261114.md','kernel-immutable-storage-audit-20261114.md']
 for name in names:
  p=root/'outputs'/name
  if p.is_file():put('evidence/'+name,p.read_bytes())
 for name in ['run-rp2040-shared-stock-build-20261114.py','preserve-source-checkpoint-20261114.py']:
  put('scripts/'+name,(root/'work'/name).read_bytes())
 old_note=run('show',f'{old}:{base}/CONTINUE-HERE.md').decode()
 marker='\n## Current shared-stock integration\n'
 if marker in old_note:old_note=old_note.split(marker)[0]
 note=old_note+marker+f'''
Shared-source parent: {head}. Experimental neutral factory now constructs its value store AFTER the actual prepared resident/software/native owners have passed the authenticated assignment, using the exact emitted Cord-sink and Step-output associations. Every original logical slot, generation, byte quota, queue capacity and canonical/native result extent is preserved. Unknown custody retains full original physical stock rather than deriving a reduction.

Full original Source76 CPU preparation and Root preflight pass with the selected shared store: retained189469 B, peak191508 B (CPU64 requested allocations; same exclusions as earlier receipts), versus210354/212393 B before:20885 B saved in each. CPU original uninstrumented factory SHA256 2cbb4e777fcccc76afbe8f88d25aa69c8af30c14c916c06e5ea280408337dd9a. no_std original modules compile. Runtime139testsPASS/1ignored; topology5testsPASS; both scoped strictClippy PASS. No admission/Start/sensor or native clock effects occurred; all loans returned.

Actual ARM release links, ELF SHA256 03e7cfcdff0ddfad4e9abbb7bfb695efc4c8597397f8631562772c4bf1113807. Flash margin49844 B; static extent209088 B (__sheap-__sdata, includes512B uninitialized); stack region61248 B. These are LINK/geometry observations, not execution/heap/stack fit. Installed heap remains131072 B. DO NOT FLASH as a qualified appliance: real target memory fit, autonomous Source composition, Fore/Face/Mask and durable history/restart integration are still unfinished. Devices remain on earlier firmware.

Next substantive gate: reduce duplicated immutable kernel/Plan preparation metadata under existing workspace-custody obligations (#6341); consult any saved kernel immutable-storage audit. Keep semantic Source work, hardware facts, exact Plans and prepared execution separate. Do not increase grants, infer lifetime bounds from Kind names or replace Source behavior with bespoke task loops. Then repeat full preparation and actual target resource qualification before admission/physical tests. Existing neutral service still refuses structured-fore-mask-unavailable.

Current dev refreshed and already ancestor of integration HEAD. Experiments remain in draftPR6902; qualified shared changes in PR6837. Private keys/credentials/journals remain intentionally excluded.
'''
 put('CONTINUE-HERE.md',note.encode())
 tree=git('write-tree');commit=git('commit-tree',tree,'-p',old,'-p',head,input=b'Preserve selected shared-stock Source factory and measured preparation savings\n')
 for p in paths:
  if (repo/p).is_file():assert run('show',f'{commit}:{p}')==(repo/p).read_bytes(),p
 git('update-ref','refs/heads/'+branch,commit,old)
 subprocess.run(['git','push','origin',branch],cwd=repo,check=True)
 receipt={'checkpoint':commit,'qualified_shared_parent':head,'source_files_byte_verified':len(paths),'private_state_excluded':True}
 (root/'outputs/origin-shared-stock-checkpoint-20261114.json').write_text(json.dumps(receipt,indent=2)+'\n')
 print(json.dumps(receipt))
finally:
 if os.path.exists(index):os.unlink(index)
