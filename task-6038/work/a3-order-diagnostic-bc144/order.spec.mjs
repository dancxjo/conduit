import { test, expect } from '/home/dancxjo/Documents/Codex/2026-10-19/goal-6038/work/owner-presentation/proof/browser/node_modules/@playwright/test/index.mjs';
for (const attack of ['none', 'column-reverse', 'order']) test(`preacknowledgement rejects changed visual action order (${attack})`, async ({ page }, info) => {
 await page.goto('/proof/browser/signal-dom-host.test.html');
 const result = await page.evaluate(async attack => {
  await import('/targets/browser/handbook/owner-face-component.mjs');
  const { assertOwnerFaceReachability } = await import('/targets/browser/handbook/owner-face-reachability.mjs');
  const ids = ['first', 'second', 'third'];
  const view = { body_id:'fixture/body',face_id:'fixture/face',face_revision:1,show_id:'fixture/show',show_state:'prepared',interactions_admitted:false,
   subjects:ids.map(id=>({identity:`fixture/${id}`,name:id,role:'Info',disclosure:'Primary',text:[],properties:[],unmodeled_property_count:0,values:[]})),relationships:[],
   actions:ids.map(id=>({identity:`fixture/action/${id}`,target:`fixture/${id}`,name:`Act ${id}`,availability:'available',disclosure:'CurrentAction',arguments:[]})) };
  const el=document.createElement('conduit-owner-face');document.body.replaceChildren(el);let calls=0;el.show(view,{invoke:async()=>calls++});
  const link=el.shadowRoot.querySelector('link');if(!link.sheet)await new Promise((r,j)=>{link.addEventListener('load',r,{once:true});link.addEventListener('error',j,{once:true});});
  assertOwnerFaceReachability(el,view);
  const surface=el.shadowRoot.querySelector('.owner-face-document');
  if(attack==='column-reverse'){surface.style.display='flex';surface.style.flexDirection='column-reverse';}
  if(attack==='order'){surface.style.display='flex';surface.style.flexDirection='column';[...surface.children].forEach((n,i)=>n.style.order=String(-i));}
  const controls=[...el.shadowRoot.querySelectorAll('button')];
  const boxes=controls.map(b=>({name:b.textContent,action:b.closest('[data-owner-action]').dataset.ownerAction,y:b.getBoundingClientRect().y,tabIndex:b.tabIndex}));
  let admitted=true,code=null;try{assertOwnerFaceReachability(el,view);}catch(e){admitted=false;code=e.code;}
  return {attack,sourceOrder:view.actions.map(a=>a.identity),domOrder:boxes.map(b=>b.action),visualOrder:[...boxes].sort((a,b)=>a.y-b.y).map(b=>b.action),boxes,admitted,code,calls,
   boundary:'Adversarial runtime style mutation of trusted renderer; not a tampered artifact admitted by the selected Back hash checker; no Body or owner effects'};
 },attack);
 await info.attach('observation.json',{body:JSON.stringify(result,null,2),contentType:'application/json'});await page.screenshot({path:info.outputPath('observed.png')});console.log('ORDER_OBSERVATION '+JSON.stringify(result));
 expect(result.calls).toBe(0);expect(result.domOrder).toEqual(result.sourceOrder);
 if(attack==='none'){expect(result.visualOrder).toEqual(result.sourceOrder);expect(result.admitted).toBe(true);}
 else{expect(result.visualOrder).not.toEqual(result.sourceOrder);expect(result.admitted).toBe(false);}
});
