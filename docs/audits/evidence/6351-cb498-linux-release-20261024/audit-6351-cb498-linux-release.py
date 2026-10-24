"""Read-only independent supported Linux release custody; no build or installation."""
import pathlib,json,hashlib,subprocess,struct,re,shlex
ROOT=pathlib.Path.cwd();R=ROOT/'work/6267-review';S='cb4983f58b3f785734c166243514cbb848ea2f9a';T='033712120b7c5ad812a18969e98602f3edf17d24';OUT=ROOT/'work/6351-cb498-linux-release';TARGET=ROOT/'work/6351-cb498-linux-release-target'
def sha(b):return hashlib.sha256(b).hexdigest()
def git(*a):return subprocess.check_output(['git','-C',str(R),*a],text=True).strip()
def raw(p):return subprocess.check_output(['git','-C',str(R),'show',S+':'+p])
def main():
 p=ROOT/'outputs/6351-cb498-linux-release-resume-local-proof.json';r=json.loads(p.read_bytes());assert r['status']=='passed'and r['source']==S and r['tree']==T and r['clean_after']and r['protected1033_verified_before']and r['protected1033_verified_after'];assert len(r['steps'])==1
 step=r['steps'][0];assert step['command']==['cargo','+stable','xtask','make','host','release','--platform','linux','--output',str(OUT)]and step['exit_code']==0 and step['before_available_bytes']>=7*1024**3
 b=(ROOT/step['log']).read_bytes();assert sha(b)==step['sha256']and b'SEALED existing-computer Host releases'in b;assert len(re.findall(rb'Finished[^\n]*release',b))>=2;assert git('rev-parse',S+'^{tree}')==T
 checkout=pathlib.Path('/tmp/conduit-6351-cb498-linux-release-source');assert subprocess.check_output(['git','-C',str(checkout),'rev-parse','HEAD'],text=True).strip()==S;assert not subprocess.check_output(['git','-C',str(checkout),'status','--porcelain'],text=True).strip()
 assert r['environment']['CARGO_TARGET_DIR']==str(TARGET)and r['environment']['CARGO_PROFILE_RELEASE_DEBUG']=='0'and r['environment']['CARGO_BUILD_JOBS']=='2'and r['environment']['CARGO_INCREMENTAL']=='0'
 interrupted=json.loads((ROOT/'outputs/6351-cb498-linux-release-interrupted-status.json').read_bytes());assert interrupted['terminal_exit_code']is None and interrupted['termination_cause'].startswith('UNKNOWN');assert sha(pathlib.Path(interrupted['log']).read_bytes())==interrupted['log_sha256']=='b115ec40fead48daeb59585954ddf1ffe3a0a7585b159690e0b5388342fc45a1'
 products={}
 for name,machine,artifact in [('conduit-linux-x86_64',62,TARGET/'release/conduit'),('conduit-linux-aarch64',183,TARGET/'aarch64-unknown-linux-gnu/release/conduit')]:
  data=(OUT/name).read_bytes();assert data[:6]==b'\x7fELF\x02\x01'and struct.unpack_from('<H',data,18)[0]==machine and 0<len(data)<=64*1024**2;assert sha(data)==sha(artifact.read_bytes());products[name]={'path':str(OUT/name),'bytes':len(data),'sha256':sha(data),'ELF_class':64,'endianness':'little','ELF_machine':machine,'matches_actual_cargo_release_artifact':True}
 assert products['conduit-linux-x86_64']['sha256']=='3282019327d4b96634a784ec4e6f9ae35ece0fb39d44ec00145f333efbab4a62'==interrupted['partial_x86']['sha256'];assert products['conduit-linux-x86_64']['bytes']==61228760
 sourceinstaller=raw('products/conduit/install/install-linux-x86_64.sh');assert (OUT/'install-linux-x86_64.sh').read_bytes()==sourceinstaller
 names=['hosted-linux-x86_64.json','raspios-bookworm-pi4-model-b-rev-1.5-4gb.json','raspios-bookworm-zero-2-w-rev-1.0.json','raspios-bookworm-zero-2-wh-rev-1.0.json'];manifests=[]
 for name in names:
  data=(OUT/name).read_bytes();m=json.loads(data);assert m['schema']=='conduit.release/host-bundle@1'and m['source_identity']==S and m['output']=='native-bundle';digest=hashlib.sha256(b'conduit.release/host-bundle-content@1\0');entries=[]
  for f in m['files']:
   file=OUT/f['path'];assert file.parent==OUT;data=file.read_bytes();assert len(data)==f['bytes']and 'sha256:'+sha(data)==f['sha256']and 0<len(data)<=64*1024**2;digest.update(f['path'].encode()+b'\0'+f['sha256'].encode()+b'\n');entries.append(f)
  assert m['bundle_sha256']=='sha256:'+digest.hexdigest();manifests.append({'path':str(OUT/name),'sha256':sha((OUT/name).read_bytes()),'target_id':m['target_id'],'bundle_sha256':m['bundle_sha256'],'files':entries})
 actual={}
 for e in r['executables']:
  file=pathlib.Path(e['path']);data=file.read_bytes();assert data[:4]==b'\x7fELF'and len(data)==e['bytes']and sha(data)==e['sha256'];actual[str(file.resolve())]=e
 bindings=[];extra=[]
 for text in re.findall(r'^\s*Running `([^`]+)`\s*$',b.decode(),re.M):
  argv=shlex.split(text);file=pathlib.Path(argv[0]);file=file if file.is_absolute()else pathlib.Path('/tmp/conduit-6351-cb498-linux-release-source')/file;key=str(file.resolve())
  if key not in actual:
   assert file.resolve().is_relative_to(checkout/'target/xtask-host-release');data=file.read_bytes();assert data[:4]==b'\x7fELF';entry={'path':str(file.resolve()),'bytes':len(data),'sha256':sha(data),'qualification':'Additional supportedrouter hostrelease helper outside154private-target runnerinventory'};extra.append(entry);actual[key]=entry
  bindings.append({'argv':argv,'actual_ELF':actual[key]})
 assert bindings
 protected=[]
 for file,n in [('ready-four-combined-local-proof',138),('ready-four-supported-supplement-local-proof',104),('ready-four-native-local-proof',81),('6389-be722-local-proof',103),('6389-be722-runtime-supported-local-proof',248),('next-three-focused-local-proof',182),('next-two-face-proof-local-proof',92),('6396-linked-validation-local-proof',85)]:
  rows=json.loads((ROOT/f'outputs/{file}.json').read_bytes())['executables'];assert len(rows)==n
  for e in rows:assert sha(pathlib.Path(e['path']).read_bytes())==e['sha256']
  protected+=rows
 assert len(protected)==1033
 result={'status':'passed','source':S,'tree':T,'supported_command':step['command'],'log':step['log'],'raw_log_sha256':step['sha256'],'terminal_exit_code':0,'SEALED_marker_verified':True,'source_bound_host_release_rs_sha256':sha(raw('tools/xtask/src/commands/host_release.rs')),'actual4_manifests':manifests,'actual_release_products':products,'installer_source_blob':git('rev-parse',S+':products/conduit/install/install-linux-x86_64.sh'),'installer_sha256':sha(sourceinstaller),'all_runner_ELFs_rehashed':len(r['executables']),'additional_actual_Running_ELFs_rehashed':extra,'Cargo_Running_bindings':bindings,'prior1033_rehashed_unchanged':True,'x86_unchanged_from_interrupted_build':True,'prior_unknown_interruption_preserved':interrupted['log_sha256'],'limits':['Local supported Linux build/seal at exactcb only; no installation/launch/liveLook/physical/CI claim','OwnerlaterGUI77two-browser-file correspondence is separate; no latestownerheadexecution relabel','No64MiBnative/20MiBbrowser boundchange, browserrebuild, manualdebugstrip or opt/LTOoverride bythisaudit','BothELFarchitectures structurallyverified; neitherreleasedHost executable is claimedexecuted']}
 (ROOT/'outputs/6351-cb498-linux-release-independent-review.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps({k:result[k]for k in ['status','source','all_runner_ELFs_rehashed','prior1033_rehashed_unchanged']},indent=2))
if __name__=='__main__':main()
