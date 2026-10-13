from pathlib import Path
import subprocess,os,time,json,hashlib
root=Path.cwd();w=root/'work/corpus-cursor';e=os.environ.copy();e.update(CARGO_TARGET_DIR='/home/dancxjo/conduit/target',CARGO_BUILD_JOBS='4',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CARGO_INCREMENTAL='0')
changed=subprocess.check_output(['git','diff','--name-only'],text=True).splitlines()+['mechanisms/implementations/burn-model/src/corpus.rs','mechanisms/implementations/burn-model/tests/corpus_cursor.rs','mechanisms/implementations/burn-model/tests/common/corpus.rs']
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();before={p:sha(p) for p in changed}
commands=[('burn-final',['test','--locked','-p','conduit-burn-model']),('plot-final',['test','--locked','-p','conduit-std-host','--features','burn-model','--test','burn_plot_journey']),('ai-final',['test','--locked','-p','conduit-ai','--lib','--test','training','--test','training_lifecycle','--test','native_training_vocabulary']),('clippy-final',['clippy','--locked','-p','conduit-burn-model','-p','conduit-ai','-p','conduit-std-host','--features','conduit-std-host/burn-model','--lib','--test','corpus_cursor','--test','checkpoint_contract','--test','training','--test','training_lifecycle','--test','native_training_vocabulary','--test','burn_plot_journey','--','-D','warnings'])]
results=[]
for name,args in commands:
 t=time.monotonic()
 with (w/(name+'.log')).open('w') as f:r=subprocess.run(['cargo','+stable']+args,env=e,stdout=f,stderr=subprocess.STDOUT)
 result={'name':name,'command':['cargo','+stable']+args,'exit':r.returncode,'seconds':time.monotonic()-t};results.append(result);print(json.dumps(result),flush=True)
 (w/'final-gates-result.json').write_text(json.dumps({'base':'d932da60c7f04bde62c797126e298e38aa9819d0','sources':before,'unchanged':all(sha(p)==h for p,h in before.items()),'results':results},indent=2))
 if r.returncode:break
