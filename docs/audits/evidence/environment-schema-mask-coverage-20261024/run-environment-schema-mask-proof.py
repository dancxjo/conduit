from pathlib import Path
import subprocess,os,sys,json,hashlib,time,re
root=Path('/home/dancxjo/Documents/Codex/2026-10-21/please-finish-and-close-any-conduit');w=root/'work/6418-optional-custody-pressure';target=root/'work/6418-optional-custody-target';tmp=root/'work/environment-schema-mask-tmp';tmp.mkdir(exist_ok=True);phase=sys.argv[1];log=root/f'outputs/environment-schema-mask-{phase}.log';receipt=root/f'outputs/environment-schema-mask-{phase}.json';assert not log.exists() and not receipt.exists();assert not subprocess.check_output(['git','-C',str(w),'status','--porcelain']).strip();h=subprocess.check_output(['git','-C',str(w),'rev-parse','HEAD'],text=True).strip();free=os.statvfs(root).f_bavail*os.statvfs(root).f_frsize;assert free>=7*1024**3
env=os.environ.copy();env.update(CARGO_TARGET_DIR=str(target),CARGO_BUILD_JOBS='2',CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',TMPDIR=str(tmp));start=time.time();commands=[]
if phase=='core-full':commands=[['cargo','+stable','test','--locked','-p','conduit-core','--lib','--','--nocapture','--test-threads=1']]
elif phase=='core':commands=[['cargo','+stable','test','--locked','-p','conduit-core','--lib','structured_info','--','--nocapture','--test-threads=1']]
elif phase=='helper':commands=[['rustc','+stable','--edition=2021','-D','warnings','--crate-name','environment_schema_mask_helper',str(root/'work/environment-schema-mask-helper.rs'),'-o',str(root/'work/environment-schema-mask-helper')],[str(root/'work/environment-schema-mask-helper'),str(w)]]
elif phase=='strict-core':commands=[['cargo','+stable','clippy','--locked','-p','conduit-core','--lib','--tests','--','-D','warnings']]
elif phase=='strict-helper':commands=[['rustup','run','stable','clippy-driver','--edition=2021','--emit=metadata','--crate-name','environment_schema_mask_helper',str(root/'work/environment-schema-mask-helper.rs'),'-o',str(tmp/'environment-schema-mask-helper.rmeta'),'-D','warnings']]
else:raise ValueError(phase)
steps=[]
with log.open('wb') as f:
 for argv in commands:
  p=subprocess.Popen(argv,cwd=w,env=env,stdout=f,stderr=subprocess.STDOUT);code=p.wait();steps.append({'argv':argv,'pid':p.pid,'exit':code})
  if code!=0:break
raw=log.read_bytes();r={'phase':phase,'head':h,'commands':steps,'exit':code,'seconds':time.time()-start,'root_free_before':free,'source_clean_after':not subprocess.check_output(['git','-C',str(w),'status','--porcelain']).strip(),'environment':{k:env[k] for k in ['CARGO_TARGET_DIR','CARGO_BUILD_JOBS','CARGO_INCREMENTAL','CARGO_PROFILE_DEV_DEBUG','CARGO_PROFILE_TEST_DEBUG','TMPDIR']},'log':str(log),'log_bytes':len(raw),'log_sha256':hashlib.sha256(raw).hexdigest(),'actual_executables':[],'test_results':[s for s in raw.decode(errors='replace').splitlines() if 'test result:' in s]}
for s in raw.decode(errors='replace').splitlines():
 if 'Running ' not in s or '(' not in s or ')' not in s:continue
 name=s.rsplit('(',1)[1].split(')',1)[0];p=Path(name)
 if not p.is_absolute():p=w/p
 if p.is_file():
  b=p.read_bytes()
  if b[:4]==b'\x7fELF':r['actual_executables'].append({'path':str(p),'bytes':len(b),'sha256':hashlib.sha256(b).hexdigest()})
if phase=='helper' and code==0:
 s=raw.decode();r['meaningful_feature_guard']={'new_fifth_custody_case_passed':'test one_retained_face_resolves_pages_and_typed_frames_without_new_source_events ... ok' in s,'exactly_five_custody_tests':'running 5 tests' in s,'both_features_enabled':'face-projection,display-mask' in s,'standalone_mask_invoked':'plots/seven-segment-mask/Cargo.toml' in s};assert all(r['meaningful_feature_guard'].values()),r['meaningful_feature_guard']
receipt.write_text(json.dumps(r,indent=2)+'\n');print(json.dumps({'phase':phase,'exit':code,'head':h,'receipt':str(receipt),'tests':r['test_results']}),flush=True)
