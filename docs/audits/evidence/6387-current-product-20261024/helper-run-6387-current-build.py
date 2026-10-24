import os,subprocess,json,time,shutil
from pathlib import Path
root=Path.cwd();source=root/'work/6393-permanent-cord-bound';target=root/'work/6393-permanent-cord-target';out=root/'outputs/6387-current-product';out.mkdir(exist_ok=True)
env=os.environ.copy();env.update(CARGO_TARGET_DIR=str(target),CARGO_BUILD_JOBS='4',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CARGO_INCREMENTAL='0')
r={'source':'a6353d7f189358c159bd718092cf2fa53b0aef4a','started':time.time(),'steps':[]}
steps=[('browser-wasm',['cargo','+stable','build','--locked','-p','conduit-browser-runtime','--target','wasm32-unknown-unknown','--release'],{'CARGO_PROFILE_RELEASE_OPT_LEVEL':'z','CARGO_PROFILE_RELEASE_LTO':'fat','CARGO_PROFILE_RELEASE_CODEGEN_UNITS':'1'}),('workbench',['cargo','+stable','build','--locked','-p','conduit-browser-patchbay-workbench','--bin','conduit-browser-patchbay-workbench'],{})]
for name,cmd,extra in steps:
 assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=source,text=True).strip()==r['source']
 assert not subprocess.check_output(['git','status','--porcelain'],cwd=source,text=True)
 free=shutil.disk_usage(root).free;assert free>5*1024**3
 print('Starting',name,'free',free,flush=True);start=time.time()
 with (out/(name+'.log')).open('w') as log:code=subprocess.call(cmd,cwd=source,env={**env,**extra},stdout=log,stderr=subprocess.STDOUT)
 r['steps'].append({'name':name,'command':cmd,'environment':{**{k:env[k] for k in ['CARGO_TARGET_DIR','CARGO_BUILD_JOBS','CARGO_PROFILE_DEV_DEBUG','CARGO_INCREMENTAL']},**extra},'exit':code,'elapsed':time.time()-start});(out/'build-receipt.json').write_text(json.dumps(r,indent=2)+'\n');print(name,code,flush=True)
 if code:raise SystemExit(code)
