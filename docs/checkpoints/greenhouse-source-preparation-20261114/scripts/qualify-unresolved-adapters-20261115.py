import os,subprocess,json
from pathlib import Path
root=Path(__file__).resolve().parent.parent
repo=root/'work/environment-extraction-integration-20261111'
env=dict(os.environ,CARGO_TARGET_DIR='/tmp/conduit-source-member-control-20261111',CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CARGO_BUILD_JOBS='4')
commands=[('mcu-unresolved-stock-clippy-20261115',['cargo','+1.98.1','clippy','--locked','--manifest-path','targets/mcu/host-runtime/Cargo.toml','--all-features','--lib','--no-deps','--','-D','warnings']),('source-unresolved-stock-clippy-20261115',['cargo','+1.98.1','clippy','--locked','-p','conduit-mcu-source-build','--lib','--tests','--no-deps','--','-D','warnings'])]
results=[]
for name,cmd in commands:
 with (root/'outputs'/f'{name}.log').open('w') as log: code=subprocess.run(cmd,cwd=repo,env=env,stdout=log,stderr=subprocess.STDOUT).returncode
 results.append({'command':cmd,'exit':code,'log':f'outputs/{name}.log'})
 print(json.dumps(results[-1]),flush=True)
 if code:break
(root/'outputs/unresolved-adapter-clippy-20261115.json').write_text(json.dumps(results,indent=2)+'\n')
raise SystemExit(code)
