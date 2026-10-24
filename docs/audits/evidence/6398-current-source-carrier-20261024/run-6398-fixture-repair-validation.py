import pathlib,subprocess,json,hashlib,shutil,os,re,time
r=pathlib.Path.cwd();p=pathlib.Path('/tmp/conduit-6398-carrier-refresh-source');source='002b2b44a833b5670e38415ecce3f652687cb372';branch='repair/6398-current-source-carrier-20261024';receipt=r/'outputs/6398-fixture-repair-validation.json';assert not receipt.exists()
def git(*a):return subprocess.check_output(['git','-C',str(p),*a],text=True).strip()
def sha(f):
 h=hashlib.sha256()
 with pathlib.Path(f).open('rb')as stream:
  for b in iter(lambda:stream.read(1024*1024),b''):h.update(b)
 return h.hexdigest()
producer=json.loads((r/'outputs/6398-carrier-producer-resume-local-proof.json').read_text());review=json.loads((r/'outputs/6398-generated-carrier-independent-review.json').read_text());assert producer['status']==review['status']=='passed'and producer['actual_passed']==2 and review['file_custody']['you-would.wav']['unchanged'];assert git('rev-parse','HEAD')==source and not git('status','--porcelain');assert shutil.disk_usage(r).free>=7*1024**3
live=json.loads(subprocess.check_output(['gh','pr','view','6398','--repo','dancxjo/conduit','--json','headRefOid,state'],text=True));assert live['headRefOid']==source and live['state']=='OPEN'
assert len(producer['executables'])==225
for e in producer['executables']:assert sha(e['path'])==e['sha256']
old=r/'outputs/6398-prior-shared-carrier-1e1a9';site=p/'site/evidence/shared-speech-carrier';new=r/'work/6398-carrier-refresh/shared-carrier';names=['you-would.native','you-would.wav','you-would.json'];assert all(sha(site/n)==sha(old/n)for n in ['manifest.json',*names]);manifest=json.loads((site/'manifest.json').read_text());manifest['producer_code_commit']=source;manifest['command']=' '.join(producer['command'])
for name in names:
 assert review['file_custody'][name]['new_sha256']==sha(new/name);shutil.copy2(new/name,site/name);manifest['files'][name]={'bytes':(site/name).stat().st_size,'sha256':sha(site/name)}
(site/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n');changed=git('diff','--name-only').splitlines();expected={f'site/evidence/shared-speech-carrier/{n}'for n in ['manifest.json','you-would.native','you-would.json']};assert set(changed)==expected;assert sha(site/'you-would.wav')==sha(old/'you-would.wav');git('diff','--check');git('switch','-c',branch);git('add','--',*sorted(expected));git('commit','-m','Regenerate shared speech carrier from finite-analysis checked Source');head=git('rev-parse','HEAD');assert not git('status','--porcelain');x={'producer_code_source':source,'fixture_commit':head,'fixture_tree':git('rev-parse','HEAD^{tree}'),'branch':branch,'owning_language_source_unchanged':True,'changed_paths':changed,'waveform_unchanged':True,'prior_negative':'outputs/6398-carrier-producer-local-proof.json','producer_proof':'outputs/6398-carrier-producer-resume-local-proof.json','independent_review':'outputs/6398-generated-carrier-independent-review.json','steps':[],'started':time.time(),'limits':['Private producer Source002b distinct from fixture commit; no old artifact relabel.','This branch is a handoff; owner PR branch unchanged.','No hosted CI, merged acceptance, release, parent4907 or physical/listening claim.']};receipt.write_text(json.dumps(x,indent=2)+'\n')
env=dict(os.environ,CARGO_TARGET_DIR=str(r/'work/6398-carrier-refresh-target'),CARGO_BUILD_JOBS='2',CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0')
steps=[('publication-negative-controls',['node','--test','--test-reporter=tap','proof/ci/pipeline-shared-speech-evidence.spec.mjs'],11),('owning-speech-regressions',['cargo','+stable','test','--locked','-p','conduit-speech','--features','semantic-bindings,kernel','--test','carried_formant','--test','shared_carrier_projections','--','--nocapture','--test-threads=1'],8),('owning-speech-strict-lint',['cargo','+stable','clippy','--locked','-p','conduit-speech','--features','semantic-bindings,kernel','--test','carried_formant','--test','shared_carrier_projections','--','-D','warnings'],0)]
for name,cmd,count in steps:
 assert shutil.disk_usage(r).free>=4*1024**3;log=r/f'work/6398-fixture-{name}.log';assert not log.exists();print('START',name,flush=True)
 with log.open('wb')as f:result=subprocess.run(cmd,cwd=p,env=env,stdout=f,stderr=subprocess.STDOUT)
 raw=log.read_text(errors='replace');counts=list(map(int,re.findall(r'test result: ok\. (\d+) passed; 0 failed; 0 ignored;',raw)));actual=sum(counts)
 if name=='publication-negative-controls':actual=int(re.search(r'^# tests (\d+)$',raw,re.M)[1]);assert re.search(r'^# fail 0$',raw,re.M)
 row={'name':name,'command':cmd,'log':str(log),'sha256':sha(log),'exit_code':result.returncode,'actual_passed':actual,'expected_passed':count};x['steps'].append(row);x['status']='running'if result.returncode==0 and actual==count else 'failed-first-negative-preserved';receipt.write_text(json.dumps(x,indent=2)+'\n');print(row,flush=True)
 if x['status']=='failed-first-negative-preserved':raise SystemExit('First negative preserved; no further command')
for e in producer['executables']:assert sha(e['path'])==e['sha256']
assert not git('status','--porcelain');x.update(producer225_verified_before_after=True,status='passed',finished=time.time(),clean_after=True)
x['executables']=[]
for f in (r/'work/6398-carrier-refresh-target').rglob('*'):
 if f.is_file()and os.access(f,os.X_OK):
  with f.open('rb')as stream:magic=stream.read(4)
  if magic==b'\x7fELF':x['executables'].append({'path':str(f),'bytes':f.stat().st_size,'sha256':sha(f)})
receipt.write_text(json.dumps(x,indent=2)+'\n');print('PASS',head,flush=True)
