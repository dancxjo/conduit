import os,subprocess,pathlib,json,hashlib,time,shutil,re
root=pathlib.Path.cwd();checkout=root/'work/ready-four-combined-proof';target=root/'work/ready-four-combined-target';tmp=root/'work/ready-four-combined-tmp';tmp.mkdir(exist_ok=True);env=dict(os.environ,CARGO_TARGET_DIR=str(target),TMPDIR=str(tmp),CARGO_BUILD_JOBS='2',CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0')
def git(*a):return subprocess.check_output(['git',*a],cwd=checkout,text=True).strip()
head=git('rev-parse','HEAD');assert head=='e9f8787ccbf3a0fda086a5e4d568cb7f597d34e1';tree=git('rev-parse','HEAD^{tree}');assert tree=='d35051b7b0b512ef82e76fd37902479a65c43a06';assert not git('status','--porcelain');composition=json.loads((root/'outputs/ready-four-combined-source.json').read_text());union=[]
for row in composition['rows']:
 assert git('show','-s','--format=%P',row['local_merge']).split()==[row['actual_parent'],row['head']]
 assert git('rev-parse',row['local_merge']+'^{tree}')==row['composed_tree']
 assert git('diff','--raw','--no-abbrev',row['original_base'],row['head'])==git('diff','--raw','--no-abbrev',row['actual_parent'],row['local_merge'])
 subprocess.run(['git','merge-base','--is-ancestor',row['head'],head],cwd=checkout,check=True);union+=row['paths']
assert len(set(union))==32 and sorted(set(union))==git('diff','--name-only',composition['base'],head).splitlines()
r={'source':head,'tree':tree,'runtime_tree':git('rev-parse','HEAD:bodies/environment/runtime'),'mask_tree':git('rev-parse','HEAD:plots/seven-segment-mask'),'fore_tree':git('rev-parse','HEAD:architecture/assigned-plan'),'composition_verified':True,'all_32_paths_exact':True,'environment':{k:env[k]for k in ['CARGO_TARGET_DIR','TMPDIR','CARGO_BUILD_JOBS','CARGO_INCREMENTAL','CARGO_PROFILE_DEV_DEBUG','CARGO_PROFILE_TEST_DEBUG']},'steps':[],'started':time.time()};receipt=root/'outputs/ready-four-combined-local-proof.json';steps=[]
for capture in [False,True]:
 for op in ['test','clippy']:
  cmd=['cargo','+stable',op,'--locked','--manifest-path','bodies/environment/runtime/Cargo.toml'];cmd+=['--features','source-capture']if capture else [];cmd+=['--all-targets','--','-D','warnings']if op=='clippy'else [];steps.append(('runtime-'+('capture'if capture else'default')+('-strict'if op=='clippy'else''),cmd,(36 if capture else 35)if op=='test'else 0))
for op in ['test','clippy']:
 cmd=['cargo','+stable',op,'--locked','--manifest-path','plots/seven-segment-mask/Cargo.toml'];cmd+=['--all-targets','--','-D','warnings']if op=='clippy'else [];steps.append(('mask'+('-strict'if op=='clippy'else'-tests'),cmd,0 if op=='clippy'else 13))
for allfeatures in [False,True]:
 for op in ['test','clippy']:
  cmd=['cargo','+stable',op,'--locked','-p','conduit-assigned-plan'];cmd+=['--lib']if op=='test'else['--all-targets'];cmd+=['--all-features']if allfeatures else [];cmd+=['--','-D','warnings']if op=='clippy'else [];steps.append(('fore-'+('all-features'if allfeatures else'default')+('-strict'if op=='clippy'else'-tests'),cmd,9 if op=='test'else 0))
for name,cmd,expected in steps:
 free=shutil.disk_usage(root).free
 if free<7*1024**3:r.update(status='stopped-before-next-step-insufficient-headroom',next_step=name,available_bytes=free);receipt.write_text(json.dumps(r,indent=2)+'\n');raise SystemExit('Headroom below7GiB; no next command started')
 log=root/f'work/ready-four-combined-{name}.log';print('START',name,'free',free,cmd,flush=True)
 with log.open('wb')as f:p=subprocess.run(cmd,cwd=checkout,env=env,stdout=f,stderr=subprocess.STDOUT)
 raw=log.read_bytes();counts=list(map(int,re.findall(r'test result: ok\. (\d+) passed; 0 failed; 0 ignored;',raw.decode())));named=[line for line in raw.decode().splitlines()if line.startswith('test ')and line.endswith(' ... ok')];running=re.findall(r'Running[^\n]*\(([^)]+)\)',raw.decode());row={'name':name,'command':cmd,'log':str(log.relative_to(root)),'sha256':hashlib.sha256(raw).hexdigest(),'exit_code':p.returncode,'before_available_bytes':free,'actual_test_counts':counts,'actual_passed':sum(counts),'actual_named_successes':named,'cargo_running_executables':running};r['steps'].append(row);receipt.write_text(json.dumps(r,indent=2)+'\n');print({k:v for k,v in row.items()if k not in ['actual_named_successes','cargo_running_executables','command']},flush=True);assert p.returncode==0 and sum(counts)==expected
r.update(finished=time.time(),clean_after=not git('status','--porcelain'),status='passed');r['executables']=[]
for p in target.rglob('*'):
 if p.is_file()and os.access(p,os.X_OK):
  with p.open('rb')as f:magic=f.read(4)
  if magic==b'\x7fELF':r['executables'].append({'path':str(p.relative_to(root)),'sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'bytes':p.stat().st_size})
receipt.write_text(json.dumps(r,indent=2)+'\n');assert r['clean_after']and sum(x['actual_passed']for x in r['steps'])==102;print('PASS',len(r['executables']),'ELFs',flush=True)
