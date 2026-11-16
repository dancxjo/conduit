"""Bounded fresh CPU Root, original whole Source and admitted execution; no hardware.

Uses original Make generated workset data and unchanged production Source
composition modules. --fingerprint must name a completed Cargo test graph.
"""
import argparse,hashlib,json,os,re,subprocess
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__)
p.add_argument('--compile-only',action='store_true')
p.add_argument('--generated',type=Path,required=True)
p.add_argument('--fingerprint',type=Path,required=True)
p.add_argument('--output',type=Path,required=True)
a=p.parse_args();repo=Path(__file__).resolve().parents[5];a.output.mkdir(parents=True,exist_ok=True)
cache=a.fingerprint.parents[2];deps=cache/'deps';root=cache/'.fingerprint'
selected={}
selected_features={}
def resolve(name,value):
 expected=int(value).to_bytes(8,'little').hex();found=[]
 for folder in root.iterdir():
  marker=folder/f'lib-{name}'
  if marker.is_file() and marker.read_text().strip()==expected:
   lib=deps/f'lib{name}-{folder.name.rsplit("-",1)[1]}.rlib'
   manifests=list(folder.glob(f'lib-{name}.json'))
   if lib.is_file() and len(manifests)==1:found.append((lib,manifests[0]))
 if not found:return
 assert len(found)==1,(name,found)
 lib,manifest=found[0]
 if name in selected:
  assert selected[name]==lib,(name,selected[name],lib)
  return
 selected[name]=lib
 metadata=json.loads(manifest.read_text())
 selected_features[name]=metadata.get('features')
 for entry in metadata['deps']:
  if not entry[2]:resolve(entry[1],entry[3])
for entry in json.loads(a.fingerprint.read_text())['deps']:
 if not entry[2]:resolve(entry[1],entry[3])
if os.environ.get('CPU_RUN_SOURCE'):
 assert 'execution-workspace' not in selected_features['conduit_mcu_host_runtime'],selected_features
 assert 'execution-workspace' not in selected_features['conduit_kernel'],selected_features
# Read-only generated inputs; production Source modules come from THIS frozen
# checkout, not mutable reconstructed template copies alongside those inputs.
generated=a.output/'generated';generated.mkdir(exist_ok=True)
for src in a.generated.iterdir():
 if src.is_file() and src.suffix=='.rs':
  dst=generated/src.name
  if dst.exists() or dst.is_symlink():dst.unlink()
  if src.name.startswith('source_'):
   name=src.name.removeprefix('source_');original=repo/'targets/mcu/source-build/src/source_composition'/name
   assert original.is_file(),original
   dst.symlink_to(original)
  else:dst.symlink_to(src.resolve())
# Also emit newly registered Source siblings absent from frozen Make inputs.
for original in (repo/'targets/mcu/source-build/src/source_composition').glob('*.rs'):
 dst=generated/('source_'+original.name)
 if dst.exists() or dst.is_symlink():dst.unlink()
 dst.symlink_to(original)
env=dict(os.environ,OUT_DIR=str(generated.resolve()),CPU_BOARD_SOURCE=str(repo/'targets/esp32/firmware/host/src'))
command=['rustc','--edition=2021','--cfg','fore_carrier','--cfg','record_service','--cfg','assigned_workset','-L',f'dependency={deps}','--crate-name','a422_cpu_factory',str(Path(__file__).with_name('probe.rs')),'-o',str(a.output/'probe')]
for name,lib in sorted(selected.items()):command+=['--extern',f'{name}={lib}']
with (a.output/'compile.log').open('w') as log:result=subprocess.run(command,env=env,stdout=log,stderr=subprocess.STDOUT)
print(json.dumps({'phase':'compile','exit':result.returncode,'log':str(a.output/'compile.log')}),flush=True)
if result.returncode:raise SystemExit(result.returncode)
if a.compile_only:raise SystemExit(0)
with (a.output/'run.log').open('w') as log:result=subprocess.run([str(a.output/'probe')],env=env,stdout=log,stderr=subprocess.STDOUT)
print(json.dumps({'phase':'cpu-factory','exit':result.returncode,'log':str(a.output/'run.log')}),flush=True)
digest=lambda path:hashlib.sha256(path.read_bytes()).hexdigest()
trace=(a.output/'run.log').read_text()
started='CPU_FACTORY_STARTED' in trace
entered='original_complete_factory_called=true' in trace
effects='CPU_SOURCE_HISTORY' in trace
receipt={
 'scope':'Fresh public Root signed three-fragment ingress and unchanged complete factory; conditional original renderer/admission/Start',
 'fresh_public_root_admission':'CPU_AUTHORITY_GATE public_root_spawn=true' in (a.output/'run.log').read_text(),
 'source_commit':subprocess.check_output(['git','rev-parse','HEAD'],cwd=repo,text=True).strip(),
 'fingerprint':str(a.fingerprint),'fingerprint_sha256':digest(a.fingerprint),
 'production_templates':{p.name:{'path':str(p.resolve()),'sha256':digest(p)} for p in generated.glob('source_*.rs')},
 'generated_inputs':{p.name:{'path':str(p.resolve()),'sha256':digest(p)} for p in generated.glob('*.rs') if not p.name.startswith('source_')},
 'library_paths':{name:str(lib) for name,lib in selected.items()},
 'library_features':selected_features,
 'fixture_sources_sha256':{p.name:digest(p) for p in sorted(Path(__file__).parent.glob('*.rs'))},
 'constructor_exit':result.returncode,'compile_log_sha256':digest(a.output/'compile.log'),'run_log_sha256':digest(a.output/'run.log'),
 'whole_factory_called':entered,'preparation_result':'prepared' if started else 'sampling-kernel/Tables(Layout)' if result.returncode==0 else 'failed; inspect log','admission':started,'Start':started,'hardware_effects':False,'original_Source_execution_completed':effects,
 'not_proved':([] if started else ['Successful complete Source preparation','Renderer preparation and whole admission/Start'])+([] if effects else ['Provider effects or continuing Play'])+['Target stack/heap/ABI fit','Physical device execution'],
}
(a.output/'receipt.json').write_text(json.dumps(receipt,indent=2)+'\n')
raise SystemExit(result.returncode)
