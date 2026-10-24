import subprocess,json,urllib.request,os,time,hashlib
from pathlib import Path
root=Path.cwd();out=root/'outputs/6387-current-product';source=root/'work/6393-permanent-cord-bound';target=root/'work/6393-permanent-cord-target';plot=out/'quantity_reproduction.conduit';plot.write_text('''plot quantity_reproduction {
 scalar: scalar/literal(value = 500000)
 map: math/map-quantity
 wrap: structured-info/wrap-quantity
 display: face/quantity("Measured quantity")
 scalar.value >> map.in
 map >> wrap >> display
}
''');env={**os.environ,'CONDUIT_BROWSER_RUNTIME_WASM':str(target/'wasm32-unknown-unknown/release/conduit_browser_runtime.wasm')}
records=[];err=(out/'minimal-server-stderr.log').open('w');stdout=(out/'minimal-server-stdout.log').open('w');p=subprocess.Popen([str(target/'debug/conduit-browser-patchbay-workbench'),'--plot','Empty Plot',str(plot)],cwd=source,env=env,stdout=subprocess.PIPE,stderr=err,text=True)
try:
 line=p.stdout.readline();stdout.write(line);stdout.flush();assert line.startswith('PATCHBAY_HTML_URL='),line;url=line.strip().split('=',1)[1].rstrip('/')
 def request(path,body=None):
  req=urllib.request.Request(url+path,data=None if body is None else json.dumps(body).encode(),headers={'Content-Type':'application/json'});return json.load(urllib.request.urlopen(req,timeout=30))
 state=request('/api/snapshot');(out/'minimal-initial-face.json').write_text(json.dumps(state,indent=2))
 selected_plot=next(s['identity'] for s in state['face']['subjects'] if s['role']=='Plot' and s['name']=='Empty Plot')
 for intent in ['open','birth','wake','plan','play']:
  action=next(a for a in state['face']['actions'] if a['intent']=='conduit.intent/'+intent+'@1' and a['availability']=='Available' and (intent not in ['open','birth'] or a['target']==selected_plot))
  req={'face_id':state['face']['identity'],'face_revision':state['face']['revision'],'kind':'invoke','action_id':action['identity']}
  state=request('/api/interaction',req);records.append({'intent':intent,'request':req,'disposition':state['interaction']['last_disposition'],'face':state['face']});assert records[-1]['disposition']=='Succeeded',records[-1]['disposition']
 (out/'minimal-final-face.json').write_text(json.dumps(state,indent=2));assert state['face']['basis']['plan_id'] and state['face']['basis']['active_play_id'];assert p.poll() is None
finally:
 p.terminate();p.wait(timeout=10);err.close();stdout.close();result={'source_commit':'a6353d7f189358c159bd718092cf2fa53b0aef4a','source':plot.read_text(),'records':records,'server_returncode':p.returncode,'termination':'harness SIGTERM after assertions','stderr':(out/'minimal-server-stderr.log').read_text()};(out/'minimal-result.json').write_text(json.dumps(result,indent=2)+'\n')
print('Five current minimal lifecycle actions succeeded')
