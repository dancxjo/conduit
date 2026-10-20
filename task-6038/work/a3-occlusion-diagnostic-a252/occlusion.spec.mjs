import {test,expect} from "/home/dancxjo/Documents/Codex/2026-10-19/goal-6038/work/owner-presentation/proof/browser/node_modules/@playwright/test/index.mjs";
const view={body_id:'fixture/body',face_id:'fixture/face',face_revision:1,show_id:'fixture/show',show_state:'available',interactions_admitted:true,
 subjects:[{identity:'fixture/task',name:'Read the handbook',role:'Info',disclosure:'Primary',text:[],properties:[],unmodeled_property_count:0,values:[{name:'Completion',text:'Open'}]}],relationships:[],
 actions:[{identity:'fixture/complete',target:'fixture/task',name:'Complete task',availability:'available',disclosure:'CurrentAction',arguments:[]}]};
for(const scope of ['none','shadow','document']) test(`occluded offered control must refuse (${scope})`,async({page},testInfo)=>{
 await page.goto('/proof/browser/signal-dom-host.test.html');
 const observed=await page.evaluate(async({view,scope})=>{
  await import('/targets/browser/handbook/owner-face-component.mjs');
  const {assertOwnerFaceReachability}=await import('/targets/browser/handbook/owner-face-reachability.mjs');
  const element=document.createElement('conduit-owner-face');document.body.replaceChildren(element);let callbacks=0;
  element.show(view,{invoke:async()=>{callbacks++;}});
  const link=element.shadowRoot.querySelector('link');
  if(!link.sheet) await new Promise((resolve,reject)=>{link.addEventListener('load',resolve,{once:true});link.addEventListener('error',reject,{once:true});});
  assertOwnerFaceReachability(element,view);
  const control=element.shadowRoot.querySelector('button'),box=control.getBoundingClientRect();
  if(!(box.width>0&&box.height>0)) throw new Error('Baseline has no button box');
  const x=box.x+box.width/2,y=box.y+box.height/2;
  let blocker=null;
  if(scope!=='none'){
   blocker=document.createElement('div');blocker.id='pointer-blocker';blocker.textContent='Decoration covers the offered action';
   blocker.style.cssText=`position:fixed;left:${box.left}px;top:${box.top}px;width:${box.width}px;height:${box.height}px;z-index:2147483647;background:white;color:black;pointer-events:auto`;
   (scope==='shadow'?element.shadowRoot:document.body).append(blocker);
  }
  const top=document.elementFromPoint(x,y);let deepest=top;
  for(let depth=0;depth<8&&deepest?.shadowRoot;depth++){
   const next=deepest.shadowRoot.elementFromPoint(x,y);if(!next||next===deepest)break;deepest=next;
  }
  let admitted=true,code=null;
  try{assertOwnerFaceReachability(element,view);}catch(error){admitted=false;code=error.code;}
  return {scope,baselinePassed:true,hitIsBlocker:deepest===blocker&&blocker!==null,hitIsControl:deepest===control||control.contains(deepest),admitted,code,callbacks,box:{x:box.x,y:box.y,width:box.width,height:box.height}};
 },{view,scope});
 await testInfo.attach('observation.json',{body:JSON.stringify(observed,null,2),contentType:'application/json'});
 await page.screenshot({path:testInfo.outputPath('observed.png')});
 console.log('OCCLUSION_OBSERVATION '+JSON.stringify(observed));
 expect(observed.callbacks).toBe(0);
 expect(observed.baselinePassed).toBe(true);
 if(scope==='none'){expect(observed.hitIsControl).toBe(true);expect(observed.admitted).toBe(true);}
 else{expect(observed.hitIsBlocker).toBe(true);expect(observed).toMatchObject({admitted:false,code:'RendererActionInaccessible'});}
});
