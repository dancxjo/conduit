import pathlib,json,subprocess,hashlib,gzip,tempfile,os,time
root=pathlib.Path.cwd();anchor=root/'work/6267-review';source=root/'work/6411-payload-ceiling-source';head='51c2dee0c0607585c6f20dc4991394909f613bc9';branch='evidence/ready-prs-compatibility-20261024';prefix='docs/audits/evidence/ready-prs-compatibility-20261024/'+head[:9]
def git(*a,input=None,env=None):return subprocess.check_output(['git','-C',str(anchor),*a],input=input,env=env).decode().strip()
def sha(b):return hashlib.sha256(b).hexdigest()
r=json.loads((root/'outputs/omnibus-ready-combined-local-proof.json').read_bytes());assert r['source']==head and r['status']=='passed'and r['clean_after']and len(r['steps'])==8;assert all(s['exit_code']==0 for s in r['steps']);assert sum(sum(s['actual_passed_counts'])for s in r['steps'])==60;assert not subprocess.check_output(['git','-C',str(source),'status','--porcelain'],text=True)
inv={str(pathlib.Path(e['path']).resolve()):e for e in r['executables']};actual=set()
for e in r['executables']:
 b=pathlib.Path(e['path']).read_bytes();assert len(b)==e['bytes']and sha(b)==e['sha256']
for s in r['steps']:
 assert sha(pathlib.Path(s['log']).read_bytes())==s['sha256']
 for p in s['actual_test_Running']:
  p=str(pathlib.Path(p).resolve());assert p in inv;actual.add(p)
payload={};paths=['outputs/omnibus-ready-combined-local-proof.json','outputs/omnibus-ready-prs-composition-1408.json','outputs/omnibus-ready-first-headroom-stop.json','outputs/omnibus-ready-origin-recovery-publication.json','outputs/6410-owner-join-fix-adoption-review.json','work/run-omnibus-ready-proof.py','work/resume-omnibus-ready-proof.py','work/publish-omnibus-ready-compatibility.py',*[str(pathlib.Path(s['log']).relative_to(root))for s in r['steps']]]
for p in paths:
 b=(root/p).read_bytes();n=pathlib.Path(p).name;payload[n+('.gz'if n.endswith('.log')else'')]=gzip.compress(b,mtime=0)if n.endswith('.log')else b
for i,p in enumerate(sorted(actual)):payload[f'actual-test-{i:02}.elf.gz']=gzip.compress(pathlib.Path(p).read_bytes(),mtime=0)
payload['actual-running-map.json']=(json.dumps([{'packet_path':f'actual-test-{i:02}.elf.gz',**inv[p]}for i,p in enumerate(sorted(actual))],indent=2)+'\n').encode();payload['source-closure.json']=(json.dumps({'source':head,'tree':r['tree'],'objects':{p:git('rev-parse',head+':'+p)for p in ['architecture/core','architecture/plot','semantics/data','targets/std','bodies/environment/make','bodies/environment/runtime','tools/xtask/src/suites/resource_reference_equality.rs','tools/xtask/src/commands/environment_output.rs']},'root_inputs':r['root_inputs'],'inventory_scope':'Retained mixed owned target inventory, including reused/nonexecuted older artifacts. Only actual Running bindings establish execution for current source; oldcf141 artifacts separately archived before reuse.'},indent=2)+'\n').encode()
payload['README.md']=('''# Reviewed ready-PR compatibility tree

Published integration/recovery51c2dee0c0607585c6f20dc4991394909f613bc9 preserves725b795(#6402),ebf989413(#6410),cf42934d(#6411),plusRootrepairs6adc488 and5ea707 asancestors. OneNativefixtureconflict keeps exactconstructor+decoderrefusals/retentionharness from5ea andowner supported-suiteregistration. No productionvalidator weakened; noownerbranch changed.

Atclean tree4f27e0a451d2cc716476e7784ed678a5a105dc64: supportedResource15 cases (includingnestedgeneratedconsumer),Core8,constructor29 andsupporteddisplay8 allPASS=60 across15actualtestexecutables; all8commandstagesincludingplannedstrictlibrary/targetcheckspass. Rawargv/exits/logs, code/tree/rootinputs andactualtestELFs retained. Resource stage passed before a deliberateheadroomstop; later stages resumed without repeating it. Stop receipt retained asoperationalnegative, nottestfailure.

The mixed target inventory includesolder/reused/nonexecutedartifacts; currentSource executionclaimsbindonly actualRunning. Priorcf141executables/allnoncompilerOUTSource are preserved separately in a local roundtripverifiedarchive before reuse, withcustodyrecord referencedinreceipt.

Owner6410 has since published682c912 with an equivalentjoinfix anddifferentregression. It isnotbyte-identical; these60tests doNOTclaim executionof682c912. Its freshCandidate40101562075 remainsrequired. Becauseowneradoptionresolvedthehandoff, thisintegrationbranchisrecoveryonly: no competingPR orCIcampaign. Candidate/acceptedIntegration/publication/hardware/fullparentacceptancearenotclaimed.
''').encode();payload['manifest.json']=(json.dumps({'source':head,'tree':r['tree'],'actual_cases':60,'actual_test_executables':len(actual),'files':[{'path':n,'bytes':len(b),'sha256':sha(b)}for n,b in payload.items()]},indent=2)+'\n').encode();assert not git('ls-remote','origin','refs/heads/'+branch)
with tempfile.TemporaryDirectory(dir=root/'work',prefix='ready-compatibility-index-')as td:
 env=dict(os.environ,GIT_INDEX_FILE=td+'/index');git('read-tree',head,env=env)
 for n,b in payload.items():git('update-index','--add','--cacheinfo','100644',git('hash-object','-w','--stdin',input=b,env=env),prefix+'/'+n,env=env)
 c=git('commit-tree',git('write-tree',env=env),'-p',head,input=b'Retain combined reference constructor and payload compatibility proof\n',env=env);git('update-ref','refs/heads/'+branch,c,'0'*40);subprocess.run(['git','-C',str(anchor),'push','--force-with-lease=refs/heads/'+branch+':','origin','refs/heads/'+branch],check=True);assert git('ls-remote','origin','refs/heads/'+branch).split()[0]==c
out={'at':time.time(),'source':head,'evidence':c,'branch':branch,'remote_verified':True,'cases':60,'actual_test_executables':len(actual),'mixed_retained_executable_inventory':len(r['executables']),'files':len(payload),'no_new_PR':True,'url':'https://github.com/dancxjo/conduit/tree/'+c+'/'+prefix};(root/'outputs/omnibus-ready-compatibility-publication.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(out,indent=2))
