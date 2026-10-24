import os,subprocess,pathlib,json,hashlib,time
root=pathlib.Path.cwd();repo=root/'work/6393-permanent-cord-bound';source='4e9cad06aaa21a59257d9d42181395ff17ed0b52';base='5b266161926f920bbf266288fe0f1f757d419632'
def git(*a):return subprocess.check_output(['git','-C',str(repo),*a],text=True).strip()
assert git('rev-parse','HEAD')==source and not git('status','--porcelain');env=dict(os.environ,CARGO_TARGET_DIR=str(root/'work/6393-permanent-cord-target'),CARGO_BUILD_JOBS='4',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CARGO_INCREMENTAL='0',TMPDIR=str(root/'work/reviewed-omnibus-environment-delta-proof-tmp'));r={'source':source,'base':base,'steps':[],'status':'running'};receipt=root/'outputs/6422-complete-local-preflight.json'
for name,args in [('setup',['xtask','ci','pipeline','setup-ci']),('preflight',['xtask','ci','pipeline','preflight',base])]:
 log=root/('outputs/6422-complete-'+name+'.log');start=time.time()
 with log.open('wb')as f:p=subprocess.run(['cargo','+stable',*args],cwd=repo,env=env,stdout=f,stderr=subprocess.STDOUT)
 b=log.read_bytes();r['steps'].append({'name':name,'argv':['cargo','+stable',*args],'exit_code':p.returncode,'log':str(log),'sha256':hashlib.sha256(b).hexdigest(),'start':start,'end':time.time()});r['status']='running'if not p.returncode else'failed';receipt.write_text(json.dumps(r,indent=2));print(name,p.returncode,flush=True)
 assert git('rev-parse','HEAD')==source and not git('status','--porcelain')
 if p.returncode:break
else:r['status']='passed'
r['clean_after']=not git('status','--porcelain');receipt.write_text(json.dumps(r,indent=2));print('final',r['status'],flush=True)
