import os,subprocess,json,re,hashlib
from pathlib import Path
root=Path(__file__).resolve().parent.parent;repo=root/'work/environment-extraction-integration-20261111'
cache=root/'work/cache-backing/rp2040-source-fit-20261113'
outputs=list(cache.glob('thumbv6m-none-eabi/release/build/conduit-rp2040-host-*/out/workset_0_layout.rs'))
selected=[p.parent for p in outputs if 'pub const ABNORMAL_CORD_COUNT:usize=0;' in p.read_text() and 'pub const EXTERNAL_INGRESS_CORD_COUNT:usize=3;' in p.read_text()];assert len(selected)==1
out=selected[0];logpath=root/'outputs/positive-cord-terminal-factory-20261115.log'
temporary=root/'work/source-proof-tmp-20261115';temporary.mkdir(exist_ok=True)
env=dict(os.environ,TMPDIR=str(temporary),CARGO_TARGET_DIR='/tmp/conduit-source-member-control-20261111',CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CARGO_BUILD_JOBS='4',CONDUIT_NEUTRAL_CPU_OUT_DIR=str(out))
env.pop('CONDUIT_NEUTRAL_DIAGNOSE_KERNEL',None)
cmd=['cargo','+1.98.1','test','--locked','-p','conduit-host-rp2040','--test','neutral_source','--','--ignored','--nocapture']
with logpath.open('w') as log:result=subprocess.run(cmd,cwd=repo,env=env,stdout=log,stderr=subprocess.STDOUT)
text=logpath.read_text();measurements={k:int(v) for k,v in re.findall(r'(cpu_positive_[a-z_]+|cpu_inline_[a-z_]+|original_heap_bytes)=(\d+)',text)}
files=['targets/rp2040/firmware/host/src/neutral_factory.rs','targets/rp2040/firmware/host/src/neutral_workset.rs','targets/rp2040/make/tests/neutral_source.rs','targets/rp2040/make/tests/neutral_source/exercise.rs','targets/rp2040/make/tests/neutral_source/positive.rs']
receipt={'assurance':'Original complete factory, Root preflight and no_std modules, CPU64 requested allocation proof only; no target runtime fit or Start','command':cmd,'exit':result.returncode,'source_checkpoint':subprocess.check_output(['git','rev-parse','HEAD'],cwd=repo,text=True).strip(),'files':{p:hashlib.sha256((repo/p).read_bytes()).hexdigest() for p in files},'selected_generated_out':str(out),'measurements':measurements,'log_sha256':hashlib.sha256(logpath.read_bytes()).hexdigest(),'diagnostic_factory_copy':False,'admission':False,'Start':False,'physical_deployment':False,'target_heap_fit_proved':False,'heap_grant_bytes':131072,'selected_cord_terminal_stock':3,'scope':'76Gears95Cords97queue slots27software47resident2nativeowners; original30314-byteassignment','exclusions':['preexisting Root/equipment/residents/issuer grants','allocator overhead/rounding','stack']}
if result.returncode==0:
 assert 'factory_diagnostic_copy=false' in text
 assert 'test result: ok. 1 passed;' in text
 assert 'all loans returned, zero native calls' in text
 old=json.loads((root/'outputs/positive-unresolved-factory-20261115.json').read_text())['measurements']
 receipt['incremental_CPU64_retained_saving_bytes']=old['cpu_positive_whole_factory_retained_requested_bytes']-measurements['cpu_positive_whole_factory_retained_requested_bytes']
 receipt['combined_CPU64_retained_saving_from_original_bytes']=210354-measurements['cpu_positive_whole_factory_retained_requested_bytes']
(root/'outputs/positive-cord-terminal-factory-20261115.json').write_text(json.dumps(receipt,indent=2)+'\n');print(json.dumps(receipt),flush=True)
raise SystemExit(result.returncode)
