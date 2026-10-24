"""One guarded new terminal Add after the successful same-Body browser Add."""
from pathlib import Path
import os,json,hashlib,subprocess,time,fcntl,signal
os.umask(0o077)
w=Path(__file__).resolve().parent
live=w/'6335-todo-live-fa59a9cdf-shortqmp'
source='fa59a9cdf6cf2af69cec86973e09865cb1f25ec1'
state=Path(json.loads((live/'owner-state-location.json').read_text())['private_state'])
installation=json.loads((state/'installation.json').read_text())
binary=installation['product_executable']
assert installation['release_source_identity']==source
assert json.loads((w/'6335-todo-dispatch.json').read_text())['status']=='command-passed-independent-review-pending'
assert json.loads((w/'6335-todo-fa59-independent-review.json').read_text())['status']=='PASS scoped Todo supplement'
expected=json.loads((live/'owner-before-invitation.json').read_text())['truth']['last_execution']
prior=json.loads((state/'body/owner-execution.json').read_text())['last_execution']
assert prior==expected and prior['verified'] and prior['read_terminal']=='Completed'
assert prior['write']['terminal']=='Completed'
assert prior['restored_fore_sha256']==prior['write']['committed_fore_sha256']
body=json.loads((live/'todo-committed.json').read_text())['bodyId']
browser=json.loads((live/'capture/browser-add-evidence.json').read_text())
equipment=w/'6335-todo-proof/coordinate.mjs'
equipment_digest=hashlib.sha256(equipment.read_bytes()).hexdigest()
assert equipment_digest==json.loads((w/'6335-todo-proof/equipment-ready.json').read_text())['files']['coordinate.mjs']
assert "await addForm.getByRole('button',{name:addAction.name,exact:true}).click();" in equipment.read_text()
browser_write=prior['write'];browser_interaction=browser_write['initiating_action'];browser_show=browser_write['initiating_show']
assert prior['body_id']==body and browser_interaction['action_id']=='todo.add'
assert browser_interaction['show_id']==browser['before_show']
assert bytes(browser_interaction['arguments'][0]['value']).decode()==browser['text']
browser_face=json.loads((live/'capture/owner-before-add.json').read_text())['face']
browser_action=next(a for a in browser_face['actions'] if a['identity']=='todo.add')
for name in ('host-owner.lock','body-owner.lock'):
 with (state/name).open('r+') as lock:fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
out=w/'5372-current-terminal-fa59';out.mkdir(mode=0o700)
(out/'original-browser-write.json').write_text(json.dumps(browser_write,indent=2))
logs=[(out/'service.stdout').open('wb'),(out/'service.stderr').open('wb')]
service=subprocess.Popen([binary,'host','service','run','--state-dir',str(state)],stdout=logs[0],stderr=logs[1],start_new_session=True)
def owner(*args):
 result=subprocess.run([binary,*args,'--state-dir',str(state)],text=True,capture_output=True,timeout=10)
 if result.returncode:
  (out/'failed-owner.stderr').write_text(result.stderr)
  raise RuntimeError('Public owner command refused; private diagnostics retained')
 return json.loads(result.stdout)
try:
 deadline=time.monotonic()+15
 while True:
  assert service.poll() is None,'owner service terminated'
  try:before=owner('body','face','--json');break
  except (RuntimeError,subprocess.TimeoutExpired):
   if time.monotonic()>=deadline:raise
   time.sleep(.1)
 assert before['schema']=='conduit.body/local-face-snapshot@1'
 assert before['face']['basis']['body_id']==body
 items=[s for s in before['face']['subjects'] if s['identity'].startswith('todo/item/')]
 assert len(items)==1
 terminal_action=next(a for a in before['face']['actions'] if a['identity']=='todo.add')
 assert terminal_action==browser_action,'complete canonical action contract changed'
 name='Verify current terminal counterpart Add'
 assert not any(s['name']==name for s in items)
 keys='wardrobe wear\nwardrobe prefer\nshow\nevidence\nactions\naction todo.add '+name+'\nevidence\nquit\n'
 (out/'terminal.input').write_text(keys)
 (out/'canonical-before-terminal.json').write_text(json.dumps(before['face'],indent=2))
 result=subprocess.run([binary,'body','terminal','--owner-show','--state-dir',str(state)],input=keys,text=True,capture_output=True,timeout=40)
 (out/'terminal.stdout').write_text(result.stdout);(out/'terminal.stderr').write_text(result.stderr)
 assert result.returncode==0 and 'Action refused:' not in result.stdout
 assert result.stdout.count('Todo saved at revision')==1
 after=owner('body','face','--json')
 assert after['face']['basis']['body_id']==body
 items=[s for s in after['face']['subjects'] if s['identity'].startswith('todo/item/')]
 assert len(items)==2 and sum(s['name']==name for s in items)==1
 current=json.loads((state/'body/owner-execution.json').read_text())['last_execution']
 write=current['write'];interaction=write['initiating_action'];show=write['initiating_show']
 assert current['verified'] and current['read_terminal']=='Completed' and write['terminal']=='Completed'
 assert interaction['action_id']=='todo.add' and interaction['target']==browser_interaction['target']=='todo/list'
 assert bytes(interaction['arguments'][0]['value']).decode()==name
 assert write['body_id']==current['body_id']==body
 assert current['restored_fore_sha256']==write['committed_fore_sha256']
 evidence=[json.loads(line[len('Owner terminal evidence '):]) for line in result.stdout.splitlines() if line.startswith('Owner terminal evidence ')]
 initiating=next(e for e in evidence if e['show_id']==interaction['show_id'])
 assert initiating['owner_selected_show_current'] and initiating['show_state']=='available'
 assert initiating['face_id']==interaction['face_id']
 assert initiating['mask_plan_id']==show['planned_mask']['plan']['plan_id']
 assert browser_show['planned_mask']['plan']['plan_id']!=initiating['mask_plan_id']
 assert browser_interaction['show_id']!=initiating['show_id']
 record={'schema':'conduit.proof/mask-correspondence-continuation@1','source':source,'body_id':body,
  'browser_input':'trusted browser pointer click; exact frozen6335 coordinator and Add receipt retained','browser_producer_sha256':equipment_digest,
  'terminal_input':'one scripted public owner-terminal action command',
  'canonical_action_contract_identical':True,'canonical_action':browser_action,
  'prior_items':1,'terminal_items':2,'terminal_mutations':1,
  'browser_write':browser_write,'browser_read':{k:v for k,v in prior.items() if k!='write'},
  'terminal_show':initiating,'terminal_write':write,'terminal_read':{k:v for k,v in current.items() if k!='write'},
  'resulting_face_id':after['face']['identity'],
  'scope':'Same actual installed Body after verified browser Add and native read-only Show; fresh owner Boot and independently admitted terminal Mask. Different current Face revisions/Shows, no physical/human claim; one new command, no uncertain replay.'}
 (out/'receipt.json').write_text(json.dumps(record,indent=2))
 (out/'canonical-after-terminal.json').write_text(json.dumps(after['face'],indent=2))
 print(json.dumps({'status':'passed','source':source,'terminal_mutations':1,'items':2}))
finally:
 if service.poll() is None:
  os.killpg(service.pid,signal.SIGTERM)
  try:service.wait(timeout=10)
  except subprocess.TimeoutExpired:os.killpg(service.pid,signal.SIGKILL);service.wait(timeout=5)
 for log in logs:log.close()
