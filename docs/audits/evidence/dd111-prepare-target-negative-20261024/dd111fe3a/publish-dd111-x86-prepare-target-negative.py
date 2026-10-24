import pathlib,json,subprocess,hashlib,gzip,tempfile,os,time
root=pathlib.Path.cwd();anchor=root/'work/6267-review';source='dd111fe3a81fb578d1e2271620eeb8b21ef60ddd';branch='evidence/dd111-prepare-target-negative-20261024';prefix='docs/audits/evidence/dd111-prepare-target-negative-20261024/'+source[:9]
def git(*a,input=None,env=None):return subprocess.check_output(['git','-C',str(anchor),*a],input=input,env=env).decode().strip()
def sha(b):return hashlib.sha256(b).hexdigest()
paths=['outputs/dd111-x86-prepare-target-diagnosis.json','outputs/dd111-x86-failed-log-custody.json','outputs/dd111-x86-diagnostics-custody.json','outputs/dd111-x86-120881450848-actual-job.log','outputs/dd111-journey-serial.log','outputs/dd111-journey-failure-registers.json','outputs/dd111-journey-failure-stack.json','outputs/dd111-x86_64-product-proof.json','outputs/dd111-x86-diagnostics-12563026912.zip','work/publish-dd111-x86-prepare-target-negative.py'];payload={}
for p in paths:
 b=(root/p).read_bytes();n=pathlib.Path(p).name;payload[n+('.gz'if n.endswith('.log')else'')]=gzip.compress(b,mtime=0)if n.endswith('.log')else b
payload['README.md']=('''# Exact x86 normal-product negative

Actual Integration40096456482 / x86job120881450848 at dd111fe3a81fb578d1e2271620eeb8b21ef60ddd fails waiting for a complete planned record. Retained guest serial instead establishes Application entry41 Commit Fault(WorkExhausted), prepare-target slot2/target3 refusal, and native-body-application-initialization-refused. This is an additional initialization/preparation blocker, distinct from original6318 PatchbayPublish Commit4496.

The exact9.75MB diagnostics ZIP is preserved with the SHA256 from its actual upload log. It includes actualguestELF8cd5ea52..., build/resolved manifests, complete481912-byte serial, register/stack snapshots andscreenshots. Individual rawjob/serial logs and independent diagnosis are also retained.

Internal Commit subphase remainsUNKNOWN. PostfaultRoot/compositor registers do not locate faultedApplicationwork. Aggregate41entries/124interrupts/1preemption is not per-entryRTCdebit. No flake attribution, Source-to-RTCconversion, timeout/grant/backing increase, workflowrerun or current-source reproduction. SuccessoracceptedIntegration is stillrequired. OriginalP06318scope and independent6311/6317authority/continuation obligations remain intact.
''').encode();payload['manifest.json']=(json.dumps({'source':source,'run':40096456482,'job':120881450848,'files':[{'path':n,'bytes':len(b),'sha256':sha(b)}for n,b in payload.items()]},indent=2)+'\n').encode();assert not git('ls-remote','origin','refs/heads/'+branch)
with tempfile.TemporaryDirectory(dir=root/'work',prefix='dd111-negative-index-')as td:
 env=dict(os.environ,GIT_INDEX_FILE=td+'/index');git('read-tree',source,env=env)
 for n,b in payload.items():git('update-index','--add','--cacheinfo','100644',git('hash-object','-w','--stdin',input=b,env=env),prefix+'/'+n,env=env)
 c=git('commit-tree',git('write-tree',env=env),'-p',source,input=b'Retain exact normal product PrepareTarget Commit exhaustion negative\n',env=env);git('update-ref','refs/heads/'+branch,c,'0'*40);subprocess.run(['git','-C',str(anchor),'push','--force-with-lease=refs/heads/'+branch+':','origin','refs/heads/'+branch],check=True);assert git('ls-remote','origin','refs/heads/'+branch).split()[0]==c
out={'at':time.time(),'source':source,'evidence':c,'branch':branch,'remote_verified':True,'files':len(payload),'url':'https://github.com/dancxjo/conduit/tree/'+c+'/'+prefix};(root/'outputs/dd111-x86-prepare-target-negative-publication.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(out,indent=2))
