"""Internal focused owning proof. No source edits, CI calls, or owner writes."""
import pathlib,subprocess,os,json,hashlib,time,re,shutil
ROOT=pathlib.Path(__file__).resolve().parent.parent;REPO=ROOT/'work/6267-review'
HEAD='19e5233c44f12bfbd710015c226f8ef94020f057'
WT=ROOT/'work/6385-fore-proof';TARGET=ROOT/'work/6385-fore-target';TMP=ROOT/'work/6385-fore-tmp'
BRANCH='prep/6385-independent-fore-proof-20261024'
def git(*args):return subprocess.check_output(['git','-C',str(REPO),*args],text=True).strip()
assert shutil.disk_usage(ROOT).free>=7*1024**3
assert not WT.exists() and not TARGET.exists() and not TMP.exists()
subprocess.run(['git','-C',str(REPO),'worktree','add','-b',BRANCH,str(WT),HEAD],check=True)
TARGET.mkdir();TMP.mkdir()
env=dict(os.environ,CARGO_TARGET_DIR=str(TARGET),TMPDIR=str(TMP),CARGO_BUILD_JOBS='2',CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0')
def clean():return subprocess.check_output(['git','status','--porcelain=v1','--untracked-files=all'],cwd=WT,text=True)==''
assert clean()
steps=[]
commands=[('default-tests',['cargo','+stable','test','--locked','-p','conduit-assigned-plan','--lib'],9),('all-feature-tests',['cargo','+stable','test','--locked','-p','conduit-assigned-plan','--lib','--all-features'],9),('default-strict',['cargo','+stable','clippy','--locked','-p','conduit-assigned-plan','--all-targets','--','-D','warnings'],None),('all-feature-strict',['cargo','+stable','clippy','--locked','-p','conduit-assigned-plan','--all-targets','--all-features','--','-D','warnings'],None)]
out={'source':HEAD,'tree':git('rev-parse',HEAD+'^{tree}'),'branch':BRANCH,'worktree':str(WT),'started':time.time(),'before_available_bytes':shutil.disk_usage(ROOT).free,'environment':{k:env[k] for k in ['CARGO_TARGET_DIR','TMPDIR','CARGO_BUILD_JOBS','CARGO_INCREMENTAL','CARGO_PROFILE_DEV_DEBUG','CARGO_PROFILE_TEST_DEBUG']},'steps':steps,'status':'running'}
receipt=ROOT/'outputs/6385-fore-local-proof.json'
for name,command,count in commands:
 assert shutil.disk_usage(ROOT).free>=7*1024**3
 log=ROOT/f'work/6385-fore-{name}.log';started=time.time()
 with log.open('wb') as f:result=subprocess.run(command,cwd=WT,env=env,stdout=f,stderr=subprocess.STDOUT)
 raw=log.read_bytes();text=re.sub(rb'\x1b\[[0-9;]*m',b'',raw)
 row={'name':name,'command':command,'log':str(log.relative_to(ROOT)),'sha256':hashlib.sha256(raw).hexdigest(),'exit_code':result.returncode,'started':started,'finished':time.time()};steps.append(row)
 if count is not None:
  row['actual_test_counts']=[int(x) for x in re.findall(rb'test result: ok\. (\d+) passed;',text)]
  assert row['actual_test_counts']==[count]
  assert not re.search(rb'test result:.* [1-9]\d* (?:failed|ignored);',text)
 else:assert b'Finished' in text and b'`dev` profile' in text
 out['clean_after']=clean();receipt.write_text(json.dumps(out,indent=2)+'\n')
 assert result.returncode==0 and out['clean_after']
executables=[]
for path in sorted(TARGET.rglob('*')):
 if not path.is_file():continue
 with path.open('rb') as f:magic=f.read(4)
 if magic!=b'\x7fELF':continue
 data=path.read_bytes();executables.append({'path':str(path.relative_to(ROOT)),'bytes':len(data),'sha256':hashlib.sha256(data).hexdigest()})
out.update(status='passed',finished=time.time(),clean_after=clean(),executables=executables,after_available_bytes=shutil.disk_usage(ROOT).free)
assert out['clean_after'] and executables
receipt.write_text(json.dumps(out,indent=2)+'\n')
print(json.dumps({'status':out['status'],'source':HEAD,'test_counts':[9,9],'strict_profiles':2,'actual_elfs':len(executables)}),flush=True)
