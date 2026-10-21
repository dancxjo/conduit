import json,hashlib,subprocess,time,os
from pathlib import Path
base=Path('/home/dancxjo/Documents/Codex/2026-10-21/goal-find-the-first-available-github')
prior=base/'work/6209-live-component/browser'
r=json.loads((prior/'receipt.json').read_text());last=json.loads((prior/'add-2-mutation.json').read_text())
state=Path((base/'work/6209-state.path').read_text().strip())
install=json.loads((state/'installation.json').read_text())
binary=install['product_executable'];source='a2b2f631aa9c784608502ebab95b2f5cd72d52d7'
assert install['release_source_identity']==r['owner_source_commit']==r['browser_runtime_source_commit']==r['handbook_ui_source_commit']==source
assert r['source_relation']=='exact-source' and len(r['adds'])==2
assert r['adds'][0]['activation']=='native-pointer-click'
execution=json.loads((state/'body/owner-execution.json').read_text())['last_execution']
assert execution['verified'] and execution['body_id']==r['body_id']
assert execution['restored_fore_sha256']==last['committed_fore_sha256']
assert execution['write']['interaction_id']==last['interaction_id']
out=base/'work/5372-correspondence-continuation'
out.mkdir(mode=0o700) # Refuse reuse: never replay an uncertain action.
log=(out/'service.stderr').open('wb')
service=subprocess.Popen([binary,'host','service','run','--state-dir',str(state)],stdout=subprocess.DEVNULL,stderr=log)
def owner(*args):
 p=subprocess.run([binary,*args,'--state-dir',str(state)],capture_output=True,text=True,timeout=10)
 assert p.returncode==0,p.stderr
 return json.loads(p.stdout)
try:
 deadline=time.monotonic()+10
 while True:
  assert service.poll() is None,'owner service terminated'
  try:before=owner('body','face','--json');break
  except (AssertionError,subprocess.TimeoutExpired):
   if time.monotonic()>=deadline:raise
   time.sleep(.1)
 assert before['face']['basis']['body_id']==r['body_id']
 items=[s for s in before['face']['subjects'] if s['role']=='Item']
 assert len(items)==2
 name='Verify terminal counterpart Add'
 assert not any(s['name']==name for s in items)
 action=next(a for a in before['face']['actions'] if a['identity']=='todo.add')
 browser_action=next(a for a in r['adds'][0]['before']['actions'] if a['identity']=='todo.add')
 assert action['target']==browser_action['target']=='todo/list'
 assert action['intent']==browser_action['intent']
 keys='wardrobe wear\nwardrobe prefer\nshow\nevidence\nactions\naction todo.add '+name+'\nevidence\nquit\n'
 (out/'terminal.input').write_text(keys)
 (out/'before-face.json').write_text(json.dumps(before))
 p=subprocess.run([binary,'body','terminal','--owner-show','--state-dir',str(state)],input=keys,text=True,capture_output=True,timeout=30)
 (out/'terminal.stdout').write_text(p.stdout);(out/'terminal.stderr').write_text(p.stderr)
 assert p.returncode==0,p.stderr
 assert 'Action refused:' not in p.stdout
 assert p.stdout.count('Todo saved at revision')==1
 after=owner('body','face','--json')
 assert after['face']['basis']['body_id']==r['body_id']
 after_items=[s for s in after['face']['subjects'] if s['role']=='Item']
 assert len(after_items)==3 and sum(s['name']==name for s in after_items)==1
 current=json.loads((state/'body/owner-execution.json').read_text())['last_execution']
 write=current['write'];interaction=write['initiating_action'];show=write['initiating_show']
 assert current['verified'] and current['read_terminal']=='Completed' and write['terminal']=='Completed'
 assert interaction['action_id']=='todo.add' and interaction['target']=='todo/list'
 assert bytes(interaction['arguments'][0]['value']).decode()==name
 assert write['body_id']==current['body_id']==r['body_id']
 assert current['restored_fore_sha256']==write['committed_fore_sha256']
 evidence=[json.loads(line[len('Owner terminal evidence '):]) for line in p.stdout.splitlines() if line.startswith('Owner terminal evidence ')]
 initiating=next(e for e in evidence if e['show_id']==interaction['show_id'])
 assert initiating['owner_selected_show_current'] and initiating['show_state']=='available'
 assert initiating['face_id']==interaction['face_id'] and initiating['mask_plan_id']==show['planned_mask']['plan']['plan_id']
 browser=r['adds'][0]['before']
 assert browser['mask_plan_id']!=initiating['mask_plan_id'] and browser['show_id']!=initiating['show_id']
 record={'schema':'conduit.proof/mask-correspondence-continuation@1','source':source,'body_id':r['body_id'],
  'prior_browser_receipt_sha256':hashlib.sha256((prior/'receipt.json').read_bytes()).hexdigest(),
  'prior_checkpoint_sha256':last['committed_fore_sha256'],'prior_mutations':2,'terminal_mutations':1,
  'action_id':'todo.add','target':'todo/list','browser_input':'trusted native pointer click','terminal_input':'scripted owner terminal action command',
  'browser_show':{k:browser[k] for k in ['face_id','face_revision','mask_plot_id','mask_plan_id','mask_play_id','show_id','route']},
  'terminal_show':initiating,'terminal_interaction_id':write['interaction_id'],
  'write_plan_id':write['plan_id'],'write_play':write['play'],'write_sign':write['terminal_sign'],
  'read_plan_id':current['read_plan_id'],'read_play':current['read_play'],'read_sign':current['read_terminal_sign'],
  'resulting_face_id':after['face']['identity'],'committed_fore_sha256':write['committed_fore_sha256'],
  'scope':'Guarded continuation of the same known-successful source-pinned installation after service restart; distinct current Faces/Shows, same Body and offered action. Scripted terminal output/input, not human physical input. No old action replay.'}
 (out/'receipt.json').write_text(json.dumps(record,indent=2)+'\n')
 print(json.dumps({'status':'passed','body_id':r['body_id'],'accepted_terminal_adds':1}))
finally:
 service.terminate()
 try:service.wait(timeout=5)
 except subprocess.TimeoutExpired:service.kill();service.wait()
 log.close()
