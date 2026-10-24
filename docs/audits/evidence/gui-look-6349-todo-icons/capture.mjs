import { spawn } from 'node:child_process';
import { chromium } from './gui-pr-update/proof/browser/node_modules/playwright/index.mjs';
import { mkdir } from 'node:fs/promises';
const server=spawn(process.execPath,['proof/browser/static-server.mjs','4187'],{cwd:process.cwd(),stdio:'ignore'});
let browser;
try {
 for(let i=0;i<50;i++){try{const r=await fetch('http://127.0.0.1:4187/proof/browser/signal-dom-host.test.html');if(r.ok)break;}catch{} await new Promise(r=>setTimeout(r,100));}
 browser=await chromium.launch({headless:true});const page=await browser.newPage({viewport:{width:1000,height:600}});
 await page.goto('http://127.0.0.1:4187/proof/browser/signal-dom-host.test.html');
 await page.evaluate(async()=>{
 await import('/targets/browser/handbook/owner-face-component.mjs');
 document.body.style.cssText='margin:24px;background:#f5f2eb';
 const caption=document.createElement('p');caption.textContent='Renderer preview · Todo action icons · no Body execution receipt';caption.style.font='14px system-ui';
 const node=document.createElement('conduit-owner-face');
 const actions=[['add','Add task'],['complete','Complete task'],['remove','Remove task']].map(([verb,name])=>({identity:`fixture/${verb}`,target:'fixture/task',intent:`todo/${verb}@1`,name,availability:'available',disclosure:'CurrentAction',arguments:[]}));
 node.show({body_id:'fixture/body',face_id:'fixture/face',face_revision:1,show_id:'fixture/show',show_state:'available',interactions_admitted:true,subjects:[{identity:'fixture/task',name:'Read the handbook',role:'Item',disclosure:'Primary',text:[],properties:[],unmodeled_property_count:0,values:[]}],relationships:[],actions},{invoke:async()=>{}});
 document.body.replaceChildren(caption,node);
 });
 await page.getByRole('button',{name:'Remove task',exact:true}).waitFor();await page.evaluate(()=>document.fonts.ready);
 await mkdir('../..//outputs',{recursive:true});
 await page.screenshot({path:'../../outputs/todo-action-icons.png'});
}finally{await browser?.close();server.kill();}
