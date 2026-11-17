"""Bounded manual rustc component proof, NOT Cargo/Make/firmware qualification.
Reuses only this lane's Cargo-built immutable dependency/form artifacts. The
local resolver receipts pin actual rlib bytes for the emitted-code test helper;
they are manual qualification manifests, not Cargo-produced fingerprints.
"""
import json, os, re, subprocess, hashlib, shutil, tomllib, sys
from pathlib import Path
repo=Path.cwd(); base=repo/'target/green-workspace-qualification'
graph=json.loads((base/'own-graph.json').read_text()); olddeps=repo/'target/debug/deps'
mode=sys.argv[1]; out=base/mode; deps=out/'deps'; fps=out/'.fingerprint'; deps.mkdir(parents=True,exist_ok=True);fps.mkdir(exist_ok=True)
changes={'conduit_embedded_build'} if mode=='off' else {'conduit_kernel','conduit_embedded_build'}
while True:
 expanded=changes|{n for n,(_,j) in graph.items() if any(d[1] in changes for d in j['deps'] if not d[2])}
 if expanded==changes:break
 changes=expanded
# Feature-off only changes generator and test owner; no dependent API changed.
if mode=='off':changes={'conduit_embedded_build'}
selected={n:(repo/lib).resolve() for n,(lib,_) in graph.items()}; records=[]; done=set()
feature_owners={'conduit_kernel','conduit_composite','conduit_semantic_catalog','conduit_embedded_build','conduit_mcu_host_runtime'}
def budget():
 used=sum(p.stat().st_size for p in base.rglob('*') if p.is_file() and not p.is_symlink())
 assert used<600*1048576,('600MiB component cap',used)
 assert shutil.disk_usage(repo).free>3*1024**3,'3GiB host disk floor'
def metadata(name):
 lib,j=graph[name]; suffix=Path(lib).name.removeprefix('lib').removesuffix('.rlib')
 text=(olddeps/(suffix+'.d')).read_text()
 first=text.split(': ',1)[1].splitlines()[0].split()[0]
 source=(repo/first).resolve(); package=source.parent
 while not (package/'Cargo.toml').is_file():package=package.parent
 cargo=tomllib.loads((package/'Cargo.toml').read_text())['package']; env=dict(os.environ,CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CARGO_MANIFEST_DIR=str(package),CARGO_PKG_NAME=cargo['name'],CARGO_PKG_VERSION=cargo['version'])
 for key,value in re.findall(r'^# env-dep:([^=]+)=(.*)$',text,re.M):env[key]=value
 cfg=[]
 if 'OUT_DIR' in env:
  od=Path(env['OUT_DIR']); output=od.parent/'output'
  assert od.is_dir()
  # All reused Forms/schemas are produced by unchanged Core/Plot authoring,
  # not by changed resident-library generation or execution feature selection.
  build=package/'build.rs'; assert build.is_file()
  body=build.read_text(); assert 'conduit_embedded_build' not in body and 'execution-workspace' not in body
  record={'package':name,'out_dir':str(od),'build_rs_sha256':hashlib.sha256(build.read_bytes()).hexdigest(),'files':{str(p.relative_to(od)):hashlib.sha256(p.read_bytes()).hexdigest() for p in od.rglob('*') if p.is_file()}}
  if output.exists():
   for line in output.read_text().splitlines():
    if line.startswith('cargo:rustc-cfg='):cfg+=['--cfg',line.split('=',1)[1]]
    if line.startswith('cargo:rustc-env='):
     k,v=line.split('=',1)[1].split('=',1);env[k]=v
  records.append({'reused_immutable_forms':record})
 return source,package,cargo,env,cfg

