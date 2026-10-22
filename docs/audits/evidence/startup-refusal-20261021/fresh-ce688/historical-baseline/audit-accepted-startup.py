from pathlib import Path
import hashlib,json
r=Path('/home/dancxjo/Documents/Codex/2026-10-21/please-finish-and-close-any-conduit');p=r/'work/6180-integration-12400001216/packet/conduitos-x86_64/evidence/journey-serial.log';q=p.with_name('journey-qmp.log');lines=p.read_text().splitlines()
expected=[('wake','begin'),('wake','accepted'),('plan','begin'),('plan','accepted'),('play','begin'),('play','accepted')]
stages=[{'line':n,'stage':l.split()[1],'outcome':l.split()[2]} for n,l in enumerate(lines,1) if l.startswith('CONDUIT_STARTUP_STAGE ')]
assert [(x['stage'],x['outcome']) for x in stages]==expected
assert lines.count('CONDUIT_CRECHE_CHECKPOINT body-born-lulled')==1
records=[]
for n in [375,462,584,614]:
 tag,raw=lines[n-1].split(' ',1);assert tag=='CONDUIT_PRODUCT_JOURNEY';j=json.loads(raw);records.append({'line':n,**{k:j[k] for k in ['status','revision','body_id','wake_id','plan_id','active_play_id']}})
assert [x['revision'] for x in records]==[3,4,5,6]
assert [x['status'] for x in records]==['born-lulled','awake','planned','quiescent-awaiting-input']
assert len({x['body_id'] for x in records})==1 and len({x['plan_id'] for x in records[1:]})==1
assert all(x['active_play_id'] is None for x in records[:-1]) and records[-1]['active_play_id']
qs=q.read_text().splitlines();start=json.loads(qs[113]);end=json.loads(qs[117]);s=json.loads(start['message']);e=json.loads(end['message'])
assert s['execute']=='input-send-event' and s['arguments']['events'][0]['data']['down'] is True and s['arguments']['events'][0]['data']['key']['data']=='ret'
assert e['execute']=='screendump' and e['arguments']['filename'].endswith('/body-playing.ppm')
ns=end['observer_elapsed_ns']-start['observer_elapsed_ns'];assert ns==3986458450
out={'status':'verified','source':'f3e582ce98ede25394a392de4ed06e0b6b086133','integration_run':39715578317,'packet_files':{str(z.relative_to(r)):{'bytes':z.stat().st_size,'sha256':hashlib.sha256(z.read_bytes()).hexdigest()} for z in [p,q]},'stages':stages,'journey_records':records,'observer_command_interval':{'birth_enter_qmp_line':114,'body_playing_capture_qmp_line':118,'nanoseconds':ns,'seconds':ns/1e9,'scope':'QMP key-down command to screendump command; includes observer overhead; not Birth ACK to Play ACK, calibrated guest latency or speedup'},'scope':'Historical accepted normal QEMU startup only. Does not establish current ce688 execution, refusal controls or full6179 closure.'}
(r/'outputs/6179-accepted-f3-startup-baseline-audit.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps({'status':'verified','stages':len(stages),'journey_revisions':[x['revision'] for x in records],'observer_seconds':ns/1e9}))
