import { readFile } from 'node:fs/promises';
import { expect, test } from '@playwright/test';

// Deterministic renderer conformance; source-backed owner proof is retained by
// todo-owner-browser's first-add entrance, not this callback fixture.
const view = {body_id:'fixture/body',face_id:'fixture/face',face_revision:1,show_id:'fixture/show',
  show_state:'prepared',interactions_admitted:false,subjects:[],relationships:[],
  actions:[{identity:'fixture/add',target:'fixture/list',name:'Add item',availability:'available',
    disclosure:'CurrentAction',arguments:[{name:'text',value_name:'Item text',value_kind:'value/text',maximum_bytes:64,choices:[]}]}]};
for (const attack of ['none','hidden','transparent','inert','clipped','pseudo','unnamed','overflow','aria-hidden','role','tabindex','scale','disabled','aria-disabled','aria-label','aria-labelledby','read-only','read-only-forged-enabled','prepared-forged-enabled']) {
  test(`generic renderer checks named reachable action before Show acknowledgement (${attack})`, async ({page}) => {
    await page.goto('/proof/browser/signal-dom-host.test.html');
    const result = await page.evaluate(async ({view,attack}) => {
      await import('/targets/browser/handbook/owner-face-component.mjs');
      const {assertOwnerFaceReachability} = await import('/targets/browser/handbook/owner-face-reachability.mjs');
      const element = document.createElement('conduit-owner-face');
      document.body.replaceChildren(element);
      if (['disabled','aria-disabled','aria-label','aria-labelledby'].includes(attack)) {
        view={...view,show_state:'available',interactions_admitted:true};
      }
      if (attack.startsWith('read-only')) view={...view,show_state:'available',interactions_admitted:false};
      element.show(view,{invoke:async()=>{throw new Error('preparation cannot invoke');}});
      if (attack==='hidden') element.hidden=true;
      if (attack==='transparent') element.style.opacity='0';
      if (attack==='inert') element.inert=true;
      if (attack==='aria-hidden') element.setAttribute('aria-hidden','true');
      if (attack==='scale') {element.style.display='block';element.style.transform='scale(0)';}
      if (attack==='overflow') {element.style.display='block';element.style.overflow='hidden';element.style.height='1px';}
      if (attack==='clipped') element.style.clipPath='inset(100%)';
      const button=element.shadowRoot.querySelector('button');
      if (attack==='pseudo') {const pseudo=document.createElement('div');pseudo.textContent=button.textContent;pseudo.tabIndex=0;button.replaceWith(pseudo);}
      if (attack==='unnamed') button.textContent='';
      if (attack==='role') button.setAttribute('role','presentation');
      if (attack==='tabindex') button.tabIndex=-1;
      if (attack==='disabled') button.disabled=true;
      if (attack.endsWith('forged-enabled')) button.disabled=false;
      if (attack==='aria-disabled') button.setAttribute('aria-disabled','true');
      if (attack==='aria-label') button.setAttribute('aria-label','Start a different action');
      if (attack==='aria-labelledby') button.setAttribute('aria-labelledby','forged-name');
      try {assertOwnerFaceReachability(element,view);return {admitted:true};}
      catch(error) {return {admitted:false,code:error.code};}
    },{view,attack});
    expect(result).toEqual(['none','read-only'].includes(attack)?{admitted:true}:{admitted:false,code:'RendererActionInaccessible'});
  });
}

