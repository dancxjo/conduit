import pathlib,json,hashlib,os,re,subprocess,time
root=pathlib.Path.cwd();old=root/'work/native-child-regeneration/scoped-out-final';out=root/'work/native-record-recipe/checked-out';out.mkdir(exist_ok=True)
manifest=json.loads((root/'work/native-child-regeneration/graft-manifest-final.json').read_text())
hash=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
for name,expected in manifest['output_files'].items():
 p=old/name
 assert hash(p)==expected,(name,'cached output mismatch')
 if name!='semantic_types.rs':
  dest=out/name
  if not dest.exists():os.link(p,dest)
  assert hash(dest)==expected
header=(old/'semantic_types.rs').read_text()
pattern=r'value\.record_field\(("[^"\n]+")\)\.map_err\(NativeBindingRefusal::InvalidValue\)\?'
new,count=re.subn(pattern,r'family.record_field(Self::PREPARED_DESCRIPTOR, value, \1)?',header)
assert count>0
(out/'semantic_types.rs').write_text(new)
deps=root/'work/native-reuse-sdk-target/debug/deps'
plot=max(deps.glob('libconduit_plot-*.rlib'),key=lambda p:p.stat().st_mtime)
metadata=subprocess.check_output(['rustc','+stable','-Z','ls=all',str(plot)],env={**os.environ,'RUSTC_BOOTSTRAP':'1'},text=True)
core_name=re.search(r'^\d+ (conduit_core-[0-9a-f]+) hash ',metadata,re.M).group(1)
libs={'conduit_plot':plot,'conduit_core':deps/('lib'+core_name+'.rlib')}
closure={}
for name in re.findall(r'^\d+ ([\w-]+) hash ',metadata,re.M):
 for extension in ['rlib','so']:
  path=deps/('lib'+name+'.'+extension)
  if path.exists():closure[str(path.relative_to(root))]=hash(path)

cmd=['rustc','+stable','--crate-name','conduit_language','--edition=2021','semantics/language/src/lib.rs','--crate-type','rlib','-C','debuginfo=0','-C','metadata=native_record_recipe_merge_4907','-L','dependency='+str(deps),'-o',str(root/'work/native-record-recipe/libconduit_language.rlib')]
for name,path in libs.items():cmd+=['--extern',name+'='+str(path)]
inputs={str(p.relative_to(root)):hash(p) for p in (root/'semantics/language/src').rglob('*.rs')}
inputs.update({str(p.relative_to(root)):hash(p) for p in libs.values()});inputs.update(closure)
receipt={'cached_manifest_sha256':hash(root/'work/native-child-regeneration/graft-manifest-final.json'),'verified_cached_files':len(manifest['output_files']),'old_header_sha256':hashlib.sha256(header.encode()).hexdigest(),'new_header_sha256':hashlib.sha256(new.encode()).hexdigest(),'mechanical_root_field_replacements':count,'command':cmd,'inputs':inputs,'source_check_claim':False}
start=time.monotonic()
with open(root/'work/native-record-recipe/language-compile.log','w') as log:
 result=subprocess.run(cmd,env={**os.environ,'OUT_DIR':str(out),'CARGO_MANIFEST_DIR':str(root/'semantics/language'),'CARGO_PKG_VERSION':'0.1.0'},stdout=log,stderr=subprocess.STDOUT)
receipt.update({'compile_exit':result.returncode,'seconds':time.monotonic()-start,'inputs_unchanged':all(hash(root/p)==sha for p,sha in inputs.items()),'cached_payloads_unchanged':all(hash(old/name)==sha and (name=='semantic_types.rs' or hash(out/name)==sha) for name,sha in manifest['output_files'].items())})
if result.returncode==0:receipt['language_rlib_sha256']=hash(root/'work/native-record-recipe/libconduit_language.rlib')
(root/'work/native-record-recipe/language-compile-receipt.json').write_text(json.dumps(receipt,indent=2)+'\n')
print({k:v for k,v in receipt.items() if k!='inputs' and k!='command'},flush=True)
