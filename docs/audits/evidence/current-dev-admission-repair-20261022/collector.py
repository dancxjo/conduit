import json,hashlib,gzip,subprocess,shutil,re
from pathlib import Path
base=Path('/home/dancxjo/Documents/Codex/2026-10-21/please-finish-and-close-any-conduit')
root=base/'work/6252-native-fixtures'; source='077a2a535b4ec30d1f09ec6c2884c22bbffd1c57'
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()==source
assert not subprocess.check_output(['git','status','--porcelain'],cwd=root,text=True)
stage=base/'work/6252-joined-evidence-stage';assert not stage.exists()
phases={}
for name,count in [('focused',9),('catalog',3),('strict',2)]:
 p=base/f'work/6252-077a2a535-{name}-checks.json';v=json.loads(p.read_text());assert v['source']==source and len(v['checks'])==count
 assert all(x['exit_code']==0 for x in v['checks'])
 if name in ['focused','catalog']:assert all(x.get('pass_verified') is True for x in v['checks'])
 if name in ['focused','catalog']:
  for row in v['checks']:
   expected=1 if name=='focused' else row['expected_cases']
   if expected is not None:
    counts=re.findall(r'^test result: ok\. (\d+) passed; 0 failed;',Path(row['log']).read_text(),re.M)
    assert counts and sum(map(int,counts))==expected
 phases[name]=(p,v)
sdk_path=base/'work/6252-077a2a535-sdk-check.json';sdk=json.loads(sdk_path.read_text());assert sdk['source']==source and sdk['exit_code']==0 and sdk['counts']['tests']==6 and sdk['counts']['pass']==6 and all(sdk['counts'][k]==0 for k in ['fail','cancelled','skipped','todo'])
catalog=base/'work/6252-077a2a535-catalog.json';v=json.loads(catalog.read_text());patchbay=next(x for x in v['plots'] if x['slug']=='patchbay');assert len(v['plots'])==94 and patchbay['source']==(root/'plots/patchbay/main.conduit').read_text() and patchbay['required_kinds']==['patchbay/accept-interaction']
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
retain(sdk_path);retain(base/'work/6252-077a2a535-sdk-errors.log',compress=True);retain(catalog,compress=True)
for name in ['6252-run-077-focused.py','6252-run-077-catalog-and-strict.py','6252-run-077-strict.py','6252-run-077-all.py','6252-native-fixture-peer-review.md']:retain(base/'work'/name)
negatives=[('6252-ededf3c51-native-checks.json','default-feature command selected zero tests; not a product refusal'),('6252-ededf3c51-native-feature-checks.json','featured compile refused missing test-only ToString trait'),('6252-4370813a3-focused-checks.json','two geometry cases passed; crowded-choice constructor refused noncanonical membership')]
negative_receipts=[]
for name,meaning in negatives:
 p=base/'work'/name;v=json.loads(p.read_text());retain(p,'prior-negatives/'+name)
 for row in v['checks']:retain(Path(row['log']),'prior-negatives/'+Path(row['log']).name,True)
 negative_receipts.append(dict(source=v['source'],meaning=meaning,receipt='prior-negatives/'+name))
mixed=base/'work/6252-643aa7e33-mixed-preserved'
old=json.loads((mixed/'custody.json').read_text())
assert old['source']=='643aa7e3347a53e66108c8e0e9188510bfc7d13d'
for row in old['files']:
 p=mixed/row['file'];assert len(p.read_bytes())==row['bytes'] and hashlib.sha256(p.read_bytes()).hexdigest()==row['sha256']
 retain(p,'prior-negatives/643-mixed/'+p.name,p.suffix=='.log' or p.name.endswith('-catalog.json'))
retain(mixed/'custody.json','prior-negatives/643-mixed/custody.json')
negative_receipts.append(dict(source=old['source'],meaning='18 focused cases/catalog/formatting passed; strict lint refused retired unused workspace-view reader',receipt='prior-negatives/643-mixed/custody.json'))
for commit in ['596aab0e980bb697748d44281c3ed4955f510982','3564ed5e933872fe8a0f7c40d94a1da9aa4288f0','3a26f22efcc051f218f9ed400671f3fd5b1916db']:subprocess.run(['git','merge-base','--is-ancestor',commit,source],cwd=root,check=True)
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()==source
assert not subprocess.check_output(['git','status','--porcelain'],cwd=root,text=True)
receipt=dict(source=source,tree=subprocess.check_output(['git','rev-parse',source+'^{tree}'],cwd=root,text=True).strip(),source_clean=True,focused_cases=9,catalog_cases=3,sdk_cases=6,total_fresh_cases=18,catalog_plots=94,patchbay_source_document_id=patchbay['source_document_id'],patchbay_checked_plot_id=patchbay['checked_plot_id'],prior_negatives=negative_receipts,files=stored)
(stage/'receipt.json').write_text(json.dumps(receipt,indent=2)+'\n')
(stage/'README.md').write_text('''# Joined current-development admission repair

Clean executed Source `077a2a535b4ec30d1f09ec6c2884c22bbffd1c57` contains the published #6252 baseline head596aab and #6255 native-owner head3564 as ancestors. The semantic merge retains one canonical Patchbay catalog registration with its checked Fore, all offer/Source identity assertions, 140 advertised capabilities and the distinct 124 ordinary browser executors.

All18 fresh cases passed: six formerly failing native cases, two browser cases, actual141-capability refusal, two actual catalog Source/Fore/refusal cases, the updated Memory Lantern/Patchbay catalog regression, and six browser SDK cases (including124-executor admission/125 refusal). The supported bundle-workspace-catalog command passed with94 Plots and exact Patchbay Source. Workspace formatting and combined strict all-target Clippy for ConduitOS/native-compositor, browser runtime and xtask passed. Default stack, two build workers, no incremental compilation and no dev/test debug symbols; exact commands and source-bound original logs are retained.

The native fixture repairs compare complete cold authoring/Fore meaning, actual graph omission counts, checked finite-choice realization and typed fallback, and initialized document Current preserved across held release with original Body/Plan/Play and no-repaint assertions. Valid Face JSON whitespace reaches actual Fore byte pressure before exhausting the unchanged three-value production storage grant. Discovery accounts for the actual new installed canonical Back; workload/Plan authority and advertisement byte bounds are unchanged.

Earlier zero-selection, compile and noncanonical fixture failures, plus the joined643 strict failure on a retired unused receipt reader, remain losslessly retained with distinct source identities. The final correction removes only that unreferenced obsolete reader; canonical current Face/Show/Boot/draw receipt checks remain in use. Original failed Candidate39779112554 browser/admission logs remain in the preceding catalog/browser mixed-result packets. No failure is relabelled as a passing execution.

This is hosted focused execution, SDK unit execution, catalog integration, formatting and strict compilation/lint evidence. No QEMU ran at this Source. Historical wide/narrow captures remain bound to their original b1e18996/f04edf sources; serialized image bindings do not replace hashing absent image payloads. Reviewed Candidate, normal merge and relevant Integration remain required. This does not close #6179, #5994, #5608, #6190 or any broader platform/confinement requirement. A documentary successor is not the executed Source.
''')
(stage/'SHA256SUMS').write_text(''.join(hashlib.sha256(p.read_bytes()).hexdigest()+'  '+str(p.relative_to(stage))+'\n' for p in sorted(stage.rglob('*')) if p.is_file() and p.name!='SHA256SUMS'))
print(json.dumps(dict(source=source,stage=str(stage),files=len(stored),fresh_cases=18)))
