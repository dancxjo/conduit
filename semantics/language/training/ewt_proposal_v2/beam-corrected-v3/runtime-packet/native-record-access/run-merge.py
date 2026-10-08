import pathlib,json,hashlib,subprocess,os,time
root=pathlib.Path.cwd();directory=root/'work/native-record-recipe';out=directory/'checked-out';hash=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
graph=json.loads((directory/'language-compile-receipt.json').read_text());assert graph['compile_exit']==0 and graph['inputs_unchanged'] and graph['cached_payloads_unchanged']
libs={}
for p in graph['inputs']:
 name=pathlib.Path(p).name
 for crate in ['conduit_core','conduit_plot']:
  if name.startswith('lib'+crate+'-'):libs[crate]=root/p
libs['conduit_language']=directory/'libconduit_language.rlib'
inputs=dict(graph['inputs']);inputs.update({str(p.relative_to(root)):hash(p) for p in out.iterdir() if p.is_file()})
inputs[str((directory/'libconduit_language.rlib').relative_to(root))]=graph['language_rlib_sha256']
original_manifest=json.loads((root/'work/native-child-regeneration/graft-manifest-final.json').read_text())
source_paths={}
for original, expected in original_manifest['original_selected_inputs'].items():
 if original.endswith('.conduit'):
  relative=original.split('/conduit-4907-production-custody/')[1]
  path=root/relative
  if not path.exists():path=pathlib.Path(original)
  assert hash(path)==expected,(str(path),'original Source mismatch')
  source_paths[str(path)]=expected
inputs.update(source_paths)

fixture=root/'work/coherent-merged-beam/fixture.rs';inputs[str(fixture.relative_to(root))]=hash(fixture)
for p in (root/'work/native-child-rank').glob('epoch-*.bin'):inputs[str(p.relative_to(root))]=hash(p)
cmd=['rustc','+stable','--edition=2021',str(fixture),'-L','dependency='+str(root/'work/native-reuse-sdk-target/debug/deps'),'-C','debuginfo=0','-C','link-arg=-Wl,--strip-debug','-o',str(directory/'merge-fixture')]
for name,path in libs.items():cmd+=['--extern',name+'='+str(path)]
start=time.monotonic()
with open(directory/'merge-compile.log','w') as log:r=subprocess.run(cmd,stdout=log,stderr=subprocess.STDOUT)
receipt={'compile_exit':r.returncode,'compile_command':cmd,'inputs':inputs}
if r.returncode==0:
 receipt['elf_sha256']=hash(directory/'merge-fixture')
 with open(directory/'merge-runtime.log','w') as log:r=subprocess.run([str(directory/'merge-fixture'),str(out)],stdout=log,stderr=subprocess.STDOUT)
 receipt['runtime_exit']=r.returncode
receipt['seconds']=time.monotonic()-start;receipt['inputs_unchanged']=all(hash(root/p)==sha for p,sha in inputs.items())
(directory/'merge-result.json').write_text(json.dumps(receipt,indent=2)+'\n');print({k:v for k,v in receipt.items() if k not in ['inputs','compile_command']},flush=True)
