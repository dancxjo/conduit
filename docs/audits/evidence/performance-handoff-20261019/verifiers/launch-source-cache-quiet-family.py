#!/usr/bin/env python3
"""Record the monitor PID, then exec the unchanged preregistered controller."""
import argparse,datetime,hashlib,json,os,sys
from pathlib import Path
p=argparse.ArgumentParser(add_help=False);p.add_argument('--launch-receipt',type=Path,required=True);p.add_argument('--registration',type=Path,required=True);a,rest=p.parse_known_args()
controller=Path(__file__).with_name('run-source-cache-quiet-family.py');sha=lambda f:hashlib.sha256(f.read_bytes()).hexdigest();reg=json.loads(a.registration.read_text());assert sha(controller)==reg['controller_sha256']
receipt={'schema':'conduit.quiet-monitor-launch@1','monitor_pid':os.getpid(),'created_at_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'controller_sha256':sha(controller),'registration_sha256':sha(a.registration),'launcher_sha256':sha(Path(__file__))}
with a.launch_receipt.open('x') as f:json.dump(receipt,f,indent=2);f.write('\n')
os.execv(sys.executable,[sys.executable,str(controller),'--registration',str(a.registration),*rest])
