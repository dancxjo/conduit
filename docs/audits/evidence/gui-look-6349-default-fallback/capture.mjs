// Internal renderer evidence. Canonical admission fixture views are not Body receipts.
import { readFile, mkdir, writeFile } from 'node:fs/promises';
import { spawn } from 'node:child_process';
import assert from 'node:assert/strict';
import { chromium } from './gui-pr-update/proof/browser/node_modules/playwright/index.mjs';
const server=spawn(process.execPath,['proof/browser/static-server.mjs','4187'],{cwd:process.cwd(),stdio:'ignore'});
let browser;
const records=[];
try {
 for(let i=0;i<50;i++){try{if((await fetch('http://127.0.0.1:4187/proof/browser/signal-dom-host.test.html')).ok)break;}catch{}await new Promise(r=>setTimeout(r,100));}
 browser=await chromium.launch({headless:true});
 for(const [name,width,forced] of [['normal',1100,false],['forced',390,true]]){
  const view=JSON.parse(await readFile(`../nonvisual-view-${name}.json`,'utf8'));
  assert.equal(view.component.expression.outcome.PhosphorFallback.refusal,'MissingVisualFacet');
  assert.equal(view.component.expression.expression.identity,'conduit.look/phosphor@1');
  const page=await browser.newPage({viewport:{width,height:1000}});
  await page.emulateMedia({forcedColors:forced?'active':'none',reducedMotion:'reduce'});
  const errors=[];page.on('pageerror',e=>errors.push(e.message));
  await page.route('**/nonvisual-renderer-fixture', route => route.fulfill({
   status:200, contentType:'text/html', body:'<!doctype html><html lang="en"><meta charset="utf-8"><title>Default Look renderer fixture</title><body></body></html>',
  }));
  await page.goto('http://127.0.0.1:4187/nonvisual-renderer-fixture');
  await page.evaluate(async view=>{
   await import('/targets/browser/handbook/owner-face-component.mjs');
   document.body.style.cssText='margin:12px;background:#05070b';
   const element=document.createElement('conduit-owner-face');
   globalThis.calls=[];element.show(view,{invoke:async call=>globalThis.calls.push(call)});
   document.body.replaceChildren(element);
  },view);
  const face=page.locator('conduit-owner-face');
  const button=face.getByRole('button',{name:'complete',exact:true}).first();
  await button.waitFor();await page.evaluate(()=>document.fonts.ready);
  const metrics=await button.evaluate(node=>({font:Number.parseFloat(getComputedStyle(node).fontSize),width:node.getBoundingClientRect().width,height:node.getBoundingClientRect().height}));
  assert.ok(metrics.width>=44&&metrics.height>=44);
  const overflow=await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth);assert.equal(overflow,false);
  await mkdir('../../outputs',{recursive:true});
  await button.scrollIntoViewIfNeeded();
  await button.focus();
  await page.screenshot({path:`../../outputs/nonvisual-look-${name}.png`});
  await button.focus();await page.keyboard.press('Enter');
  const calls=await page.evaluate(()=>globalThis.calls);
  assert.equal(calls.length,1);assert.deepEqual(calls[0].action,view.actions.find(a=>a.intent==='todo/complete@1'));
  assert.deepEqual(calls[0].view,view);assert.deepEqual(errors,[]);
  records.push({name,width,forced_colors:forced,metrics,actions:view.actions.length,subjects:view.subjects.length,semantic_face:view.face_id,show:view.show_id,keyboard_action:calls[0].action,overflow,page_errors:errors});
  await page.close();
 }
 assert.equal(records[1].metrics.font,2*records[0].metrics.font);
 await writeFile('../nonvisual-renderer-receipt.json',JSON.stringify({proof:'synthetic Face fixture with real canonical Look admission and browser renderer; not a browser Mask Plan/Show or Body receipt',records},null,2)+'\n');
 console.log(JSON.stringify({captures:records.length,font_scales:records.map(r=>r.metrics.font),status:'PASS'}));
}finally{await browser?.close();server.kill();}
