from pathlib import Path
import subprocess,os,re
root=Path('work/lint-target/debug/deps').resolve();core=max(root.glob('libconduitos_application_core-*.rlib'),key=lambda x:x.stat().st_mtime);info=subprocess.check_output(['rustc','+stable','-Zls=all',str(core)],env={**os.environ,'RUSTC_BOOTSTRAP':'1'},text=True);args=['rustc','+stable','--edition=2024','work/6207-retained-model-proof/main.rs','-L','dependency='+str(root),'-o','work/6207-retained-model-proof/probe']
for crate in ['conduitos_application_core','serde_json','miniz_oxide']:
 name=core.stem[3:] if crate=='conduitos_application_core' else re.search(r'\b('+crate+r'-[0-9a-f]+) hash',info)[1];args+=['--extern',crate+'='+str(root/('lib'+name+'.rlib'))]
r=subprocess.run(args,capture_output=True,text=True);print(r.stderr[:1500]);
if not r.returncode:
 r=subprocess.run(['work/6207-retained-model-proof/probe'],capture_output=True,text=True);print(r.stdout);print(r.stderr);print('exit',r.returncode);Path('work/6207-retained-model-proof/result.txt').write_text(r.stdout+r.stderr+'exit='+str(r.returncode)+'\n')
