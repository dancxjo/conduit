"""Compile current Source/RP build-helper API only. No Make or target build."""
from pathlib import Path
import json,subprocess,os,shutil
repo=Path.cwd();b=repo/'target/green-workspace-qualification';out=b/'on';deps=out/'deps'
root=json.loads((out/'commands.json').read_text())[-1]['command'];extern=[]
for i,a in enumerate(root):
 if a=='--extern':extern+=root[i:i+2]
def build(name,source,manifest,library=True,features=()):
 assert sum(p.stat().st_size for p in b.rglob('*') if p.is_file() and not p.is_symlink())<600*1048576
 assert shutil.disk_usage(repo).free>3*1024**3
 dest=deps/(('lib'+name+'-rp_component.rlib') if library else name+'-rp_component')
 cmd=['rustc','+1.98.1','--edition=2021','--crate-name',name,'--crate-type=rlib' if library else '--crate-type=bin',str(source),'-C','debuginfo=0','-C','link-arg=-Wl,--strip-debug','-L','dependency='+str(deps),'-L','dependency='+str(repo/'target/debug/deps'),'-o',str(dest)]+extern
 for feature in features:cmd+=['--cfg','feature="'+feature+'"']
 env=dict(os.environ,CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CARGO_MANIFEST_DIR=str(manifest),CARGO_PKG_NAME=name.replace('_','-'),CARGO_PKG_VERSION='0.1.0')
 log=out/(name+'-rp-component.log')
 with log.open('w') as f:r=subprocess.run(cmd,env=env,stdout=f,stderr=subprocess.STDOUT)
 (out/(name+'-rp-command.json')).write_text(json.dumps(cmd,indent=2))
 print(name,r.returncode,flush=True)
 if r.returncode:print(log.read_text()[-6000:]);raise SystemExit(r.returncode)
 extern.extend(['--extern',name+'='+str(dest)])
 return dest
build('conduit_mcu_source_build',repo/'targets/mcu/source-build/src/lib.rs',repo/'targets/mcu/source-build',features=['execution-workspace'])
build('conduit_host_rp2040',repo/'targets/rp2040/make/src/lib.rs',repo/'targets/rp2040/make')
h=out/'i2c-build-helper.rs';h.write_text('#[path='+json.dumps(str(repo/'targets/rp2040/firmware/host/i2c_workset_build.rs'))+'] mod actual_i2c_build;\nfn main() {}\n')
build('green_i2c_build_helper',h,repo/'targets/rp2040/firmware/host',library=False,features=['execution-workspace'])
