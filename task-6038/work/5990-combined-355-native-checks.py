import subprocess,os,pathlib,json,sys
root=pathlib.Path('/home/dancxjo/Documents/Codex/2026-10-19/goal-6038/work/native-panels')
logs=root.parent/'5990-combined-355-logs'
env=os.environ.copy();env.update(CARGO_TARGET_DIR=str(root.parent/'native-panels-target'),CARGO_BUILD_JOBS='4',CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CARGO_PROFILE_TEST_OPT_LEVEL='1',CARGO_PROFILE_TEST_DEBUG_ASSERTIONS='true',CARGO_PROFILE_TEST_OVERFLOW_CHECKS='true',CARGO_PROFILE_TEST_STRIP='symbols')
results=[]
for name in ['native_face_scene','front_door::arrival','native_workset','spore_join','bounded_websocket','native_owner_admission']:
    path=logs/(name.replace('::','-')+'.log')
    with path.open('w') as output:
        result=subprocess.run(['cargo','+stable','test','-p','conduitos','--features','native-compositor','--lib',name],cwd=root,env=env,stdout=output,stderr=subprocess.STDOUT)
    results.append({'filter':name,'exit_code':result.returncode,'log':str(path)})
    (logs/'native-results.json').write_text(json.dumps(results,indent=2)+'\n')
    print(name,result.returncode,flush=True)
    if result.returncode:sys.exit(result.returncode)
with (logs/'clippy-package.log').open('w') as output:
    result=subprocess.run(['cargo','+stable','clippy','-p','conduitos','--features','native-compositor','--lib','--tests','--','-D','warnings'],cwd=root,env=env,stdout=output,stderr=subprocess.STDOUT)
print('clippy',result.returncode,flush=True)
sys.exit(result.returncode)
