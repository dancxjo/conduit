import { chromium } from '/home/dancxjo/conduit/node_modules/playwright/index.mjs';
import fs from 'node:fs';
const out='/home/dancxjo/Documents/Codex/2026-10-21/please-finish-and-close-any-conduit/outputs/6351-repaired-captures';fs.mkdirSync(out,{recursive:true});
const browser=await chromium.launch();const page=await browser.newPage({viewport:{width:1200,height:800}});
await page.goto('http://127.0.0.1:46752/proof/browser/signal-dom-host.test.html');
for(const mode of ['normal','forced']) {
 await page.emulateMedia({forcedColors:mode==='forced'?'active':'none'});
 await page.evaluate(async()=>{
  await import('/targets/browser/handbook/owner-face-component.mjs');
  const element=document.createElement('conduit-owner-face');document.body.replaceChildren(element);
  document.body.style.cssText='margin:32px;background:#d9dfde';
  const view={body_id:'fixture/body',face_id:'fixture/face',face_revision:1,show_id:'fixture/show',show_state:'available',interactions_admitted:true,
   subjects:[{identity:'fixture/task',name:'Read the handbook',role:'Body',disclosure:'Primary',text:['Renderer fixture: Phosphor palette with engineering typography.'],properties:[],unmodeled_property_count:0,values:[]}],relationships:[],
   actions:[{identity:'fixture/complete',target:'fixture/task',name:'Complete task',availability:'available',disclosure:'CurrentAction',arguments:[]}],
   component:{maximum_columns:1,reflow_width:720,gutter:12,subject_regions:[null],groups:[],expression:{expression:{identity:'fixture/selected-look',version:2,density:'compact',palette:'phosphor',typography:'engineering',text_scale_percent:100}}}};
  element.show(view,{invoke:async()=>{}});
 });
 await page.waitForFunction(()=>document.querySelector('conduit-owner-face')?.shadowRoot.querySelector('h4')?.getBoundingClientRect().height>0);
 await page.screenshot({path:`${out}/${mode}.png`});
}
await browser.close();
