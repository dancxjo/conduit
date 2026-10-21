from pathlib import Path
import os,subprocess,json,time,shutil,hashlib,re,sys
if sys.argv[1:] != ["--cache-handed-off"]:
    print("Prepared only; explicit cache handoff required. No workflow launched.")
    raise SystemExit(0)
T=Path('/home/dancxjo/Documents/Codex/2026-10-19/goal-6038');W=T/'work/selected-look-reconciled';O=T/'outputs/5628-reconciled-253dcb985';H='253dcb985eb8890ddf873d672d3a282f2366e6db'
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=W,text=True).strip()==H
assert not subprocess.check_output(['git','status','--porcelain'],cwd=W,text=True).strip()
O.mkdir(exist_ok=True)
assert not (O/'workspace.log').exists()
env=dict(os.environ,CARGO_TARGET_DIR=str(T/'work/selected-typography-host-target'),CARGO_BUILD_JOBS='4',CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CARGO_PROFILE_TEST_OPT_LEVEL='1',CARGO_PROFILE_TEST_DEBUG_ASSERTIONS='true',CARGO_PROFILE_TEST_OVERFLOW_CHECKS='true',RUST_TEST_THREADS='1')
free=shutil.disk_usage(T).free;assert free>20*1024**3
args=['cargo','+stable','xtask','check','workspace','--locked'];start=time.time()
r=dict(source=H,tree=subprocess.check_output(['git','rev-parse','HEAD^{tree}'],cwd=W,text=True).strip(),args=args,status='running',free_before=free,environment={k:v for k,v in env.items() if k.startswith('CARGO_') or k=='RUST_TEST_THREADS'})
(O/'workspace-status.json').write_text(json.dumps(r,indent=2)+'\n')
with (O/'workspace.log').open('wb') as log:p=subprocess.run(args,cwd=W,env=env,stdout=log,stderr=subprocess.STDOUT)
r.update(status=p.returncode,seconds=time.time()-start,free_after=shutil.disk_usage(T).free);(O/'workspace-status.json').write_text(json.dumps(r,indent=2)+'\n')
lines=(O/'workspace.log').read_text().splitlines();warnings=[f'{i}: {l}' for i,l in enumerate(lines,1) if l.startswith('warning:')];ignored=[f'{i}: {l}' for i,l in enumerate(lines,1) if ' ... ignored' in l]
(O/'workspace-warnings.txt').write_text('\n'.join(warnings)+'\n');(O/'workspace-ignored-tests.txt').write_text('\n'.join(ignored)+'\n')
r.update(log_sha256=hashlib.sha256((O/'workspace.log').read_bytes()).hexdigest(),test_results=[l for l in lines if l.startswith('test result:')],warning_header_occurrences=len(warnings),ignored_test_occurrences=len(ignored),boundary='Exact reconciled253 source full-workspace validation; prior589 and heldbranch proofs remain source-pinned separately. Warning headers include repeated cross-check occurrences, not distinct defects. Product/guest evidence separate; no stable or human claim.')
(O/'receipt.json').write_text(json.dumps(r,indent=2)+'\n');print(json.dumps({k:r[k] for k in ['source','status','seconds','free_after','log_sha256']},indent=2),flush=True)
raise SystemExit(p.returncode)
