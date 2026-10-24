from pathlib import Path
import os,json,time,subprocess,hashlib,shutil
w=Path(__file__).resolve().parent;wt=w/'6065-current-native-materialization';cache=w/'6065-build-target';out=w/'6065-focused-regressions-fdc2a582d';marker=w/'6065-build-cache-ownership.json'
assert json.loads(marker.read_text())['status']=='idle';assert shutil.disk_usage(w).free>12*2**30
out.mkdir();paths=json.loads((w/'6065-actual-beam-export-fix-fdc2a582d/receipt.json').read_text())['owning_source_sha256']
def snapshot():return {p:hashlib.sha256((wt/p).read_bytes()).hexdigest() for p in paths}
sources=snapshot();r={'source_state':'WIP, not finalclean acceptance','owning_source_sha256':sources,'started':time.time(),'checks':[]};marker.write_text(json.dumps({'owner':'Root/6065','status':'focused-regressions-running','pid':os.getpid(),'cache':str(cache)}))
env=os.environ.copy()
for k in ['RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS','CONDUITOS_BUILD_ID','CONDUITOS_IMAGE_ID','CONDUITOS_MAKE_RECORD']:env.pop(k,None)
env.update(CARGO_TARGET_DIR=str(cache),CARGO_BUILD_JOBS='2',CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0')
commands=[['test','--locked','-p','conduit-plot','--test','prepared_native_admission','--test','prepared_native_occurrences','--test','prepared_native_optional','--test','prepared_native_decode','--','--test-threads=1'],['test','--locked','-p','conduit-language','--test','prepared_beam_admission','--','--test-threads=1'],['clippy','--locked','-p','conduit-plot','-p','conduit-language','--lib','--','-D','warnings']]
try:
 for i,args in enumerate(commands):
  c={'argv':['cargo','+stable']+args,'started':time.time(),'log':f'check-{i}.log'}
  with (out/c['log']).open('w') as f:c['exit_code']=subprocess.call(c['argv'],cwd=wt,env=env,stdout=f,stderr=subprocess.STDOUT)
  c.update(ended=time.time(),log_sha256=hashlib.sha256((out/c['log']).read_bytes()).hexdigest());r['checks'].append(c)
  if c['exit_code']:break
 r['source_unchanged']=snapshot()==sources;r['status']='passed' if len(r['checks'])==len(commands) and all(c['exit_code']==0 for c in r['checks']) and r['source_unchanged'] else 'failed'
finally:
 r['ended']=time.time();(out/'receipt.json').write_text(json.dumps(r,indent=2)+'\n');marker.write_text(json.dumps({'owner':'Root/6065','status':'idle','receipt':str(out/'receipt.json')}))
print(json.dumps(r))
