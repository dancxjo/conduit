import pathlib,os,json,hashlib,gzip,subprocess,tempfile,argparse,re
ROOT=pathlib.Path.cwd();REPO=ROOT/'work/6267-review';SOURCE='e9f8787ccbf3a0fda086a5e4d568cb7f597d34e1';TREE='d35051b7b0b512ef82e76fd37902479a65c43a06';BRANCH='evidence/ready-four-native-owning-20261024';PREFIX='proof/language/ready-four-native-owning-20261024/e9f8787cc'
def sha(b):return hashlib.sha256(b).hexdigest()
def git(*a,input=None,env=None):return subprocess.check_output(['git','-C',str(REPO),*a],input=input,env=env).decode().strip()
def main():
 p=argparse.ArgumentParser();p.add_argument('--publish',action='store_true');a=p.parse_args();assert a.publish
 r=json.loads((ROOT/'outputs/ready-four-native-local-proof.json').read_text());assert r['source']==SOURCE and r['tree']==TREE and r['status']=='passed'and r['clean_after']and r['old138_and104_verified_before']and r['old138_and104_verified_after'];assert len(r['steps'])==4
 for s,count in zip(r['steps'],[15,1,1,0]):
  data=(ROOT/s['log']).read_bytes();assert s['exit_code']==0 and s['actual_passed']==count and sha(data)==s['sha256'] and s['before_available_bytes']>=7*1024**3;assert sum(map(int,re.findall(rb'test result: ok\. (\d+) passed; 0 failed; 0 ignored;',data)))==count
 assert r['steps'][3]['command']==['cargo','+stable','clippy','--locked','-p','conduit-plot','-p','conduit-language','--lib','--','-D','warnings']
 for e in r['executables']:assert sha((ROOT/e['path']).read_bytes())==e['sha256']
 for file,n in [('outputs/ready-four-combined-local-proof.json',138),('outputs/ready-four-supported-supplement-local-proof.json',104)]:
  old=json.loads((ROOT/file).read_bytes());assert len(old['executables'])==n
  for e in old['executables']:assert sha((ROOT/e['path']).read_bytes())==e['sha256']
 for row in r['source_files']:assert git('rev-parse',SOURCE+':'+row['path'])==row['blob'] and sha(subprocess.check_output(['git','-C',str(REPO),'show',SOURCE+':'+row['path']]))==row['sha256']
 checkout=ROOT/'work/ready-four-combined-proof';assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=checkout,text=True).strip()==SOURCE and not subprocess.check_output(['git','status','--porcelain'],cwd=checkout,text=True).strip();assert git('rev-parse',SOURCE+'^{tree}')==TREE;assert not git('ls-remote','origin','refs/heads/'+BRANCH)
 payload={};inputs=[]
 for name in ['outputs/ready-four-native-local-proof.json','outputs/ready-four-combined-source.json','outputs/ready-four-combined-proof-publication.json','outputs/ready-four-supported-supplement-publication.json','work/run-ready-four-native-proof.py',__file__,*[s['log']for s in r['steps']]]:
  path=pathlib.Path(name);path=path if path.is_absolute()else ROOT/path;data=path.read_bytes();n=path.name;stored=data
  if n.endswith('.log'):n+='.gz';stored=gzip.compress(data,mtime=0)
  payload[n]=stored;inputs.append({'file':n,'raw_sha256':sha(data),'raw_bytes':len(data),'stored_sha256':sha(stored)})
 beamstep=next(s for s in r['steps']if s['name']=='actual-beam');assert len(beamstep['cargo_running_executables'])==1;beam=pathlib.Path(beamstep['cargo_running_executables'][0]);assert any((ROOT/e['path']).resolve()==beam.resolve()for e in r['executables']);data=beam.read_bytes();stored=gzip.compress(data,mtime=0);binary={'actual_Cargo_Running_path':str(beam),'raw_sha256':sha(data),'raw_bytes':len(data),'stored':len(stored)<=30*1024**2}
 if binary['stored']:payload['actual-beam-test.elf.gz']=stored;binary.update(file='actual-beam-test.elf.gz',stored_sha256=sha(stored),stored_bytes=len(stored))
 payload['actual-beam-custody.json']=(json.dumps(binary,indent=2)+'\n').encode();payload['README.md']=b'''# Fresh Native owning proof for the exact mixed dependency tree

Actual clean execution sourcee9f8787ccbf3a0fda086a5e4d568cb7f597d34e1/tree d35051b7b0b512ef82e76fd37902479a65c43a06 includes the reviewed Fore dependency. Four exact owning commands passed:15Plot outer regressions, actual four-slot Beam test1, existing Beam admission1 and strictPlot+Language LIBRARY Clippy. This is fresh execution at e9f, not local90a or historicalWIP execution relabeled. Source/package hashes, fullargv/exit/rawlogs, actualCargoRunningELFs and retainedlocal executable hashes join the proof. The actualBeam binary is losslessly hosted if within the bounded packet size, with an explicit custody record.

Current generated_rust::exercise compiles CURRENTemitted bindings, executes4innergeneric consumer cases with failclosed status/count assertions, and compiles a no_std consumer before outerPASS. Its existing helper deletes temporary inner binaries; no exactinner/no_std ELFhash custody is invented. The helper's exactsource blob/hash is retained. ActualBeam uses two private pool instances and checks all allocation/reallocation/deallocation counters through retainedfinish and changed/foreign/cancel/incomplete/pressure refusal paths while values/gates remain alive.

The prior138mixed and104supported-check actual executable hashes remain unchanged before/after; neither cache was written. Their immutable packets remain distinct. This new CPU owning proof permits future explicitly reviewed exact-d350 correspondence only after independent audit and full required gates. It is not currentdev delivery, Candidate/Integration acceptance, installedCurrent/controller/epoch publication, nativeUI/firmware/physical proof or parent completion. Strict lint is library-only, notalltargets.
''';payload['manifest.json']=(json.dumps({'source':SOURCE,'tree':TREE,'raw_inputs':inputs,'actual_Beam_binary':binary,'files':[{'path':n,'bytes':len(b),'sha256':sha(b)}for n,b in payload.items()]},indent=2)+'\n').encode()
 with tempfile.TemporaryDirectory(prefix='ready-four-native-index-',dir=ROOT/'work')as tmp:
  env=dict(os.environ,GIT_INDEX_FILE=str(pathlib.Path(tmp)/'index'));git('read-tree',SOURCE,env=env)
  for n,b in payload.items():git('update-index','--add','--cacheinfo','100644',git('hash-object','-w','--stdin',input=b,env=env),PREFIX+'/'+n,env=env)
  commit=git('commit-tree',git('write-tree',env=env),'-p',SOURCE,input=b'Retain fresh exact mixed-tree Native owning and Beam proof\n',env=env);git('update-ref','refs/heads/'+BRANCH,commit,'0'*40);subprocess.run(['git','-C',str(REPO),'push','origin','refs/heads/'+BRANCH+':refs/heads/'+BRANCH],check=True);assert git('ls-remote','origin','refs/heads/'+BRANCH).split()[0]==commit
 out={'source':SOURCE,'tree':TREE,'branch':BRANCH,'commit':commit,'prefix':PREFIX,'files':len(payload),'actual_outer_tests':17,'generated_inner_tests':4,'strict_scope':'Plot+Language library only','old242_executables_unchanged':True,'actual_Beam_binary':binary,'remote_verified':True,'url':'https://github.com/dancxjo/conduit/tree/'+commit+'/'+PREFIX};(ROOT/'outputs/ready-four-native-proof-publication.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(out,indent=2))
if __name__=='__main__':main()
