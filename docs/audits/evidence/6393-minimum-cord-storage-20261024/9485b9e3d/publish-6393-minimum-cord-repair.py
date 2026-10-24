import pathlib,json,subprocess,hashlib,gzip,tempfile,os,time,re
root=pathlib.Path.cwd();source=root/'work/6393-permanent-cord-bound';anchor=root/'work/6267-review';head='9485b9e3d2a2ddec797ff4054b8c68066245d865';negative='9ba29d98b6dc29aa599a81dd7dc170125dce9e9d';branch='evidence/6393-minimum-cord-storage-20261024';prefix='docs/audits/evidence/6393-minimum-cord-storage-20261024/'+head[:9]
def git(*a,input=None,env=None):return subprocess.check_output(['git','-C',str(anchor),*a],input=input,env=env).decode().strip()
def sha(b):return hashlib.sha256(b).hexdigest()
r=json.loads((root/'outputs/6393-permanent-cord-positive.json').read_bytes());assert r['source']==head and r['status']=='passed'and r['clean_after']and len(r['steps'])==5;assert not subprocess.check_output(['git','-C',str(source),'status','--porcelain'],text=True)
actual={};executed={};cases=0
for step in r['steps']:
 assert step['exit_code']==0;log=pathlib.Path(step['log']).read_bytes();assert sha(log)==step['log_sha256'];current=None
 for e in step['actual_test_executables']:
  data=pathlib.Path(e['path']).read_bytes();assert len(data)==e['bytes']and sha(data)==e['sha256'];executed[e['path']]=e
 for line in log.decode(errors='replace').splitlines():
  match=re.search(r'Running[^\n]*\(([^\n)]+)\)',line)
  if match:
   p=pathlib.Path(match[1]);current=str(p if p.is_absolute()else source/p)
  match=re.search(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;',line)
  if match:
   n,failed,ignored=map(int,match.groups());assert failed==ignored==0;cases+=n
   if n:
    assert current in executed;entry=actual.setdefault(current,{'executable':executed[current],'command_cases':[]});entry['command_cases'].append({'step':step['name'],'passed':n})
assert cases==113 and len(actual)==5
neg=json.loads((root/'outputs/6393-permanent-cord-corrected-negative.json').read_bytes());assert neg['source']==negative and neg['exit_code']==101 and neg['clean_after'];raw=pathlib.Path(neg['log']).read_bytes();assert sha(raw)==neg['log_sha256']and raw.count(b'undersized Plan admitted=true; unexpected refusal=None')==3
custody=json.loads((root/'outputs/6393-minimum-storage-source-format-custody.json').read_bytes());assert custody['source']==head and custody['format_exit']==custody['diff_check_exit']==0
payload={}
paths=['outputs/6393-permanent-cord-positive.json','outputs/6393-permanent-cord-corrected-negative.json','outputs/6393-corrected-negative-source.json','outputs/6393-minimum-storage-source-format-custody.json','outputs/6393-initial-negative-proof-qualification.json','outputs/6393-permanent-cord-first-negative.json','outputs/6393-first-repair-fixture-failure.json','outputs/6393-independent-review.json','outputs/6393-minimum-repair-independent-review.json','work/6393-permanent-cord-first-negative.log','work/6393-first-repair-fixture-failure.log','work/6393-permanent-cord-corrected-negative.log','work/6393-permanent-cord-final-format.log','work/run-6393-permanent-cord-negative.py','work/run-6393-permanent-cord-corrected-negative.py','work/run-6393-permanent-cord-positive.py','work/apply-6393-permanent-cord-fence.py','work/publish-6393-minimum-cord-repair.py',*[str(pathlib.Path(s['log']).relative_to(root))for s in r['steps']]]
for p in paths:
 b=(root/p).read_bytes();n=pathlib.Path(p).name;payload[n+('.gz'if n.endswith('.log')else'')]=gzip.compress(b,mtime=0)if n.endswith('.log')else b
for p in ['outputs/6393-permanent-cord-corrected-negative-actual-test.elf.gz','outputs/6393-permanent-cord-first-negative-actual-test.elf.gz','outputs/6393-first-repair-fixture-failure-actual-test.elf.gz']:payload[pathlib.Path(p).name]=(root/p).read_bytes()
primary=[]
for i,(p,e)in enumerate(sorted(actual.items())):
 name=f'positive-owning-test-{i:02}.elf.gz';payload[name]=gzip.compress(pathlib.Path(p).read_bytes(),mtime=0);primary.append({'packet_path':name,**e})
payload['actual-running-map.json']=(json.dumps({'positive_cases':cases,'positive_assertion_executables':primary,'all_executed_test_harnesses':list(executed.values()),'scope':'All actual Running ELF hashes reverified. Five binaries carry113 nonzero test cases. Zero-selected-test harness bytes remain locally retained; their metadata and complete raw logs are here.'},indent=2)+'\n').encode()
payload['source-closure.json']=(json.dumps({'positive':head,'positive_tree':r['tree'],'authentic_negative':negative,'original_pr_head':'60f39853d1d81756a77c66b18b3630560faa0b04','negative_test_only_delta':custody['negative_production_delta'],'production_objects':{p:git('rev-parse',head+':'+p)for p in ['architecture/core','architecture/planner','architecture/plot','semantics/catalog','targets/std','Cargo.lock','.cargo/config.toml']},'root_inputs':{p:sha(subprocess.check_output(['git','-C',str(anchor),'show',head+':'+p]))for p in ['Cargo.toml','Cargo.lock','.cargo/config.toml']}},indent=2)+'\n').encode()
payload['README.md']=b'''# Refuse permanently undersized canonical Quantity Cords

The exact original draft60f39853d could admit fixed canonical raw/wrapped Quantity Cords one byte below their required encoding, or clamp inferred storage below that encoding to fit a Host ceiling. The math completion then had no possible delivery capacity. Corrected negative9ba29d98b changes ONLY tests and prints actual admitted=true for all three unsupported Plans. Its raw log and actual executable are retained.

Repair9485b9e3d adds existing ByteLength exact constraints to only the already scoped canonical Quantity Fores and enforces explicit minimum byte lengths at final planning admission. Generic maximum bounds remain upper bounds, so smaller Cords for variable-length values remain eligible. NormalClose/Quiescence carry no value requirement; abnormal terminals use their own output minimum. Defaults, profiles, backpressure and kernel rules remain unchanged.

At clean9485 source, installed math10 (including all3exact storage-refusal diagnostics), completion/backpressure5, planner86 and focused catalog12 pass:113 cases,0ignored,5actual assertion-bearing test executables. Three owning libraries pass strict Clippy; whole-workspace formatting and patch hygiene pass. Direct Cargo tests are focused local proof, not Candidate/Integration, new GUI/WASM, publication or issue closure. Existing current-source GUI enactment remains separately owned.

Earlier first-negative18c and first-repair297 logs/executables remain qualified historical evidence: the two explicit-Cord fixtures used pre-expansion names and changed no Cord. They do NOT prove undersized explicit admission. The Host-ceiling fixture was valid. Corrected fixtures select exact planned Kind and assert exactly one changed expanded payload Cord; authentic9ba negative replaces the earlier invalid claim. Test-only negative variants are preserved rather than erased.

No owner branch was changed and no competing PR was opened. Source repair and exact negative are published on named recovery refs for adoption into draft6393.
'''
payload['manifest.json']=(json.dumps({'source':head,'negative':negative,'cases':cases,'positive_assertion_executables':len(actual),'files':[{'path':n,'bytes':len(b),'sha256':sha(b)}for n,b in payload.items()]},indent=2)+'\n').encode()
for b,h in [('repair/6393-permanent-cord-bound-20261024',head),('proof-source/6393-corrected-cord-negative-20261024',negative)]:
 assert not git('ls-remote','origin','refs/heads/'+b);subprocess.run(['git','-C',str(anchor),'push','--force-with-lease=refs/heads/'+b+':','origin',h+':refs/heads/'+b],check=True);assert git('ls-remote','origin','refs/heads/'+b).split()[0]==h
assert not git('ls-remote','origin','refs/heads/'+branch)
with tempfile.TemporaryDirectory(dir=root/'work',prefix='6393-minimum-proof-index-')as td:
 env=dict(os.environ,GIT_INDEX_FILE=td+'/index');git('read-tree',head,env=env)
 for n,b in payload.items():git('update-index','--add','--cacheinfo','100644',git('hash-object','-w','--stdin',input=b,env=env),prefix+'/'+n,env=env)
 c=git('commit-tree',git('write-tree',env=env),'-p',head,input=b'Retain authentic undersized Cord negatives and minimum-storage repair proof\n',env=env);git('update-ref','refs/heads/'+branch,c,'0'*40);subprocess.run(['git','-C',str(anchor),'push','--force-with-lease=refs/heads/'+branch+':','origin','refs/heads/'+branch],check=True);assert git('ls-remote','origin','refs/heads/'+branch).split()[0]==c
out={'at':time.time(),'source':head,'negative':negative,'evidence':c,'branch':branch,'remote_verified':True,'cases':cases,'positive_assertion_executables':len(actual),'files':len(payload),'no_new_PR':True,'url':'https://github.com/dancxjo/conduit/tree/'+c+'/'+prefix};(root/'outputs/6393-minimum-cord-repair-publication.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(out,indent=2))
