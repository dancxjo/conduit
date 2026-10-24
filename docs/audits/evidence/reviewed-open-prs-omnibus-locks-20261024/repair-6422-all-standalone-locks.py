import subprocess,pathlib,json,os,re,hashlib,tomllib,time
root=pathlib.Path.cwd();repo=root/'work/6393-permanent-cord-bound';parent='1cf7f4e7de59240a5d84fd736557d06664811dda'
def git(*a):return subprocess.check_output(['git','-C',str(repo),*a],text=True).strip()
assert git('rev-parse','HEAD')==parent and not git('status','--porcelain');log=(root/'outputs/6422-candidate-quick-120938926019-direct.log').read_text();manifests=re.findall(r'Z - ([^\s]+/Cargo.toml):',log);assert len(manifests)==13
baseenv=dict(os.environ,CARGO_TARGET_DIR=str(root/'work/6393-permanent-cord-target'),CARGO_BUILD_JOBS='4',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CARGO_INCREMENTAL='0',TMPDIR=str(root/'work/reviewed-omnibus-environment-delta-proof-tmp'))
r={'parent':parent,'manifests':manifests,'steps':[],'status':'running'};receipt=root/'outputs/6422-all-standalone-locks-refresh.json'
def run(name,args,env=baseenv):
 out=root/('outputs/6422-'+name+'.log');start=time.time()
 with out.open('wb')as f:p=subprocess.run(['cargo','+stable',*args],cwd=repo,env=env,stdout=f,stderr=subprocess.STDOUT)
 b=out.read_bytes();r['steps'].append({'name':name,'argv':['cargo','+stable',*args],'exit_code':p.returncode,'log':str(out),'sha256':hashlib.sha256(b).hexdigest(),'start':start,'end':time.time()});receipt.write_text(json.dumps(r,indent=2));print(name,p.returncode,flush=True);return p.returncode
assert run('all-standalone-locks-before',['xtask','ci','standalone-locks','--locked'])!=0
changes=[]
for i,m in enumerate(manifests):
 lock=repo/pathlib.Path(m).with_name('Cargo.lock');before=lock.read_bytes();old=tomllib.loads(before.decode());assert run(f'standalone-refresh-{i:02}', ['update','--offline','--workspace','--manifest-path',m])==0
 after=lock.read_bytes();new=tomllib.loads(after.decode());registry=lambda d:{(p['name'],p['version'],p.get('source'))for p in d['package']if p.get('source')};removed=registry(old)-registry(new);added=registry(new)-registry(old);assert not removed,(m,'unexpected existing registry change',removed)
 changes.append({'manifest':m,'lock':str(lock.relative_to(repo)),'before_sha256':hashlib.sha256(before).hexdigest(),'after_sha256':hashlib.sha256(after).hexdigest(),'added_registry_packages':sorted(added),'removed_registry_packages':sorted(removed)})
r['changes']=changes;actual=set(git('diff','--name-only').splitlines());assert actual=={x['lock']for x in changes};r['only_named_lockfiles_changed']=True
assert run('all-standalone-locks-after',['xtask','ci','standalone-locks','--locked'])==0
r['status']='locked_graphs_passed_pending_commit_and_full_preflight';receipt.write_text(json.dumps(r,indent=2));print('refreshed',len(changes),'exact locks; existing registry versions preserved',flush=True)
