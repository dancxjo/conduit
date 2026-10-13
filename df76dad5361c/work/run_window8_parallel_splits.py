"""Deduplicate paused sequential orchestration; preserve all three natural exits."""
import hashlib, json, os, pathlib, signal, subprocess, time
cwd=pathlib.Path(__file__).resolve().parent.parent
project=pathlib.Path('/home/dancxjo/Documents/Codex/2026-10-06/goal-please-finish-and-close-4907')
root=project/'outputs/native-admission-family/vocative-candidate-08425a-prepared-r2'
original_parent=2898758
authored=2898759
def stat(pid):
    fields=pathlib.Path(f'/proc/{pid}/stat').read_text().rsplit(')',1)[1].split()
    return {'state':fields[0],'parent':int(fields[1]),'start_ticks':int(fields[19]),'wait_status':int(fields[49])}
def cmd(pid):return pathlib.Path(f'/proc/{pid}/cmdline').read_bytes().replace(b'\0',b' ').decode()
inputs=json.loads((cwd/'work/window8-vocative-evaluation-inputs.json').read_text())
for name,item in inputs['files'].items():assert hashlib.sha256(pathlib.Path(name).read_bytes()).hexdigest()==item['sha256'],name
selected=json.loads((cwd/'work/window8-vocative-runner-selected-inputs.json').read_text())
for name,digest in selected.items():assert hashlib.sha256((cwd/name).read_bytes()).hexdigest()==digest,name
launch=json.loads((root/'launch-receipt.json').read_text())
binary=pathlib.Path(launch['binary'])
assert hashlib.sha256(binary.read_bytes()).hexdigest()==launch['binary_sha256']
authored_start=stat(authored)
assert authored_start['parent']==original_parent and authored_start['state'] not in ['Z','X']
assert 'run_window8_prepared_candidate_evaluation.py' in cmd(original_parent)
assert 'parser_window8_cached_corpus_model-a0684c2a8dd1523d' in cmd(authored)
parent_start=stat(original_parent)
assert not (root/'dev').exists() and not (root/'test').exists()
os.kill(original_parent,signal.SIGSTOP)
for _ in range(100):
    if stat(original_parent)['state']=='T':break
    time.sleep(.01)
assert stat(original_parent)['state']=='T'
assert stat(authored)['start_ticks']==authored_start['start_ticks']
control={'original_orchestration_parent':original_parent,'original_parent_start_ticks':parent_start['start_ticks'],'authored_child':authored,'authored_start_ticks':authored_start['start_ticks'],'action':'SIGSTOP parent only; authored child continues unchanged; DEV and TEST launched independently; pending original queue cannot run','binary_sha256':launch['binary_sha256'],'selected_inputs':selected,'control_started_unix':time.time(),'receipt_scope':'All three processes retain frozen previous Core accounting; no generic repaired resource-proof claim'}
(root/'parallel-orchestration-control.json').write_text(json.dumps(control,indent=2)+'\n')
children={};logs={}
for split in ['dev','test']:
    output=root/split
    output.mkdir(exist_ok=False)
    env=os.environ.copy();env.update(WINDOW8_MODEL_DIR=str(project/'work/window8/v3-vocative-candidate-pinned-teaching'),WINDOW8_TEACHING_ROWS=str(cwd/'semantics/language/training/ewt_joint_v3_window8/vocative_training.json'),WINDOW8_EVALUATION_ROWS=str(project/f'work/window8/vocative-evaluation-slice/{split}.json'),WINDOW8_EVALUATION_OUTPUT=str(output),WINDOW8_PREPARED_NATIVE='1')
    logs[split]=(root/f'{split}.log').open('w')
    child=subprocess.Popen([str(binary),'actual_cached_window8_reviewed_clause_decode','--exact','--ignored','--nocapture'],cwd=cwd,env=env,stdout=logs[split],stderr=subprocess.STDOUT)
    children[split]={'process':child,'start':time.time()}
    control[split+'_child']=child.pid
(root/'parallel-orchestration-control.json').write_text(json.dumps(control,indent=2)+'\n')
print('parallel split children', {k:v['process'].pid for k,v in children.items()},flush=True)
authored_done=False
while children or not authored_done:
    if not authored_done:
        state=stat(authored)
        assert state['start_ticks']==authored_start['start_ticks']
        if state['state']=='Z':
            code=os.waitstatus_to_exitcode(state['wait_status'])
            elapsed=float(pathlib.Path('/proc/uptime').read_text().split()[0])-state['start_ticks']/os.sysconf('SC_CLK_TCK')
            receipt={'exit_code':code,'raw_wait_status':state['wait_status'],'authoritative_status':'Linux /proc zombie stat exit_code before retiring paused parent','elapsed_seconds':elapsed,'authored_child':authored,'start_ticks':state['start_ticks'],'log':'authored_vocative.log'}
            (root/'authored_vocative-terminal.json').write_text(json.dumps(receipt,indent=2)+'\n')
            assert stat(original_parent)['start_ticks']==parent_start['start_ticks'] and stat(original_parent)['state']=='T'
            os.kill(original_parent,signal.SIGKILL)
            control['original_parent_retired_after_authored_natural_exit']=True
            control['authored_exit_code']=code
            (root/'parallel-orchestration-control.json').write_text(json.dumps(control,indent=2)+'\n')
            authored_done=True
            print('authored_vocative terminal',code,flush=True)
    for split,item in list(children.items()):
        code=item['process'].poll()
        if code is not None:
            logs[split].close()
            (root/f'{split}-terminal.json').write_text(json.dumps({'exit_code':code,'elapsed_seconds':time.time()-item['start'],'child':item['process'].pid,'authoritative_status':'subprocess wait result'},indent=2)+'\n')
            print(split,'terminal',code,flush=True)
            del children[split]
    time.sleep(1)
