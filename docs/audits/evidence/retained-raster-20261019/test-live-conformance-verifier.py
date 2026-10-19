"""Semantic corruption checks after refreshing the capture's internal hashes."""
import argparse,hashlib,importlib.util,json,tarfile,tempfile
from pathlib import Path
base=Path(__file__).resolve().parent;spec=importlib.util.spec_from_file_location('live_verifier',base/'verify-live-conformance.py');v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v)
def run(archive,scratch):
 results=[]
 with tempfile.TemporaryDirectory(prefix='live-verifier-mutations-',dir=scratch) as tmp:
  with tarfile.open(archive) as source:source.extractall(tmp,filter='data')
  root=Path(tmp)/'live-conformance-1';v.verify(root)
  targets=['journey-frames/manifest.json','journey-frames/latency.json','journey-frames/serial-observations.jsonl','journey-proof.json','journey-qmp.log'];original={name:(root/name).read_bytes() for name in targets}
  def write(name,value,lines=False):
   (root/name).write_text(''.join(json.dumps(x)+'\n' for x in value) if lines else json.dumps(value))
  def refresh():
   manifest=v.load(root/'journey-frames/manifest.json')
   for field in ['serial_observation','latency']:
    entry=manifest[field];entry['sha256']=v.sha((root/'journey-frames'/entry['file']).read_bytes())
   write('journey-frames/manifest.json',manifest)
  def trial(name,change):
   try:
    change();refresh()
    try:v.verify(root)
    except AssertionError:results.append({'mutation':name,'refused':True})
    else:raise RuntimeError('accepted mutation '+name)
   finally:
    for name,data in original.items():(root/name).write_bytes(data)
  def latency(field,value):
   d=v.load(root/'journey-frames/latency.json');d[field]=value;write('journey-frames/latency.json',d)
  trial('wrong source with refreshed latency hash',lambda:latency('source_commit','wrong'))
  def metric():
   d=v.load(root/'journey-frames/latency.json');d['pointer_actions'][0]['command_to_checkbox_present_observed_ns']+=1;write('journey-frames/latency.json',d)
  trial('changed feedback duration with refreshed latency hash',metric)
  def serial_gap():
   d=[json.loads(x) for x in (root/'journey-frames/serial-observations.jsonl').read_text().splitlines()];d[1]['serial_byte_start']+=1;write('journey-frames/serial-observations.jsonl',d,True)
  trial('serial coverage gap with refreshed observation hash',serial_gap)
  def no_finish():
   d=[json.loads(x) for x in (root/'journey-frames/serial-observations.jsonl').read_text().splitlines()];write('journey-frames/serial-observations.jsonl',d[:-1],True)
  trial('missing final drain with refreshed observation hash',no_finish)
  def clock():
   d=[json.loads(x) for x in (root/'journey-qmp.log').read_text().splitlines()];d[0]['observer_timestamp_ns']=0;write('journey-qmp.log',d,True)
  trial('QMP clock precedes observer epoch',clock)
  def stale():
   d=v.load(root/'journey-proof.json');d['pointer_actions']['presses'][1]=d['pointer_actions']['presses'][0];write('journey-proof.json',d)
  trial('reused prior pointer gesture',stale)
  def pixels():
   d=v.load(root/'journey-frames/manifest.json');d['checkpoints'][0]['frame']['pixel_sha256']='0'*64;write('journey-frames/manifest.json',d)
  trial('forged pixel digest despite genuine PNG file hash',pixels)
  def duplicate_ack():
   d=[json.loads(x) for x in (root/'journey-qmp.log').read_text().splitlines()]
   for i,row in enumerate(d):
    message=json.loads(row['message'])
    if row['direction']=='command' and message.get('execute')=='input-send-event' and any(e['type']=='btn' and e['data'].get('button')=='left' and e['data'].get('down') is True for e in message['arguments']['events']):
     j=next(j for j in range(i+1,len(d)) if d[j]['direction']=='response' and json.loads(d[j]['message']).get('id')==message['id']);d.insert(j+1,d[j]);break
   else:raise RuntimeError('missing fixture press')
   write('journey-qmp.log',d,True)
  trial('duplicate acknowledgement within one QMP request lifetime',duplicate_ack)
  v.verify(root)
 return {'archive_sha256':hashlib.sha256(archive.read_bytes()).hexdigest(),'cases':results,'all_refused':all(x['refused'] for x in results),'scope':'Semantic verifier tests on relocated capture, with internal latency/observation hashes refreshed. Archive-container custody is verified separately. Original capture restored and reverified after mutations.'}
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('archive',type=Path);p.add_argument('--scratch',type=Path);p.add_argument('--output',type=Path);a=p.parse_args();text=json.dumps(run(a.archive,a.scratch),indent=2)+'\n'
 if a.output:
  with a.output.open('x') as f:f.write(text)
 else:print(text,end='')
