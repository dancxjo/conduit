from pathlib import Path
import os,json,time,subprocess,hashlib,shutil,re
w=Path(__file__).resolve().parent;wt=w/'6065-current-native-materialization';cache=w/'6065-build-target';out=w/'6065-actual-beam-export-fix-fdc2a582d';marker=w/'6065-build-cache-ownership.json'
assert shutil.disk_usage(w).free>12*2**30;assert json.loads(marker.read_text())['status']=='idle';cache.mkdir(exist_ok=True);out.mkdir(exist_ok=False)
paths=['semantics/language/src/lib.rs','semantics/language/build.rs','semantics/language/tests/actual_beam_prepared_decode.rs','architecture/plot/src/rust_binding/prepared_native.rs','architecture/plot/src/rust_binding/mod.rs','architecture/plot/src/rust_binding/prepared_decode.rs','architecture/plot/src/rust_binding/generate_prepared_storage.rs','architecture/plot/src/rust_binding/generate_prepared_storage/emit.rs','architecture/plot/tests/prepared_native_decode.rs']
def snapshot():return {p:hashlib.sha256((wt/p).read_bytes()).hexdigest() for p in paths}
head=subprocess.check_output(['git','rev-parse','HEAD'],cwd=wt,text=True).strip();sources=snapshot();owner={'owner':'Root/6065-current-native-materialization','status':'generic-current-emitted-Rust-test-running','pid':os.getpid(),'cache':str(cache),'base':head,'no_other_use_until_release':True};marker.write_text(json.dumps(owner,indent=2)+'\n')
env=os.environ.copy()
for k in ['RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS','CONDUITOS_BUILD_ID','CONDUITOS_IMAGE_ID','CONDUITOS_MAKE_RECORD']:env.pop(k,None)
env.update(CARGO_TARGET_DIR=str(cache),CARGO_BUILD_JOBS='2',CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0')
argv=['cargo','+stable','test','--locked','-p','conduit-language','--test','actual_beam_prepared_decode','--','--test-threads=1','--nocapture'];r={'base':head,'source_state':'isolated preparation WIP; exact nine owning source hashes, not finalcleanSourceacceptance','owning_source_sha256':sources,'argv':argv,'started':time.time(),'status':'running','proof_class':'current-generated Rust/no_std and finite Native materialization owning tests; no controller/Current authority','free_before':shutil.disk_usage(w).free};(out/'receipt.json').write_text(json.dumps(r,indent=2)+'\n')
try:
 with (out/'test.log').open('w') as f:r['exit_code']=subprocess.call(argv,cwd=wt,env=env,stdout=f,stderr=subprocess.STDOUT)
 r['owning_source_unchanged']=snapshot()==sources;r['status']='passed-owning-generic-preparation' if r['exit_code']==0 and r['owning_source_unchanged'] else 'failed'
 text=(out/'test.log').read_text();r['retained_artifacts']=[]
 for p in sorted((cache/'debug/deps').glob('actual_beam_prepared_decode-*')):
  if p.is_file() and p.read_bytes()[:4]==b'\x7fELF':
   dest=out/p.name;shutil.copyfile(p,dest);r['retained_artifacts'].append({'file':dest.name,'bytes':dest.stat().st_size,'sha256':hashlib.sha256(dest.read_bytes()).hexdigest()})
finally:
 r.update(ended=time.time(),free_after=shutil.disk_usage(w).free,log_sha256=hashlib.sha256((out/'test.log').read_bytes()).hexdigest());(out/'receipt.json').write_text(json.dumps(r,indent=2)+'\n');owner.update(status='idle',receipt=str(out/'receipt.json'),no_other_use_until_release=False);owner.pop('pid',None);marker.write_text(json.dumps(owner,indent=2)+'\n')
print(json.dumps(r))
