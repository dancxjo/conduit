"""Pause one actual MCU owner at idle; prove local work and exact backlog export.

This is attended physical proof, not a power-cycle or network-reconnect test.
The owner is always resumed after the pause. Raw output belongs outside Git.
"""
import argparse
import os
import signal
import time
import json
import urllib.request
from urllib.parse import urlparse
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--history", action="store_true", help="also prove missed observation replay over a 150-second pause")
parser.add_argument("pid", type=int)
parser.add_argument("run", type=Path)
parser.add_argument("reading_url")
args = parser.parse_args()
pid = args.pid
run = args.run.resolve()
proc = Path(f"/proc/{pid}")
command = proc.joinpath("cmdline").read_bytes().split(b"\0")
assert Path(os.fsdecode(command[0])).name == "conduit-environment-host"
assert command[1].startswith(b"tcp://") and command[3] == b"960"
assert Path(os.fsdecode(command[2])).resolve() == run.parents[1]
assert urlparse(args.reading_url).hostname == urlparse(os.fsdecode(command[1])).hostname
assert (run / "body-playing.json").is_file()
def rows():
 raw=(run/'display-control.ndjson').read_bytes()
 complete=raw[:raw.rfind(b'\n')+1]
 return [json.loads(s) for s in complete.splitlines() if s]
def reading():
 with urllib.request.urlopen(args.reading_url, timeout=4) as response:return json.load(response)
def history(after=0):
 endpoint=args.reading_url.rsplit('/',1)[0]+f'/history?after={after}'
 with urllib.request.urlopen(endpoint, timeout=4) as response:return json.load(response)
before=reading()
history_before=history()['committed'] if args.history else None
prior=rows();last_seq=max(r['request']['sequence'] for r in prior)
latest=max(prior,key=lambda r:r['request']['sequence'])['reply']
calls=latest['worker']['native_calls'];commits=latest['local_evidence']['committed']
previous=proc.joinpath('wchan').read_text().strip();deadline=time.monotonic()+5
while time.monotonic()<deadline:
 current=proc.joinpath('wchan').read_text().strip()
 if current=='hrtimer_nanosleep' and previous!=current:break
 previous=current;time.sleep(.001)
else:raise RuntimeError('No verified idle boundary; owner left running')
owner = os.pidfd_open(pid)
assert proc.joinpath("cmdline").read_bytes().split(b"\0") == command
assert any(
    link.readlink() == run / "native-observations.ndjson"
    for link in proc.joinpath("fd").iterdir()
)
print(json.dumps({'event':'outage-before','pid':pid,'sample':before['sequence'],'native_calls':calls,'committed':commits,'sequence':last_seq,'idle':current,'utc':time.time()}),flush=True)
try:
 signal.pidfd_send_signal(owner, signal.SIGSTOP)
 deadline=time.monotonic()+1
 while '\nState:\tT' not in proc.joinpath('status').read_text():
  if time.monotonic()>deadline:raise RuntimeError('Owner did not stop')
  time.sleep(.001)
 # Capture the final durable prefix only after STOP has taken effect. The
 # earlier HTTP/log reads raced a still-running owner and are not a watermark.
 stopped_rows=rows()
 last_seq=max(r['request']['sequence'] for r in stopped_rows)
 stopped_latest=max(stopped_rows,key=lambda r:r['request']['sequence'])['reply']
 calls=stopped_latest['worker']['native_calls'];commits=stopped_latest['local_evidence']['committed']
 if args.history:history_before=history()['committed']
 print(json.dumps({'event':'outage-stopped-baseline','sequence':last_seq,'native_calls':calls,'committed':commits,'history_committed':history_before}),flush=True)
 time.sleep(50 if args.history else 45)
 mid=reading();print(json.dumps({'event':'outage-midpoint','sample':mid['sequence'],'age_ms':mid['age_ms'],'owner_stopped':'\nState:\tT' in proc.joinpath('status').read_text()}),flush=True)
 time.sleep(50 if args.history else 45)
 if args.history:
  second=reading();print(json.dumps({'event':'history-outage-second-minute','sample':second['sequence'],'owner_stopped':'\nState:\tT' in proc.joinpath('status').read_text()}),flush=True)
  time.sleep(50)
 after=reading();assert after['sequence']>before['sequence'],(before,after)
 assert '\nState:\tT' in proc.joinpath('status').read_text()
 if args.history:
  history_after=history(history_before)
  assert history_after['committed']>=history_before+2,history_after
  records=bytes.fromhex(history_after['records'])
  assert len(records)>=42
  missed=[int.from_bytes(records[i:i+8],'little') for i in range(0,len(records),21)]
  assert missed==list(range(history_before+1,history_before+1+len(missed)))
  print(json.dumps({'event':'history-outage-retained','before':history_before,'after':history_after['committed'],'missed_sequences':missed}),flush=True)
 print(json.dumps({'event':'outage-before-resume','sample':after['sequence'],'age_ms':after['age_ms'],'owner_stopped':True,'utc':time.time()}),flush=True)
finally:
 signal.pidfd_send_signal(owner, signal.SIGCONT)
 print(json.dumps({'event':'outage-resumed','utc':time.time()}),flush=True)
deadline=time.monotonic()+120
while time.monotonic()<deadline:
 os.kill(pid,0)
 following=sorted((r for r in rows() if r['request']['sequence']>last_seq),key=lambda r:r['request']['sequence'])
 if following:
  first=following[0]['reply'];local=first['local_evidence']
  assert first['worker']['failure'] is None,first['worker']
  assert first['worker']['native_calls']>=calls+20,first['worker']
  assert local['committed']>=commits+20,local
  target=local['committed']
  latest=following[-1]['reply']
  if latest['local_evidence']['accepted']>=target:
   # Acceptance follows the controller's signature verification and fsync.
   for invocation in range(commits, target):
    assert len(list((run / 'display-evidence').glob(f'{invocation:020d}-*.bin'))) == 1
   if args.history:
    retained=[]
    for path in run.glob('observed-history-*.json'):
     document=json.loads(path.read_bytes());payload=bytes.fromhex(document['records'])
     retained.extend(int.from_bytes(payload[i:i+8],'little') for i in range(0,len(payload),21))
    required=set(range(history_before+1,history_after['committed']+1))
    if not required.issubset(retained):time.sleep(1);continue
    print(json.dumps({'event':'history-outage-export-verified','required_sequences':sorted(required),'retained_sequences':sorted(set(retained))}),flush=True)
   print(json.dumps({'event':'outage-proof-passed','native_calls_before':calls,'native_calls_after':first['worker']['native_calls'],'committed_before':commits,'first_reply':local,'latest':latest['local_evidence'],'owner_alive':True}),flush=True)
   break
 time.sleep(1)
else:raise RuntimeError('Evidence backlog did not drain in finite recovery window')

os.close(owner)
