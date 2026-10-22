import json,hashlib,gzip,subprocess,shutil,re
from pathlib import Path
base=Path('/home/dancxjo/Documents/Codex/2026-10-21/please-finish-and-close-any-conduit')
root=base/'work/6252-native-fixtures'; source='0dad64726989f01b79de8f4bb4cd990b2787f040'
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()==source
assert not subprocess.check_output(['git','status','--porcelain'],cwd=root,text=True)
stage=base/'work/6252-successor-0dad-stage';assert not stage.exists()
phases={}
for name,count in [('focused',9),('catalog',3),('strict',2)]:
 p=base/f'work/6252-0dad64726-{name}-checks.json';v=json.loads(p.read_text());assert v['source']==source and len(v['checks'])==count
 assert all(x['exit_code']==0 for x in v['checks'])
 if name in ['focused','catalog']:assert all(x.get('pass_verified') is True for x in v['checks'])
 if name in ['focused','catalog']:
  for row in v['checks']:
   expected=1 if name=='focused' else row['expected_cases']
   if expected is not None:
    counts=re.findall(r'^test result: ok\. (\d+) passed; 0 failed;',Path(row['log']).read_text(),re.M)
    assert counts and sum(map(int,counts))==expected
 phases[name]=(p,v)
sdk_path=base/'work/6252-0dad64726-sdk-check.json';sdk=json.loads(sdk_path.read_text());assert sdk['source']==source and sdk['exit_code']==0 and sdk['counts']['tests']==6 and sdk['counts']['pass']==6 and all(sdk['counts'][k]==0 for k in ['fail','cancelled','skipped','todo'])
catalog=base/'work/6252-0dad64726-catalog.json';v=json.loads(catalog.read_text());patchbay=next(x for x in v['plots'] if x['slug']=='patchbay');assert len(v['plots'])==94 and patchbay['source']==(root/'plots/patchbay/main.conduit').read_text() and patchbay['required_kinds']==['patchbay/accept-interaction']
strict=phases['strict'][1]['checks'][-1]['command'];assert all(x in strict for x in ['conduitos','conduit-browser-runtime','xtask','--all-targets','conduitos/native-compositor','-D','warnings'])
stage.mkdir();stored=[]
def retain(path,dest=None,compress=False):
 raw=path.read_bytes();name=dest or path.name;out=stage/(name+('.gz' if compress else ''));out.parent.mkdir(parents=True,exist_ok=True);assert not out.exists()
 out.write_bytes(gzip.compress(raw,mtime=0) if compress else raw)
 if compress:assert gzip.decompress(out.read_bytes())==raw
 stored.append(dict(file=str(out.relative_to(stage)),original_path=str(path),raw_bytes=len(raw),raw_sha256=hashlib.sha256(raw).hexdigest(),stored_bytes=out.stat().st_size,stored_sha256=hashlib.sha256(out.read_bytes()).hexdigest()))
for p,v in phases.values():
 retain(p)
 for x in v['checks']:retain(Path(x['log']),compress=True)
