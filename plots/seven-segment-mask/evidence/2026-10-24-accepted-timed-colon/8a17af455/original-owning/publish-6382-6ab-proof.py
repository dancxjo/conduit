import pathlib,subprocess,os,json,hashlib,gzip,tempfile,argparse
ROOT=pathlib.Path.cwd();REPO=ROOT/'work/6267-review';SOURCE='6ab212c0016048791bcd014f3931f6658c1fe712';RECOVERY='prep/6382-independent-proof-20261024';BRANCH='evidence/6382-6ab-independent-proof-20261024';DEST='plots/seven-segment-mask/evidence/2026-10-24-timed-colon-mask/6ab212c00'
def sha(b):return hashlib.sha256(b).hexdigest()
def git(*a,input=None,env=None):return subprocess.check_output(['git','-C',str(REPO),*a],input=input,env=env).decode().strip()
def main():
 p=argparse.ArgumentParser();p.add_argument('--publish',action='store_true');a=p.parse_args();assert a.publish
 r=json.loads((ROOT/'outputs/6382-6ab-local-proof.json').read_text());assert r['source']==SOURCE and r['clean_after']and r['finished']and len(r['steps'])==2 and r['steps'][0]['actual_passed']==13
 for s in r['steps']:assert s['exit_code']==0 and sha((ROOT/s['log']).read_bytes())==s['sha256']
 for e in r['executables']:assert sha((ROOT/e['path']).read_bytes())==e['sha256']
 text=(ROOT/r['steps'][0]['log']).read_text()
 for name in ['explicit_colon_fraction_look_retains_tenths_on_colon_only_glass','timed_mask_pages_keep_one_face_and_stop_at_the_admitted_lifetime']:assert 'test '+name+' ... ok'in text
 checkout=ROOT/'work/6382-6ab-proof';assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=checkout,text=True).strip()==SOURCE;assert not subprocess.check_output(['git','status','--porcelain'],cwd=checkout,text=True).strip();assert r['tree']==git('rev-parse',SOURCE+'^{tree}')
 pr=json.loads((ROOT/'outputs/6382-current-pr.json').read_text());assert git('ls-remote','origin','refs/heads/'+pr['head']['ref']).split()[0]==SOURCE
 for branch in [RECOVERY,BRANCH]:assert not git('ls-remote','origin','refs/heads/'+branch)
 payload={};inputs=[]
 for name in ['outputs/6382-6ab-local-proof.json','outputs/6382-6ab-independent-source-review.json','outputs/6381-current-requirements.json','work/run-6382-6ab-proof.py',__file__,*[s['log']for s in r['steps']]]:
  pth=pathlib.Path(name);pth=pth if pth.is_absolute()else ROOT/pth;raw=pth.read_bytes();n=pth.name;data=raw
  if n.endswith('.log'):n+='.gz';data=gzip.compress(raw,mtime=0)
  payload[n]=data;inputs.append({'file':n,'raw_sha256':sha(raw),'stored_sha256':sha(data),'raw_bytes':len(raw)})
 payload['README.md']=b'''# Independent finite timed Mask proof

Actual clean execution source6ab212c0016048791bcd014f3931f6658c1fe712. Existing standalone conduit-seven-segment-mask package tests passed13, including explicit colon-only67:6F mapping, timed temperature/humidity pages from one immutable Face, invalidzero bounds and expiry unavailable, and retained rounding/stale/wiring/Form/Source protocol tests. Strict all-target Clippy passed. The existing public environment Mask route also runs unrelated boolean Forms and forces a shared cache; this proof uses the permitted narrow internal exact-manifest Cargo test and lint commands, without inventing a public route.

Complete raw logs are losslessly compressed; source/tree/argv/environment/exit/clean state and retained local actual executable hashes are included. Binaries remain local and are not hosted here. The source review preserves its earlier point-in-time missing-proof finding; this actual later proof fills the package execution gap.

No actual colon wiring, GPIO/native scheduling, acquisition, Body membership, appliance deployment, physical timing, Candidate/Integration or issue closure is established. The Mask dependency differs from earlier borrowed-runtime proof; unchanged runtime subtree alone cannot relabel earlier execution as including this new Mask. Original PR/source branches are unchanged.
''';payload['manifest.json']=(json.dumps({'source':SOURCE,'tree':r['tree'],'mask_tree':git('rev-parse',SOURCE+':plots/seven-segment-mask'),'raw_inputs':inputs,'files':[{'path':n,'bytes':len(b),'sha256':sha(b)}for n,b in payload.items()]},indent=2)+'\n').encode()
 with tempfile.TemporaryDirectory(prefix='6382-evidence-index-',dir=ROOT/'work')as tmp:
  env=dict(os.environ,GIT_INDEX_FILE=str(pathlib.Path(tmp)/'index'));git('read-tree',SOURCE,env=env)
  for n,b in payload.items():git('update-index','--add','--cacheinfo','100644',git('hash-object','-w','--stdin',input=b,env=env),DEST+'/'+n,env=env)
  commit=git('commit-tree',git('write-tree',env=env),'-p',SOURCE,input=b'Retain exact-source finite timed Mask independent proof\n',env=env);git('update-ref','refs/heads/'+BRANCH,commit,'0'*40);subprocess.run(['git','-C',str(REPO),'push','origin','refs/heads/'+RECOVERY+':refs/heads/'+RECOVERY,'refs/heads/'+BRANCH+':refs/heads/'+BRANCH],check=True)
  for b,want in [(RECOVERY,SOURCE),(BRANCH,commit)]:assert git('ls-remote','origin','refs/heads/'+b).split()[0]==want
 out={'source':SOURCE,'source_branch':RECOVERY,'branch':BRANCH,'commit':commit,'prefix':DEST,'files':len(payload),'remote_verified':True,'url':'https://github.com/dancxjo/conduit/tree/'+commit+'/'+DEST};(ROOT/'outputs/6382-6ab-proof-publication.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(out,indent=2))
if __name__=='__main__':main()
