import pathlib,subprocess,json,hashlib,re
ROOT=pathlib.Path(__file__).resolve().parent.parent;R=ROOT/'work/6267-review';H='cc51be1376775bd5ffd77845ff1c3e3fc3a6c02a';S='002b2b44a833b5670e38415ecce3f652687cb372';PRE='site/evidence/shared-speech-carrier/'
def git(*a):return subprocess.check_output(['git','-C',str(R),*a],text=True).strip()
def raw(ref,p):return subprocess.check_output(['git','-C',str(R),'show',ref+':'+p])
def sha(b):return hashlib.sha256(b).hexdigest()
assert git('show','-s','--format=%P',H)==S;paths=git('diff','--name-only',S,H).splitlines();assert paths==[PRE+x for x in ['manifest.json','you-would.json','you-would.native']]
assert raw(S,PRE+'you-would.wav')==raw(H,PRE+'you-would.wav')
m=json.loads(raw(H,PRE+'manifest.json'));producer=json.loads((ROOT/'outputs/6398-carrier-producer-resume-local-proof.json').read_bytes());assert m['producer_code_commit']==S and m['command']==' '.join(producer['command'])
for name,row in m['files'].items():
 b=raw(H,PRE+name);assert len(b)==row['bytes']and sha(b)==row['sha256'];assert b==(ROOT/'work/6398-carrier-refresh/shared-carrier'/name).read_bytes()
assert git('rev-parse',S+':semantics/language')==git('rev-parse',H+':semantics/language') and git('rev-parse',S+':semantics/speech')==git('rev-parse',H+':semantics/speech')
p=json.loads((ROOT/'outputs/6398-fixture-repair-validation.json').read_bytes());assert p['fixture_commit']==H and p['producer_code_source']==S;rows=[];bindings=[]
for step in p['steps']:
 b=pathlib.Path(step['log']).read_bytes();assert sha(b)==step['sha256']and step['exit_code']==0 and step['actual_passed']==step['expected_passed'];rows.append(step)
 if step['name']=='publication-negative-controls':assert b'# pass 11'in b
 elif step['name']=='owning-speech-regressions':
  assert list(map(int,re.findall(rb'test result: ok\. (\d+) passed; 0 failed; 0 ignored;',b)))==[5,3];bindings=re.findall(r'^\s*Running .*?\(([^)]+)\)\s*$',b.decode(),re.M);assert len(bindings)==2
 elif step['name']=='owning-speech-strict-lint':assert step['command']==['cargo','+stable','clippy','--locked','-p','conduit-speech','--features','semantic-bindings,kernel','--test','carried_formant','--test','shared_carrier_projections','--','-D','warnings']and b'Finished'in b
out={'status':'source-manifest-passed-validation-pending','source':H,'producer_source':S,'three_fixture_paths':paths,'manifest_sha256':sha(raw(H,PRE+'manifest.json')),'producer_manifest_exact_source_command_and_3bytes_verified':True,'WAV_unchanged':True,'all_Language_and_Speech_source_subtrees_unchanged':True,'validation_steps_verified':rows,'actual_Running_bindings':bindings,'limits':['Produceractual002b distinctfromfixturecommitcc51; original1e1a9fixtureandfirstsparsecompilefailurepreserved.','No originalownerbranchmutation/hostedCI/normaldelivery/Integration/sharedpublication/parent4907/physical/listeningclaim.','RawRustproducer+reloadcarrysemanticproof; thisauditjoinsactualsource/file/command/log/executablecustody.']}
if p['status']=='passed':
 assert len(rows)==3 and p['clean_after']and p['producer225_verified_before_after'];inventory={str(pathlib.Path(e['path']).resolve()):e for e in p['executables']}
 for e in p['executables']:
  b=pathlib.Path(e['path']).read_bytes();assert b[:4]==b'\x7fELF'and len(b)==e['bytes']and sha(b)==e['sha256']
 for path in bindings:assert str(pathlib.Path(path).resolve())in inventory
 assert len(producer['executables'])==225
 for old in producer['executables']:
  current=inventory[str(pathlib.Path(old['path']).resolve())];assert current['sha256']==old['sha256'] and current['bytes']==old['bytes']
 out.update(status='passed',actual_owning_tests=8,actual_publication_controls=11,strict_scope='conduit-speech carried_formant+shared_carrier_projections only; semantic-bindings,kernel; -Dwarnings',actual_ELFs_rehashed=len(inventory),producer225_unchanged_receipt_verified=True)
(ROOT/'outputs/6398-fixture-repair-independent-review.json').write_text(json.dumps(out,indent=2)+'\n');print(out['status'])
