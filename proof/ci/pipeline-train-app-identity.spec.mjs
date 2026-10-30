import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
const helper = fileURLToPath(new URL('../../tools/ci/pipeline/train-app-identity.py', import.meta.url));
test('actual Python transport pins HTTPS identity GET, Bearer JWT, response bound, timeout and no redirects', () => {
  const program = `import runpy,sys,os,io,types,json
m=runpy.run_path(sys.argv[1]); os.environ['GH_TOKEN']='header.payload.signature'; sys.argv=['helper','app/installations/303']
class Response:
 def __enter__(self): return self
 def __exit__(self,*args): pass
 def read(self,bound):
  assert bound==2097153
  return b'{"id":303}'
class Opener:
 def open(self,request,timeout):
  assert request.full_url=='https://api.github.com/app/installations/303'
  assert request.get_method()=='GET' and request.data is None
  assert request.headers['Authorization']=='Bearer header.payload.signature'
  assert timeout==60
  return Response()
def opener(handler):
 assert handler().redirect_request(None,None,302,None,None,'https://foreign.invalid') is None
 return Opener()
m['urllib'].request.build_opener=opener
output=io.BytesIO(); original=sys.stdout; sys.stdout=types.SimpleNamespace(buffer=output,flush=lambda:None)
m['main'](); sys.stdout=original
assert json.loads(output.getvalue())=={'id':303}
for endpoint in ['app/installations/303/access_tokens','https://foreign.invalid','app?redirect=1']:
 sys.argv=['helper',endpoint]
 try: m['main'](); raise AssertionError('endpoint admitted')
 except ValueError: pass
`;
  const result = spawnSync('python3', ['-c', program, helper], { encoding: 'utf8' });
  assert.equal(result.status, 0, result.stderr);
});
test('actual transport failure cannot echo credential or traceback', () => {
  const secret = 'invalid-fixture-secret';
  const result = spawnSync('python3', [helper, 'app'], { encoding: 'utf8', env: { ...process.env, GH_TOKEN: secret } });
  assert.equal(result.status, 1); assert.equal(result.stdout, '');
  assert.equal(result.stderr, 'App identity observation refused\n');
});
