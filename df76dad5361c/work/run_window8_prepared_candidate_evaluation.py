"""Sequential actual pinned evaluation; refuses any changed input or prior output."""
import hashlib, json, os, pathlib, subprocess, sys, time
cwd = pathlib.Path(__file__).resolve().parent.parent
project = pathlib.Path('/home/dancxjo/Documents/Codex/2026-10-06/goal-please-finish-and-close-4907')
inputs = json.loads((cwd/'work/window8-vocative-evaluation-inputs.json').read_text())
selected = json.loads((cwd/'work/window8-vocative-runner-selected-inputs.json').read_text())
for name, item in inputs['files'].items():
    assert hashlib.sha256(pathlib.Path(name).read_bytes()).hexdigest() == item['sha256'], name
for name, digest in selected.items():
    assert hashlib.sha256((cwd/name).read_bytes()).hexdigest() == digest, name
binary = pathlib.Path(sys.argv[1]).resolve(strict=True)
root = project/'outputs/native-admission-family/vocative-candidate-08425a-prepared-r2'
root.mkdir(exist_ok=False)
proof = {'inputs': inputs, 'selected_runner_files': selected, 'binary': str(binary), 'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(), 'native_output_admission_opt_in': True, 'boundary': 'Actual candidate evaluation; no production model replacement; 8 authored cases and narrow 3DEV/5TEST profile slice only'}
(root/'launch-receipt.json').write_text(json.dumps(proof,indent=2)+'\n')
base = project/'work/window8'
model = base/'v3-vocative-candidate-pinned-teaching'
teaching = cwd/'semantics/language/training/ewt_joint_v3_window8/vocative_training.json'
for split in ['authored_vocative','dev','test']:
    env = os.environ.copy()
    env.update(WINDOW8_MODEL_DIR=str(model), WINDOW8_TEACHING_ROWS=str(teaching), WINDOW8_EVALUATION_ROWS=str(base/'vocative-evaluation-slice'/f'{split}.json'), WINDOW8_EVALUATION_OUTPUT=str(root/split), WINDOW8_PREPARED_NATIVE='1')
    started = time.time()
    with (root/f'{split}.log').open('w') as log:
        result = subprocess.run([str(binary),'actual_cached_window8_reviewed_clause_decode','--exact','--ignored','--nocapture'],cwd=cwd,env=env,stdout=log,stderr=subprocess.STDOUT)
    (root/f'{split}-terminal.json').write_text(json.dumps({'exit_code':result.returncode,'elapsed_seconds':time.time()-started},indent=2)+'\n')
    print(split, 'terminal', result.returncode, flush=True)
    if result.returncode: sys.exit(result.returncode)
