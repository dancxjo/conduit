from pathlib import Path
import subprocess,json,hashlib,os
repo=Path.cwd(); work=repo.parent
source=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip()
assert not subprocess.check_output(['git','status','--porcelain'],text=True).strip()
output=Path('/tmp/c6349-gui/browser-narrow-reflow-before');assert not output.exists()
binary=(repo/'target/debug/conduit-browser-patchbay-workbench').resolve()
command=['node','proof/browser/node_modules/@playwright/test/cli.js','test','--config','proof/browser/patchbay-html.playwright.config.mjs','--workers=1','--retries=0','--output',str(output),'patchbay-look-options.spec.mjs','patchbay-panel-furniture.spec.mjs']
with (work/'browser-narrow-reflow-before.log').open('wb') as log:
 proc=subprocess.Popen(command,cwd=repo,stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
 handle={'source':source,'pid':proc.pid,'proc_start':Path(f'/proc/{proc.pid}/stat').read_text().split()[21],'command':command,'binary':str(binary),'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'output':str(output)}
 (work/'browser-narrow-reflow-before-handle.json').write_text(json.dumps(handle,indent=2)+'\n');print(json.dumps(handle),flush=True)
 result=proc.wait()
 report={**handle,'exit':result,'source_unchanged':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip()==source,'tracked_tree_clean':not subprocess.check_output(['git','status','--porcelain'],text=True).strip()}
 (work/'browser-narrow-reflow-before-result.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report),flush=True)