for (const attack of ['none','hidden-label','hidden-input','aria-label']) {
  test(`production CSS native radio keeps a certified visible named label (${attack})`, async ({page}) => {
    await page.goto('/proof/browser/signal-dom-host.test.html');
    const result=await page.evaluate(async ({base,attack})=>{
      await import('/targets/browser/handbook/owner-face-component.mjs');
      const {assertOwnerFaceReachability}=await import('/targets/browser/handbook/owner-face-reachability.mjs');
      const subject=(identity,name)=>({identity,name,role:'Info',disclosure:'Primary',text:[],properties:[],
        unmodeled_property_count:0,values:[]});
      const choices={...base,show_state:'available',interactions_admitted:true,
        subjects:[{...subject('group','Mode'),semantic_role:'conduit.face/choice-group@1',choice_multiplicity:'exclusive'},
          ...[0,1].map(index=>({...subject(`option/${index}`,`Mode ${index}`),flags:[{name:'selected',value:index===0}]}))],
        relationships:[0,1].map(index=>({kind:'Contains',source:'group',target:`option/${index}`})),
        actions:[0,1].map(index=>({identity:`select/${index}`,target:`option/${index}`,name:`Select ${index}`,
          availability:'available',disclosure:'CurrentAction',arguments:[]}))};
      const element=document.createElement('conduit-owner-face');document.body.replaceChildren(element);
      element.show(choices,{invoke:async()=>{}});
      const look=element.shadowRoot.querySelector('link');
      if(!look.sheet) await new Promise((resolve,reject)=>{look.addEventListener('load',resolve,{once:true});look.addEventListener('error',reject,{once:true});});
      const input=element.shadowRoot.querySelector('input[type=radio]');
      if(attack==='hidden-label') input.labels[0].style.opacity='0';
      if(attack==='hidden-input') input.hidden=true;
      if(attack==='aria-label') input.setAttribute('aria-label','Misleading action');
      try {assertOwnerFaceReachability(element,choices);return {admitted:true,opacity:getComputedStyle(input).opacity};}
      catch(error) {return {admitted:false,code:error.code};}
    },{base:view,attack});
    expect(result).toEqual(attack==='none'?{admitted:true,opacity:'0'}:{admitted:false,code:'RendererActionInaccessible'});
    if(attack==='none') await expect(page.getByRole('radio',{name:'Mode 1',exact:true})).toBeEnabled();
  });
}

for (const corruptDependency of [false,true]) {
  test(`production package loader admits the actual owner module graph (${corruptDependency?'mismatched':'current'})`, async ({page}) => {
    const template=JSON.parse(await readFile('targets/browser/handbook/handbook.application.template.json','utf8'));
    const modules=await Promise.all(['owner-participation','owner-face-component','owner-face-reachability'].map(async role=>{
      const resource=template.resources.find(resource=>resource.role===role);
      let source=await readFile(`targets/browser/handbook/${resource.path}`,'utf8');
      if(corruptDependency && role==='owner-participation') source=source.replace('"./owner-face-reachability.mjs"',"'./owner-face-reachability.mjs'");
      return {...resource,source};
    }));
    await page.goto('/proof/browser/signal-dom-host.test.html');
    await page.evaluate(async modules=>{
      document.body.innerHTML='<main id="workspace"></main>';
      const meta=document.createElement('meta');meta.name='conduit-application-package';meta.content='/fixture-owner-application.json';document.head.append(meta);
      const encoder=new TextEncoder();
      const hash=async bytes=>'sha256:'+Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',bytes)),byte=>byte.toString(16).padStart(2,'0')).join('');
      // Only package module admission is exercised; no SDK Host/Body is invented.
      const entry='import "./owner-participation.mjs"; export async function startApplication(){document.querySelector("#workspace").textContent="Owner modules admitted";}';
      const definitions=[{role:'application-module',kind:'module',path:'entry.mjs',source:entry,
        dependencies:[{role:'owner-participation',specifier:'./owner-participation.mjs'}]},...modules,
        {role:'runtime',kind:'wasm',path:'runtime.wasm',source:'unused fixture envelope'}];
      const resources=[],bodies=new Map();
      for(const definition of definitions){const bytes=encoder.encode(definition.source);bodies.set(definition.path,bytes);
        resources.push({role:definition.role,kind:definition.kind,path:definition.path,maximum_bytes:bytes.length,
          sha256:await hash(bytes),dependencies:definition.dependencies??[]});}
      const manifest={schema:'conduit.browser/application-package@1',application_id:'fixture/owner-module-admission',
        state_compatibility:{identity:'fixture/owner-module-state',version:1},host_implementations:['browser/indexeddb@1'],resources};
      const lines=['conduit.browser/application-package-content@1','application\0'+manifest.application_id,
        'state\0'+manifest.state_compatibility.identity+'\0'+1,'host-implementation\0browser/indexeddb@1'];
      for(const resource of resources) lines.push('resource\0'+[resource.role,resource.kind,resource.path,resource.maximum_bytes,
        resource.sha256,resource.dependencies.map(dep=>dep.role+'='+dep.specifier).join(',')].join('\0'));
      manifest.package_digest=await hash(encoder.encode(lines.join('\n')+'\n'));
      globalThis.fetch=async url=>new Response(String(url).endsWith('/fixture-owner-application.json')?JSON.stringify(manifest):bodies.get(new URL(url).pathname.split('/').at(-1)));
      await import('/targets/browser/host/assets/browser-application-loader.mjs');
    },modules);
    await expect(page.locator('#workspace')).toHaveText(corruptDependency
      ?'application module owner-participation did not use its declared dependency':'Owner modules admitted');
  });
}
