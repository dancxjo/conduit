"""Publish refreshed Root B proof ONLY after Root confirms a successful suite.

Requires --publish and a Root-written source/command/exit/log/hash receipt.
Without --publish this exits before any network, Git writes or publication.
"""
import argparse,datetime,gzip,hashlib,json,os,pathlib,subprocess,tempfile
ROOT=pathlib.Path.cwd();REPO=ROOT/'work/6267-review';SOURCE='ed1b1cdb9e4cd8edd618211a189bcbd863240e18';OLD='ef5bba08ec65469a332c463b94d3a2be6210a841';BASE='7e9ccd0c6850d4f5782663f65ec7f5472109edb0';TREE='f1f9a85cc9756ca2a22371b48ebb02c8b6ce5014';BRANCH='evidence/reviewed-prs-omnibus-b-refresh-20261024';DEST='docs/audits/evidence/reviewed-prs-omnibus-b-refresh-20261024/ed1b1cdb9';LOG='work/6369-refreshed-binding-proof.log'
def sha(b):return hashlib.sha256(b).hexdigest()
def git(*args,input=None,env=None):return subprocess.check_output(['git','-C',str(REPO),*args],input=input,env=env).decode().strip()
def main():
 p=argparse.ArgumentParser();p.add_argument('--publish',action='store_true');p.add_argument('--proof-receipt',type=pathlib.Path);p.add_argument('--additional-audit',action='append',type=pathlib.Path,default=[]);a=p.parse_args();assert a.publish,'Producer prepared only; explicit Root-authorized --publish required before network/writes';assert a.proof_receipt,'Root completed execution receipt required'
 receipt=json.loads(a.proof_receipt.read_text());assert receipt['source']==SOURCE and receipt['exit_code']==0 and receipt['log']==LOG;assert receipt['command'] in (['cargo','+stable','xtask','check','tm1637-output'], 'cargo +stable xtask check tm1637-output');rawlog=(ROOT/LOG).read_bytes();assert sha(rawlog)==receipt.get('sha256',receipt.get('log_sha256'));text=rawlog.decode();assert '10 passed; 0 failed; 0 ignored'in text and '4 passed; 0 failed; 0 ignored'in text;assert 'ordinary_selected_refusal_retains_provider_then_exact_transfer_and_retirement ... ok'in text;assert 'tm1637-output.wire-only'in text and 'tm1637-output.lint'in text
 assert git('show','-s','--format=%P',SOURCE).split()==[OLD,BASE];assert git('rev-parse',SOURCE+'^{tree}')==git('rev-parse',OLD+'^{tree}')==TREE
 recipe=json.loads((ROOT/'outputs/6369-7e9-semantic-conflict-resolution-recipe.json').read_text());sourceaudit=json.loads((ROOT/'outputs/6369-refreshed-ed1b-source-recipe.json').read_text());assert sourceaudit['source']==SOURCE and sourceaudit['entire_tree_equals_prior_source'];inventory=json.loads((ROOT/'outputs/reviewed-prs-omnibus-b-source.json').read_text());assert len(inventory['paths'])==28
 correspondence=[]
 for path in inventory['paths']:
  old=git('rev-parse',OLD+':'+path);new=git('rev-parse',SOURCE+':'+path);assert old==new;correspondence.append({'path':path,'prior_ef5_blob':old,'refreshed_ed1_blob':new})
 expected_delta=sourceaudit['actual_delivery_delta_against_refresh_base'];assert len(expected_delta)==6 and git('diff','--name-only',BASE,SOURCE).splitlines()==expected_delta
 for row in recipe['four_conflicts']:assert git('rev-parse',SOURCE+':'+row['path'])==row['root_frozen']==row['retained_repair'];assert git('rev-parse',BASE+':'+row['path'])==row['current_dev']==row['original_owner']
 for ref in [OLD,BASE,*recipe['original_refs_preserved_by_root_parent']]:subprocess.run(['git','-C',str(REPO),'merge-base','--is-ancestor',ref,SOURCE],check=True)
 # Safe new-ref-only publication. Existing evidence refs are never overwritten.
 assert not git('ls-remote','origin','refs/heads/'+BRANCH),'Evidence branch already exists; stop, inspect/reuse instead of overwriting'
 paths=[a.proof_receipt,ROOT/LOG,ROOT/'outputs/6369-7e9-semantic-conflict-resolution-recipe.json',ROOT/'outputs/6369-refreshed-ed1b-source-recipe.json',ROOT/'outputs/reviewed-prs-omnibus-b-independent-composition-review.json',ROOT/'outputs/reviewed-prs-omnibus-b-publication.json',pathlib.Path(__file__),*a.additional_audit]
 payload={};rows=[]
 for path in paths:
  data=path.read_bytes();encoded=gzip.compress(data,mtime=0)if path.suffix=='.log'else data;name=path.name+('.gz'if path.suffix=='.log'else'');assert name not in payload;payload[name]=encoded;rows.append({'file':name,'stored_sha256':sha(encoded),'raw_sha256':sha(data),'raw_bytes':len(data),'encoding':'gzip'if path.suffix=='.log'else'identity'})
 audit={'source':SOURCE,'parents':[OLD,BASE],'tree':TREE,'prior_source':OLD,'entire_tree_identity_verified':True,'all28_feature_correspondence':correspondence,'actual_six_path_delta_against_refresh_base':expected_delta,'fresh_execution':receipt,'prior_proof':{'commit':'85c0b1400c51f60ac5aef5698be06a263e48de36','url':'https://github.com/dancxjo/conduit/tree/85c0b1400c51f60ac5aef5698be06a263e48de36/docs/audits/evidence/reviewed-prs-omnibus-b-20261024/ef5bba08e','qualification':'Actual prior Plot/preflight/TM execution remains ef5, not relabeled ed1. Fresh supported binding suite receipt/log above establishes only its actual ed1 command.'},'observed_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'limits':['No fresh Candidate or Integration result inferred from identical trees','No source branch push or PR/issue mutation by this producer','No new native/physical/ESP installation or parent completion claim']};payload['refreshed-source-proof.json']=(json.dumps(audit,indent=2)+'\n').encode();payload['README.md']=b'''# Refreshed Root B immutable proof

This packet is parented at actual refreshed source ed1b1cdb9, whose normal merge parents are ef5bba08e and current-dev7e9ccd0c6. The entire tree is byte-identical to the prior tested f1f9 tree. Four conflicts arose from the original TM owner squash bytes in dev versus the reviewed retained-provider repair. Exact reviewed repair blobs were preserved; no manual semantic rewrite or new bound was introduced. The residual delivery delta is exactly six retention-repair files; all28 original feature objects and owner/repair ancestry remain intact.

The fresh supported tm1637-output command has its own Root-confirmed zero exit, actual source, command and hashed raw log here. It includes wire-only checking, ten owner plus four wire tests, and strict lint. Earlier Plot, preflight and other owning checks remain qualified to their actual ef5 source in the linked original85c0 packet. Identical bytes do not relabel those executions as ed1 and do not replace required new exact-head Candidate or applicable Integration.

Only this new evidence branch is created. No source branch push, PR/issue mutation, new native/physical/ESP installation or parent closure is performed by publication.
''';payload['manifest.json']=(json.dumps({'source':SOURCE,'input_encodings':rows,'files':[{'path':n,'bytes':len(b),'sha256':sha(b)}for n,b in payload.items()]},indent=2)+'\n').encode()
 with tempfile.TemporaryDirectory(prefix='root-b-refresh-index-',dir=ROOT/'work/6318-proof-tmp')as tmp:
  env=dict(os.environ,GIT_INDEX_FILE=str(pathlib.Path(tmp)/'index'));git('read-tree',SOURCE,env=env)
  for n,b in payload.items():git('update-index','--add','--cacheinfo','100644',git('hash-object','-w','--stdin',input=b,env=env),DEST+'/'+n,env=env)
  commit=git('commit-tree',git('write-tree',env=env),'-p',SOURCE,input=b'Retain refreshed identical-tree Root B binding proof and semantic merge custody\n',env=env);git('update-ref','refs/heads/'+BRANCH,commit,'0'*40);subprocess.run(['git','-C',str(REPO),'push','origin','refs/heads/'+BRANCH+':refs/heads/'+BRANCH],check=True);assert git('ls-remote','origin','refs/heads/'+BRANCH).split()[0]==commit
 out={'source':SOURCE,'execution_source':SOURCE,'branch':BRANCH,'commit':commit,'prefix':DEST,'files':len(payload),'remote_verified':True,'url':'https://github.com/dancxjo/conduit/tree/'+commit+'/'+DEST};(ROOT/'outputs/reviewed-prs-omnibus-b-refresh-publication.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(out,indent=2))
if __name__=='__main__':main()