retain(sdk_path);retain(base/'work/6252-0dad64726-sdk-errors.log',compress=True);retain(catalog,compress=True)
for name in ['6252-run-0dad-focused.py','6252-run-0dad-catalog-and-strict.py','6252-run-0dad-strict.py','6252-run-0dad-all.py','6252-native-fixture-peer-review.md']:retain(base/'work'/name)
negative_receipts=[]
p=base/'work/6252-373527c72-focused-checks.json';v=json.loads(p.read_text());assert v['source']=='373527c72ae571b5f17a03903e11b2aef7a70502' and v['checks'][-1]['pass_verified'] is False
retain(p,'prior-selection/'+p.name)
for row in v['checks']:retain(Path(row['log']),'prior-selection/'+Path(row['log']).name,True)
negative_receipts.append(dict(source=v['source'],meaning='renamed owner choice test selected zero cases; runner refused pass; product did not fail',receipt='prior-selection/'+p.name))
app_path=base/'work/6252-0dad64726-app-core-check.json';app=json.loads(app_path.read_text());assert app['source']==source and app['exit_code']==0 and app['pass_verified'] is True and app['harness_cases']==45 and app['fixed_grant_marker'] is True
app_log=Path(app['log']);text=app_log.read_text();assert sum(map(int,re.findall(r'^test result: ok\. (\d+) passed; 0 failed;',text,re.M)))==45 and 'data::codec::tests::reusable_workspace_preserves_exact_wire_and_graph_custody_bytes ... ok' in text and 'SKIP' not in text
bitmap=re.search(r'PASS exact bitmap grant=(\d+) Init peak=(\d+) Graph/ProjectGraph peak=(\d+) canonical peak=(\d+) combined rounded-live-headroom=(\d+)',text);assert bitmap
bitmap=dict(zip(['grant_bytes','init_peak_bytes','graph_peak_bytes','canonical_peak_bytes','combined_headroom_bytes'],map(int,bitmap.groups())))
retain(app_path);retain(app_log,compress=True)
resource_path=base/'work/6252-0dad64726-resource-check.json';resource=json.loads(resource_path.read_text());assert resource['source']==source and resource['exit_code']==0 and resource['counts']['tests']==4 and resource['counts']['pass']==4 and all(resource['counts'][k]==0 for k in ['fail','cancelled','skipped','todo'])
retain(resource_path);retain(base/'work/6252-0dad64726-resource-tests.log',compress=True)
for commit in ['1dfd0ec64593663a8876624741a531b107029ce2','84dcb6ec58e71f82099809eed590f66d60740c2e','29617cc14fcc8213d7202a772aeda36d311d13b6','073882803f516f437d707af4ca6fcc4bb4fe1b9b']:subprocess.run(['git','merge-base','--is-ancestor',commit,source],cwd=root,check=True)
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()==source
assert not subprocess.check_output(['git','status','--porcelain'],cwd=root,text=True)
receipt=dict(source=source,tree=subprocess.check_output(['git','rev-parse',source+'^{tree}'],cwd=root,text=True).strip(),source_clean=True,focused_cases=9,catalog_cases=3,sdk_cases=6,application_core_cases=45,resource_cases=4,total_fresh_cases=67,fixed_grant_host_executable=bitmap,catalog_plots=94,patchbay_source_document_id=patchbay['source_document_id'],patchbay_checked_plot_id=patchbay['checked_plot_id'],prior_negatives=negative_receipts,files=stored)
(stage/'receipt.json').write_text(json.dumps(receipt,indent=2)+'\n')
(stage/'README.md').write_text("# Joined successor after the live baseline repair\n\nClean executed Source `0dad64726989f01b79de8f4bb4cd990b2787f040` retains published baseline head1dfd, native-owner head84dcb6, the earlier checked077 recovery and the role-only packaging repair as ancestors. It keeps the owner's stronger choice/Fore/release fixtures and refreshed startup.catz; canonical current native startup/Face/Show/drawn graph receipts; distinct124 SDK executors/140 membership offers; and both real storage-pressure refusal and repeated byte Full before cancellation under unchanged production grants.\n\nAll67 fresh tests passed:9 focused native/browser/Candidate cases,45 full application-core cases including the exact retained wire/graph codec guard,6 SDK cases,4 actual resource/dependency/style package cases, and3 catalog cases. The Linux host fixed-grant executable also passed: its exact bitmap grant/peaks/headroom are retained in receipt.json and the original log. It is not a physical/QEMU heap measurement. Supported94-Plot catalog assembly, workspace formatting and combined strict all-target ConduitOS/native-compositor + browser-runtime + xtask Clippy passed. Exact commands, default-stack/two-worker environment and lossless logs are retained.\n\nThe actual40187-byte body-host module has a reviewed48KiB role ceiling; the unchanged96-resource/32MiB total loader bounds still apply. The producer tests preserve the actual dependency closure and each byte hash, reject the old bound/one-byte undersizing/missing dependency, and account for85 declared resources/22384640 maximum bytes. This is resource/producer unit proof, not a complete staged browser application or browser execution.\n\nThe earlier18-case077 checkpoint and its original failures remain in the parent packet. The recorded Candidate39787413371 browser failure (preview2329fab,40187>32768) remains in the role-specific packet. The caught renamed-test zero-selection at373 is retained here separately and is not a product failure. No prior failure is relabelled as passing.\n\nNo QEMU ran at this Source. Wide/narrow captures remain bound to b1e18996/f04edf. Todo proof's outer2MiB subprocess snapshot bound remains distinct from per-Face guest bounds. Source-bound Candidate/current product journey, normal merge and relevant Integration remain acceptance gates; #6179/#5994/#5608/#6190 and broader platform/confinement scope stay open. A later documentary head is not this executed Source.\n")
(stage/'SHA256SUMS').write_text(''.join(hashlib.sha256(p.read_bytes()).hexdigest()+'  '+str(p.relative_to(stage))+'\n' for p in sorted(stage.rglob('*')) if p.is_file() and p.name!='SHA256SUMS'))
print(json.dumps(dict(source=source,stage=str(stage),files=len(stored),fresh_cases=67)))
