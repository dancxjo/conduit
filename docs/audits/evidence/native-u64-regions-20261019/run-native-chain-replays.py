#!/usr/bin/env python3
"""Fresh, sequential CPU-4 processes of one frozen worker; never rebuild here."""
import argparse,hashlib,json,os,subprocess
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--worker',required=True);p.add_argument('--source',required=True);p.add_argument('--cwd',required=True);p.add_argument('--public',required=True);p.add_argument('--output-prefix',required=True);a=p.parse_args()
worker=Path(a.worker).resolve();cwd=Path(a.cwd).resolve();public=Path(a.public).resolve();prefix=Path(a.output_prefix).resolve()
sha=lambda path:hashlib.sha256(path.read_bytes()).hexdigest()
worker_hash=sha(worker)
def git(*args):return subprocess.check_output(['git',*args],cwd=cwd,text=True).strip()
assert 4 in os.sched_getaffinity(0)
assert git('rev-parse','HEAD')==a.source and not git('status','--porcelain')
assert public.parent==prefix.parent
for index in (1,2,3):
 output=prefix.with_name(prefix.name+'-'+str(index));output.mkdir()
 before=git('rev-parse','HEAD');clean_before=not git('status','--porcelain')
 command=['taskset','-c','4',str(worker),'--output',str(output),'--values','1000000','--repetitions','3','--decisions','1000','--samples','1000','--hosted-pipeline','true']
 with (output/'stdout.log').open('wb') as stdout,(output/'stderr.log').open('wb') as stderr:
  result=subprocess.run(command,cwd=cwd,stdout=stdout,stderr=stderr)
 after=git('rev-parse','HEAD');clean_after=not git('status','--porcelain')
 receipt={'source_before':before,'source_after':after,'clean_before':clean_before,'clean_after':clean_after,'worker_sha256':sha(worker),'expected_worker_sha256':worker_hash,'affinity':'4','command':command,'exit_code':result.returncode,'conformance_capture':public.name,'artifact_sha256':{str(x.relative_to(output)):sha(x) for x in sorted(output.rglob('*')) if x.is_file()}}
 (output/'capture-status.json').write_text(json.dumps(receipt,indent=2)+'\n')
 assert result.returncode==0 and before==after==a.source and clean_before and clean_after
 assert sha(worker)==worker_hash
 print(str(output),flush=True)
