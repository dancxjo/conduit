import os,subprocess,json,time,hashlib,shutil,gzip
from pathlib import Path
root=Path.cwd();out=root/'outputs/6387-current-product';src=root/'work/6393-permanent-cord-bound';target=root/'work/6393-permanent-cord-target';runner=root/'work/6387-current-browser-runner';expected='a6353d7f189358c159bd718092cf2fa53b0aef4a'
def sha(p):
 h=hashlib.sha256()
 with p.open('rb') as f:
  while b:=f.read(1048576):h.update(b)
 return h.hexdigest()
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=src,text=True).strip()==expected
assert not subprocess.check_output(['git','status','--porcelain'],cwd=src,text=True)
assert json.load(open(out/'build-receipt.json'))['steps'][-1]['exit']==0
artifacts={}
for name,p in [('workbench',target/'debug/conduit-browser-patchbay-workbench'),('browser-wasm',target/'wasm32-unknown-unknown/release/conduit_browser_runtime.wasm')]:
 d=sha(p);dest=out/(name+('.elf.gz' if name=='workbench' else '.wasm.gz'))
 with p.open('rb') as f,gzip.open(dest,'wb',compresslevel=3) as g:shutil.copyfileobj(f,g,1048576)
 with gzip.open(dest,'rb') as f:
  h=hashlib.sha256()
  while b:=f.read(1048576):h.update(b)
 assert h.hexdigest()==d
 artifacts[name]={'path':str(p),'sha256':d,'bytes':p.stat().st_size,'sealed':str(dest),'sealed_sha256':sha(dest)}
receipt={'source':expected,'artifacts':artifacts,'steps':[],'started':time.time()};(out/'execution-receipt.json').write_text(json.dumps(receipt,indent=2)+'\n')
env={**os.environ,'CONDUIT_BROWSER_RUNTIME_WASM':str(target/'wasm32-unknown-unknown/release/conduit_browser_runtime.wasm'),'PLAYWRIGHT_JSON_OUTPUT_NAME':str(out/'browser-report.json')}
steps=[('minimal',['python3',str(root/'work/6387-current-minimal.py')],root),('original-nine-gear',['node','proof/browser/node_modules/@playwright/test/cli.js','test','--config','proof/browser/patchbay-html.playwright.config.mjs','proof/browser/patchbay-authoring.spec.mjs','--grep','^catalog queries and two durable layouts decorate the same live Plot$','--workers=1','--retries=0','--reporter=line,json','--output',str(out/'browser-results')],runner)]
for name,cmd,cwd in steps:
 print('Starting',name,flush=True);start=time.time()
 with (out/(name+'.log')).open('w') as f:code=subprocess.call(cmd,cwd=cwd,env=env,stdout=f,stderr=subprocess.STDOUT)
 receipt['steps'].append({'name':name,'command':cmd,'cwd':str(cwd),'exit':code,'elapsed':time.time()-start});receipt['final_source_head']=subprocess.check_output(['git','rev-parse','HEAD'],cwd=src,text=True).strip();receipt['final_source_status']=subprocess.check_output(['git','status','--porcelain'],cwd=src,text=True);(out/'execution-receipt.json').write_text(json.dumps(receipt,indent=2)+'\n');print(name,code,flush=True)
 if code:raise SystemExit(code)
