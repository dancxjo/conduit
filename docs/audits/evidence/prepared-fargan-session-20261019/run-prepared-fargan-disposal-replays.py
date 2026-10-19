from pathlib import Path
import subprocess,json,hashlib,time
base=Path('/home/dancxjo/Documents/Codex/2026-10-18/goal-optimize-performance-referenced-pasted-text');repo=base/'work/conduit-session';worker=base/'work/fargan-prepared-disposal-worker';fixture='/home/dancxjo/Documents/Codex/2026-10-17/goal-please-finish-and-close-https/work/fargan-development/fixture';source='e4fbac12b38ff0f787fe626f4c587cbd175ab5b8'
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def clean():return subprocess.check_output(['git','status','--porcelain'],cwd=repo,text=True)=='' and subprocess.check_output(['git','rev-parse','HEAD'],cwd=repo,text=True).strip()==source
assert clean();expected=json.loads((base/'work/fargan-prepared-disposal-worker.json').read_text())['worker_sha256'];assert sha(worker)==expected
for i in range(1,4):
 assert clean();out=base/f'work/fargan-prepared-disposal-replay-{i}';out.mkdir();args=['taskset','-c','4',str(worker),fixture,str(out),'2','prepared'];started=time.time()
 with (out/'stdout.log').open('w') as stdout,(out/'stderr.log').open('w') as stderr:r=subprocess.run(args,cwd=repo,stdout=stdout,stderr=stderr)
 assert r.returncode==0,(i,r.returncode);assert clean();assert sha(worker)==expected
 manifest={str(p.relative_to(out)):sha(p) for p in sorted(out.rglob('*')) if p.is_file()}
 receipt={'schema':'conduit.fargan-runtime-performance-command@1','git_sha':source,'final_git_sha':source,'source_custody_verified':True,'conformance_success':True,'conformance_capture':'fargan-prepared-disposal-public','conformance_receipt_sha256':sha(base/'work/fargan-prepared-disposal-public/status.json'),'worker_success':True,'worker_exit_code':r.returncode,'owners':'prepared','repetitions':2,'worker_sha256':expected,'command':args,'wall_seconds':time.time()-started,'artifact_sha256':manifest};(out/'status.json').write_text(json.dumps(receipt,indent=2)+'\n')
 subprocess.run(['python3',str(base/'work/verify-prepared-fargan.py'),str(out),'--source',source,'--output',str(base/f'outputs/prepared-kernel-session/disposal-replay-{i}-verification.json')],check=True)
 print(json.dumps({'replay':i,'exit':0,'wall_seconds':receipt['wall_seconds']}),flush=True)
