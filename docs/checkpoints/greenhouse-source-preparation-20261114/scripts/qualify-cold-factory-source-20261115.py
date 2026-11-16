import os,subprocess,json
from pathlib import Path
root=Path(__file__).resolve().parent.parent;repo=root/'work/environment-extraction-integration-20261111'
cache=root/'work/cache-backing/rp2040-source-fit-20261113'
selected=[p.parent for p in cache.glob('thumbv6m-none-eabi/release/build/conduit-rp2040-host-*/out/workset_0_layout.rs') if 'pub const EXTERNAL_INGRESS_CORD_COUNT:usize=3;' in p.read_text()];assert len(selected)==1
scratch=root/'work/source-proof-tmp-20261115';scratch.mkdir(exist_ok=True)
env=dict(os.environ,CARGO_TARGET_DIR='/tmp/conduit-source-member-control-20261111',CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CARGO_BUILD_JOBS='4',TMPDIR=str(scratch),CONDUIT_NEUTRAL_CPU_OUT_DIR=str(selected[0]))
commands=[('positive-cold-factory-wrapper-20261115',['python3',str(root/'work/measure-cold-factory-20261115.py')]),('neutral-execution-cold-factory-20261115',['cargo','+1.98.1','test','--locked','-p','conduit-host-rp2040','--test','neutral_execution','--','--ignored','--nocapture'])]
results=[]
for name,cmd in commands:
 with (root/'outputs'/f'{name}.log').open('w') as log:code=subprocess.run(cmd,cwd=repo,env=env,stdout=log,stderr=subprocess.STDOUT).returncode
 results.append({'command':cmd,'exit':code,'log':f'outputs/{name}.log'});print(json.dumps(results[-1]),flush=True)
 if code:break
(root/'outputs/cold-factory-source-qualification-20261115.json').write_text(json.dumps(results,indent=2)+'\n')
raise SystemExit(code)
