import pathlib,json,subprocess,hashlib,gzip,tempfile,os,time
root=pathlib.Path.cwd();anchor=root/'work/6267-review';head='6adc488e20284a93d980b91563b94b6e51f758ba';branch='evidence/6410-join-ci-regressions-20261024';prefix='docs/audits/evidence/6410-join-ci-regressions-20261024/'+head[:9]
def git(*a,input=None,env=None):return subprocess.check_output(['git','-C',str(anchor),*a],input=input,env=env).decode().strip()
def sha(b):return hashlib.sha256(b).hexdigest()
r=json.loads((root/'outputs/6410-ci-failure-supplement-positive.json').read_bytes());prior=json.loads((root/'outputs/6410-branch-join-positive.json').read_bytes());assert r['source']==head==prior['source'] and r['status']=='passed'and r['clean_after'];assert r['steps'][0]['actual_passed_counts']==[11,3] and all(s['exit_code']==0 for s in r['steps'])
for e in [*prior['executables'],*r['executables']]:
 b=pathlib.Path(e['path']).read_bytes();assert len(b)==e['bytes']and sha(b)==e['sha256']
paths=['outputs/6410-ci-failure-supplement-positive.json','outputs/6410-admission-failed-log-custody.json','outputs/6410-branch-join-positive.json','work/run-6410-ci-failure-supplement.py','work/publish-6410-ci-failure-supplement.py','outputs/6410-admission-120883461531-actual-job.log',*[str(pathlib.Path(s['log']).relative_to(root))for s in r['steps']]];payload={}
for p in paths:
 b=(root/p).read_bytes();name=pathlib.Path(p).name;payload[name+('.gz'if name.endswith('.log')else'')]=gzip.compress(b,mtime=0)if name.endswith('.log')else b
for i,p in enumerate(r['steps'][0]['actual_running']):
 assert any(e['path']==p for e in r['executables']);payload[f'actual-test-{i}.elf.gz']=gzip.compress(pathlib.Path(p).read_bytes(),mtime=0)
payload['README.md']=('''# Actual CI regression supplement

Original Candidate40097476352 admissionjob120883461531 at ebf989413 fails bme280_lifecycle(9pass/2fail) anddevice_frames(2pass/1fail). Immutable actualjoblog retained; no repeatedworkflowlogdownload. Themissingdevice-framefield matches the independentlyreproduced conditionaljoin bug.

At clean repaired6adc488e20284a93d980b91563b94b6e51f758ba, both entiretargets nowPASS11+3, allzeroignored; scopedPlotlibrary/twotargetstrictClippyPASS. NofurtherSourcechangeaftertheoriginal15-caseproof. Allprior21ELFs pluscurrentELFs rehashedunchanged;2newactualRunningtestELFs retained. Thus29actualcases across6distincttargets pass atsameSource, withstrictlibrary/all6owningtargets acrossthetworuns.

Earlier15-caseproof, minimalnegativeandrepair: https://github.com/dancxjo/conduit/tree/2fd28f06e337176b04eb25bc0346fc90908e5835/docs/audits/evidence/6410-tape-branch-join-20261024/6adc488e2

Local source-bound repair proof, notowneradoption/newCandidate/Integration/MCU/hardwareacceptance. ExistingownerPRbranchunchanged, nonewPR. FreshCIrequiredafteradoption.
''').encode();payload['manifest.json']=(json.dumps({'source':head,'tree':r['tree'],'prior21_ELFs_unchanged':True,'current_ELFs_rehashed':len(r['executables']),'files':[{'path':n,'bytes':len(b),'sha256':sha(b)}for n,b in payload.items()]},indent=2)+'\n').encode();assert not git('ls-remote','origin','refs/heads/'+branch)
with tempfile.TemporaryDirectory(dir=root/'work',prefix='6410-ci-evidence-index-')as td:
 env=dict(os.environ,GIT_INDEX_FILE=td+'/index');git('read-tree',head,env=env)
 for n,b in payload.items():git('update-index','--add','--cacheinfo','100644',git('hash-object','-w','--stdin',input=b,env=env),prefix+'/'+n,env=env)
 c=git('commit-tree',git('write-tree',env=env),'-p',head,input=b'Retain actual failed CI targets passing with conditional join repair\n',env=env);git('update-ref','refs/heads/'+branch,c,'0'*40);subprocess.run(['git','-C',str(anchor),'push','--force-with-lease=refs/heads/'+branch+':','origin','refs/heads/'+branch],check=True);assert git('ls-remote','origin','refs/heads/'+branch).split()[0]==c
out={'at':time.time(),'source':head,'evidence':c,'branch':branch,'remote_verified':True,'actual_supplement_cases':14,'total_distinct_target_cases_at_same_source':29,'prior21_ELFs_unchanged':True,'current_ELFs_rehashed':len(r['executables']),'files':len(payload),'url':'https://github.com/dancxjo/conduit/tree/'+c+'/'+prefix};(root/'outputs/6410-ci-failure-supplement-publication.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(out,indent=2))
