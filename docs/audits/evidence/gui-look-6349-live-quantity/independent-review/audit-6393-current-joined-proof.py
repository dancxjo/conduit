import pathlib,json,subprocess,hashlib,re
R=pathlib.Path.cwd();G=R/'work/6267-review';Q='3db193770cd049e7d10e1ef76b9f9c22f047c429';P='9083e6a0611cbcdf16832536d79f767a7ba19748';D='docs/audits/evidence/gui-look-6349-live-quantity/'
def raw(ref,path):return subprocess.check_output(['git','-C',str(G),'show',ref+':'+path])
def git(*a):return subprocess.check_output(['git','-C',str(G),*a],text=True).strip()
def sha(b):return hashlib.sha256(b).hexdigest()
m=json.loads(raw(P,D+'manifest.json'));build=git('rev-parse',m['production_build_ref']);test=git('rev-parse',m['test_ref']);assert git('rev-parse',m['runtime_review_ref'])==Q
sums=raw(P,D+'SHA256SUMS').decode().splitlines();verified=[]
for line in sums:
 digest,name=line.split(maxsplit=1);name=name.lstrip('*');b=raw(P,D+name);assert sha(b)==digest;verified.append(name)
assert len(verified)==12
log=raw(P,D+'gui-checks.log');assert re.search(rb'17 passed \(2\.8m\)',log) and b'1 failed'not in log
old=json.loads((R/'outputs/6393-independent-source-proof-review.json').read_text());rows=[]
for path in old['exact_source_paths_compared']:
 b=raw(Q,path);assert b==raw(build,path)==raw(test,path);rows.append({'path':path,'blob':git('rev-parse',Q+':'+path),'sha256':sha(b)})
assert git('diff','--name-only',build,test).splitlines()==['proof/browser/patchbay-authoring.spec.mjs']
face=json.loads(raw(P,D+'face-receipt.json'));assert face['face']['basis']==m['basis'] and face['face']['identity']==m['face_id'] and face['face']['revision']==m['revision'];assert m['basis']['active_play_id']and len(m['basis']['sign_ids'])==10
assert raw(P,D+'live-authoring-wide.png')==(R/'outputs/6351-local-live-quantity-capture/live-authoring-wide.png').read_bytes()
assert not m['runtime_wasm_rebuilt'] and not m['native_acceptance']
out={'runtime_head':Q,'packet':P,'production_build_source':build,'actual_test_source':test,'payload_checksums_verified':len(verified),'actual_terminal_browser_tests':17,'all8_runtime_code_paths_match_joined_execution_source':rows,'build_to_test_only_capture_test_change':True,'screenshot_face_basis_verified':True,'limits':['Actual local combined browser/server proof, not isolated Quantity head execution','8 runtime files match, but full build dependency closure is not claimed identical between isolated and joined sources','Existing sealed WASM reused; no fresh WASM/native/CI or acceptance inferred','Old af17 code had different catalog construction; current3db contains palette/offer correction','Earlierfailed browser run and OutputBlocked/TextTooLong remain preserved'],'passed':True};(R/'outputs/6393-current-joined-proof-independent-review.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps({k:v for k,v in out.items()if k!='all8_runtime_code_paths_match_joined_execution_source'},indent=2))
