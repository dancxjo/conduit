"""Verify a relocated QEMU journey's raw timing correlation; no timing acceptance."""
import argparse, hashlib, json, struct, io
from PIL import Image
from pathlib import Path
CLOCK = 'host/process-monotonic-nanoseconds@1'
def sha(data): return hashlib.sha256(data).hexdigest()
def load(path): return json.loads(path.read_text())
def verify(root):
 frames=root/'journey-frames';manifest=load(frames/'manifest.json');proof=load(root/'journey-proof.json');latency=load(frames/'latency.json');role=load(root/'first-run-role.json')
 assert manifest['status']=='complete' and len(manifest['checkpoints'])==22
 context=manifest['context'];start=context['observer_clock']['start_ns'];assert context['observer_clock']['id']==CLOCK
 for key in ['source_commit','image_sha256']: assert context[key]==proof[key]==latency[key]
 assert context['source_commit'].startswith(role['source']) and 'conformance-only' in role['role']
 assert sha((root/'guest/conduitos.iso').read_bytes())==context['image_sha256']
 for key in ['serial_observation','latency']:
  entry=manifest[key];assert Path(entry['file']).name==entry['file'];assert sha((frames/entry['file']).read_bytes())==entry['sha256']
 for entry in manifest['checkpoints']:
  frame=entry['frame'];assert Path(frame['png']).name==frame['png'];data=(frames/frame['png']).read_bytes();assert sha(data)==frame['png_sha256'] and len(data)==frame['png_bytes']
  assert data[:8]==b'\x89PNG\r\n\x1a\n' and struct.unpack('>II',data[16:24])==(frame['width'],frame['height'])==(1280,800)
  with Image.open(io.BytesIO(data)) as image: image.verify()
  with Image.open(io.BytesIO(data)) as image:
   assert image.format=='PNG' and image.size==(1280,800) and image.mode in ['RGB','RGBA']
   assert sha(image.convert('RGBA').tobytes())==frame['pixel_sha256']
 raw=(root/'journey-serial.log').read_bytes();rows=[json.loads(x) for x in (frames/'serial-observations.jsonl').read_text().splitlines()];records=[];offset=0;last=start
 assert rows and rows[-1]['kind']=='finish'
 for index,row in enumerate(rows[:-1]):
  assert row['kind']=='record' and row['index']==index and row['observer_clock']==CLOCK
  assert row['serial_byte_start']==offset;end=row['serial_byte_end'];assert offset<end<=len(raw)
  line=raw[offset:end];assert line.endswith(b'\n') and b'\n' not in line[:-1] and sha(line)==row['sha256']
  began=row['read_started_ns'];observed=row['read_completed_ns'];eof=row['last_eof_observed_ns']
  assert start<=began<=observed and observed>=last and (eof is None or isinstance(eof,int) and eof<=began)
  records.append((line.decode(),row));offset=end;last=observed
 end=rows[-1];assert end['observer_clock']==CLOCK and end['observer_timestamp_ns']>=last
 assert end['serial_bytes']==len(raw) and end['complete_records']==len(records) and end['incomplete_tail_bytes']==len(raw)-offset and b'\n' not in raw[offset:]
 def exact(prefix,value):
  matches=[row for text,row in records if text.startswith(prefix) and json.loads(text[len(prefix):])==value]
  assert len(matches)==1;return matches[0]
 stages=[{'stage':text[len('CONDUIT_BOOT_STAGE '):].rstrip('\r\n'),'observed_ns':row['read_completed_ns'],'serial_byte_start':row['serial_byte_start']} for text,row in records if text.startswith('CONDUIT_BOOT_STAGE ')]
 assert stages==latency['boot_stage_observations'];ready=[x for x in stages if x['stage']=='front-door-ready'];assert len(ready)==1
 assert latency['host_observer_to_front_door_ready_ns']==ready[0]['observed_ns']-start
 capture=[x for x in manifest['checkpoints'] if x['checkpoint']=='front-door-ready'];assert len(capture)==1
 assert start<=capture[0]['capture_started_ns']<=capture[0]['capture_completed_ns']==latency['first_framebuffer_capture_completed_ns']
 assert capture[0]['frame']==latency['first_framebuffer']
 trace=[];last=start
 for text in (root/'journey-qmp.log').read_text().splitlines():
  row=json.loads(text);assert row['observer_clock']==CLOCK and row['direction'] in ['command','response'];assert row['observer_timestamp_ns']>=last;last=row['observer_timestamp_ns'];trace.append((row,json.loads(row['message'])))
 commands=[]
 for index,(row,message) in enumerate(trace):
  if row['direction']!='command' or message.get('execute')!='input-send-event':continue
  downs=[event for event in message['arguments']['events'] if event['type']=='btn' and event['data'].get('button')=='left' and event['data'].get('down') is True]
  if not downs:continue
  assert len(downs)==1;following=[]
  for answer,payload in trace[index+1:]:
   if answer['direction']=='command':break
   if payload.get('id')==message['id']:following.append((answer,payload))
  assert len(following)==1 and 'return' in following[0][1] and 'error' not in following[0][1]
  assert following[0][0]['observer_timestamp_ns']>=row['observer_timestamp_ns'];commands.append((message['id'],row['observer_timestamp_ns'],following[0][0]['observer_timestamp_ns']))
 actions=proof['pointer_actions'];assert actions['status']=='passed' and len(commands)==len(actions['presses'])==len(latency['pointer_actions'])==2 and len(actions['source_changes'])==4
 previous_sequence=previous_end=-1
 for index,(command,press,metric) in enumerate(zip(commands,actions['presses'],latency['pointer_actions'])):
  change=actions['source_changes'][index+2];physical=exact('CONDUIT_NATIVE_POINTER ',press);presented=exact('CONDUIT_CRECHE_CHANGE ',change)
  assert press['primary_pressed'] is True and press['press_edge'] is True and press['sequence']>previous_sequence
  for left,right in [('face_id','before_face_id'),('show_id','before_show_id'),('face_revision','before_revision')]:assert press[left]==change[right]
  assert all(isinstance(change[k],str) and change[k] for k in ['before_face_id','after_face_id','before_show_id','after_show_id']) and change['before_show_id']!=change['after_show_id']
  assert previous_end<physical['serial_byte_start']<presented['serial_byte_start'] and command[1]<=physical['read_completed_ns']
  expected={'qmp_id':command[0],'command_ns':command[1],'qmp_accepted_ns':command[2],'physical_sequence':press['sequence'],'before_show_id':change['before_show_id'],'after_show_id':change['after_show_id'],'command_to_pointer_render_observed_ns':physical['read_completed_ns']-command[1],'command_to_checkbox_present_observed_ns':presented['read_completed_ns']-command[1],'pointer_serial_byte_start':physical['serial_byte_start'],'change_serial_byte_start':presented['serial_byte_start']}
  assert metric==expected;previous_sequence=press['sequence'];previous_end=presented['serial_byte_start']
 return {'source':context['source_commit'],'image_sha256':context['image_sha256'],'checkpoints_verified':22,'png_hashes_and_dimensions_verified':22,'independent_rgba_pixel_digests_verified':22,'serial_records_verified':len(records),'incomplete_serial_tail_bytes':end['incomplete_tail_bytes'],'pointer_rows_reconstructed':2,'scope':'Relocated raw evidence and exact host-observation reconstruction. PNG integrity, dimensions and all 22 RGBA pixel digests independently decoded with Pillow. Conformance-only run; no accepted performance comparison or physical display latency claim.'}
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('capture',type=Path);p.add_argument('--output',type=Path);a=p.parse_args();result=verify(a.capture);text=json.dumps(result,indent=2)+'\n'
 if a.output:
  with a.output.open('x') as f:f.write(text)
 else:print(text,end='')
