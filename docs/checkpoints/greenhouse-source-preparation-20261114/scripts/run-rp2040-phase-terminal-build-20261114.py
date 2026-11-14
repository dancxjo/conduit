import json,os,signal,subprocess,time
from pathlib import Path
task=Path(__file__).resolve().parent.parent
repo=task/'work/environment-extraction-integration-20261111'
command=['python3',str(task/'work/rp2040-source-fit-build.py'),'--candidate','work/two-stage-source-candidate-20261114','--log','outputs/rp2040-phase-terminal-link-20261114.log']
floor=2*1024**3
def free():
 s=os.statvfs(task);return s.f_bavail*s.f_frsize
assert free()>floor
process=subprocess.Popen(command,cwd=task,start_new_session=True)
minimum=free();terminated=False
while process.poll() is None:
 minimum=min(minimum,free())
 if minimum<floor:
  terminated=True;os.killpg(process.pid,signal.SIGTERM);break
 time.sleep(3)
result=process.wait()
receipt={'command':command,'pid':process.pid,'exit':result,'minimum_root_free_bytes':minimum,'watchdog_floor_bytes':floor,'watchdog_terminated':terminated,'source_checkpoint':subprocess.check_output(['git','rev-parse','HEAD'],cwd=repo,text=True).strip(),'physical_deployment':False}
(task/'outputs/rp2040-phase-terminal-build-receipt-20261114.json').write_text(json.dumps(receipt,indent=2)+'\n')
print(json.dumps(receipt),flush=True)
raise SystemExit(result)
