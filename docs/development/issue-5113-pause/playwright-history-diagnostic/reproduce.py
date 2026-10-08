import json, os, pathlib, subprocess, tempfile
repo=pathlib.Path.cwd()
cli=repo/'proof/browser/node_modules/@playwright/test/cli.js'
config=(repo/'proof/browser/playwright.config.mjs').as_uri()
def run(*args, cwd=None, env=None):
 return subprocess.check_output(args,cwd=cwd,env=env,stderr=subprocess.STDOUT,text=True).strip()
results=[]
with tempfile.TemporaryDirectory(prefix='conduit-playwright-history-') as tmp:
 root=pathlib.Path(tmp); origin=root/'origin'
 run('git','init',str(origin))
 run('git','config','user.name','History proof',cwd=origin)
 run('git','config','user.email','history@example.invalid',cwd=origin)
 for name in ['ancestor','base','head']:
  run('git','-c','commit.gpgsign=false','commit','--allow-empty','-m',name,cwd=origin)
 head=run('git','rev-parse','HEAD',cwd=origin)
 base=run('git','rev-parse','HEAD~1',cwd=origin)
 ancestor=run('git','rev-parse','HEAD~2',cwd=origin)
 event=root/'event.json';event.write_text(json.dumps({'pull_request':{'title':'history proof','number':1,'base':{'sha':base}}}))
 for fixed in [False,True]:
  checkout=root/('fixed' if fixed else 'default');run('git','clone','--no-local',str(origin),str(checkout))
  (checkout/'history.spec.cjs').write_text('const {test,expect}=require('+json.dumps(str(repo/'proof/browser/node_modules/@playwright/test'))+');test("metadata only",()=>expect(1).toBe(1));')
  (checkout/'playwright.config.mjs').write_text('import original from '+json.dumps(config)+';const config={...original,testDir:".",testMatch:["history.spec.cjs"],webServer:undefined,outputDir:"results"};'+('' if fixed else 'delete config.captureGitInfo;')+'export default config;')
  env={**os.environ,'CI':'1','GITHUB_ACTIONS':'true','GITHUB_EVENT_PATH':str(event),'GITHUB_SHA':head,'GITHUB_REPOSITORY':'fixture/history'}
  output=run('node',str(cli),'test','--config',str(checkout/'playwright.config.mjs'),cwd=checkout,env=env)
  shallow=run('git','rev-parse','--is-shallow-repository',cwd=checkout)
  ancestry=subprocess.run(['git','merge-base','--is-ancestor',ancestor,'HEAD'],cwd=checkout).returncode
  assert shallow==('false' if fixed else 'true'),(shallow,output)
  assert ancestry==(0 if fixed else 1),ancestry
  results.append({'diff_capture_disabled':fixed,'shallow':shallow,'retained_ancestor_exit':ancestry,'test_passed':'1 passed' in output})
print(json.dumps(results,indent=2))
