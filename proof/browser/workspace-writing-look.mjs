// Local product proof: pass an explicitly staged Workspace and evidence directory.
// Uses actual Birth/Wake/editing, not fixture action callbacks.
import assert from 'node:assert/strict';
import {mkdir,writeFile,readFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {chromium} from 'playwright';
import {startStaticProduct} from './static-product-server.mjs';
import {enterMemoryText,memoryEditor} from './workspace-editor.mjs';
const product=process.argv[2],output=process.argv[3];
if(!product||!output)throw new Error('usage: workspace-writing-look.mjs STAGED_PRODUCT EVIDENCE_DIRECTORY');await mkdir(output,{recursive:true});
const server=await startStaticProduct(product,'/conduit/workspace/');const browser=await chromium.launch();
const report={browser_errors:[],canonical_resident_look_selection:false,workspace_chrome_looks:[]};
try{
 const page=await browser.newPage({viewport:{width:1440,height:1050}});page.on('pageerror',e=>report.browser_errors.push(e.message));
 await page.goto(server.url);const birth=page.locator('.body-birth-runner');
 await birth.getByLabel('Friendly Body name',{exact:true}).waitFor({timeout:60000});
 await birth.getByLabel('Friendly Body name',{exact:true}).fill('A little room to write');
 await birth.getByRole('button',{name:'Birth Body',exact:true}).click();
 await page.waitForFunction(()=>document.querySelector('[data-play-state]').textContent==='Lulled',null,{timeout:60000});
 await page.getByRole('button',{name:'wake body',exact:true}).click();
 await page.waitForFunction(()=>document.querySelector('[data-play-state]').textContent==='Playing',null,{timeout:60000});
 await page.getByRole('navigation',{name:'Your plots'}).getByRole('button',{name:'Memory Lantern',exact:true}).click();
 const text='A little progress, every day.\n\nChoose a name. Add a Plot. Make something move.\n\nΔοκιμή · hello, 世界 🪞';
 await enterMemoryText(page,text);await page.locator('[data-body-tutorial] > summary').click();
 const editor=memoryEditor(page),identity=()=>page.evaluate(()=>{const c=globalThis.__conduitWorkspace.current();return{body_id:c.body_id,plan_id:c.plan_id,active_play_id:c.active_play_id}});
 const baseline=await identity();report.identity=baseline;report.text=text;
 const revision=await editor.getAttribute('data-revision');
 await editor.evaluate(node=>{globalThis.retainedEditor=node;node.setSelectionRange(2,17,'backward')});
 for(const look of ['editorial','engineering','phosphor']){
  await page.locator('[data-workspace-look-picker]').selectOption(look);
  assert.deepEqual(await identity(),baseline);assert.equal(await editor.inputValue(),text);assert.equal(await editor.getAttribute('data-revision'),revision);
  assert.equal(await editor.evaluate(node=>node===globalThis.retainedEditor),true);
  assert.deepEqual(await editor.evaluate(node=>[node.selectionStart,node.selectionEnd,node.selectionDirection]),[2,17,'backward']);
  const style=await editor.evaluate(node=>{const c=getComputedStyle(node);return{font_px:parseFloat(c.fontSize),font:c.fontFamily,ink:c.color,paper:c.backgroundColor}});
  assert.equal(style.font_px,16);assert.notEqual(style.ink,style.paper);
  assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false);
  await page.screenshot({path:`${output}/writing-${look}.png`,fullPage:true});report.workspace_chrome_looks.push({look,...style});
 }
 await page.setViewportSize({width:390,height:1000});await page.addStyleTag({content:'html{font-size:32px!important}'});
 await page.emulateMedia({forcedColors:'active',reducedMotion:'reduce'});
 await editor.focus();assert.equal(await editor.evaluate(node=>node===document.activeElement),true);
 assert.equal(await editor.evaluate(node=>parseFloat(getComputedStyle(node).fontSize)),32);
 assert.equal(await page.locator('[data-workspace-look-picker]').evaluate(node=>parseFloat(getComputedStyle(node).fontSize)),26);
 assert.ok(await page.locator('#surface-title').evaluate(node=>parseFloat(getComputedStyle(node).fontSize))>=32);
 assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false);
 const rect=await editor.boundingBox();assert.ok(rect.x>=0&&rect.x+rect.width<=390);
 const probe=await page.evaluate(()=>{const p=document.createElement('i');p.style.cssText='color:CanvasText;background:Canvas;outline-color:Highlight';document.body.append(p);const c=getComputedStyle(p);const result={ink:c.color,paper:c.backgroundColor,focus:c.outlineColor};p.remove();return result});
 assert.deepEqual(await editor.evaluate(node=>{const c=getComputedStyle(node);return{ink:c.color,paper:c.backgroundColor,focus:c.outlineColor}}),probe);
 await enterMemoryText(page,text+'\nStill writing.');assert.deepEqual(await identity(),baseline);
 await page.screenshot({path:`${output}/writing-narrow-forced.png`,fullPage:true});
 report.narrow={width:390,editor_font_px:32,horizontal_overflow:false,forced_colors:true,keyboard_focus:true,subsequent_authoritative_edit:true};
 assert.deepEqual(report.browser_errors,[]);
 const hash=bytes=>createHash('sha256').update(bytes).digest('hex');report.runtime_sha256=hash(await readFile(`${product}/runtime.wasm`));
 for(const asset of ['conduit.css','workspace.css','browser-text-editor.mjs'])report[asset+'_sha256']=hash(await readFile(`${product}/${asset}`));
 await writeFile(`${output}/report.json`,JSON.stringify(report,null,2)+'\n');console.log('PASS actual Birth/Wake and authoritative Unicode edits, retained editor/revision/selection/Body/Plan/Play under three chrome Looks, 16px/32px fonts and narrow forced colors');
}finally{await browser.close();server.child.kill()}