def compile_crate(name,test=False):
 budget();source,package,cargo,env,cfg=metadata(name)
 features=json.loads(graph[name][1]['features'])
 if mode=='on' and name in feature_owners:features=sorted(set(features+['execution-workspace']))
 suffix='component_'+mode
 dest=deps/(name+'-'+suffix if test else 'lib'+name+'-'+suffix+'.rlib')
 cmd=['rustc','+1.98.1','--edition=2021','--crate-name',name,'--test' if test else '--crate-type=rlib',str(source),'-C','debuginfo=0','-C','metadata=green_component_'+mode+'_'+name,'-L','dependency='+str(deps),'-L','dependency='+str(olddeps),'-o',str(dest)]+cfg
 for feature in features:cmd+=['--cfg','feature="'+feature+'"']
 for n,path in selected.items():
  if n!=name:cmd+=['--extern',n+'='+str(path)]
 log=out/(name+('-test' if test else '')+'.log')
 print('compile',mode,name,'test' if test else 'lib',flush=True)
 with log.open('w') as stream:result=subprocess.run(cmd,cwd=repo,env=env,stdout=stream,stderr=subprocess.STDOUT)
 records.append({'package':name,'test':test,'features':features,'command':cmd,'log':str(log),'exit':result.returncode})
 (out/'commands.json').write_text(json.dumps(records,indent=2))
 if result.returncode:
  print(log.read_text()[-6000:]);raise SystemExit(result.returncode)
 if not test:selected[name]=dest
 return dest
while changes-done:
 ready=[n for n in changes-done if not any(d[1] in changes-done for d in graph[n][1]['deps'] if not d[2])]
 assert ready,'dependency cycle'
 for name in sorted(ready):compile_crate(name);done.add(name)
# The existing emitted-code helper resolves exact byte-pinned rlibs by these
# local manual receipts. No global Cargo cache/fingerprint is modified.
def budget(); manual_receipts(owner,exe):
 rows=[]
 for i,(name,path) in enumerate(sorted(selected.items())):
  key=hashlib.sha256(path.read_bytes()).digest()[:8]; tag=key.hex(); folder=fps/(name.replace('_','-')+'-'+tag);folder.mkdir(exist_ok=True)
  (folder/('lib-'+name)).write_text(tag)
  link=deps/('lib'+name+'-'+tag+'.rlib')
  if not link.exists():link.symlink_to(path)
  rows.append([i,name,False,int.from_bytes(key,'little')])
 folder=fps/(owner.replace('_','-')+'-'+exe.name.rsplit('-',1)[1]);folder.mkdir(exist_ok=True)
 (folder/('test-lib-'+owner+'.json')).write_text(json.dumps({'kind':'manual-rustc-component-byte-pins-not-Cargo','deps':rows}))
# Test owner is intentionally compiled from the current changed Source.
owner='conduit_mcu_source_build'
source=repo/'targets/mcu/source-build/src/lib.rs';exe=deps/(owner+'-component_'+mode)
env=dict(os.environ,CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CARGO_MANIFEST_DIR=str(source.parent.parent),CARGO_PKG_NAME='conduit-mcu-source-build',CARGO_PKG_VERSION='0.1.0')
cmd=['rustc','+1.98.1','--edition=2021','--test','--crate-name',owner,str(source),'-C','debuginfo=0','-C','link-arg=-Wl,--strip-debug','-L','dependency='+str(deps),'-L','dependency='+str(olddeps),'-o',str(exe)]
if mode=='on':cmd+=['--cfg','feature="execution-workspace"']
for n,path in selected.items():cmd+=['--extern',n+'='+str(path)]
budget();log=out/'source-test-compile.log';print('compile Source tests',mode,flush=True)
with log.open('w') as stream:r=subprocess.run(cmd,env=env,stdout=stream,stderr=subprocess.STDOUT)
records.append({'package':owner,'test':True,'command':cmd,'exit':r.returncode,'log':str(log)})
(out/'commands.json').write_text(json.dumps(records,indent=2))
if r.returncode:print(log.read_text()[-6000:]);raise SystemExit(r.returncode)
budget(); manual_receipts(owner,exe)
print('ready',str(exe),flush=True)
