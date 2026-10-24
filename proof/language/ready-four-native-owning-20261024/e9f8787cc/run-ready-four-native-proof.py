import pathlib,os,json,hashlib,subprocess,time,re,shutil
root=pathlib.Path.cwd();checkout=root/'work/ready-four-combined-proof';target=root/'work/ready-four-native-target';tmp=root/'work/ready-four-native-tmp';tmp.mkdir(exist_ok=True);env=dict(os.environ,CARGO_TARGET_DIR=str(target),TMPDIR=str(tmp),CARGO_BUILD_JOBS='2',CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0')
def git(*a):return subprocess.check_output(['git',*a],cwd=checkout,text=True).strip()
def sha(b):return hashlib.sha256(b).hexdigest()
source=git('rev-parse','HEAD');assert source=='e9f8787ccbf3a0fda086a5e4d568cb7f597d34e1'and git('rev-parse','HEAD^{tree}')=='d35051b7b0b512ef82e76fd37902479a65c43a06'and not git('status','--porcelain')
protected=[]
for file,count in [('outputs/ready-four-combined-local-proof.json',138),('outputs/ready-four-supported-supplement-local-proof.json',104)]:
 proof=json.loads((root/file).read_text());assert len(proof['executables'])==count;protected+=proof['executables']
for e in protected:assert sha((root/e['path']).read_bytes())==e['sha256']
recovered=json.loads((root/'outputs/6384-recovered-provenance-independent-review.json').read_text());commands=[('plot-outer-regressions',recovered['raw_command_proof'][0]['argv'],15),('actual-beam',recovered['beam_command'],1),('beam-admission',recovered['raw_command_proof'][1]['argv'],1),('strict-libraries',recovered['strict_library_lint']['actual_argv'],0)]
paths=list(recovered['nine_owning_source_hashes'])+['architecture/plot/tests/common/generated_rust.rs','architecture/plot/tests/common/allocation_probe.rs','architecture/plot/src/rust_binding/prepared_native/traversal.rs'];sourcefiles=[]
for path in paths:
 data=(checkout/path).read_bytes();sourcefiles.append({'path':path,'blob':git('rev-parse','HEAD:'+path),'sha256':sha(data)})
r={'source':source,'tree':git('rev-parse','HEAD^{tree}'),'package_trees':{p:git('rev-parse','HEAD:'+p)for p in ['architecture/plot','semantics/language','architecture/assigned-plan']},'source_files':sourcefiles,'environment':{k:env[k]for k in ['CARGO_TARGET_DIR','TMPDIR','CARGO_BUILD_JOBS','CARGO_INCREMENTAL','CARGO_PROFILE_DEV_DEBUG','CARGO_PROFILE_TEST_DEBUG']},'started':time.time(),'old138_and104_verified_before':True,'steps':[]};receipt=root/'outputs/ready-four-native-local-proof.json'
for name,cmd,count in commands:
 free=shutil.disk_usage(root).free
 if free<7*1024**3:r.update(status='stopped-before-next-step-headroom',next_step=name,available_bytes=free);receipt.write_text(json.dumps(r,indent=2)+'\n');raise SystemExit('Insufficientheadroom;nextstepnotstarted')
 log=root/f'work/ready-four-native-{name}.log';print('START',name,'free',free,cmd,flush=True)
 with log.open('wb')as f:p=subprocess.run(cmd,cwd=checkout,env=env,stdout=f,stderr=subprocess.STDOUT)
 data=log.read_bytes();counts=list(map(int,re.findall(rb'test result: ok\. (\d+) passed; 0 failed; 0 ignored;',data)));row={'name':name,'command':cmd,'log':str(log.relative_to(root)),'sha256':sha(data),'exit_code':p.returncode,'before_available_bytes':free,'actual_test_counts':counts,'actual_passed':sum(counts),'actual_named_successes':re.findall(r'^test (\S+) \.\.\. ok$',data.decode(),re.M),'cargo_running_executables':re.findall(r'^\s*Running .*?\(([^)]+)\)\s*$',data.decode(),re.M)};r['steps'].append(row);receipt.write_text(json.dumps(r,indent=2)+'\n');print({k:v for k,v in row.items()if k not in ['actual_named_successes','cargo_running_executables','command']},flush=True);assert p.returncode==0 and sum(counts)==count
for e in protected:assert sha((root/e['path']).read_bytes())==e['sha256']
for row in sourcefiles:assert sha((checkout/row['path']).read_bytes())==row['sha256']
r.update(old138_and104_verified_after=True,finished=time.time(),clean_after=not git('status','--porcelain'),status='passed');r['executables']=[]
for p in target.rglob('*'):
 if p.is_file()and os.access(p,os.X_OK):
  with p.open('rb')as f:magic=f.read(4)
  if magic==b'\x7fELF':r['executables'].append({'path':str(p.relative_to(root)),'bytes':p.stat().st_size,'sha256':sha(p.read_bytes())})
r['qualification']=['ActualfreshowningNativeexecutionise9f/d350, NOT90a orhistoricalWIP relabeled.','15PlotouterPASS includes CURRENTgeneratedstd4inner/no_std via failclosedsourceboundhelper; temporaryinnerbinaries removedbyexistinghelper, noinnerELFhashclaim.','ActualBeam two-usepool throughretainedfinish/refusals checksalloc/realloc/dealloc allzero.','StrictPlot+LanguageLIBRARYonly; notalltargets.','NoCandidate/Integration/installedCurrent/controller/nativeUI/firmware/physical/parentcompletion.'];receipt.write_text(json.dumps(r,indent=2)+'\n');assert r['clean_after'];print('PASS',len(r['executables']),'ELFs;protected242unchanged',flush=True)
