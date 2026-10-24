import pathlib,subprocess,json,os,time,hashlib,re,shutil
ROOT=pathlib.Path.cwd();W=ROOT/'work/6402-native-reference-fixture-repair';T=ROOT/'work/6402-native-reference-target';TMP=ROOT/'work/6402-native-reference-tmp';TMP.mkdir(exist_ok=True);S=subprocess.check_output(['git','-C',str(W),'rev-parse','HEAD'],text=True).strip();assert S.startswith('5ea707e47')and not subprocess.check_output(['git','-C',str(W),'status','--porcelain']);env=dict(os.environ,CARGO_TARGET_DIR=str(T),TMPDIR=str(TMP),CARGO_BUILD_JOBS='2',CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CONDUIT_NATIVE_REFERENCE_PROOF_RETAIN='1');r={'source':S,'base':'567a4816dcf919dd2c049505494ae604f60d90b1','tree':subprocess.check_output(['git','-C',str(W),'rev-parse','HEAD^{tree}'],text=True).strip(),'started':time.time(),'steps':[],'environment':{k:env[k]for k in ['CARGO_TARGET_DIR','TMPDIR','CARGO_BUILD_JOBS','CARGO_INCREMENTAL','CARGO_PROFILE_DEV_DEBUG','CARGO_PROFILE_TEST_DEBUG','CONDUIT_NATIVE_REFERENCE_PROOF_RETAIN']}};receipt=ROOT/'outputs/6402-native-reference-fixture-local-proof.json'
for name,command in [('generated-reference-test',['cargo','+stable','test','--locked','-p','conduit-data','--test','native_reference_bindings','--','--nocapture','--test-threads=1']),('scoped-strict',['cargo','+stable','clippy','--locked','-p','conduit-data','--test','native_reference_bindings','--','-D','warnings'])]:
 free=shutil.disk_usage(ROOT).free;assert free>=7*1024**3;log=ROOT/f'work/6402-{name}.log';print('START',name,free,flush=True)
 with log.open('wb')as f:p=subprocess.run(command,cwd=W,env=env,stdout=f,stderr=subprocess.STDOUT)
 b=log.read_bytes();r['steps'].append({'name':name,'command':command,'exit_code':p.returncode,'before_available_bytes':free,'log':str(log),'sha256':hashlib.sha256(b).hexdigest(),'actual_passed_counts':list(map(int,re.findall(rb'test result: ok\. (\d+) passed; 0 failed; 0 ignored;',b))),'actual_Running':re.findall(r'^\s*Running .*?\(([^)]+)\)\s*$',b.decode(),re.M)});r['status']='running'if p.returncode==0 else'first-negative-preserved';receipt.write_text(json.dumps(r,indent=2)+'\n');print(name,p.returncode,flush=True)
 if p.returncode:raise SystemExit(p.returncode)
r.update(status='passed',finished=time.time(),clean_after=not subprocess.check_output(['git','-C',str(W),'status','--porcelain']),executables=[])
for root in [T,TMP]:
 for p in root.rglob('*'):
  if p.is_file()and os.access(p,os.X_OK):
   with p.open('rb')as f:magic=f.read(4)
   if magic==b'\x7fELF':r['executables'].append({'path':str(p),'bytes':p.stat().st_size,'sha256':hashlib.sha256(p.read_bytes()).hexdigest()})
receipt.write_text(json.dumps(r,indent=2)+'\n');assert r['clean_after'];print('PASS',len(r['executables']),flush=True)
