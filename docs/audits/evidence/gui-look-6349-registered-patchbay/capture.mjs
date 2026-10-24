// Internal checked-Mask CPU view -> real renderer evidence. No Body effect.
import {spawn} from 'node:child_process';
import {readFile,writeFile} from 'node:fs/promises';
import assert from 'node:assert/strict';
import {chromium} from './gui-pr-update/proof/browser/node_modules/playwright/index.mjs';
const server=spawn(process.execPath,['proof/browser/static-server.mjs','4187'],{cwd:process.cwd(),stdio:'ignore'});
let browser;const records=[];let semantics;
try{
 for(let i=0;i<50;i++){try{if((await fetch('http://127.0.0.1:4187/proof/browser/signal-dom-host.test.html')).ok)break;}catch{}await new Promise(r=>setTimeout(r,100));}
 browser=await chromium.launch({headless:true});
 for(const [name,width,forced] of [['phosphor',1100,false],['editorial',1100,false],['default',1100,false],['phosphor-forced',390,true]]){
  const view=JSON.parse(await readFile(`../registered-patchbay-${name}.json`,'utf8'));
  const current={face_id:view.face_id,subjects:view.subjects,actions:view.actions,relationships:view.relationships};
  if(semantics)assert.deepEqual(current,semantics);else semantics=current;
  assert.equal(view.component.recipe_identity,'conduit.face/patchbay-nodes@1');assert.equal(view.component.fallback,null);
  assert.equal(view.show_state,'available');assert.equal(view.interactions_admitted,true);
  const page=await browser.newPage({viewport:{width,height:1000}});const errors=[];page.on('pageerror',e=>errors.push(e.message));
  await page.emulateMedia({forcedColors:forced?'active':'none',reducedMotion:'reduce'});
  await page.route('**/registered-component-fixture',route=>route.fulfill({status:200,contentType:'text/html',body:'<!doctype html><html lang="en"><meta charset="utf-8"><title>Registered Patchbay renderer fixture</title><body></body></html>'}));
  await page.goto('http://127.0.0.1:4187/registered-component-fixture');
  await page.evaluate(async view=>{
   await import('/targets/browser/handbook/owner-face-component.mjs');
   document.body.style.cssText='margin:12px';const node=document.createElement('conduit-owner-face');globalThis.calls=[];
   node.show(view,{invoke:async call=>globalThis.calls.push(call)});document.body.replaceChildren(node);
   document.body.style.background=node.style.getPropertyValue('--look-background');
  },view);
  const face=page.locator('conduit-owner-face');const button=face.getByRole('button',{name:'Inspect Echo node',exact:true});
  await button.waitFor();await page.evaluate(()=>document.fonts.ready);
  await page.waitForFunction(columns=>document.querySelector('conduit-owner-face').dataset.componentColumns===columns,forced?'1':'2');
  assert.equal(await face.locator('[data-face-role=Gear]').count(),1);assert.equal(await face.locator('[data-face-role=Port]').count(),2);
  assert.deepEqual(await face.locator('.owner-component-region-run').evaluateAll(nodes=>nodes.map(n=>({group:n.dataset.componentGroup,subjects:n.querySelectorAll('[data-face-role]').length}))),[{group:'0',subjects:1},{group:'1',subjects:2}]);
  const font=await button.evaluate(n=>parseFloat(getComputedStyle(n).fontSize));assert.equal(font,forced?32:16);
  await page.setViewportSize({width,height:Math.min(1400,Math.ceil((await face.boundingBox()).height)+24)});
  assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false);
  await button.focus();await page.screenshot({path:`../../outputs/checked-patchbay-${name}.png`,fullPage:true});
  await page.keyboard.press('Enter');const calls=await page.evaluate(()=>globalThis.calls);
  assert.equal(calls.length,1);assert.deepEqual(calls[0],{view,action:view.actions[0],arguments:[]});assert.deepEqual(errors,[]);
  records.push({name,width,font,face_id:view.face_id,show_id:view.show_id,mask_plot_id:view.mask_plot_id,mask_plan_id:view.mask_plan_id,look:await face.getAttribute('data-component-look'),source:view.component.configuration.component.source_document_id,subjects:view.subjects.length,actions:view.actions.length,overflow:false,page_errors:errors});
  await page.close();
 }
 await writeFile('../registered-patchbay-renderer-receipt.json',JSON.stringify({proof:'registered checked Mask Source and CPU Show over a synthetic node/port Face, then real browser renderer; no live owner execution',records},null,2)+'\n');
 console.log(JSON.stringify({captures:records.length,fonts:records.map(r=>r.font),status:'PASS'}));
}finally{await browser?.close();server.kill();}
