import pathlib,subprocess,json,hashlib,gzip,tempfile,os,time
root=pathlib.Path.cwd();anchor=root/'work/6267-review';source=root/'work/6411-payload-ceiling-source';head='cf42934dcfeadd19d645c4317e9aa71092c4865c';branch='evidence/6411-payload-ceiling-owning-20261024';prefix='docs/audits/evidence/6411-payload-ceiling-owning-20261024/'+head[:9]
def git(*a,input=None,env=None):return subprocess.check_output(['git','-C',str(anchor),*a],input=input,env=env).decode().strip()
def sha(b):return hashlib.sha256(b).hexdigest()
r=json.loads((root/'outputs/6411-payload-ceiling-local-proof.json').read_bytes());assert r['source']==head and r['status']=='passed'and r['clean_after']and len(r['steps'])==1;step=r['steps'][0];assert step['exit_code']==0 and step['test_counts']==[2,5,1];assert not subprocess.check_output(['git','-C',str(source),'status','--porcelain'],text=True)
inv={str(pathlib.Path(e['path']).resolve()):e for e in r['executables']}
for e in r['executables']:
 b=pathlib.Path(e['path']).read_bytes();assert len(b)==e['bytes']and sha(b)==e['sha256']
assert sha(pathlib.Path(step['log']).read_bytes())==step['sha256']
for p in step['actual_test_Running']:assert str(pathlib.Path(p).resolve())in inv
payload={}
for p in ['outputs/6411-payload-ceiling-local-proof.json','outputs/6411-cf429-independent-source-review.json','work/run-6411-payload-ceiling-proof.py','work/publish-6411-payload-ceiling-proof.py',str(pathlib.Path(step['log']).relative_to(root))]:
 b=(root/p).read_bytes();name=pathlib.Path(p).name;payload[name+('.gz'if name.endswith('.log')else'')]=gzip.compress(b,mtime=0)if name.endswith('.log')else b
for i,p in enumerate(step['actual_test_Running']):payload[f'actual-owning-test-{i}.elf.gz']=gzip.compress(pathlib.Path(p).read_bytes(),mtime=0)
payload['source-closure.json']=(json.dumps({'source':head,'tree':r['tree'],'objects':{p:git('rev-parse',head+':'+p)for p in ['bodies/environment/make','bodies/environment/runtime','architecture/core','architecture/plot','architecture/kernel','architecture/assigned-plan','architecture/plan-lowering','architecture/planner','tools/xtask/src/commands/environment_output.rs','tools/xtask/src/commands/dht11_probe.rs']},'root_inputs':r['root_inputs'],'actual_tests':8,'actual_executable_hashes':len(r['executables'])},indent=2)+'\n').encode();payload['README.md']=('''# Exact-source display payload ceiling proof

At clean cf42934dcfeadd19d645c4317e9aa71092c4865c, supported cargo +stable xtask prove environment display-output passes Make2, runtime packet5 and material1 tests (8total, zeroignored) and the helper's three focused strict target checks. Actualargv/exits/rawlogs, source/tree/rootinputs, retained executable hashes and3actualowningtestELFs are preserved. Source review records the external-workspace CI coverage boundary.

The651-byte fact is a conservative per-canonical-value Type ceiling. It does not establish aggregate live slot count, appliance fit, native output construction/admission, authority, firmware execution or physicalmemory reduction. The unsupported port tests retain unknownType/abnormalType refusals and exactselectedelectrical planning.

No Source changes or ownerbranchupdates; no additionalPR. Local owning proof supplements but does not replace Candidate and accepteddevIntegration. Full6295/6378 scope remains open. No new thumbv6m check is claimed for this Make-only change.
''').encode();payload['manifest.json']=(json.dumps({'source':head,'files':[{'path':n,'bytes':len(b),'sha256':sha(b)}for n,b in payload.items()]},indent=2)+'\n').encode();assert not git('ls-remote','origin','refs/heads/'+branch)
with tempfile.TemporaryDirectory(dir=root/'work',prefix='6411-evidence-index-')as td:
 env=dict(os.environ,GIT_INDEX_FILE=td+'/index');git('read-tree',head,env=env)
 for n,b in payload.items():git('update-index','--add','--cacheinfo','100644',git('hash-object','-w','--stdin',input=b,env=env),prefix+'/'+n,env=env)
 c=git('commit-tree',git('write-tree',env=env),'-p',head,input=b'Retain exact supported payload ceiling and display output proof\n',env=env);git('update-ref','refs/heads/'+branch,c,'0'*40);subprocess.run(['git','-C',str(anchor),'push','--force-with-lease=refs/heads/'+branch+':','origin','refs/heads/'+branch],check=True);assert git('ls-remote','origin','refs/heads/'+branch).split()[0]==c
out={'at':time.time(),'source':head,'evidence':c,'branch':branch,'files':len(payload),'remote_verified':True,'actual_cases':8,'actual_executables_rehashed':len(r['executables']),'url':'https://github.com/dancxjo/conduit/tree/'+c+'/'+prefix};(root/'outputs/6411-payload-ceiling-proof-publication.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(out,indent=2))
