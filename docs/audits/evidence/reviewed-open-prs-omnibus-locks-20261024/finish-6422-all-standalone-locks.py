import subprocess,pathlib,json,os,re,hashlib,tomllib,time
root=pathlib.Path.cwd();repo=root/'work/6393-permanent-cord-bound';parent='1cf7f4e7de59240a5d84fd736557d06664811dda';r=json.loads((root/'outputs/6422-all-standalone-locks-refresh.json').read_bytes());r['offline_attempt']='Stopped before publication whenDHT11resolutionattemptedunrelatedregistrydowngrades; restoringonlyknownownlockeditfromparent thenonlineworkspaceupdates preservepins.'
def gitbytes(*a):return subprocess.check_output(['git','-C',str(repo),*a])
assert gitbytes('rev-parse','HEAD').decode().strip()==parent
p='targets/rp2040/proof/dht11/Cargo.lock';(repo/p).write_bytes(gitbytes('show',parent+':'+p))
env=dict(os.environ,CARGO_TARGET_DIR=str(root/'work/6393-permanent-cord-target'),CARGO_BUILD_JOBS='4',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CARGO_INCREMENTAL='0',TMPDIR=str(root/'work/reviewed-omnibus-environment-delta-proof-tmp'));receipt=root/'outputs/6422-all-standalone-locks-refresh.json'
def run(name,args):
 log=root/('outputs/6422-'+name+'.log');start=time.time()
 with log.open('wb')as f:q=subprocess.run(['cargo','+stable',*args],cwd=repo,env=env,stdout=f,stderr=subprocess.STDOUT)
 b=log.read_bytes();r['steps'].append({'name':name,'argv':['cargo','+stable',*args],'exit_code':q.returncode,'log':str(log),'sha256':hashlib.sha256(b).hexdigest(),'start':start,'end':time.time()});receipt.write_text(json.dumps(r,indent=2));print(name,q.returncode,flush=True);return q.returncode
registry=lambda d:{(p['name'],p['version'],p.get('source'))for p in d['package']if p.get('source')};changes=[]
for i,m in enumerate(r['manifests']):
 path=str(pathlib.Path(m).with_name('Cargo.lock'));before=gitbytes('show',parent+':'+path);old=tomllib.loads(before.decode());assert run(f'standalone-refresh-online-{i:02}',['update','--workspace','--manifest-path',m])==0
 after=(repo/path).read_bytes();new=tomllib.loads(after.decode());removed=registry(old)-registry(new);added=registry(new)-registry(old);assert not removed,(m,'unexpectedexistingregistrychange',removed)
 changes.append({'manifest':m,'lock':path,'before_sha256':hashlib.sha256(before).hexdigest(),'after_sha256':hashlib.sha256(after).hexdigest(),'added_registry_packages':sorted(added),'removed_registry_packages':sorted(removed)})
r['changes']=changes;actual=set(gitbytes('diff','--name-only').decode().splitlines());assert actual=={x['lock']for x in changes};r['only_named_lockfiles_changed']=True
assert run('all-standalone-locks-after',['xtask','ci','standalone-locks','--locked'])==0
r['status']='locked_graphs_passed_pending_commit_and_full_preflight';receipt.write_text(json.dumps(r,indent=2));print('all13lockedgraphsrefreshed; existingregistrypinspreserved',flush=True)
