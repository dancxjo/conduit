import os,subprocess,pathlib,json,time,hashlib,re
root=pathlib.Path.cwd();source=root/'work/6393-permanent-cord-bound';target=root/'work/6393-permanent-cord-target';log=root/'work/6393-permanent-cord-corrected-negative.log'
def git(*args):return subprocess.check_output(['git','-C',str(source),*args],text=True).strip()
head=git('rev-parse','HEAD');assert head=='9ba29d98b6dc29aa599a81dd7dc170125dce9e9d' and not git('status','--porcelain');assert os.statvfs(root).f_bavail*os.statvfs(root).f_frsize>12*1024**3
argv=['cargo','+stable','test','--locked','-p','conduit-std-host','--lib','installed_std_tests::math_conformance::cord_capacity','--','--nocapture'];env=dict(os.environ,CARGO_TARGET_DIR=str(target),CARGO_BUILD_JOBS='4',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CARGO_INCREMENTAL='0')
start=time.time()
with log.open('wb')as out:p=subprocess.run(argv,cwd=source,env=env,stdout=out,stderr=subprocess.STDOUT)
b=log.read_bytes();s=b.decode(errors='replace');elfs=[]
for path in re.findall(r'Running[^\n]*\(([^\n)]+)\)',s):
 path=pathlib.Path(path);path=path if path.is_absolute()else source/path;data=path.read_bytes();assert data[:4]==b'\x7fELF';elfs.append({'path':str(path),'bytes':len(data),'sha256':hashlib.sha256(data).hexdigest()})
r={'source':head,'tree':git('rev-parse','HEAD^{tree}'),'clean_after':not git('status','--porcelain'),'argv':argv,'start':start,'end':time.time(),'exit_code':p.returncode,'log':str(log),'log_sha256':hashlib.sha256(b).hexdigest(),'actual_test_executables':elfs,'test_summaries':re.findall(r'test result:[^\n]+',s)}
assert git('rev-parse','HEAD')==head
(root/'outputs/6393-permanent-cord-corrected-negative.json').write_text(json.dumps(r,indent=2));print(json.dumps(r,indent=2))
