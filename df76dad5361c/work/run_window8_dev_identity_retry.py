import hashlib,json,os,pathlib,subprocess,time
cwd=pathlib.Path(__file__).resolve().parent.parent
project=pathlib.Path('/home/dancxjo/Documents/Codex/2026-10-06/goal-please-finish-and-close-4907')
inputs=json.loads((cwd/'work/window8-vocative-evaluation-inputs.json').read_text())
for name,item in inputs['files'].items():assert hashlib.sha256(pathlib.Path(name).read_bytes()).hexdigest()==item['sha256'],name
selected=json.loads((cwd/'work/window8-vocative-runner-selected-inputs.json').read_text())
for name,digest in selected.items():assert hashlib.sha256((cwd/name).read_bytes()).hexdigest()==digest,name
retry=cwd/'semantics/language/tests/parser_window8_dev_identity_retry.rs'
published=pathlib.Path('/home/dancxjo/conduit-4907-evaluation-identity/semantics/language/tests/parser_window8_cached_corpus_model.rs')
assert retry.read_bytes()==published.read_bytes()
binary=cwd/'target/debug/deps/parser_window8_dev_identity_retry-19eae81c269e985a'
root=project/'outputs/native-admission-family/vocative-candidate-08425a-dev-r3'
root.mkdir(exist_ok=False)
receipt={'boundary':'All3 DEV rows retried after identifier-framing failure; no denominator change, no omission of inexact rows, authored/TEST remain frozen; original evaluation IDs retained in rows','identity_fix_commit':'440512f82','retry_source_sha256':hashlib.sha256(retry.read_bytes()).hexdigest(),'binary':str(binary),'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'unchanged_baseline_selected_files':selected,'inputs':inputs,'started_unix':time.time(),'resource_scope':'Frozen baseline SDK accounting with measured slack; no repaired generic resource-proof claim'}
(root/'launch-receipt.json').write_text(json.dumps(receipt,indent=2)+'\n')
env=os.environ.copy();env.update(WINDOW8_MODEL_DIR=str(project/'work/window8/v3-vocative-candidate-pinned-teaching'),WINDOW8_TEACHING_ROWS=str(cwd/'semantics/language/training/ewt_joint_v3_window8/vocative_training.json'),WINDOW8_EVALUATION_ROWS=str(project/'work/window8/vocative-evaluation-slice/dev.json'),WINDOW8_EVALUATION_OUTPUT=str(root/'dev'),WINDOW8_PREPARED_NATIVE='1')
with (root/'dev.log').open('w') as log:
 result=subprocess.run([str(binary),'actual_cached_window8_reviewed_clause_decode','--exact','--ignored','--nocapture'],cwd=cwd,env=env,stdout=log,stderr=subprocess.STDOUT)
(root/'dev-terminal.json').write_text(json.dumps({'exit_code':result.returncode,'elapsed_seconds':time.time()-receipt['started_unix'],'authoritative_status':'subprocess wait result'},indent=2)+'\n')
print('DEV-r3 terminal',result.returncode,flush=True)
raise SystemExit(result.returncode)
