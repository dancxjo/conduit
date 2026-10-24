from pathlib import Path
import subprocess,os,json,time,hashlib,sys
root=Path('/home/dancxjo/Documents/Codex/2026-10-21/please-finish-and-close-any-conduit');w=root/'work/6418-optional-custody-pressure';phase=sys.argv[1];target=root/'work/6418-optional-custody-target';log=root/f'outputs/6418-{phase}.log';receipt=root/f'outputs/6418-{phase}.json';assert not log.exists() and not receipt.exists();assert not subprocess.check_output(['git','-C',str(w),'status','--porcelain']).strip();head=subprocess.check_output(['git','-C',str(w),'rev-parse','HEAD'],text=True).strip();free=os.statvfs(root).f_bavail*os.statvfs(root).f_frsize;assert free>=7*1024**3
if phase=='negative':argv=['cargo','+stable','test','--locked','-p','conduit-plot','--lib','custody_pressure_tests','--','--nocapture','--test-threads=1']
elif phase=='green':argv=['cargo','+stable','test','--locked','-p','conduit-plot','--lib','custody_pressure_tests','--','--nocapture','--test-threads=1']
elif phase=='strict':argv=['cargo','+stable','clippy','--locked','-p','conduit-plot','--lib','--tests','--','-D','warnings']
elif phase=='integration':argv=['cargo','+stable','test','--locked','-p','conduit-plot','--test','native_law_custody','--test','native_law_packages','--test','native_law_families','--','--nocapture','--test-threads=1']
else:raise ValueError(phase)
env=os.environ.copy();env.update(CARGO_TARGET_DIR=str(target),CARGO_BUILD_JOBS='2',CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0');started=time.time()
with log.open('wb') as f:p=subprocess.Popen(argv,cwd=w,env=env,stdout=f,stderr=subprocess.STDOUT);pid=p.pid;code=p.wait()
r={'head':head,'phase':phase,'argv':argv,'pid':pid,'exit':code,'seconds':time.time()-started,'root_free_before':free,'log':str(log),'log_bytes':log.stat().st_size,'log_sha256':hashlib.sha256(log.read_bytes()).hexdigest(),'source_clean_after':not subprocess.check_output(['git','-C',str(w),'status','--porcelain']).strip(),'environment':{k:env[k] for k in ['CARGO_TARGET_DIR','CARGO_BUILD_JOBS','CARGO_INCREMENTAL','CARGO_PROFILE_DEV_DEBUG','CARGO_PROFILE_TEST_DEBUG']},'actual_executables':[]}
for p in (target/'debug/deps').glob('*'):
 if p.is_file() and p.open('rb').read(4)==b'\x7fELF' and (p.name.startswith('conduit_plot-') or p.name.startswith('native_law_')):r['actual_executables'].append({'path':str(p),'bytes':p.stat().st_size,'sha256':hashlib.sha256(p.read_bytes()).hexdigest()})
receipt.write_text(json.dumps(r,indent=2)+'\n');print(json.dumps({'receipt':str(receipt),'exit':code,'head':head}),flush=True)
