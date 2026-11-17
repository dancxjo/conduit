"""Original owning component tests only; manual rustc, no firmware claim."""
from pathlib import Path
import json,subprocess,os,shutil
repo=Path.cwd();base=repo/'target/green-workspace-qualification';out=base/'on';deps=out/'deps'
records=json.loads((out/'commands.json').read_text());root=records[-1]['command'];extern=[]
for i,x in enumerate(root):
 if x=='--extern':extern+=root[i:i+2]
common=['rustc','+1.98.1','--edition=2021','--test','-C','debuginfo=0','-C','link-arg=-Wl,--strip-debug','-L','dependency='+str(deps),'-L','dependency='+str(repo/'target/debug/deps')]
def compile_test(name,source,manifest,features):
 used=sum(p.stat().st_size for p in base.rglob('*') if p.is_file() and not p.is_symlink());assert used<600*1048576
 assert shutil.disk_usage(repo).free>3*1024**3
 exe=deps/(name+'-owning_on');cmd=common+['--crate-name',name,str(source),'-o',str(exe)]+extern
 for f in features:cmd+=['--cfg','feature="'+f+'"']
 env=dict(os.environ,CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CARGO_MANIFEST_DIR=str(manifest),CARGO_PKG_NAME=name.replace('_','-'),CARGO_PKG_VERSION='0.1.0')
 log=out/(name+'-owning-compile.log')
 with log.open('w') as f:r=subprocess.run(cmd,env=env,stdout=f,stderr=subprocess.STDOUT)
 (out/(name+'-owning-command.json')).write_text(json.dumps(cmd,indent=2))
 if r.returncode:print(log.read_text()[-6000:]);raise SystemExit(r.returncode)
 return exe
for n,path,features,filters in [
 ('conduit_kernel','architecture/kernel',['alloc','default','step-test-support','execution-workspace'],['scheduler::execution_workspace::tests::']),
 ('conduit_mcu_host_runtime','targets/mcu/host-runtime',['execution','execution-workspace'],['execution::execution_workspace::tests::']),
]:
 print('compile owning',n,flush=True);exe=compile_test(n,repo/path/'src/lib.rs',repo/path,features)
 for filter in filters:
  log=out/(n+'-owning-tests.log')
  with log.open('w') as f:r=subprocess.run([str(exe),filter,'--nocapture'],stdout=f,stderr=subprocess.STDOUT)
  print(n,'test-exit',r.returncode,flush=True)
  if r.returncode:print(log.read_text()[-6000:]);raise SystemExit(r.returncode)
# Exact zero-inventory source + original tests, through current generator API.
source=out/'empty-inventory-owning.rs';source.write_text('pub use conduit_embedded_build::{generate_resident_library,ResidentExpressionRefusal};\n#[path='+json.dumps(str(repo/'make/embedded-build/src/resident_library_empty_tests.rs'))+'] mod empty;\n')
exe=compile_test('green_empty_inventory',source,repo/'make/embedded-build',['execution-workspace'])
with (out/'empty-inventory-tests.log').open('w') as f:r=subprocess.run([str(exe),'--nocapture'],stdout=f,stderr=subprocess.STDOUT)
print('empty-inventory test-exit',r.returncode,flush=True)
if r.returncode:raise SystemExit(r.returncode)
# Recompile only Source test owner after adding DHT parity; dependencies unchanged.
print('recompile Source test owner',flush=True)
cmd=root.copy();cmd[cmd.index('-o')+1]=str(deps/'conduit_mcu_source_build-component_on')
if 'link-arg=-Wl,--strip-debug' not in cmd:cmd+=['-C','link-arg=-Wl,--strip-debug']
env=dict(os.environ,CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CARGO_MANIFEST_DIR=str(repo/'targets/mcu/source-build'),CARGO_PKG_NAME='conduit-mcu-source-build',CARGO_PKG_VERSION='0.1.0')
with (out/'source-dht-parity-compile.log').open('w') as f:r=subprocess.run(cmd,env=env,stdout=f,stderr=subprocess.STDOUT)
if r.returncode:raise SystemExit(r.returncode)
with (out/'source-dht-parity-tests.log').open('w') as f:r=subprocess.run([cmd[cmd.index('-o')+1],'original_dht_owned_inventory_reports_no_workspace_claim','--nocapture'],stdout=f,stderr=subprocess.STDOUT)
print('DHT parity test-exit',r.returncode,flush=True)
raise SystemExit(r.returncode)
