import { chromium } from './gui-pr-update/proof/browser/node_modules/playwright/index.mjs';
import { mkdir, writeFile, readFile } from 'node:fs/promises';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
const phase = process.argv[2]; assert(['before', 'after'].includes(phase));
const folder = new URL(`./handbook-chrome-${phase}/`, import.meta.url); await mkdir(folder, {recursive:true});
const browser = await chromium.launch({ headless:true });
const results=[];
try {
 for (const [name,width,scale,forced] of [['wide',1280,100,false],['narrow-forced',390,200,true]]) {
  const context=await browser.newContext({viewport:{width,height:1000},forcedColors:forced?'active':'none',reducedMotion:'reduce'});
  const page=await context.newPage();const errors=[];page.on('pageerror',e=>errors.push(e.message));
  await page.route('**/local-handbook-chrome-proof', route=>route.fulfill({contentType:'text/html',body:'<!doctype html><html><head><meta charset="utf-8"></head><body></body></html>'}));
  await page.goto('http://127.0.0.1:4196/local-handbook-chrome-proof');
  await page.evaluate(async scale=>{
   document.body.innerHTML='<main class="site-content"><section data-handbook-application></section></main>';
   document.documentElement.style.fontSize=`${scale}%`;
   for(const href of ['/targets/browser/host/assets/conduit.css','/targets/browser/handbook/application.css']) {
    const link=document.createElement('link');link.rel='stylesheet';link.href=href;document.head.append(link);
    await new Promise((resolve,reject)=>{link.onload=resolve;link.onerror=reject});
   }
   const {startApplication}=await import('/targets/browser/handbook/application.mjs');
   const specification={name:'Handbook chrome proof',initialPlots:['demo'],plots:[{role:'demo-source',name:'demo',title:'A Unicode example — café 世界',lesson:'Read the exact source. Recovery leaves the Body unchanged.'}]};
   const source='plot demo (\n) {\n # café 世界 — retained source\n}\n';
   globalThis.expectedSource=source;
   await startApplication({text:role=>role==='birth-specification'?JSON.stringify(specification):source,
    manifest:{applicationId:'local-chrome-proof',resources:[]},storage:{readJson:async()=>null},
    browser:async()=>{const error=new Error('Deliberate opening refusal for the recovery-UI proof; no Body was created.');error.code='ProofOpeningRefusal';throw error;}});
  },scale);
  await page.getByRole('button',{name:'Download displayed source',exact:true}).focus();
  assert.equal(await page.getByRole('button',{name:'Download displayed source',exact:true}).evaluate(n=>document.activeElement===n),true);
  const downloadPromise=page.waitForEvent('download');await page.keyboard.press('Enter');const download=await downloadPromise;
  const stream=await download.createReadStream();let data='';for await(const chunk of stream)data+=chunk.toString();
  assert.equal(data,await page.evaluate(()=>expectedSource));
  const disclosure=page.locator('summary').filter({hasText:'Inspect source provenance'});await disclosure.focus();await page.keyboard.press('Enter');
  assert.equal(await disclosure.evaluate(n=>n.parentElement.open),true);
  const source=page.getByRole('textbox',{name:'Plot source',exact:true});await source.fill('plot demo () { # draft café 世界 }');
  assert.equal(await source.inputValue(),'plot demo () { # draft café 世界 }');
  await page.locator('[data-check]').filter({hasText:'Edited source, not yet checked or applied.'}).waitFor();
  const bodyDisclosure=page.locator('summary').filter({hasText:'Your body and browser'});
  if(phase==='after'){await bodyDisclosure.focus();await page.keyboard.press('Enter');assert.equal(await bodyDisclosure.evaluate(n=>n.parentElement.open),true);}
  const result=await page.evaluate(()=>({overflow:document.documentElement.scrollWidth>innerWidth,
   source:document.querySelector('textarea').value,evidence_font_px:parseFloat(getComputedStyle(document.querySelector('[data-source-provenance]')).fontSize),source_font_px:parseFloat(getComputedStyle(document.querySelector('textarea')).fontSize),status:document.querySelector('[data-session-status]').textContent,
   buttons:[...document.querySelectorAll('.handbook-application button')].map(n=>({label:n.textContent.trim(),disabled:n.disabled,height:n.getBoundingClientRect().height,icons:n.querySelectorAll('svg[aria-hidden="true"]').length})),
   focus:document.querySelector('summary').getAttribute('role'),icons:[...document.querySelectorAll('svg')].every(n=>n.getAttribute('aria-hidden')==='true'&&n.getAttribute('focusable')==='false')}));
  if(phase==='after'){assert.equal(result.overflow,false);assert(result.buttons.every(b=>b.height>=44));assert(result.buttons.every(b=>b.icons===1));assert.equal(result.icons,true);assert(result.source_font_px>=16*scale/100);assert(result.evidence_font_px>=16*scale/100);}
  assert.deepEqual(errors,[]);
  if(phase==='after'){await bodyDisclosure.focus();await page.keyboard.press('Enter');}
  await page.evaluate(()=>scrollTo(0,0));
  const path=new URL(`${name}.png`,folder);await page.screenshot({path:path.pathname,fullPage:true});
  results.push({name,width,scale,forced,...result,errors,png_sha256:createHash('sha256').update(await readFile(path)).digest('hex')});
  await context.close();
 }
 await writeFile(new URL('receipt.json',folder),JSON.stringify({phase,scope:'actual Handbook module and CSS with deliberate opening refusal; native download/disclosure/source editing; not a live Body or storage-failure receipt',results},null,2)+'\n');
 console.log(JSON.stringify(results.map(r=>({name:r.name,overflow:r.overflow,buttons:r.buttons.length})),null,2));
}finally{await browser.close()}
