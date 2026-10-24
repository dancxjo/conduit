"""Inert default. Publish ONLY after Root independent review and explicit --publish."""
import pathlib,subprocess,json,hashlib,argparse,os,tempfile
ROOT=pathlib.Path.cwd();R=ROOT/'work/6267-review';S='8734a4994af36d8b4c28d3b2bd39cc197d65e1ae';T='d63189bd7311480c447bbc8128fbf09098577fd2';B='evidence/next-two-face-proof-20261024';P='docs/audits/evidence/next-two-face-proof-20261024/8734a4994';D=ROOT/'work/next-two-face-proof-packet'
def git(*a,input=None,env=None):return subprocess.check_output(['git','-C',str(R),*a],input=input,env=env).decode().strip()
def sha(b):return hashlib.sha256(b).hexdigest()
def main():
 p=argparse.ArgumentParser();p.add_argument('--publish',action='store_true');p.add_argument('--review');a=p.parse_args()
 if not a.publish:print('Inert: local packet and Root independent review required before explicit --publish --review FILE');return
 assert a.review;reviewpath=pathlib.Path(a.review);review=json.loads(reviewpath.read_bytes());assert review['status']=='passed'and review['source']==S and review['tree']==T
 m=json.loads((D/'manifest.json').read_bytes());assert m['source']==S and m['tree']==T;payload={}
 for row in m['files']:
  b=(D/row['path']).read_bytes();assert len(b)==row['bytes']and sha(b)==row['sha256']and row['path']not in payload;payload[row['path']]=b
 payload['original-manifest.json']=(D/'manifest.json').read_bytes();payload['root-independent-review.json']=reviewpath.read_bytes();payload['publish-next-two-face-proof.py']=pathlib.Path(__file__).read_bytes();payload['manifest.json']=(json.dumps({'source':S,'tree':T,'files':[{'path':n,'bytes':len(b),'sha256':sha(b)}for n,b in payload.items()]},indent=2)+'\n').encode()
 assert git('rev-parse',S+'^{tree}')==T;assert not git('ls-remote','origin','refs/heads/'+B)
 with tempfile.TemporaryDirectory(prefix='nexttwo-evidence-',dir=ROOT/'work')as td:
  env=dict(os.environ,GIT_INDEX_FILE=str(pathlib.Path(td)/'index'));git('read-tree',S,env=env)
  for n,b in payload.items():git('update-index','--add','--cacheinfo','100644',git('hash-object','-w','--stdin',input=b,env=env),P+'/'+n,env=env)
  commit=git('commit-tree',git('write-tree',env=env),'-p',S,input=b'Retain actual two-feature environmental Face combined proof\n',env=env);git('update-ref','refs/heads/'+B,commit,'0'*40);subprocess.run(['git','-C',str(R),'push','--force-with-lease=refs/heads/'+B+':','origin','refs/heads/'+B],check=True)
 assert git('ls-remote','origin','refs/heads/'+B).split()[0]==commit
 for n,b in payload.items():assert subprocess.check_output(['git','-C',str(R),'show',commit+':'+P+'/'+n])==b
 out={'source':S,'tree':T,'commit':commit,'branch':B,'prefix':P,'remote_verified':True,'source_recoverable_as_direct_parent':True,'files':len(payload),'url':'https://github.com/dancxjo/conduit/tree/'+commit+'/'+P};(ROOT/'outputs/next-two-face-proof-publication.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(out,indent=2))
if __name__=='__main__':main()
