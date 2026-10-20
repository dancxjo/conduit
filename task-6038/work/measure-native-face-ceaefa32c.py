"""Measure exact existing JSON byte spans; do not reserialize or alter guest logs."""
from pathlib import Path
import json,hashlib
T=Path('/home/dancxjo/Documents/Codex/2026-10-19/goal-6038');O=T/'outputs'
source='ceaefa32c00dd9a15aa4365f831cf68862771dd4'
names=['5990-normal-journey-ceaefa32c/x86_64/journey-serial.log','5990-panels-wide-ceaefa32c/serial.log','5990-panels-narrow-ceaefa32c/serial.log','5990-pointer-wide-ceaefa32c-cold-birth/serial.log','5990-pointer-narrow-ceaefa32c-cold-birth/serial.log']
decoder=json.JSONDecoder();observations=[];inputs={}
for name in names:
 p=O/name;inputs[name]=hashlib.sha256(p.read_bytes()).hexdigest()
 for number,line in enumerate(p.read_text().splitlines(),1):
  if not line.startswith('CONDUIT_NATIVE_FACE '):continue
  raw=line.split(' ',1)[1];record=json.loads(raw);i=1;found=[]
  while i<len(raw):
   while raw[i].isspace() or raw[i]==',':i+=1
   if raw[i]=='}':break
   key,i=decoder.raw_decode(raw,i)
   while raw[i].isspace():i+=1
   assert raw[i]==':';i+=1
   while raw[i].isspace():i+=1
   start=i;value,i=decoder.raw_decode(raw,i)
   if key=='face':
    span=raw[start:i];assert json.loads(span)==record['face']==value;found.append(span.encode())
  assert len(found)==1
  data=found[0];f=record['face'];candidates=sum(x['role']=='Plot' for x in f['subjects'])
  observations.append({'input':name,'line':number,'face_id':f['identity'],'revision':f['revision'],'face_bytes':len(data),'observation_json_bytes':len(raw.encode()),'sha256':hashlib.sha256(data).hexdigest(),'plot_candidates':candidates})
assert observations
r={'source':source,'method':'Exact UTF-8 span of top-level face field, parsed independently and compared with fully decoded record; complete observation JSON bytes measured separately','inputs':inputs,'observations_count':len(observations),'maximum_face_bytes':max(x['face_bytes'] for x in observations),'maximum_observation_json_bytes':max(x['observation_json_bytes'] for x in observations),'existing_limits':{'native_mask_snapshot_bytes':49152,'routed_join_frame_bytes':32768,'observer_record_bytes':98304},'boundary':'Observed current stage05 normal-product states only, including five-candidate long-Unicode/refusal/filter states. Not C1/C2, maximum16-Plot fixture, whole-route envelope/transport admission, runtime allocation/latency, physical or human proof. No limit change.','observations':observations}
rows=[]
for name in names[1:]:
 p=O/name;p=p.parent/'receipt.json';a=json.loads(p.read_text());assert a['source']==source and a['status']=='passed'
 rows.append({'receipt':str(p.relative_to(O)),'receipt_sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'viewport':a['viewport'],'arrival_observed_ms':a['arrival_observed_ms']})
r['supplement_arrival_observations']={'method':'Host monotonic elapsed from immediately before QEMU Popen to read-only observation of CONDUIT_BOOT_STAGE front-door-ready, including QMP connection; four separately booted runs, one observation per run','runs':rows,'boundary':'Local emulator observations, uncontrolled host/cache conditions. No speedup, cold-cache, realtime, human-perceived readiness, worst-case or other-Host claim. No workload repeated for timing.'}
D=O/'native-face-byte-audit-ceaefa32c';D.mkdir(exist_ok=False);(D/'receipt.json').write_text(json.dumps(r,indent=2)+'\n');print(json.dumps({k:v for k,v in r.items() if k not in ['observations','inputs']},indent=2))
