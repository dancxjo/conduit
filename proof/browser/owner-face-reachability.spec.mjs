import { expect, test } from '@playwright/test';

// Deterministic renderer conformance; source-backed owner proof is retained by
// todo-owner-browser's first-add entrance, not this callback fixture.
const view = {body_id:'fixture/body',face_id:'fixture/face',face_revision:1,show_id:'fixture/show',
  show_state:'prepared',interactions_admitted:false,subjects:[],relationships:[],
  actions:[{identity:'fixture/add',target:'fixture/list',name:'Add item',availability:'available',
    disclosure:'CurrentAction',arguments:[{name:'text',value_name:'Item text',value_kind:'value/text',maximum_bytes:64,choices:[]}]}]};
for (const attack of ['none','hidden','transparent','inert','clipped','pseudo','unnamed','overflow','aria-hidden','role','tabindex','scale']) {
  test(`generic renderer checks named reachable action before Show acknowledgement (${attack})`, async ({page}) => {
    await page.goto('/proof/browser/signal-dom-host.test.html');
    const result = await page.evaluate(async ({view,attack}) => {
      await import('/targets/browser/handbook/owner-face-component.mjs');
      const {assertOwnerFaceReachability} = await import('/targets/browser/handbook/owner-face-reachability.mjs');
      const element = document.createElement('conduit-owner-face');
      document.body.replaceChildren(element);
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
      try {assertOwnerFaceReachability(element,view);return {admitted:true};}
      catch(error) {return {admitted:false,code:error.code};}
    },{view,attack});
    expect(result).toEqual(attack==='none'?{admitted:true}:{admitted:false,code:'RendererActionInaccessible'});
  });
}
