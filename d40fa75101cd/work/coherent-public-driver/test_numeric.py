from pathlib import Path
import json,subprocess
repo=Path(__file__).resolve().parents[2];project=Path('/home/dancxjo/Documents/Codex/2026-10-06/goal-please-finish-and-close-4907');old=project/'work/fargan/public-model-driver';w=repo/'work/coherent-public-driver';src=w/'numeric-storage-tests.rs'
if not src.exists():
 s=(old/'native-profile-original-candidate-tests.rs').read_text();s=s.replace('/home/dancxjo/conduit-4898-common-greeting/architecture/plot/tests/common/allocation_probe.rs',str(w/'allocation_probe.rs'));src.write_text(s)
 p=Path('/home/dancxjo/conduit-4898-common-greeting/architecture/plot/tests/common/allocation_probe.rs');s=p.read_text().replace('pub allocations: usize,','pub allocations: usize,\n    pub requested_bytes: usize,').replace('allocations: 0, reallocations: 0,','allocations: 0, requested_bytes: 0, reallocations: 0,').replace('let mut state = observation.get();','let mut state = observation.get();\n                state.requested_bytes += added;');(w/'allocation_probe.rs').write_text(s)
manifest=json.loads((w/'coherent-sdk-rlibs.json').read_text());args=['rustc','+stable','--edition=2024','--test',str(src),'--cfg','feature="kernel-operation-owners"','-L','dependency='+str(repo/'work/pair-storage-target/debug/deps'),'--extern','conduit_ai='+str(w/'numeric-owner/libconduit_ai_numeric_union.rlib')]
for a in manifest['artifacts'].values():
 if a['crate']!='conduit_ai':args+=['--extern',a['crate']+'='+a['path']]
args+=['-C','debuginfo=0','-o',str(w/'numeric-storage-tests')]
(w/'numeric-tests-command.json').write_text(json.dumps(args,indent=2))
with(w/'numeric-tests-build.log').open('w')as log:r=subprocess.run(args,stdout=log,stderr=subprocess.STDOUT)
print('exit',r.returncode);raise SystemExit(r.returncode)
